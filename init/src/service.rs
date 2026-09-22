//! Service management for RavenInit

use std::ffi::CString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::io::RawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use nix::sys::signal::{self, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{self, fork, setsid, ForkResult, Pid};

use serde::{Deserialize, Serialize};

use crate::config::{ServiceConfig, ServiceType};

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

/// How long a service's `pre_exec` hook may run before the start is failed.
///
/// A hook is setup the daemon expects somebody else to have done, and it runs
/// on PID 1's only thread, so an unbounded wait here is a machine that stops
/// booting mid-list with nothing said. The documented case is sshd's
/// `ssh-keygen -A`, which blocks in `getrandom(2)` until the kernel CRNG is
/// initialised -- on a first boot, or in a VM with no virtio-rng, that can be
/// a very long time. Thirty seconds is long enough for the hooks people
/// actually write and short enough that a hook which is never going to finish
/// becomes a failed service, with the reason in its log, rather than a boot
/// that never ends.
pub const PRE_EXEC_TIMEOUT: Duration = Duration::from_secs(30);

/// How long [`Service::wait_for_exit`] waits for a SIGKILLed service to be
/// reaped before it gives up and leaves the corpse to the main loop.
///
/// SIGKILL is normally acted on within microseconds, so this is generous for
/// every process that can die at all. The ones that cannot -- a task wedged in
/// uninterruptible sleep on a yanked USB volume is the case that matters -- do
/// not become killable by being waited on longer, and waiting on them is how
/// PID 1 stops supervising the machine. See the giving-up branch there for
/// what is traded away.
const SIGKILL_REAP_GRACE: Duration = Duration::from_millis(500);

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

/// Put the signal state back the way a freshly `exec`'d program expects to
/// find it: every disposition at its default, and nothing blocked.
///
/// PID 1 is a Rust program, and Rust's runtime sets SIGPIPE to SIG_IGN before
/// `main` runs. SIG_IGN survives `execve`, `login(1)` does not reset it, and
/// POSIX requires a shell to leave an inherited-ignored signal ignored -- so
/// without this, every command the person at the console ever ran inherited
/// init's ignored SIGPIPE, four processes down the line. What that looks like
/// is `cat /dev/zero | head -c1` printing "write error: Broken pipe" instead
/// of `cat` dying quietly, and, worse, any producer in a pipeline that does
/// not check its write return value never noticing that the reader is gone.
///
/// `Command` already does this -- std's `do_exec` empties the signal mask and
/// resets SIGPIPE before it execs -- which is why every service started the
/// ordinary way is clean and only the hand-rolled tty path was not.
///
/// The loop stops at 31 deliberately: 32 and 33 are glibc's, reserved for the
/// threading implementation, and SIGKILL and SIGSTOP cannot be reset at all.
///
/// # Safety
///
/// Must be called only in a child of `fork` that has not yet `exec`ed, where
/// this process is the only thread and these calls are therefore safe. Every
/// call it makes is a raw syscall over stack memory: nothing allocates, and
/// nothing takes a lock.
unsafe fn reset_signals_for_exec() {
    for sig in 1..=31 {
        if sig == libc::SIGKILL || sig == libc::SIGSTOP {
            continue;
        }
        libc::signal(sig, libc::SIG_DFL);
    }

    let mut empty: libc::sigset_t = std::mem::zeroed();
    libc::sigemptyset(&mut empty);
    libc::sigprocmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
}

/// Say something from a forked child, with `write(2)` and nothing else.
///
/// `log::error!` formats, allocates and takes the logger's mutex, none of
/// which a child between `fork` and `exec` may do: the heap it holds is a
/// snapshot taken mid-`fork`, so a lock some other thread held at that instant
/// stays held here forever. The message itself is therefore formatted in the
/// parent and this only puts the bytes on the descriptor.
///
/// # Safety
///
/// Same window as [`reset_signals_for_exec`].
unsafe fn child_say(message: &[u8]) {
    // Nothing to do about a short or failed write from here, and nowhere to
    // report it to; the message is a diagnostic, not the work.
    libc::write(2, message.as_ptr().cast(), message.len());
}

/// Say it and leave, without running any of the parent's exit machinery.
///
/// `_exit(2)` rather than `std::process::exit`, which runs `atexit` handlers
/// and `std::rt`'s cleanup registered by *init* and flushes init's inherited
/// stdio buffers -- printing whatever PID 1 had buffered a second time, out of
/// a process that is only a copy of it.
///
/// # Safety
///
/// Same window as [`reset_signals_for_exec`].
unsafe fn child_abort(message: &[u8]) -> ! {
    child_say(message);
    libc::_exit(1)
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

/// How a `type = "oneshot"` service is getting on with the one thing it was
/// asked to do.
///
/// Deliberately not a [`ServiceState`] variant. The states describe what the
/// supervisor is holding -- a live child, a corpse with a status, a process it
/// signalled -- and a one-shot that has finished is exactly the same object as
/// a daemon that has exited; nothing about the bookkeeping differs. What
/// differs is what it *means*, and meaning belongs to the definition, which is
/// where `type` is. Threading a sixth state through `describe_state`, `list`,
/// `status`, the snapshot and `adopt` would buy the same four words at the
/// price of a state that `adopt` has to reconstruct across a re-exec from a
/// pid it no longer has.
///
/// So this is derived, every time, from the state and the exit status that
/// were already being kept. It costs nothing and it cannot fall out of step
/// with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneshotOutcome {
    /// Still going. The work has been started and has not finished.
    Running,
    /// Exited zero: the one thing it was for is done.
    Completed,
    /// Exited non-zero, carrying the status it exited with.
    ///
    /// The distinction this whole type exists for. A daemon exiting is a
    /// daemon that stopped; a one-shot exiting is a one-shot that worked --
    /// unless it did this, and then it is the only kind of exit on the machine
    /// that is unambiguously a failure.
    Failed(i32),
    /// Killed by a signal, stopped by an operator, or never run in this boot:
    /// it did not finish and it did not fail, and nothing more can be said
    /// about it than the words the supervisor already has for a process in
    /// that condition.
    Unfinished,
}

/// What one service looks like to the raven-init that replaces this one.
///
/// A re-exec keeps every service process where it is; only the supervisor is
/// swapped. This is the part of [`Service`] that survives the swap. Nothing
/// here is an `Instant` or a `Child`: the first does not mean anything in
/// another process and the second cannot be sent to one -- the new supervisor
/// tracks the process by pid alone, which is all `waitpid(-1)` needs.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceSnapshot {
    /// The definition this process was started from. Kept even when the new
    /// supervisor's config no longer lists it, for the same reason `reload`
    /// keeps an orphaned definition: a process nobody can name is a process
    /// nobody can stop.
    pub config: ServiceConfig,
    /// Live pid, or `None` for a service that is not running.
    pub pid: Option<i32>,
    /// When the current run started, and when its ready path was first seen,
    /// as CLOCK_MONOTONIC readings -- seconds since the kernel started, the
    /// same scale `blame` prints and `timeline::monotonic_secs` reads.
    ///
    /// Absolute rather than "how long ago", and floating point rather than
    /// whole seconds, for one reason each. The pair of `u64` seconds these
    /// replaced was written with `as_secs()`, which floors, so every re-exec
    /// quantised every service's times to the second -- in a supervisor whose
    /// `blame` prints three decimal places and exists to attribute tens of
    /// milliseconds. And a "how long ago" has to be turned back into a time by
    /// the reader, which adds the wall time of the hand-off itself -- write
    /// the file, exec, read it back, adopt -- to every service's age, once per
    /// re-exec, forever. An absolute reading needs no `now` to reconstruct it
    /// and so carries neither error.
    ///
    /// CLOCK_MONOTONIC is a property of the boot and not of the process, and
    /// is the clock `Instant` is built on, so the number means exactly the
    /// same thing on both sides of the exec.
    #[serde(default)]
    pub started_mono: Option<f64>,
    #[serde(default)]
    pub ready_mono: Option<f64>,
    /// The first start and the first ready of this service *in this boot*,
    /// which no restart moves. What `raven-rc blame` reports; see
    /// [`Service::first_started_at`] for why they are separate fields.
    #[serde(default)]
    pub first_started_mono: Option<f64>,
    #[serde(default)]
    pub first_ready_mono: Option<f64>,
    /// When the supervisor last restarted this service, so `raven-rc status`
    /// can still say when it last came back after the supervisor itself has
    /// been replaced.
    #[serde(default)]
    pub last_restart_mono: Option<f64>,
    #[serde(default)]
    pub restart_count: u32,
    #[serde(default)]
    pub manually_stopped: bool,
    /// The seconds-since-then fields this file carried before the readings
    /// above. Still written, and still read when the reading beside them is
    /// absent.
    ///
    /// Still written because the raven-init on disk can be older than the one
    /// writing the file: `raven-rc reexec` is how a new image is tried out,
    /// and the first thing anybody does with a bad one is re-exec back to the
    /// installed one. Still read because the image being replaced is usually
    /// the older one, and without this the re-exec onto the new format would
    /// report every service as having started at no time at all.
    ///
    /// Both floor to whole seconds, which is the entire reason they were
    /// replaced, so they are consulted only when the precise field is missing.
    /// When no raven-init old enough to write them is installed anywhere they
    /// can go, and `Handoff::VERSION` need not move for that or for their
    /// arrival: unknown keys are ignored and missing keys default, so a file
    /// written by either image parses in the other.
    #[serde(default)]
    pub uptime_secs: u64,
    #[serde(default)]
    pub ready_secs_ago: Option<u64>,
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
    /// When `config.ready_path` was first seen to exist for this run, if
    /// ever. What `raven-rc status` reports about the run it is describing.
    ready_at: Option<Instant>,
    /// When this service first started in this boot -- a time no restart
    /// moves.
    ///
    /// `started_at` above is the *current run*, and `do_start` resets it every
    /// time. That is right for the restart backoff, which asks how long the
    /// run that just died lasted, and it is wrong for `raven-rc blame`, which
    /// asks what the boot looked like. Reading the current run there produced
    /// a boot timeline in which obexd started at 97618s and cawd at 2751s: a
    /// suspend and two restarts, reported under a heading that says "seconds
    /// since the kernel started" above milestones that end at seven. The
    /// footer folded the same numbers into `span 97613.069s`, which is the
    /// machine's uptime wearing a boot time's clothes.
    ///
    /// So the two questions get two fields. This one is set the first time the
    /// service starts and then only carried -- through every restart, and
    /// through a re-exec in the snapshot -- until the machine reboots and the
    /// whole supervisor begins again from nothing.
    first_started_at: Option<Instant>,
    /// When this service was first seen ready in this boot, on the same terms.
    ///
    /// `None` while `config.ready_path` names a file that has never appeared,
    /// which is a different state from having no ready path at all and is
    /// reported as one.
    first_ready_at: Option<Instant>,
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
    /// When a requested stop becomes a SIGKILL, once one has been requested.
    ///
    /// `stop_by_request` only sends SIGTERM and deliberately does not wait for
    /// the process to go -- PID 1 has a machine to supervise and SIGTERM is
    /// asynchronous. That left the one case nobody escalated: a daemon that
    /// ignores or blocks SIGTERM stayed alive *and* flagged
    /// `manually_stopped`, a state the operator could not leave, because
    /// `stop` only sent another futile signal and `start` answered "already
    /// running". Recording the deadline here instead of waiting for it keeps
    /// both properties: the reply comes back at once, and
    /// [`Service::escalate_stop_if_due`] finishes the job from the main loop.
    kill_at: Option<Instant>,
    /// Set when the most recent attempt to start this service failed before
    /// there was a process, and cleared the moment one exists.
    ///
    /// The backoff asks [`Service::last_run_was_stable`] whether the run that
    /// just ended lasted, and that question is answered from `started_at` and
    /// `exited_at` -- both of which a failed `do_start` leaves describing the
    /// *previous*, successful run, because it bails before it touches either.
    /// Without this flag a daemon that ran happily for an hour and then could
    /// not be started again (its binary removed, its filesystem not mounted)
    /// had every attempt credited with that hour: `restart_count` was zeroed
    /// on each pass, the delay was pinned at [`RESTART_BACKOFF_BASE`], and the
    /// escalating backoff -- the whole point of the constant above -- never
    /// happened. Two console lines a second for the life of the boot is
    /// precisely the flood it exists to stop.
    start_failed: bool,
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
            ready_at: None,
            first_started_at: None,
            first_ready_at: None,
            exited_at: None,
            retry_at: None,
            manually_stopped: false,
            kill_at: None,
            start_failed: false,
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
            started_mono: self.started_at.map(crate::timeline::instant_secs),
            ready_mono: self.ready_at.map(crate::timeline::instant_secs),
            first_started_mono: self.first_started_at.map(crate::timeline::instant_secs),
            first_ready_mono: self.first_ready_at.map(crate::timeline::instant_secs),
            last_restart_mono: self.last_restart.map(crate::timeline::instant_secs),
            restart_count: self.restart_count,
            manually_stopped: self.manually_stopped,
            // Written for an older raven-init that reads nothing else; see
            // `ServiceSnapshot::uptime_secs`.
            uptime_secs: self
                .started_at
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0),
            ready_secs_ago: self.ready_at.map(|t| t.elapsed().as_secs()),
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

        // Every time in the snapshot is an absolute CLOCK_MONOTONIC reading,
        // so adopting one is a conversion and not a reconstruction: nothing
        // here depends on how long the hand-off took.
        let at = |mono: Option<f64>| mono.and_then(crate::timeline::instant_from_secs);

        // The seconds-since-then fields are the fallback for a hand-off
        // written by a raven-init that predates the readings. `uptime_secs` is
        // zero both for "not running" and for "started half a second ago", so
        // it is believed only when it is not zero -- otherwise a service that
        // has never run would be adopted as having started exactly now, and
        // that invented time would go straight into the boot timeline.
        let started_at = at(snapshot.started_mono).or_else(|| {
            (snapshot.uptime_secs > 0)
                .then(|| now.checked_sub(Duration::from_secs(snapshot.uptime_secs)))
                .flatten()
        });
        let ready_at = at(snapshot.ready_mono).or_else(|| {
            snapshot
                .ready_secs_ago
                .and_then(|s| now.checked_sub(Duration::from_secs(s)))
        });
        // An older hand-off says nothing about the first start of this boot,
        // and the run it does describe is the earliest this supervisor can
        // honestly claim to know about.
        let first_started_at = at(snapshot.first_started_mono).or(started_at);
        let first_ready_at = at(snapshot.first_ready_mono).or(ready_at);
        let last_restart = at(snapshot.last_restart_mono);

        match alive {
            Some(pid) => Self {
                config,
                state: ServiceState::Running,
                child: None,
                pid: Some(pid),
                exit_status: None,
                exit_signal: None,
                restart_count: snapshot.restart_count,
                last_restart,
                started_at,
                ready_at,
                first_started_at,
                first_ready_at,
                exited_at: None,
                retry_at: None,
                // Carried, exactly as the dead branch below carries it. Alive
                // *and* manually stopped is a real state and not a
                // contradiction, because `stop` is asynchronous: it sends
                // SIGTERM and returns, so a service the operator stopped is
                // still running for as long as its stop_exec and its own
                // shutdown take. Dropping the flag here meant that a re-exec
                // inside that window handed the service back auto-restartable,
                // and the SIGTERM landing a moment later had `check_services`
                // start it straight up again -- the operator's stop reversed
                // by the supervisor, with nothing in the log saying so.
                manually_stopped: snapshot.manually_stopped,
                // A pending kill deadline is not carried across a re-exec: the
                // snapshot has no field for it, and inventing one here would
                // mean SIGKILLing a service on the strength of a stop this
                // supervisor never saw. A stop that was still in flight over a
                // re-exec therefore needs to be re-issued to escalate, which
                // is a narrow gap in a rare operation, and the honest one.
                kill_at: None,
                // Whatever start produced the times in this snapshot worked --
                // there was a process, and this branch is the one where it is
                // still there -- so the run they describe is a run the
                // stability test is entitled to believe.
                start_failed: false,
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
                last_restart,
                started_at,
                // Kept, where this branch used to drop it. The run that ended
                // in the hand-off window did become ready, and the moment it
                // did is a fact about this boot that nothing else records --
                // throwing it away made a service that died during a re-exec
                // report "exited before ready" about a run that had been ready
                // for a day. `note_ready_if_present` will not restamp it (it
                // refuses a service that is not running) and the next start
                // clears it, which is what `started_at` does too.
                ready_at,
                first_started_at,
                first_ready_at,
                exited_at: if snapshot.pid.is_some() { Some(now) } else { None },
                retry_at: None,
                manually_stopped: snapshot.manually_stopped,
                // Nothing to escalate: this branch is the one where the
                // process is already gone.
                kill_at: None,
                // As above: the snapshot describes a run that did start, and
                // the failure this flag is about is one this supervisor has
                // not had yet.
                start_failed: false,
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
    ///
    /// `append` is not a detail. The handle returned here is dup'd onto the
    /// child's stdout and stderr and is never touched again by this process,
    /// so from the exec onwards the service is writing to an *inode* rather
    /// than to a path -- which is why `crate::logrotate` rotates by copying
    /// the contents out and truncating this file in place instead of renaming
    /// it. A rename would leave every already-running service appending to the
    /// rotated file for the rest of the boot while `<name>.log` stayed empty.
    /// O_APPEND is also what makes the truncation safe from the child's side:
    /// its next write goes to offset 0 rather than to wherever its file offset
    /// had got to, so a rotated log does not come back full of NUL bytes.
    ///
    /// Nothing here checks the size. Rotation is a sweep on a timer in the
    /// main loop (`maintain_logs`, main.rs), not a check on the write path:
    /// the writes are the children's and init does not see them, and hanging
    /// it off a start instead would mean a service that never restarts is
    /// never looked at.
    fn open_log(&self) -> Option<std::fs::File> {
        let dir = Self::log_dir();
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(format!("{}.log", self.config.name)))
            .ok()
    }

    /// How long a `pre_exec` hook may take here.
    ///
    /// [`PRE_EXEC_TIMEOUT`] everywhere except the tests, which cannot afford
    /// to wait half a minute to prove that the wait ends. Overridable through
    /// the environment for the same reason `log_dir` is, and read on every
    /// start rather than cached so a test can set it and put it back.
    fn pre_exec_timeout() -> Duration {
        std::env::var("RAVEN_PRE_EXEC_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or(PRE_EXEC_TIMEOUT)
    }

    /// Run the configured `pre_exec` hook to completion, or fail the start.
    ///
    /// Bounded, and for the same reason [`Service::run_stop_exec`] is bounded:
    /// this runs on PID 1's only thread, during `start_services`, before the
    /// main loop exists. A hook that never returns used to take the boot with
    /// it -- no further service started, no control socket accepted on,
    /// nothing reaped, and a console showing the service list stopped halfway
    /// with no error on it. A hook that has stopped making progress is the
    /// service's failure, so it is reported as one: the supervisor then
    /// applies the restart backoff to it like any other failed start, which is
    /// a machine that keeps booting and says what is wrong.
    ///
    /// The kill on expiry is not waited on. A hook wedged in uninterruptible
    /// sleep would not die for that wait either, and the main loop's
    /// `waitpid(-1)` reaps whatever the kill does free.
    fn run_pre_exec(&self, program: &str, args: &[String]) -> Result<()> {
        let mut child = Command::new(program)
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
            .spawn()
            .with_context(|| format!("pre_exec: cannot run {}", program))?;

        let timeout = Self::pre_exec_timeout();
        let deadline = Instant::now() + timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        bail!("pre_exec {} exited with {}", program, status);
                    }
                    return Ok(());
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        bail!(
                            "pre_exec {} did not finish within {:?}; killed it",
                            program,
                            timeout
                        );
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => bail!("pre_exec: cannot wait on {}: {}", program, e),
            }
        }
    }

    fn do_start(&mut self) -> Result<()> {
        // Pessimistic from the first line and cleared only where a process
        // exists, so that every one of the many ways out of this function --
        // `exec_problem`, a pre_exec hook that failed or timed out, a spawn
        // that could not fork, the tty path's own errors -- leaves the flag
        // set without each of them having to remember to. What depends on it
        // is the restart backoff; see the field's own comment.
        self.start_failed = true;

        // Checked before anything is created or forked, so the reply names the
        // real problem instead of an errno from deep inside spawn().
        if let Some(problem) = exec_problem(&self.config.exec) {
            bail!("{problem}");
        }

        // Any escalation still owed to the previous run is void from here.
        // The pid this start is about to record is a different process, and a
        // deadline set for the old one would have the main loop SIGKILL the
        // new one seconds after it came up -- `stop` followed by `start`
        // inside the stop timeout is exactly how an operator restarts a
        // service by hand.
        self.kill_at = None;

        // Setup the daemon expects someone else to have done -- generating
        // host keys, say. Run to completion first, and a failure is the
        // service's failure: starting sshd with no keys just moves the error
        // into a crash loop.
        if let Some((program, args)) = self.config.pre_exec.split_first() {
            self.run_pre_exec(program, args)?;
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

        // Resource control, in two halves. The cgroup directory and the limits
        // that belong to it (memory.max, cpu.weight, io.weight) are set here,
        // in the parent, because they are properties of the directory and can
        // be written before there is a process to put in it. The per-process
        // half -- joining the cgroup, nice, oom_score_adj and the rlimits --
        // is packed up now and applied in the child below.
        //
        // A machine without cgroup2, or with a read-only /sys/fs/cgroup, gets
        // `None` here and a warning once from the cgroup module; the service
        // then starts unconfined, which is the only acceptable failure mode
        // for a supervisor that is also the only way to reach a shell.
        let cgroup = crate::cgroup::Cgroup::for_service(&self.config.name);
        if let Some(cg) = &cgroup {
            cg.apply(&self.config);
        }
        let resources = crate::cgroup::ChildResources::prepare(&self.config, cgroup.as_ref());

        // Registered BEFORE the credential drop below, and the order is not
        // incidental: `std` runs pre_exec closures in registration order, and
        // every one of these settings needs privilege the closure after it
        // gives away. Writing to a cgroup.procs init owns, lowering
        // oom_score_adj below zero, asking for a negative nice value and
        // raising a hard rlimit all require root or a capability that comes
        // with it, and `apply_credentials` calls setuid. Swap the two and the
        // service starts anyway, with none of its limits and nothing said
        // about it -- the same invisible failure the tty path refuses `user =`
        // over.
        if !resources.is_empty() {
            // SAFETY: the closure runs in the forked child between `fork` and
            // `exec`, where only async-signal-safe work is permitted. It
            // allocates nothing, takes no lock and logs nothing: every path it
            // opens was turned into a CString in this parent, every number it
            // writes was formatted here, and the calls it makes are open,
            // write, close, setpriority and setrlimit, all raw syscalls. See
            // `ChildResources::apply_in_child` for the per-call argument, and
            // `apply_credentials` below for the same reasoning applied to the
            // credential syscalls.
            unsafe {
                cmd.pre_exec(move || {
                    resources.apply_in_child();
                    Ok(())
                });
            }
        }

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

        let now = Instant::now();
        self.child = Some(child);
        self.pid = Some(pid);
        self.state = ServiceState::Running;
        self.started_at = Some(now);
        // Set once and then carried, restart after restart, because this is
        // the number `blame` reports; see `Service::first_started_at`.
        self.first_started_at.get_or_insert(now);
        self.ready_at = None;
        self.exited_at = None;
        self.exit_status = None;
        self.exit_signal = None;
        // There is a process, so `started_at` above now describes this run and
        // the stability test may believe it again.
        self.start_failed = false;

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

        // The environment block the child will exec with, assembled here in
        // the parent. The child used to call `std::env::set_var`, which is
        // glibc's `setenv`: it allocates and takes an internal lock, and
        // between `fork` and `exec` neither is allowed -- a lock another
        // thread held at the instant of the fork is held forever in the copy.
        // `execvpe` takes the block whole rather than adding to what is
        // inherited, so init's own variables are copied in alongside the
        // service's; where a definition names a variable init also has, the
        // definition's value wins, which is what `setenv` did.
        let mut env_cstr: Vec<CString> = Vec::new();
        for (key, value) in std::env::vars_os() {
            if key
                .to_str()
                .is_some_and(|k| self.config.environment.contains_key(k))
            {
                continue;
            }
            let mut pair = key.into_vec();
            pair.push(b'=');
            pair.extend_from_slice(value.as_bytes());
            // An interior NUL cannot come out of `environ`, and this is PID 1
            // with a release profile that aborts on unwind, so an impossible
            // entry is dropped rather than panicked over.
            if let Ok(entry) = CString::new(pair) {
                env_cstr.push(entry);
            }
        }
        for (key, value) in &self.config.environment {
            env_cstr.push(
                CString::new(format!("{key}={value}"))
                    .with_context(|| format!("Invalid environment entry: {key}"))?,
            );
        }

        // The two NULL-terminated pointer arrays `execvpe` wants, built here
        // for the same reason: `nix::unistd::execvp` builds this vector itself,
        // in the child, which is an allocation in the window where allocating
        // can deadlock.
        let argv: Vec<*const libc::c_char> = args_cstr
            .iter()
            .map(|a| a.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect();
        let envp: Vec<*const libc::c_char> = env_cstr
            .iter()
            .map(|e| e.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect();

        let tty_path_owned = tty_path.to_string();

        // The device as a CString, so that the child's `open` is a raw syscall
        // over a buffer this parent allocated.
        let tty_cstr =
            CString::new(tty_path).with_context(|| format!("Invalid tty path: {tty_path}"))?;

        // Everything the child might have to say, formatted here because it
        // may not format anything itself; see `child_say`. Each is a complete
        // line, so it reaches the console in one `write` and cannot be
        // interleaved with a line PID 1 is logging at the same moment.
        let say = |what: &str| {
            format!(
                "raven-init: {}: {} ({})\n",
                self.config.name, what, tty_path
            )
            .into_bytes()
        };
        let msg_setsid = say("setsid failed");
        let msg_open = say("cannot open tty");
        let msg_ctty = say("TIOCSCTTY failed, continuing");
        let msg_dup2 = say("cannot put the tty on stdin/stdout/stderr");
        let msg_fgpgrp = say("tcsetpgrp failed, continuing");
        let msg_exec = say(&format!("cannot exec {}", self.config.exec));

        // Resource control, prepared before the fork for the same reason the
        // argument vector is: after `fork` this child may not allocate, and
        // after `execvp` there is no longer anything of ours running to do it.
        //
        // This path is a hand-rolled fork/execvp and gets none of `Command`'s
        // pre_exec machinery, so everything `do_start` registers as a closure
        // has to be repeated here by hand. It is repeated rather than refused
        // -- unlike `user =` above -- because unlike a privilege drop, a
        // limit that silently did not apply to the gettys would be a gap in
        // exactly the services most likely to be left running a runaway
        // program somebody typed.
        let cgroup = crate::cgroup::Cgroup::for_service(&self.config.name);
        if let Some(cg) = &cgroup {
            cg.apply(&self.config);
        }
        let resources = crate::cgroup::ChildResources::prepare(&self.config, cgroup.as_ref());

        // Fork the process
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent process - just record the child PID
                let now = Instant::now();
                self.pid = Some(child);
                self.started_at = Some(now);
                // As in `do_start`: the boot's first start, kept across every
                // restart of this getty.
                self.first_started_at.get_or_insert(now);
                self.ready_at = None;
                self.exited_at = None;
                self.child = None; // We don't have a Child handle when using fork directly
                self.state = ServiceState::Running;
                self.exit_status = None;
                self.exit_signal = None;
                // As in `do_start`: the fork returned a child, so this run is
                // real and the times just recorded describe it.
                self.start_failed = false;

                log::info!(
                    "Service {} started with PID {} on TTY {}",
                    self.config.name,
                    child,
                    tty_path_owned
                );

                Ok(())
            }
            Ok(ForkResult::Child) => {
                // Child process - set up TTY and exec.
                //
                // Everything from here to the `execvpe` at the bottom runs
                // between `fork` and `exec` in a copy of PID 1, and keeps the
                // discipline `ChildResources::apply_in_child` documents:
                // no allocation, no lock, no logging, no `exit(3)`. Every
                // string it needs was built in the parent above.

                // 0. Start from a standard signal state, the way `Command`
                // does. Without this the console session inherits PID 1's
                // ignored SIGPIPE; see `reset_signals_for_exec` for what that
                // does to a pipeline.
                //
                // SAFETY: this is the child of a `fork` and has not yet
                // exec'd, which is the window that function requires.
                unsafe {
                    reset_signals_for_exec();
                }

                // 1. Create a new session (become session leader)
                if setsid().is_err() {
                    unsafe { child_abort(&msg_setsid) };
                }

                // 1b. Join the cgroup and take the configured limits.
                //
                // Before the TTY is opened, so that a `nofile` limit is in
                // force for every descriptor this child goes on to acquire,
                // and while the process is still root -- which on this path it
                // stays, since `user =` is refused above.
                //
                // SAFETY: this is the child of a `fork` and has not yet
                // exec'd, which is exactly the window `apply_in_child`
                // documents. Nothing it touches was allocated after the fork.
                unsafe {
                    resources.apply_in_child();
                }

                // 2. Open the TTY device
                let tty_fd: RawFd =
                    unsafe { libc::open(tty_cstr.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
                if tty_fd < 0 {
                    unsafe { child_abort(&msg_open) };
                }

                // 3. Set this TTY as the controlling terminal
                // TIOCSCTTY with arg 0 means "don't steal if already controlled"
                if unsafe { tiocsctty(tty_fd, 0) }.is_err() {
                    // Continue anyway - some systems may not require this
                    unsafe { child_say(&msg_ctty) };
                }

                // 4. Duplicate TTY fd to stdin/stdout/stderr
                for target in 0..=2 {
                    if unsafe { libc::dup2(tty_fd, target) } < 0 {
                        unsafe { child_say(&msg_dup2) };
                    }
                }

                // Close the original fd if it's not 0, 1, or 2
                if tty_fd > 2 {
                    unsafe { libc::close(tty_fd) };
                }

                // 5. Set the foreground process group to our process group
                let our_pid = unistd::getpid();
                if unsafe { libc::tcsetpgrp(0, our_pid.as_raw()) } < 0 {
                    // Continue anyway
                    unsafe { child_say(&msg_fgpgrp) };
                }

                // 6. Exec the service, with the environment built above.
                //
                // `execvpe` rather than `execvp`: the variables the definition
                // asks for are in `envp`, because setting them here would mean
                // calling `setenv` after a fork.
                unsafe { libc::execvpe(exec_cstr.as_ptr(), argv.as_ptr(), envp.as_ptr()) };

                // If we get here, exec failed
                unsafe { child_abort(&msg_exec) };
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
        // A start that failed produced no run at all, so there is nothing here
        // to call stable. The times below still hold the last run that really
        // happened -- `do_start` bails before it touches them -- and believing
        // them would credit an hour of uptime to an attempt that never got as
        // far as a process, resetting the backoff on every pass. See
        // `start_failed`.
        if self.start_failed {
            return false;
        }

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
        self.release_cgroup();

        if self.is_oneshot() {
            if status == 0 {
                // A one-shot's finish is its readiness. It is recorded through
                // `mark_ready` rather than written here so that `ready_at`
                // keeps its two writers and its first-wins rule, and because
                // everything already built on top of that field then works for
                // one-shots without knowing they exist: `blame` gets a READY
                // column and a TOOK that measure the run, the boot summary's
                // "last ready" counts a coldplug that finished at 3.2s, and
                // the hand-off carries the time across a re-exec in
                // `first_ready_mono` -- which `exited_at` is not and cannot
                // easily become, since the snapshot has no field for it.
                //
                // The moment recorded is a hair later than `exited_at`: both
                // are `Instant::now()`, taken a few instructions apart, and
                // `blame` prints milliseconds.
                self.mark_ready();
                log::info!("One-shot {} completed", self.config.name);
            } else {
                // The one exit on this machine that is unambiguously a
                // failure, and therefore the one worth the console. Every
                // other exit here is logged at info, because a daemon that
                // exits is usually about to be restarted and the restart
                // already says so at warn.
                log::error!(
                    "One-shot {} failed with status {}",
                    self.config.name,
                    status
                );
            }
        } else {
            log::info!("Service {} exited with status {}", self.config.name, status);
        }
    }

    /// Mark service as killed by signal
    pub fn mark_signaled(&mut self, signal: Signal) {
        self.state = ServiceState::Signaled;
        self.exit_signal = Some(signal);
        self.exited_at = Some(Instant::now());
        self.pid = None;
        self.child = None;
        self.release_cgroup();

        log::info!("Service {} killed by signal {:?}", self.config.name, signal);
    }

    /// Restart the service
    pub fn restart(&mut self) -> Result<()> {
        self.manually_stopped = false;
        self.restart_count = self.restart_count.saturating_add(1);
        self.last_restart = Some(Instant::now());
        // Cleared so that the next death decides a fresh delay rather than
        // comparing against a deadline that has already fired. When the start
        // below *fails* this leaves no pending retry, which is deliberate: the
        // next tick recomputes one from the now-incremented `restart_count`,
        // and `do_start` has set `start_failed`, so the schedule escalates
        // 1s, 2s, 4s ... instead of restarting from the base delay forever.
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
            log::debug!("Sending SIGTERM to {} (leader {})", self.config.name, pid);
            self.signal_everything(pid, Signal::SIGTERM);
        }
    }

    /// Send one signal to every process this service still has.
    ///
    /// The cgroup is asked first and the process group is the fallback, in
    /// that order and for that reason: a service's cgroup holds everything it
    /// forked, including the daemons that called `setsid` and left the process
    /// group init put them in, while the process group holds only what stayed.
    /// Before the slice existed, `raven-rc stop bluetoothd` reached bluetoothd
    /// and left whatever it had daemonised behind, still holding its sockets,
    /// until the next reboot.
    ///
    /// The fallback is not vestigial and must stay. A service adopted across a
    /// re-exec from a raven-init old enough to have had no cgroups is in no
    /// cgroup at all -- cgroup membership is a property of the process, so the
    /// exec that replaced the supervisor could not have changed it -- and a
    /// machine with no cgroup2 mounted has nothing but process groups from
    /// boot to shutdown. In both cases [`Cgroup::existing`] finds nothing and
    /// says so, and this is exactly the code that ran before.
    ///
    /// What does *not* change here is escalation. This sends the one signal it
    /// is given; SIGTERM first and SIGKILL after `stop_timeout` remains the
    /// caller's sequence, in `wait_for_exit` and in `shutdown_services`.
    fn signal_everything(&self, pid: Pid, signal_to_send: Signal) {
        if let Some(cgroup) = crate::cgroup::Cgroup::existing(&self.config.name) {
            if cgroup.signal_all(signal_to_send) {
                return;
            }
        }
        signal_service_group(pid, signal_to_send);
    }

    /// Give a dead service's cgroup directory back to the kernel.
    ///
    /// Called from the two places a service is recorded as having died, which
    /// is the only moment the answer can be known: while the leader lives the
    /// directory is in use by definition, and afterwards it is in use only if
    /// something the service forked outlived it -- in which case the rmdir
    /// fails with EBUSY, the directory stays, and that surviving process
    /// remains visible to `raven-rc status` and reachable by the next `stop`.
    /// See [`Cgroup::remove`] for why that failure is neither retried nor
    /// waited on.
    fn release_cgroup(&self) {
        if let Some(cgroup) = crate::cgroup::Cgroup::existing(&self.config.name) {
            cgroup.remove();
        }
    }

    /// Stop the service on an operator's request, and keep it stopped.
    ///
    /// Runs the configured `stop_exec` first so a daemon can leave cleanly --
    /// the same courtesy shutdown extends, and the reason cawd can deauthenticate
    /// from its AP instead of vanishing mid-association.
    ///
    /// Escalation is deferred rather than waited out: the deadline is recorded
    /// and [`Service::escalate_stop_if_due`] acts on it from the main loop, so
    /// the `raven-rc` client gets its reply now and PID 1 is not held for
    /// `stop_timeout` inside a control request.
    pub fn stop_by_request(&mut self) {
        self.manually_stopped = true;

        if self.has_stop_exec() {
            self.run_stop_exec();
        }

        self.stop();

        // Only where there is something to escalate against. `stop()` is a
        // no-op without a pid, and a deadline recorded for a service that was
        // already gone would be a SIGKILL aimed at nothing -- or, worse, at
        // whatever `pid` came to mean later.
        if self.pid.is_some() {
            self.kill_at =
                Some(Instant::now() + Duration::from_secs(self.config.stop_timeout as u64));
        }
    }

    /// Turn a requested stop that SIGTERM did not achieve into a SIGKILL.
    ///
    /// Called once per main-loop pass for every service. Does nothing until
    /// the deadline `stop_by_request` recorded has passed, and nothing at all
    /// for a service that stopped when it was asked to, which is all of them
    /// on a normal machine.
    ///
    /// It signals and returns: the corpse is collected by the main loop's
    /// `waitpid(-1)` on one of the next passes, exactly as for a service that
    /// died on its own. Waiting here would put the hang this exists to avoid
    /// straight back into PID 1's thread -- see [`Service::reap_after_kill`]
    /// for why no wait on a killed process can be trusted to end.
    ///
    /// Returns whether it sent the signal, for the caller's log line.
    pub fn escalate_stop_if_due(&mut self) -> bool {
        let Some(deadline) = self.kill_at else {
            return false;
        };
        if Instant::now() < deadline {
            return false;
        }

        // Spent either way: if the service is still there after a SIGKILL,
        // sending it another one every pass forever would say nothing new.
        self.kill_at = None;

        // The reaper runs at the top of the same pass, but a service that died
        // between then and now is still recorded as running, and a SIGKILL
        // sent on that record goes to a pid this supervisor no longer owns.
        self.poll_exit();
        let Some(pid) = self.pid.filter(|_| self.is_running()) else {
            return false;
        };

        log::warn!(
            "{} did not exit within {}s of being stopped; sending SIGKILL",
            self.config.name,
            self.config.stop_timeout
        );
        self.signal_everything(pid, Signal::SIGKILL);
        true
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
                        self.signal_everything(pid, Signal::SIGKILL);
                        self.reap_after_kill(pid);
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

    /// Collect a service that has just been SIGKILLed, and give up if it does
    /// not come.
    ///
    /// This used to be `waitpid(pid, None)` -- no WNOHANG, no deadline -- in a
    /// function whose own documentation says it is bounded because it runs on
    /// PID 1's thread. SIGKILL is not the guarantee that made that look safe:
    /// a task in uninterruptible sleep (the wedged USB volume that
    /// `raven-mount` was writing to when it was pulled out) does not act on
    /// any signal until its I/O errors out, which for a confused bridge chip
    /// may be never. PID 1 blocked there is a machine that reaps nothing,
    /// answers no further `raven-rc` request -- including the one that issued
    /// this restart -- notices no ready path and does not honour poweroff,
    /// until something unrelated happens to die and breaks the call with
    /// EINTR. Recovery was the reset button.
    ///
    /// So the wait has a deadline of its own, and past it the process is left
    /// where it is. What that costs is a pid the supervisor no longer tracks:
    /// if the process does eventually die, the main loop's `waitpid(-1)` reaps
    /// it and finds no service to attribute it to, and if it never dies it
    /// stays in `ps` as the D-state task it already was. A leaked corpse is
    /// survivable; a supervisor that has stopped supervising is not.
    ///
    /// The bookkeeping below is what the blocking version did on return, and
    /// deliberately not `mark_exited`/`mark_signaled`: the caller's contract
    /// is that the service is gone as far as this supervisor is concerned, and
    /// a killed service must not come back through the restart path.
    fn reap_after_kill(&mut self, pid: Pid) {
        let deadline = Instant::now() + SIGKILL_REAP_GRACE;
        loop {
            match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
                // Gone, and reaped here: nothing is left for the main loop.
                Ok(WaitStatus::Exited(..)) | Ok(WaitStatus::Signaled(..)) => break,
                Ok(WaitStatus::StillAlive) => {
                    if Instant::now() >= deadline {
                        log::error!(
                            "{} (leader {}) has not died {:?} after SIGKILL -- \
                             giving up the wait rather than blocking the supervisor; \
                             it is probably stuck in uninterruptible I/O",
                            self.config.name,
                            pid,
                            SIGKILL_REAP_GRACE
                        );
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                // ECHILD: the main loop's reaper got there first. Anything
                // else (EINTR from a SIGCHLD for an unrelated child, since the
                // handler is installed without SA_RESTART) is worth another
                // pass, and the deadline above ends it either way.
                Err(nix::errno::Errno::ECHILD) => break,
                Err(_) | Ok(_) => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }

        self.state = ServiceState::Stopped;
        self.pid = None;
        self.child = None;
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
    /// When the current run was started.
    pub fn started_at(&self) -> Option<Instant> {
        self.started_at
    }

    /// The path that proves this service ready, if its definition names one.
    pub fn ready_path(&self) -> Option<&str> {
        self.config.ready_path.as_deref()
    }

    /// When the ready path was first seen for this run, if it has been.
    pub fn ready_at(&self) -> Option<Instant> {
        self.ready_at
    }

    /// When this service first started in this boot, restarts notwithstanding.
    /// The boot timeline's start column; see the field for why it is not
    /// [`Service::started_at`].
    pub fn first_started_at(&self) -> Option<Instant> {
        self.first_started_at
    }

    /// When this service was first ready in this boot, restarts
    /// notwithstanding. `None` for a service that has never been ready, with
    /// or without a ready path to be ready at -- [`Service::ready_path`] is
    /// what tells those two apart.
    pub fn first_ready_at(&self) -> Option<Instant> {
        self.first_ready_at
    }

    /// When the supervisor last restarted this service, if it ever has.
    ///
    /// A lifetime fact rather than a boot one, which is why it is reported by
    /// `raven-rc status` and not by `blame`: the count says a service has been
    /// coming back, and this says whether that was all morning or just now.
    pub fn last_restart_at(&self) -> Option<Instant> {
        self.last_restart
    }

    /// Record that the ready path has been seen. Idempotent: the first time
    /// wins, because that is the number a boot timeline wants.
    pub fn mark_ready(&mut self) {
        if self.ready_at.is_none() {
            let now = Instant::now();
            self.ready_at = Some(now);
            self.first_ready_at.get_or_insert(now);
        }
    }

    /// Check the ready path once, without waiting, and record it if present.
    /// Returns true the first time it is seen. Cheap enough for the main
    /// loop: one stat per service that has a path and is not yet ready.
    pub fn note_ready_if_present(&mut self) -> bool {
        if self.ready_at.is_some() || !self.is_running() {
            return false;
        }
        match self.config.ready_path.as_deref() {
            Some(path) if std::path::Path::new(path).exists() => {
                let now = Instant::now();
                self.ready_at = Some(now);
                // The boot's first ready, which a later restart of this
                // service must not move: see `Service::first_ready_at`.
                self.first_ready_at.get_or_insert(now);
                true
            }
            _ => false,
        }
    }

    pub fn description(&self) -> &str {
        &self.config.description
    }

    /// Whether the config asks for automatic restart on exit.
    pub fn restart_configured(&self) -> bool {
        self.config.restart
    }

    /// Whether this service's definition says its job is to finish.
    pub fn is_oneshot(&self) -> bool {
        self.config.service_type == ServiceType::Oneshot
    }

    /// What this one-shot has to say for itself, or `None` for a service that
    /// is not one.
    ///
    /// `None` rather than a fifth variant meaning "not applicable", so that
    /// every caller has to say out loud which kind of service it is talking
    /// about before it can use any of the one-shot words. The display sites
    /// all read the same way because of it: ask, and if there is an answer,
    /// use it instead of the ordinary description.
    pub fn oneshot_outcome(&self) -> Option<OneshotOutcome> {
        if !self.is_oneshot() {
            return None;
        }

        if self.is_running() {
            return Some(OneshotOutcome::Running);
        }

        Some(match (self.state, self.exit_status) {
            (ServiceState::Exited, Some(0)) => OneshotOutcome::Completed,
            (ServiceState::Exited, Some(code)) => OneshotOutcome::Failed(code),
            // A signal is never a finish, whatever else is recorded.
            (ServiceState::Signaled, _) => OneshotOutcome::Unfinished,
            // Gone with no status recorded. Two ways to arrive here, and the
            // ready time answers both: `poll_exit`'s ECHILD branch, where the
            // main loop's reaper got the status first and this service only
            // knows the process is gone; and a service adopted across a
            // re-exec, where the hand-off carries the times and not the
            // status. `mark_exited` records a one-shot's readiness only on a
            // clean exit and `do_start` clears it on every start, so a ready
            // time on a finished one-shot means precisely "this run finished,
            // and it finished well".
            _ if self.ready_at.is_some() => OneshotOutcome::Completed,
            // Killed, stopped, or never run. Claiming a failure on no
            // evidence is worse than admitting the evidence is missing.
            _ => OneshotOutcome::Unfinished,
        })
    }

    /// Wait for a one-shot to finish, for up to `timeout`, and report how it
    /// went. `None` for a service that is not a one-shot, which has no
    /// finishing to wait for.
    ///
    /// This is what `after = ["<a one-shot>"]` is worth. A long-running
    /// service announces itself by creating a file and a dependant waits on an
    /// inotify watch for it; a one-shot announces itself by leaving, and the
    /// only way to hear that is `waitpid`. The loop is the same shape as
    /// [`Service::wait_for_exit`] -- WNOHANG on a 20ms timer -- for the same
    /// reason: this is called from the boot path and from a control request,
    /// and in both it is PID 1's only thread that is waiting, so it must be
    /// bounded and it must not block in the kernel with a signal pending.
    ///
    /// Returning [`OneshotOutcome::Running`] means the timeout ran out with
    /// the one-shot still working. That is not a failure and it is reported as
    /// itself: the caller decides what to do about a one-shot that is taking
    /// longer than its `ready_timeout`, and on this machine what it does is
    /// carry on and say so.
    pub fn wait_until_finished(&mut self, timeout: Duration) -> Option<OneshotOutcome> {
        if !self.is_oneshot() {
            return None;
        }

        let deadline = Instant::now() + timeout;
        loop {
            // Reaps the child itself. Nothing else can: during boot the main
            // loop does not exist yet, and after it does, a `raven-rc start`
            // is being served from inside that loop rather than beside it.
            self.poll_exit();

            let outcome = self.oneshot_outcome();
            if outcome != Some(OneshotOutcome::Running) {
                return outcome;
            }

            if Instant::now() >= deadline {
                return outcome;
            }

            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Kill the service (SIGKILL)
    pub fn kill(&mut self) {
        if let Some(pid) = self.pid {
            log::debug!("Sending SIGKILL to {} (leader {})", self.config.name, pid);
            self.signal_everything(pid, Signal::SIGKILL);
        }
        self.state = ServiceState::Stopped;
        self.pid = None;
        self.child = None;
    }
}

/// Signal a service and every child that stayed in its process group.
///
/// The fallback for a service with no cgroup -- see
/// [`Service::signal_everything`], which reaches for this only when the slice
/// holds nothing for the service. It is the whole of what stopping a service
/// used to be, and it is kept verbatim because the cases it covers are real:
/// no cgroup2 on the machine, and a service adopted from a raven-init that
/// predates the slice.
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

#[cfg(test)]
mod restart_backoff_tests {
    use super::*;

    /// A service that has never run, built by hand rather than started, so the
    /// times the backoff reads can be set to the ones the case is about
    /// without waiting a minute of real time for them.
    fn idle_service(exec: &str) -> Service {
        Service {
            config: ServiceConfig {
                name: "backoff-probe".to_string(),
                exec: exec.to_string(),
                restart: true,
                ..ServiceConfig::default()
            },
            state: ServiceState::Exited,
            child: None,
            pid: None,
            exit_status: Some(1),
            exit_signal: None,
            restart_count: 0,
            last_restart: None,
            started_at: None,
            ready_at: None,
            first_started_at: None,
            first_ready_at: None,
            exited_at: None,
            retry_at: None,
            manually_stopped: false,
            kill_at: None,
            start_failed: false,
        }
    }

    /// The backoff must escalate when a service that ran happily for an hour
    /// can no longer be started at all.
    ///
    /// `restart()` clears `retry_at` before calling `do_start`, and `do_start`
    /// fails -- the binary was removed, or its filesystem is not mounted --
    /// before it touches `state`, `started_at` or `exited_at`. So the next
    /// tick found no pending retry and recomputed the whole decision from
    /// fields that still described the *previous*, hour-long run:
    /// `last_run_was_stable` said yes, `restart_count` was zeroed, the delay
    /// came out at RESTART_BACKOFF_BASE, and it came out at
    /// RESTART_BACKOFF_BASE again one second later, forever. Two log lines a
    /// second to the console and to /var/log/raven/init.log for the life of
    /// the boot is precisely the flood RESTART_BACKOFF_BASE's own doc comment
    /// says the backoff exists to stop, and `raven-rc status` reported
    /// "restart count 1" through all of it.
    #[test]
    fn a_failing_start_after_a_stable_run_still_backs_off() {
        // Times are built forwards from now rather than backwards, because an
        // `Instant` an hour before this machine booted does not exist.
        let started = Instant::now();
        let died = started + RESTART_STABLE_AFTER + Duration::from_secs(1);

        let mut svc = idle_service("/nonexistent/raven-backoff-probe");
        svc.started_at = Some(started);
        svc.exited_at = Some(died);

        // The first tick after the death: the run was long, so the schedule
        // starts over at the base delay. This much was always right.
        let mut now = died;
        assert!(!svc.should_restart_at(now));
        assert_eq!(svc.retry_at(), Some(now + RESTART_BACKOFF_BASE));

        // Every attempt from here fails before there is a process. The delays
        // must double rather than stand still.
        let expected = [
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8),
            Duration::from_secs(16),
            Duration::from_secs(32),
            RESTART_BACKOFF_MAX,
            RESTART_BACKOFF_MAX,
        ];
        for (attempt, want) in expected.iter().enumerate() {
            now = svc.retry_at().expect("a restart is pending");
            assert!(svc.should_restart_at(now), "the retry is due");
            assert!(
                svc.restart().is_err(),
                "the whole case is a start that cannot succeed"
            );
            assert!(
                !svc.should_restart_at(now),
                "the failed start leaves the next retry to be decided"
            );
            assert_eq!(
                svc.retry_at(),
                Some(now + *want),
                "attempt {} must wait {:?}",
                attempt + 2,
                want
            );
        }

        // And the count an operator reads has to be the number of attempts
        // that really happened, not one forever.
        assert_eq!(svc.restart_count(), expected.len() as u32);
    }

    /// The flag exists to describe the *last start*, so a start that works
    /// must clear it -- otherwise the first fix of a broken service would
    /// leave it permanently ineligible for the stable-run reset.
    #[test]
    fn a_successful_start_clears_the_failure_flag() {
        let mut svc = idle_service("/nonexistent/raven-backoff-probe");
        assert!(svc.restart().is_err());
        assert!(svc.start_failed, "a start that never reached a process");

        svc.config.exec = "/bin/sleep".to_string();
        svc.config.args = vec!["300".to_string()];
        svc.restart().expect("this one has a binary");
        assert!(!svc.start_failed, "there is a process now");
        assert!(svc.is_running());

        svc.kill();
    }
}
