//! Service management for RavenInit

use std::ffi::CString;
use std::os::unix::io::RawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use nix::fcntl::{open, OFlag};
use nix::sys::signal::{self, Signal};
use nix::sys::stat::Mode;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{self, dup2, execvp, fork, setsid, ForkResult, Pid};

use serde::{Deserialize, Serialize};

use crate::config::ServiceConfig;

// TIOCSCTTY ioctl to set controlling terminal
nix::ioctl_write_int_bad!(tiocsctty, libc::TIOCSCTTY);

/// The delay before the first restart of a service that has just died.
///
/// Doubles on every consecutive death up to [`RESTART_BACKOFF_MAX`], and is
/// reset once a run lasts [`RESTART_STABLE_AFTER`]. The supervisor never gives
/// up: a service that cannot start is retried once a minute, forever, with
/// the reason in its log each time.
///
/// Giving up was the previous policy -- five deaths in a minute -- and it
/// was the wrong one for the services that matter most. A login daemon whose
/// greeter died in under a second hit the limit in five seconds, and from then
/// on the machine had no login screen until somebody rebooted it: the person
/// who could have run `raven-rc start ravend` had no way to reach a shell
/// to type it in. A backoff turns the same crash loop into a service that is
/// back the moment its cause is fixed, and costs one exec a minute while it
/// is not.
pub const RESTART_BACKOFF_BASE: Duration = Duration::from_secs(1);

/// The longest the supervisor waits between restarts of a crash-looping
/// service.
pub const RESTART_BACKOFF_MAX: Duration = Duration::from_secs(60);

/// A run that lasts this long is a recovery, not a crash loop, and resets the
/// backoff to [`RESTART_BACKOFF_BASE`] for whatever death comes next.
pub const RESTART_STABLE_AFTER: Duration = Duration::from_secs(60);

/// How long to wait before restart number `attempt` (counting from 1) of a
/// service whose previous runs all died quickly.
///
/// 1s, 2s, 4s, 8s, 16s, 32s, then 60s for every attempt after.
pub fn restart_delay(attempt: u32) -> Duration {
    // Shift capped well below 32 so this cannot overflow however long a
    // service has been looping; the min against MAX does the real limiting.
    let doublings = attempt.saturating_sub(1).min(16);
    RESTART_BACKOFF_BASE
        .checked_mul(1u32 << doublings)
        .unwrap_or(RESTART_BACKOFF_MAX)
        .min(RESTART_BACKOFF_MAX)
}

/// Make `cmd` exec as `account`: supplementary groups, then gid, then uid.
///
/// All three happen inside one `pre_exec` closure rather than through
/// `CommandExt::uid`/`gid`, and that is not a style choice. `std` applies the
/// uid and gid it was given *before* it runs any `pre_exec` closure, so a
/// closure calling `setgroups` would run after `setuid` had already dropped
/// the privilege `setgroups` requires, and fail with `EPERM`. The service
/// would then start with the wrong groups or not at all, depending on whether
/// the error was checked. Doing the whole sequence in the closure is what
/// makes the ordering ours to state.
///
/// The order within the closure matters for the same reason and is the
/// classic one: supplementary groups first, then the primary gid, then the
/// uid last. Each step gives away privilege the next one would need, so any
/// other order silently leaves the process over-privileged -- `setuid` first
/// is the well-known way to end up still in root's groups.
fn apply_credentials(cmd: &mut Command, account: &crate::user::Account) {
    let uid = unistd::Uid::from_raw(account.uid);
    let gid = unistd::Gid::from_raw(account.gid);
    let groups: Vec<unistd::Gid> = account
        .groups
        .iter()
        .copied()
        .map(unistd::Gid::from_raw)
        .collect();

    // SAFETY: `pre_exec` runs in the forked child between `fork` and `exec`,
    // where only async-signal-safe work is permitted. `setgroups`, `setgid`
    // and `setuid` are raw syscalls and allocate nothing -- the `Vec` they
    // read was built in the parent before the fork, and the closure only
    // borrows it. No locks are taken and nothing is logged, so there is no
    // allocator or mutex state to be inherited in a locked state from another
    // thread at the moment of fork.
    unsafe {
        cmd.pre_exec(move || {
            unistd::setgroups(&groups).map_err(std::io::Error::from)?;
            unistd::setgid(gid).map_err(std::io::Error::from)?;
            unistd::setuid(uid).map_err(std::io::Error::from)?;
            Ok(())
        });
    }
}

