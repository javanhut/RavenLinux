//! Service management for RavenInit

use std::ffi::CString;
use std::os::unix::io::RawFd;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use nix::fcntl::{open, OFlag};
use nix::sys::signal::{self, Signal};
use nix::sys::stat::Mode;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{self, dup2, execvp, fork, setsid, ForkResult, Pid};

use crate::config::ServiceConfig;

// TIOCSCTTY ioctl to set controlling terminal
nix::ioctl_write_int_bad!(tiocsctty, libc::TIOCSCTTY);

/// Restarts allowed inside [`RESTART_WINDOW`] before a service is declared
/// crash-looping rather than recovering.
const MAX_RESTARTS: u32 = 5;

/// How long a service must survive for its restart budget to be refilled.
const RESTART_WINDOW: Duration = Duration::from_secs(60);

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
    /// Service failed to start
    Failed,
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
    /// Set once the restart budget is spent, so the give-up decision is made
    /// and logged exactly once.
    ///
    /// The supervisor asks `should_restart` on every 100ms tick. Without
    /// somewhere to record the answer, a crash-looping service re-derived it
    /// each time and logged from inside the query -- ten "disabling restart"
    /// warnings a second, none of which disabled anything.
    restart_disabled: bool,
    /// When the current restart budget window opened.
    window_start: Option<Instant>,
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
            restart_disabled: false,
            window_start: None,
            manually_stopped: false,
        };

        service.do_start()?;
        Ok(service)
    }

    fn do_start(&mut self) -> Result<()> {
        // Check if this service needs TTY handling
        if let Some(tty_path) = self.config.tty.clone() {
            return self.do_start_with_tty(&tty_path);
        }

        // Standard service spawning (no TTY)
        let mut cmd = Command::new(&self.config.exec);

        // Add arguments
        cmd.args(&self.config.args);

        // Set environment
        for (key, value) in &self.config.environment {
            cmd.env(key, value);
        }

        // Set up stdio
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        // Spawn the process
        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to start {}", self.config.name))?;

        let pid = Pid::from_raw(child.id() as i32);

        self.child = Some(child);
        self.pid = Some(pid);
        self.state = ServiceState::Running;
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
        if self.manually_stopped {
            return false;
        }

        if !self.config.restart {
            return false;
        }

        // Already given up. Silent: it was logged when the decision was made.
        if self.restart_disabled {
            return false;
        }

        // A service that stayed up for a whole window is recovering, not
        // looping, so it gets its budget back.
        if let Some(started) = self.window_start {
            if started.elapsed() > RESTART_WINDOW {
                self.window_start = None;
                self.restart_count = 0;
            }
        }

        if self.restart_count >= MAX_RESTARTS {
            self.restart_disabled = true;
            self.state = ServiceState::Failed;
            log::error!(
                "Service {} failed {} times in under {}s; giving up. \
                 Fix the cause and run `raven-rc start {}`.",
                self.config.name,
                self.restart_count,
                RESTART_WINDOW.as_secs(),
                self.config.name
            );
            return false;
        }

        true
    }

    /// Mark service as exited
    pub fn mark_exited(&mut self, status: i32) {
        self.state = ServiceState::Exited;
        self.exit_status = Some(status);
        self.pid = None;
        self.child = None;

        log::info!("Service {} exited with status {}", self.config.name, status);
    }

    /// Mark service as killed by signal
    pub fn mark_signaled(&mut self, signal: Signal) {
        self.state = ServiceState::Signaled;
        self.exit_signal = Some(signal);
        self.pid = None;
        self.child = None;

        log::info!("Service {} killed by signal {:?}", self.config.name, signal);
    }

    /// Restart the service
    pub fn restart(&mut self) -> Result<()> {
        self.manually_stopped = false;
        self.restart_count += 1;
        self.last_restart = Some(Instant::now());
        self.window_start.get_or_insert_with(Instant::now);

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
            log::debug!("Sending SIGTERM to {} (PID {})", self.config.name, pid);
            let _ = signal::kill(pid, Signal::SIGTERM);
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
                        let _ = signal::kill(pid, Signal::SIGKILL);
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
    /// Clears the operator-stopped flag and the restart budget, so a service
    /// that previously tripped the restart rate limit gets a clean slate rather
    /// than inheriting a refusal to run.
    pub fn start_by_request(&mut self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }

        self.manually_stopped = false;
        self.restart_count = 0;
        self.last_restart = None;
        // An explicit start is the operator saying the cause is dealt with.
        self.restart_disabled = false;
        self.window_start = None;
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
            log::debug!("Sending SIGKILL to {} (PID {})", self.config.name, pid);
            let _ = signal::kill(pid, Signal::SIGKILL);
        }
        self.state = ServiceState::Stopped;
        self.pid = None;
        self.child = None;
    }
}