/// Why `exec` cannot be run, if it cannot.
///
/// `spawn` reports "not installed" and "not executable" alike as a bare errno,
/// and the path -- the one thing the operator needs -- is not in the message.
/// init.toml deliberately lists daemons that may never be installed (the sshd
/// entry exists so that `rvn install openssh` is all a person has to do), so a
/// missing binary is the single likeliest reason a manual start fails and it
/// deserves to be said in those words.
///
/// A bare command name is left alone: `Command` resolves it through PATH, and
/// second-guessing that lookup here would reject things that do in fact run.
fn exec_problem(exec: &str) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;

    if !exec.contains('/') {
        return None;
    }

    let Ok(meta) = std::fs::metadata(exec) else {
        return Some(format!(
            "{exec} does not exist -- whatever package provides it is not installed"
        ));
    };

    if meta.is_dir() {
        return Some(format!("{exec} is a directory, not a program"));
    }

    let mode = meta.permissions().mode();
    if mode & 0o111 == 0 {
        return Some(format!(
            "{exec} is not executable (mode {:04o})",
            mode & 0o7777
        ));
    }

    None
}

/// Service state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// Service is running
    Running,
    /// Service has exited normally
    Exited,
    /// Service was killed by a signal
    Signaled,
    /// Service is stopped
    Stopped,
    /// Service failed to start.
    ///
    /// No longer produced by the supervisor -- a service that keeps dying is
    /// backed off, not declared failed -- but kept so `raven-rc status` has a
    /// word for it if a future start path needs one.
    #[allow(dead_code)]
    Failed,
}

/// What one service looks like to the raven-init that replaces this one.
///
/// A re-exec keeps every service process where it is; only the supervisor is
/// swapped. This is the part of [`Service`] that survives the swap. Nothing
/// here is an `Instant` or a `Child`: the first does not mean anything in
/// another process and the second cannot be sent to one -- the new supervisor
/// tracks the process by pid alone, which is all `waitpid(-1)` needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceSnapshot {
    /// The definition this process was started from. Kept even when the new
    /// supervisor's config no longer lists it, for the same reason `reload`
    /// keeps an orphaned definition: a process nobody can name is a process
    /// nobody can stop.
    pub config: ServiceConfig,
    /// Live pid, or `None` for a service that is not running.
    pub pid: Option<i32>,
    /// Seconds the current run has been up, so a crash right after the swap
    /// is still recognised as the end of a long stable run.
    #[serde(default)]
    pub uptime_secs: u64,
    #[serde(default)]
    pub restart_count: u32,
    #[serde(default)]
    pub manually_stopped: bool,
}

/// A managed service
pub struct Service {
    /// Service configuration
    config: ServiceConfig,
    /// Current state
    state: ServiceState,
    /// Child process handle
    child: Option<Child>,
    /// Process ID
    pid: Option<Pid>,
    /// Exit status (if exited)
    exit_status: Option<i32>,
    /// Signal that killed the process (if signaled)
    exit_signal: Option<Signal>,
    /// Number of restart attempts
    restart_count: u32,
    /// Last restart time
    last_restart: Option<Instant>,
    /// When the current run was started, for measuring how long it lasted.
    started_at: Option<Instant>,
    /// When the current run died, for the same measurement.
    exited_at: Option<Instant>,
    /// When the pending restart is due, once the backoff has been decided.
    ///
    /// The supervisor asks `should_restart` on every 100ms tick. The decision
    /// -- and its log line -- is made once, on the first tick after a death,
    /// and recorded here; the ticks after that only compare against it.
    /// Without somewhere to record the answer, a crash-looping service once
    /// re-derived it each time and logged from inside the query, ten lines a
    /// second, forever.
    retry_at: Option<Instant>,
    /// Set when an operator stopped this service through raven-rc.
    ///
    /// Without this, `stop` is a no-op with extra steps: the supervisor sees an
    /// `Exited` service whose config says `restart = true` and starts it
    /// straight back up. Auto-restart is for services that *crash*, not for
    /// ones that were told to stop.
    manually_stopped: bool,
}

impl Service {
    /// Start a new service
    pub fn start(config: &ServiceConfig) -> Result<Self> {
        let mut service = Self {
            config: config.clone(),
            state: ServiceState::Stopped,
            child: None,
            pid: None,
            exit_status: None,
            exit_signal: None,
            restart_count: 0,
            last_restart: None,
            started_at: None,
            exited_at: None,
            retry_at: None,
            manually_stopped: false,
        };

        service.do_start()?;
        Ok(service)
    }

    /// The part of this service that a re-exec hands on.
    pub fn snapshot(&self) -> ServiceSnapshot {
        ServiceSnapshot {
            config: self.config.clone(),
            pid: if self.is_running() {
                self.pid.map(|p| p.as_raw())
            } else {
                None
            },
            uptime_secs: self
                .started_at
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0),
            restart_count: self.restart_count,
            manually_stopped: self.manually_stopped,
        }
    }

    /// Take over a service the previous supervisor left running.
    ///
    /// Nothing is forked. If the snapshot names a pid and that process still
    /// exists, the result is `Running` and owned by pid; the main loop's
    /// `waitpid(-1)` reaps it exactly as it would a child this process forked,
    /// because after the exec that is what it is -- the exec kept PID 1's
    /// identity, and children are inherited with it. A pid that is gone (it
    /// died in the hand-off window, and the kernel queued the SIGCHLD for us)
    /// comes back `Exited`, which is what lets `check_services` restart it.
    ///
    /// `config` is the definition to use from here on: normally the freshly
    /// loaded one, so a `restart` after the swap picks up an edited init.toml
    /// the same way it would after `reload`.
    pub fn adopt(snapshot: ServiceSnapshot, config: ServiceConfig) -> Self {
        let now = Instant::now();
        let alive = snapshot
            .pid
            .map(Pid::from_raw)
            .filter(|pid| signal::kill(*pid, None).is_ok());

        let started_at = now.checked_sub(Duration::from_secs(snapshot.uptime_secs));

        match alive {
            Some(pid) => Self {
                config,
                state: ServiceState::Running,
                child: None,
                pid: Some(pid),
                exit_status: None,
                exit_signal: None,
                restart_count: snapshot.restart_count,
                last_restart: None,
                started_at,
                exited_at: None,
                retry_at: None,
                manually_stopped: false,
            },
            None => Self {
                config,
                state: if snapshot.pid.is_some() {
                    ServiceState::Exited
                } else {
                    ServiceState::Stopped
                },
                child: None,
                pid: None,
                exit_status: None,
                exit_signal: None,
                restart_count: snapshot.restart_count,
                last_restart: None,
                started_at,
                exited_at: if snapshot.pid.is_some() { Some(now) } else { None },
                retry_at: None,
                manually_stopped: snapshot.manually_stopped,
            },
        }
    }

    /// Where service output goes. Overridable so tests need no /var/log.
    pub(crate) fn log_dir() -> std::path::PathBuf {
        std::env::var_os("RAVEN_SERVICE_LOG_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/var/log/raven"))
    }

    /// Open this service's log file, creating the directory on the way.
    fn open_log(&self) -> Option<std::fs::File> {
        let dir = Self::log_dir();
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(format!("{}.log", self.config.name)))
            .ok()
    }

    fn do_start(&mut self) -> Result<()> {
        // Checked before anything is created or forked, so the reply names the
        // real problem instead of an errno from deep inside spawn().
        if let Some(problem) = exec_problem(&self.config.exec) {
            bail!("{problem}");
        }

        // Setup the daemon expects someone else to have done -- generating
        // host keys, say. Run to completion first, and a failure is the
        // service's failure: starting sshd with no keys just moves the error
        // into a crash loop.
        if let Some((program, args)) = self.config.pre_exec.split_first() {
            let status = Command::new(program)
                .args(args)
                .stdin(Stdio::null())
                .stdout(match self.open_log() {
                    Some(f) => Stdio::from(f),
                    None => Stdio::null(),
                })
                .stderr(match self.open_log() {
                    Some(f) => Stdio::from(f),
                    None => Stdio::null(),
                })
                .status()
                .with_context(|| format!("pre_exec: cannot run {}", program))?;
            if !status.success() {
                bail!("pre_exec {} exited with {}", program, status);
            }
        }

        // A daemon that binds a socket under /run cannot mkdir its own parent
        // there after a boot -- /run is a fresh tmpfs every time. dbus is the
        // canonical case; see ServiceConfig::runtime_dirs.
        for dir in &self.config.runtime_dirs {
            if let Err(e) = std::fs::create_dir_all(dir) {
                log::warn!("{}: cannot create {}: {}", self.config.name, dir, e);
            }
        }

        // Check if this service needs TTY handling
        if let Some(tty_path) = self.config.tty.clone() {
            return self.do_start_with_tty(&tty_path);
        }

        // Standard service spawning (no TTY)
        let mut cmd = Command::new(&self.config.exec);

        // Give every service a process group of its own. Daemons such as
        // ravend supervise a compositor, a greeter and eventually a complete
        // desktop session beneath one PID. Signalling only that PID leaves
        // those children alive -- and, for a compositor, still holding DRM
        // master and the seat -- while init proceeds with shutdown.
        //
        // `process_group(0)` asks the child to make its pid its pgid between
        // fork and exec. It must happen before the privilege drop registered
        // below, and lets stop/kill address the complete service with a
        // negative pid without ever signalling init's own process group.
        cmd.process_group(0);

        // Add arguments
        cmd.args(&self.config.args);

        // Set environment
        for (key, value) in &self.config.environment {
            cmd.env(key, value);
        }

        // Drop to the configured account, if there is one.
        //
        // Resolved here rather than in the child so that an unknown name is a
        // start failure naming the account, instead of an exit status from a
        // process that had no way left to say what went wrong.
        if let Some(name) = &self.config.user {
            let account = crate::user::by_name(name)
                .with_context(|| format!("{}: cannot run as '{}'", self.config.name, name))?;
            apply_credentials(&mut cmd, &account);
        }

        // Set up stdio. Output goes to /var/log/raven/<name>.log, not the
        // console: inherited stdio meant every daemon's chatter -- dbus's
        // config warnings, cawd's periodic "no wireless port" -- printed
        // straight over whatever the person at the keyboard was typing, on a
        // console that (since the kernel dropped fbcon scrollback in 5.9)
        // cannot scroll back to recover. Inherit remains the fallback so a
        // read-only /var/log costs the log, not the service.
        cmd.stdin(Stdio::null());
        match self.open_log() {
            Some(file) => {
                let clone = file.try_clone().ok();
                cmd.stdout(Stdio::from(file));
                match clone {
                    Some(f) => cmd.stderr(Stdio::from(f)),
                    None => cmd.stderr(Stdio::inherit()),
                };
            }
            None => {
                cmd.stdout(Stdio::inherit());
                cmd.stderr(Stdio::inherit());
            }
        }

        // Spawn the process
        let child = cmd
            .spawn()
            .with_context(|| format!("cannot exec {}", self.config.exec))?;

        let pid = Pid::from_raw(child.id() as i32);

        self.child = Some(child);
        self.pid = Some(pid);
        self.state = ServiceState::Running;
        self.started_at = Some(Instant::now());
        self.exited_at = None;
        self.exit_status = None;
        self.exit_signal = None;

        log::debug!("Service {} started with PID {}", self.config.name, pid);

        Ok(())
    }

    /// Start a service with proper TTY session and job control setup
    fn do_start_with_tty(&mut self, tty_path: &str) -> Result<()> {
        log::debug!(
            "Starting service {} with TTY {}",
            self.config.name,
            tty_path
        );

        // `user` is refused here rather than ignored. This path forks and
        // execs by hand and does not drop privilege, so honouring the field
        // would take work that nothing yet needs -- the one service that wants
        // an account is the graphical session, which has no tty. Ignoring it
        // instead would start the process as root having been asked not to,
        // and that failure is invisible: the service comes up and looks right.
        //
        // A getty is the natural tty service and it genuinely needs root, so
        // there is no case here waiting to be unblocked. If one appears, the
        // sequence in `apply_credentials` is what belongs in the child branch
        // below, after `setsid` and before `execvp`.
        if let Some(name) = &self.config.user {
            bail!(
                "{}: `user = \"{name}\"` is not supported for a service with a tty; \
                 it would run as root instead",
                self.config.name
            );
        }

        // Prepare command and arguments as CStrings for execvp
        let exec_cstr = CString::new(self.config.exec.as_str())
            .with_context(|| format!("Invalid exec path: {}", self.config.exec))?;

        let mut args_cstr: Vec<CString> = Vec::with_capacity(self.config.args.len() + 1);
        args_cstr.push(exec_cstr.clone());
        for arg in &self.config.args {
            args_cstr.push(
                CString::new(arg.as_str()).with_context(|| format!("Invalid argument: {}", arg))?,
            );
        }

        // Prepare environment
        let env_vars: Vec<(String, String)> = self.config.environment.clone().into_iter().collect();

        let tty_path_owned = tty_path.to_string();

        // Fork the process
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent process - just record the child PID
                self.pid = Some(child);
                self.started_at = Some(Instant::now());
                self.exited_at = None;
                self.child = None; // We don't have a Child handle when using fork directly
                self.state = ServiceState::Running;
                self.exit_status = None;
                self.exit_signal = None;

                log::info!(
                    "Service {} started with PID {} on TTY {}",
                    self.config.name,
                    child,
                    tty_path_owned
                );

                Ok(())
            }
            Ok(ForkResult::Child) => {
                // Child process - set up TTY and exec

                // 1. Create a new session (become session leader)
                if let Err(e) = setsid() {
                    log::error!("setsid() failed: {}", e);
                    std::process::exit(1);
                }

                // 2. Open the TTY device
                let tty_fd: RawFd = match open(
                    tty_path_owned.as_str(),
                    OFlag::O_RDWR | OFlag::O_NOCTTY,
                    Mode::empty(),
                ) {
                    Ok(fd) => fd,
                    Err(e) => {
                        log::error!("Failed to open TTY {}: {}", tty_path_owned, e);
                        std::process::exit(1);
                    }
                };

                // 3. Set this TTY as the controlling terminal
                // TIOCSCTTY with arg 0 means "don't steal if already controlled"
                if let Err(e) = unsafe { tiocsctty(tty_fd, 0) } {
                    log::error!("TIOCSCTTY failed: {}", e);
                    // Continue anyway - some systems may not require this
                }

                // 4. Duplicate TTY fd to stdin/stdout/stderr
                if let Err(e) = dup2(tty_fd, 0) {
                    log::error!("dup2 stdin failed: {}", e);
                }
                if let Err(e) = dup2(tty_fd, 1) {
                    log::error!("dup2 stdout failed: {}", e);
                }
                if let Err(e) = dup2(tty_fd, 2) {
                    log::error!("dup2 stderr failed: {}", e);
                }

                // Close the original fd if it's not 0, 1, or 2
                if tty_fd > 2 {
                    let _ = unistd::close(tty_fd);
                }

                // 5. Set the foreground process group to our process group
                let our_pid = unistd::getpid();
                let ret = unsafe { libc::tcsetpgrp(0, our_pid.as_raw()) };
                if ret < 0 {
                    log::error!("tcsetpgrp failed: {}", std::io::Error::last_os_error());
                    // Continue anyway
                }

                // 6. Set environment variables
                for (key, value) in env_vars {
                    std::env::set_var(&key, &value);
                }

                // 7. Exec the service
                let _ = execvp(&exec_cstr, &args_cstr);

                // If we get here, exec failed
                log::error!("execvp failed for {}", self.config.exec);
                std::process::exit(1);
            }
            Err(e) => {
                anyhow::bail!("fork() failed: {}", e);
            }
        }
    }

    /// Get service name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get current state
    pub fn state(&self) -> ServiceState {
        self.state
    }

    /// Get process ID
    pub fn pid(&self) -> Option<Pid> {
        self.pid
    }

    /// Check if service should be restarted
    /// Decide whether the supervisor should restart this service.
    ///
    /// Takes `&mut` because the give-up decision is *latched*. The previous
    /// version was a pure query that logged, and it was wrong in three ways
    /// at once: it warned on every tick rather than once, it never recorded
    /// the decision, and five seconds later the same service was restarted
    /// again -- so "disabling restart" flooded the console at ~10 lines a
    /// second forever while disabling nothing. The flood also made the
    /// console unusable, which is how it was found.
    pub fn should_restart(&mut self) -> bool {
        self.should_restart_at(Instant::now())
    }

    /// [`should_restart`](Self::should_restart) against a clock the caller
    /// supplies, so the backoff schedule can be tested without waiting it out.
    pub fn should_restart_at(&mut self, now: Instant) -> bool {
        if self.manually_stopped {
            return false;
        }

        if !self.config.restart {
            return false;
        }

        let retry_at = match self.retry_at {
            Some(at) => at,
            None => {
                // First tick after the death: decide the delay, say so once.
                //
                // A run that stayed up long enough was a recovery, so the
                // death that ended it starts the schedule over rather than
                // inheriting the last crash loop's minute-long waits.
                if self.last_run_was_stable() {
                    self.restart_count = 0;
                }
                let attempt = self.restart_count.saturating_add(1);
                let delay = restart_delay(attempt);
                let at = now + delay;
                self.retry_at = Some(at);
                log::warn!(
                    "Service {} died; restarting in {}s (attempt {})",
                    self.config.name,
                    delay.as_secs(),
                    attempt
                );
                at
            }
        };

        now >= retry_at
    }

    /// Whether the run that just ended lasted [`RESTART_STABLE_AFTER`].
    fn last_run_was_stable(&self) -> bool {
        match (self.started_at, self.exited_at) {
            (Some(started), Some(exited)) => {
                exited.saturating_duration_since(started) >= RESTART_STABLE_AFTER
            }
            _ => false,
        }
    }

    /// When the pending restart is due, while one is pending.
    pub fn retry_at(&self) -> Option<Instant> {
        self.retry_at
    }

    /// Mark service as exited
    pub fn mark_exited(&mut self, status: i32) {
        self.state = ServiceState::Exited;
        self.exit_status = Some(status);
        self.exited_at = Some(Instant::now());
        self.pid = None;
        self.child = None;

        log::info!("Service {} exited with status {}", self.config.name, status);
    }

    /// Mark service as killed by signal
    pub fn mark_signaled(&mut self, signal: Signal) {
        self.state = ServiceState::Signaled;
        self.exit_signal = Some(signal);
        self.exited_at = Some(Instant::now());
        self.pid = None;
        self.child = None;

        log::info!("Service {} killed by signal {:?}", self.config.name, signal);
    }

    /// Restart the service
    pub fn restart(&mut self) -> Result<()> {
        self.manually_stopped = false;
        self.restart_count = self.restart_count.saturating_add(1);
        self.last_restart = Some(Instant::now());
        self.retry_at = None;

        log::info!(
            "Restarting service {} (attempt {})",
            self.config.name,
            self.restart_count
        );

        self.do_start()
    }

    /// Whether this service has a stop command configured.
    pub fn has_stop_exec(&self) -> bool {
        self.config.stop_exec.is_some()
    }

    /// Run the service's `stop_exec`, if it has one, and wait for it.
    ///
    /// Best-effort by design: a missing binary, a non-zero exit or a hang all
    /// fall through to the signal path rather than blocking a shutdown. The
    /// wait is bounded by `stop_timeout` because this runs from PID 1, where
    /// an unbounded wait is a hung machine.
    pub fn run_stop_exec(&mut self) {
        let Some(ref program) = self.config.stop_exec else {
            return;
        };
        if self.pid.is_none() {
            return;
        }

        log::info!(
            "Stopping {} with {} {:?}",
            self.config.name,
            program,
            self.config.stop_args
        );

        let mut child = match Command::new(program)
            .args(&self.config.stop_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                log::warn!("{}: stop_exec failed to start: {}", self.config.name, e);
                return;
            }
        };

        let deadline =
            std::time::Instant::now() + Duration::from_secs(self.config.stop_timeout as u64);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        log::warn!("{}: stop_exec exited with {}", self.config.name, status);
                    }
                    return;
                }
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        log::warn!(
                            "{}: stop_exec did not finish in {}s, killing it",
                            self.config.name,
                            self.config.stop_timeout
                        );
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    log::warn!("{}: cannot wait on stop_exec: {}", self.config.name, e);
                    return;
                }
            }
        }
    }

    /// Stop the service (SIGTERM)
    ///
    /// Does not mark the service stopped by operator intent -- shutdown uses
    /// this too, and there the distinction is meaningless. Use
    /// [`Service::stop_by_request`] for an operator-initiated stop.
    pub fn stop(&mut self) {
        if let Some(pid) = self.pid {
            log::debug!(
                "Sending SIGTERM to {} (process group {})",
                self.config.name,
                pid
            );
            signal_service_group(pid, Signal::SIGTERM);
        }
    }

    /// Stop the service on an operator's request, and keep it stopped.
    ///
    /// Runs the configured `stop_exec` first so a daemon can leave cleanly --
    /// the same courtesy shutdown extends, and the reason cawd can deauthenticate
    /// from its AP instead of vanishing mid-association.
    pub fn stop_by_request(&mut self) {
        self.manually_stopped = true;

        if self.has_stop_exec() {
            self.run_stop_exec();
        }

        self.stop();
    }

    /// Bring `state` up to date with the child process, without blocking.
    ///
    /// The main loop reaps every 100ms. A control request arriving inside that
    /// window would otherwise see a service as running when its process has
    /// already gone -- `start` immediately after `stop` being the obvious case,
    /// where the honest answer is "starting it" and the stale one is "already
    /// running".
    pub fn poll_exit(&mut self) {
        let Some(pid) = self.pid else {
            return;
        };

        match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::Exited(_, status)) => self.mark_exited(status),
            Ok(WaitStatus::Signaled(_, sig, _)) => self.mark_signaled(sig),
            Ok(WaitStatus::StillAlive) => {}
            // ECHILD: the main loop's reaper got there first.
            Err(_) => {
                self.state = ServiceState::Stopped;
                self.pid = None;
                self.child = None;
            }
            _ => {}
        }
    }

    /// Wait for a stopping service's process to actually leave.
    ///
    /// `stop_by_request` only *sends* SIGTERM. Restart has to see the process
    /// go before it starts a replacement: otherwise `is_running()` is still
    /// true a microsecond later, `start_by_request` returns early, and the
    /// restart quietly becomes a stop that never comes back.
    ///
    /// Bounded, and SIGKILLs past the deadline, because this runs on PID 1's
    /// thread -- no service is worth hanging the supervisor over.
    pub fn wait_for_exit(&mut self, timeout: Duration) {
        let Some(pid) = self.pid else {
            return;
        };

        let deadline = Instant::now() + timeout;
        loop {
            match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(_, status)) => {
                    self.mark_exited(status);
                    return;
                }
                Ok(WaitStatus::Signaled(_, sig, _)) => {
                    self.mark_signaled(sig);
                    return;
                }
                Ok(WaitStatus::StillAlive) => {
                    if Instant::now() >= deadline {
                        log::warn!(
                            "{} did not exit within {:?}, sending SIGKILL",
                            self.config.name,
                            timeout
                        );
                        signal_service_group(pid, Signal::SIGKILL);
                        let _ = waitpid(pid, None);
                        self.state = ServiceState::Stopped;
                        self.pid = None;
                        self.child = None;
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                // ECHILD: the main loop's reaper got there first.
                Err(_) => {
                    self.state = ServiceState::Stopped;
                    self.pid = None;
                    self.child = None;
                    return;
                }
                _ => {}
            }
        }
    }

    /// Start a service that is not currently running.
    ///
    /// Clears the operator-stopped flag and the restart backoff, so a service
    /// that has been crash-looping is tried again at once rather than at the
    /// end of a minute-long wait.
    pub fn start_by_request(&mut self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }

        self.manually_stopped = false;
        // An explicit start is the operator saying the cause is dealt with,
        // so the backoff starts over from the shortest delay.
        self.restart_count = 0;
        self.last_restart = None;
        self.retry_at = None;
        self.state = ServiceState::Stopped;

        self.do_start()
    }

    /// True while the service has a live process.
    pub fn is_running(&self) -> bool {
        self.pid.is_some() && self.state == ServiceState::Running
    }

    /// True when an operator stopped this service and it should stay down.
    pub fn is_manually_stopped(&self) -> bool {
        self.manually_stopped
    }

    /// How many times the supervisor has restarted this service.
    pub fn restart_count(&self) -> u32 {
        self.restart_count
    }

    /// Exit status of the last run, when it exited normally.
    pub fn exit_status(&self) -> Option<i32> {
        self.exit_status
    }

    /// Human-readable description from the service's config.
    pub fn description(&self) -> &str {
        &self.config.description
    }

    /// Whether the config asks for automatic restart on exit.
    pub fn restart_configured(&self) -> bool {
        self.config.restart
    }

    /// Kill the service (SIGKILL)
    pub fn kill(&mut self) {
        if let Some(pid) = self.pid {
            log::debug!(
                "Sending SIGKILL to {} (process group {})",
                self.config.name,
                pid
            );
            signal_service_group(pid, Signal::SIGKILL);
        }
        self.state = ServiceState::Stopped;
        self.pid = None;
        self.child = None;
    }
}

/// Signal a service and every child that stayed in its process group.
///
/// Fresh services always have `pgid == pid`, but a service adopted across an
/// init re-exec may have been started by an older raven-init that did not make
/// a group. Verify before using a negative pid so upgrading PID 1 can never
/// accidentally signal its own process group. Falling back to the leader PID
/// preserves the old behaviour for such an adopted service.
fn signal_service_group(pid: Pid, signal_to_send: Signal) {
    let target = match unistd::getpgid(Some(pid)) {
        Ok(pgid) if pgid == pid => Pid::from_raw(-pid.as_raw()),
        _ => pid,
    };
    let _ = signal::kill(target, signal_to_send);
}
