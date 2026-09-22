//! RavenInit - PID 1 Init System for RavenLinux
//!
//! A simple, robust init system that:
//! - Mounts essential filesystems (proc, sys, dev, etc.)
//! - Handles signal propagation and zombie reaping
//! - Manages service startup and shutdown
//! - Supports runlevels/targets (boot, default, shutdown)

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use nix::mount::{mount, MsFlags};
use nix::sys::reboot::{reboot, RebootMode};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::os::fd::AsFd;

mod cgroup;
mod config;
mod control;
mod demand;
mod logrotate;
mod overrides;
mod power;
mod sleepmark;
mod readiness;
mod reexec;
mod rtc;
mod service;
mod sysctl;
mod timeline;
mod user;
mod usermode;

use config::{InitConfig, ServiceConfig, ServiceType};
use service::{OneshotOutcome, Service, ServiceState};

/// Global flag for shutdown request
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static REBOOT_REQUESTED: AtomicBool = AtomicBool::new(false);
/// Set by `raven-rc reexec`; the main loop returns `LoopExit::Reexec`.
static REEXEC_REQUESTED: AtomicBool = AtomicBool::new(false);

/// The write end of the self-pipe the SIGCHLD handler pokes, so a child
/// exiting wakes the main loop's poll. -1 until `child_wakeup` sets it up.
static CHILD_WAKE_FD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

extern "C" fn on_sigchld(_: libc::c_int) {
    let fd = CHILD_WAKE_FD.load(Ordering::SeqCst);
    if fd >= 0 {
        // SAFETY: write(2) is async-signal-safe; the fd is non-blocking, so a
        // full pipe just drops the byte, and one byte is all a wake needs.
        unsafe {
            libc::write(fd, [1u8].as_ptr() as *const libc::c_void, 1);
        }
    }
}

/// The main loop used to sleep 100 ms between looks, ten wakeups a second
/// for the life of the machine, and a raven-rc request waited up to a tick.
/// Now it sleeps in poll(2) on the control socket and on this pipe, which a
/// SIGCHLD handler writes to, so a child dying or a client connecting wakes
/// it at once, and it only ticks on a timer while something is pending
/// (see control::wants_quick_tick). A self-pipe rather than a signalfd
/// because a signalfd needs SIGCHLD blocked, and a blocked signal is
/// inherited by every service raven-init spawns.
///
/// Returns the read end. `None` means the loop keeps its old timer.
fn child_wakeup() -> Option<std::os::fd::OwnedFd> {
    use nix::fcntl::OFlag;
    use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};
    let (read, write) = nix::unistd::pipe2(OFlag::O_NONBLOCK | OFlag::O_CLOEXEC).ok()?;
    let write_raw = std::os::fd::IntoRawFd::into_raw_fd(write);
    CHILD_WAKE_FD.store(write_raw, Ordering::SeqCst);
    let action = SigAction::new(
        SigHandler::Handler(on_sigchld),
        // No SA_RESTART: a blocked poll is exactly what should be interrupted.
        SaFlags::SA_NOCLDSTOP,
        SigSet::empty(),
    );
    // SAFETY: the handler only calls write(2) on a static fd.
    unsafe { sigaction(Signal::SIGCHLD, &action) }.ok()?;
    Some(read)
}

fn main() {
    // `--user`: supervise a session, not the machine. See usermode.rs.
    // `--user --check` answers "does this binary know user mode" with its
    // exit status, so a launcher can tell a new raven-init from an old one
    // that would refuse to run beside PID 1.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--user") {
        if args.iter().any(|a| a == "--check") {
            return;
        }
        if let Err(e) = run_user() {
            eprintln!("raven-init --user: {:#}", e);
            std::process::exit(1);
        }
        return;
    }

    // Refuse to be a second supervisor before doing anything at all -- not
    // even logging setup. This used to be a warning followed by "continue
    // anyway for testing purposes", which is how a raven-init started from a
    // shell on a running system ended up fighting PID 1 for cawd's
    // socket.
    //
    // Exits rather than dropping to the emergency shell: that shell exists for
    // "PID 1 could not boot the system", and stranding someone in it because
    // they typed a command on a working machine is not an improvement.
    if let Err(e) = ensure_supervisor_role() {
        eprintln!("raven-init: {}", e);
        std::process::exit(1);
    }

    // Initialize logging
    init_logging();

    timeline::mark("init started");
    log::info!("RavenInit starting...");

    // Run the init sequence
    if let Err(e) = run_init() {
        log::error!("Init failed: {:#}", e);
        // Try to drop to emergency shell
        emergency_shell();
    }
}

/// Logs to stderr (the console) and, once it can, to /var/log/raven/init.log.
///
/// The console copy is what you watch during boot; the file is what you read
/// after the console has moved on -- fbcon has had no scrollback since kernel
/// 5.9, so a message that leaves the screen is otherwise simply gone.
struct DualLogger {
    path: std::path::PathBuf,
}

impl log::Log for DualLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // Stamped with seconds since the kernel started, dmesg's clock, so
        // a slow boot can be read off the log instead of guessed at.
        let line = format!(
            "[raven-init] [{:9.3}] {}: {}\n",
            timeline::monotonic_secs(),
            record.level(),
            record.args()
        );
        // The console shows only what needs a human: WARN and ERROR. INFO
        // lines -- every "Started service", every clean exit -- go to the log
        // file alone, because after the gettys are up the console belongs to
        // whoever is logging in, and init chatter printed over a login prompt
        // reads as a broken boot to exactly the person it should reassure.
        if record.level() <= log::Level::Warn {
            eprint!("{}", line);
        }
        if let Ok(mut guard) = LOG_FILE.lock() {
            if LOG_FILE_CLOSED.load(Ordering::SeqCst) {
                return;
            }
            // Opened lazily: /var/log may not be writable until the root is
            // mounted rw, and boot must not wait on it.
            if guard.is_none() {
                if let Some(dir) = self.path.parent() {
                    std::fs::create_dir_all(dir).ok();
                }
                *guard = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)
                    .ok();
            }
            if let Some(ref mut f) = *guard {
                use std::io::Write as _;
                let _ = f.write_all(line.as_bytes());
            }
        }
    }

    fn flush(&self) {}
}

/// init.log, once opened. A static rather than a field of the logger so
/// shutdown can let go of it: the boxed logger is unreachable once installed.
static LOG_FILE: std::sync::Mutex<Option<std::fs::File>> = std::sync::Mutex::new(None);

/// Set once shutdown needs `/` read-only: from then on the log goes to the
/// console alone and its file is never reopened.
static LOG_FILE_CLOSED: AtomicBool = AtomicBool::new(false);

/// Stop writing init.log and let go of it. The closing line is written first,
/// so the file says why it ends where it does.
fn close_log_file() {
    log::info!("Closing init.log; anything further goes to the console only");
    LOG_FILE_CLOSED.store(true, Ordering::SeqCst);
    if let Ok(mut guard) = LOG_FILE.lock() {
        guard.take();
    }
}

/// Rotate and prune the log directory, if the sweep is due.
///
/// Called once per pass of the main loop and does nothing on almost all of
/// them: `Schedule` lets one pass a minute through, and the loop already wakes
/// at least every two seconds, so this adds no wakeup to an idle machine. The
/// whole cost on a pass that is not due is one comparison of two `Instant`s.
///
/// # Why init.log is handled separately
///
/// Every other file in the directory is being written by a child process
/// through an fd this supervisor handed over and cannot reach, so its rotation
/// has the copy-and-truncate window that `logrotate`'s header describes -- a
/// line written between the copy and the truncation is lost, and there is no
/// lock that would prevent it because the writer is another process.
///
/// init.log has no such problem and it would be a shame to waste that. Init is
/// the only writer of its own log and it writes through [`LOG_FILE`], so
/// holding that mutex across the rotation makes the window disappear: no line
/// can be written while the copy and the truncation happen, and the `File`
/// inside the mutex stays valid throughout because the inode never changes.
///
/// That is also why nothing inside `logrotate` logs, and why the reports are
/// collected and printed after the guard is dropped. `log::warn!` reached from
/// inside that block would try to lock the same non-reentrant mutex on the
/// same thread, and PID 1 blocked forever in its own logger does not come
/// back. Read the two `drop(guard)` points below as load-bearing, not tidy.
fn maintain_logs(
    schedule: &mut logrotate::Schedule,
    policy: &logrotate::Policy,
    dir: &Path,
    init_log: &Path,
) {
    if !schedule.due(Instant::now()) {
        return;
    }

    let mut report = logrotate::Report::default();
    for (path, _size) in logrotate::oversized(dir, policy) {
        if path == init_log {
            // The lock is taken for the rotation and released before anything
            // is said about it; see this function's doc comment.
            let Ok(guard) = LOG_FILE.lock() else { continue };
            // Shutdown has let go of init.log so the root can be remounted
            // read-only. Rotating it now would reopen a file on a filesystem
            // that is on its way to read-only, for a supervisor that is about
            // to exec /sbin/reboot.
            if LOG_FILE_CLOSED.load(Ordering::SeqCst) {
                continue;
            }
            let one = logrotate::rotate(&path, policy);
            drop(guard);
            report.done.extend(one.done);
            report.trouble.extend(one.trouble);
        } else {
            let one = logrotate::rotate(&path, policy);
            report.done.extend(one.done);
            report.trouble.extend(one.trouble);
        }
    }

    let pruned = logrotate::prune(dir, policy);
    report.done.extend(pruned.done);
    report.trouble.extend(pruned.trouble);

    for line in report.done {
        log::info!("{}", line);
    }
    for line in report.trouble {
        complain_about_logs(&line);
    }
}

/// Say something is wrong with the log directory, but only when it is not the
/// same thing we said last time.
///
/// This is on a path that runs once a minute for as long as the machine is up,
/// and WARN reaches the console -- where, after the gettys are up, it is
/// printed over whoever is logging in. A read-only /var/log or a log directory
/// on a full disk is a permanent condition, so without a latch it would be a
/// line on the console every minute until the machine was rebooted, which is
/// the same flood `Service::should_restart`'s `retry_at` latch exists to
/// prevent.
///
/// Remembering a set of recent complaints rather than one keeps it bounded
/// without making it deaf: a fault that persists is said once, and a
/// *different* fault appearing later is still said.
///
/// The repeat is emitted at debug, which as things stand means it is emitted
/// nowhere -- `DualLogger::enabled` admits Info and above and nothing reads
/// `[system] log_level`. That is the wanted behaviour today and the line is
/// written this way rather than dropped so that it reappears, in the file
/// alone and never on the console, on the day somebody wires that knob up.
fn complain_about_logs(line: &str) {
    static SAID: std::sync::Mutex<ComplaintLatch> = std::sync::Mutex::new(ComplaintLatch::new());
    let Ok(mut said) = SAID.lock() else {
        log::warn!("Log rotation: {}", line);
        return;
    };
    if said.admit(line) {
        log::warn!("Log rotation: {}", line);
    } else {
        log::debug!("Log rotation (unchanged): {}", line);
    }
}

/// How many distinct complaints the latch remembers.
///
/// The number only has to cover one sweep's worth of trouble: the flood being
/// prevented is the same handful of lines returning every minute, and a
/// machine with more than this many *different* faults at once has a
/// /var/log worth the console. Bounded because this is PID 1 and the input is
/// filenames, which a machine can invent without limit.
const COMPLAINTS_REMEMBERED: usize = 32;

/// The "have we already said this" half of [`complain_about_logs`], separated
/// so it can be tested without a global.
struct ComplaintLatch {
    /// Canonical forms of the complaints already said, oldest first.
    said: Vec<String>,
}

impl ComplaintLatch {
    const fn new() -> Self {
        Self {
            said: Vec::new(),
        }
    }

    /// Whether `line` is worth the console, recording it if so.
    fn admit(&mut self, line: &str) -> bool {
        let key = Self::key(line);
        if self.said.contains(&key) {
            return false;
        }
        // Oldest out first, so the lines still arriving are the ones
        // remembered. Dropping the newest instead would make a machine with
        // more faults than this holds re-warn about every one of them, every
        // sweep, which is the flood this exists to stop.
        if self.said.len() >= COMPLAINTS_REMEMBERED {
            self.said.remove(0);
        }
        self.said.push(key);
        true
    }

    /// What two complaints have to share to count as the same one.
    ///
    /// Every run of digits collapses to a `#`, and that is the whole point.
    /// The complaints these lines are made of carry live measurements -- the
    /// prune warning interpolates the directory's current byte total, which
    /// moves every time a service writes a line -- so comparing the text
    /// verbatim made "over the cap (201.4M)" and "over the cap (201.5M)" two
    /// different faults, and the latch never latched once. A cap overshoot
    /// that grew by a hundred kilobytes is the same permanent condition
    /// reported again, and saying it once is the contract.
    ///
    /// Collapsing numbers also folds together the generations of one file
    /// (`init.log.1`, `init.log.2`), which is right for the same reason: a
    /// rotation failing on a read-only filesystem fails on all of them, and
    /// it is one fault.
    fn key(line: &str) -> String {
        let mut key = String::with_capacity(line.len());
        let mut in_number = false;
        for ch in line.chars() {
            if ch.is_ascii_digit() {
                if !in_number {
                    key.push('#');
                    in_number = true;
                }
            } else {
                in_number = false;
                key.push(ch);
            }
        }
        key
    }
}

fn init_logging() {
    init_logging_at(std::path::PathBuf::from("/var/log/raven/init.log"));
}

fn init_logging_at(path: std::path::PathBuf) {
    let logger = Box::new(DualLogger {
        path,
    });
    if log::set_boxed_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

/// Apply `options <module> key=value...` lines from modprobe.d to drivers
/// built into the kernel.
///
/// modprobe reads those files only while loading a module. This kernel builds
/// its wireless drivers in (=y), so `options rtw88_pci disable_aspm=1` in
/// /etc/modprobe.d/rtw88.conf never reached the driver, and the 8821CE kept
/// dying in "mac power on" with its workaround sitting on disk. A built-in
/// driver's parameters still exist under /sys/module/<name>/parameters and
/// accept writes when the driver declares them 0644 (rtw88 and rtw89 do), and
/// the cards this is for probe after this point (see reprobe_orphan_pci_devices),
/// so writing them here gives the conf files the meaning they claim to have.
///
/// Loadable modules are skipped -- they have no /sys/module entry until
/// modprobe loads them, and modprobe applies the options itself.
fn apply_builtin_module_options(modprobe_d: &Path, sys_module: &Path) {
    let Ok(entries) = fs::read_dir(modprobe_d) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension() != Some(std::ffi::OsStr::new("conf")) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let mut words = line.split_whitespace();
            if words.next() != Some("options") {
                continue;
            }
            let Some(module) = words.next() else {
                continue;
            };
            // The kernel spells module names with underscores; modprobe.d
            // accepts either.
            let params = sys_module.join(module.replace('-', "_")).join("parameters");
            if !params.is_dir() {
                continue;
            }
            for assignment in words {
                let Some((key, value)) = assignment.split_once('=') else {
                    continue;
                };
                match fs::write(params.join(key), value) {
                    Ok(()) => log::info!("Set built-in {}.{}={}", module, key, value),
                    // A 0444 parameter is only settable on the kernel
                    // command line; say so rather than let the conf file
                    // keep looking like it works.
                    Err(e) => log::warn!(
                        "Cannot set built-in {}.{}={} ({}): pass {}.{}={} on the kernel command line",
                        module, key, value, e, module, key, value
                    ),
                }
            }
        }
    }
}

/// Re-probe PCI devices that have no bound driver.
///
/// Writing a device address to /sys/bus/pci/drivers_probe re-runs driver
/// matching for it. Cheap, harmless for devices that genuinely have no driver,
/// and it is what turns "probe failed at 0.9s because the firmware was not
/// mounted yet" into a working card -- with no kernel rebuild and no need to
/// know which devices are affected.
fn reprobe_orphan_pci_devices() {
    let Ok(entries) = fs::read_dir("/sys/bus/pci/devices") else {
        return;
    };

    let mut reprobed = 0;
    for entry in entries.flatten() {
        // A bound device has a `driver` symlink; an orphan does not.
        if entry.path().join("driver").exists() {
            continue;
        }
        let Some(addr) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        if fs::write("/sys/bus/pci/drivers_probe", &addr).is_ok() {
            reprobed += 1;
        }
    }

    if reprobed > 0 {
        log::info!("Re-probed {} driverless PCI device(s)", reprobed);
    }
}

/// Bring the loopback interface up.
///
/// SIOCSIFFLAGS directly rather than shelling out to `ip`: lo must come up
/// even on a system where iproute2 did not ship.
fn bring_loopback_up() {
    use std::os::fd::AsRawFd;

    let Ok(sock) = std::net::UdpSocket::bind("127.255.255.255:0").or_else(|_| {
        // Can't bind while lo is down -- an unbound socket works for ioctl too.
        std::net::UdpSocket::bind("0.0.0.0:0")
    }) else {
        log::warn!("Cannot open a socket to bring lo up");
        return;
    };

    // struct ifreq with ifr_name = "lo" and ifr_flags = IFF_UP | IFF_RUNNING.
    let mut ifreq = [0u8; 40];
    ifreq[..2].copy_from_slice(b"lo");
    let flags: libc::c_short = (libc::IFF_UP | libc::IFF_RUNNING) as libc::c_short;
    ifreq[16..18].copy_from_slice(&flags.to_ne_bytes());

    // The cast is load-bearing: musl declares ioctl's request as c_int where
    // glibc says c_ulong, and SIOCSIFFLAGS is a c_ulong constant on both. The
    // value (0x8914) fits either; only the parameter type differs. Written as
    // `as _` so it compiles against both libcs instead of failing on the one
    // this binary actually ships against.
    let rc = unsafe { libc::ioctl(sock.as_raw_fd(), libc::SIOCSIFFLAGS as _, ifreq.as_ptr()) };
    if rc == 0 {
        log::info!("Loopback interface up");
    } else {
        log::warn!("Could not bring lo up: {}", std::io::Error::last_os_error());
    }
}

/// Refuse to run as anything but PID 1, unless explicitly overridden.
///
/// raven-init is a supervisor: it mounts filesystems, claims
/// /run/raven-init.sock and starts every enabled service in init.toml. Run a
/// second copy alongside a system that is already up and the two fight over
/// all three. The visible symptom is a service that cannot get its own
/// resources back --
///
///     cawd: error: another cawd is listening on /run/caw/caw.sock
///
/// -- followed by a restart loop against a socket the *first* cawd still
/// legitimately holds. That is not a cawd bug, and no amount of restarting
/// fixes it.
///
/// Live and installed RavenLinux systems both use raven-init as PID 1. This
/// guard still protects containers, rescue environments, and accidental
/// interactive invocations from starting a second supervisor.
///
/// RAVEN_INIT_ALLOW_NONPID1=1 overrides this, for testing in a container where
/// the harness knows nothing else is running.
fn ensure_supervisor_role() -> Result<()> {
    if std::process::id() == 1 {
        return Ok(());
    }

    if std::env::var_os("RAVEN_INIT_ALLOW_NONPID1").is_some() {
        log::warn!(
            "Running as PID {} rather than PID 1 (RAVEN_INIT_ALLOW_NONPID1 is set).",
            std::process::id()
        );
        log::warn!("  Expect conflicts if another supervisor is already running.");
        return Ok(());
    }

    let pid1 = fs::read_to_string("/proc/1/comm")
        .map(|c| c.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    anyhow::bail!(
        "raven-init must be PID 1, but PID 1 is '{}' and this is PID {}.\n\
         \n\
         Starting a second supervisor makes it fight the first for the control\n\
         socket and for every service in init.toml -- a daemon whose socket the\n\
         running copy still holds will fail to start and be restarted, with a\n\
         growing delay, for as long as both are running.\n\
         \n\
         To manage the running system, use raven-rc.\n\
         To test raven-init anyway, set RAVEN_INIT_ALLOW_NONPID1=1.",
        pid1,
        std::process::id()
    );
}

/// Why the main loop returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopExit {
    /// Poweroff or reboot: stop everything and leave.
    Shutdown,
    /// Replace this supervisor with the one on disk; touch no service.
    Reexec,
}

fn run_init() -> Result<()> {
    // A re-executed init inherits a booted system: mounts, hostname, the
    // loopback, every service. Phases 1 through 2b are about the machine, not
    // about this process, and running them again is at best a no-op and at
    // worst (fstab, cgroups) a second mount on top of the first.
    let handoff = reexec::take();
    if let Some(ref h) = handoff {
        log::info!(
            "Re-executed as PID {}: adopting {} services, skipping early boot",
            std::process::id(),
            h.services.len()
        );
    }

    if handoff.is_none() {
        early_boot()?;
    }

    // Phase 3: Load configuration
    log::info!("Phase 3: Loading configuration");
    let mut config = config::load()?;
    overrides::apply_kernel_cmdline_overrides(&mut config)?;
    overrides::fixup_getty_login_programs(&mut config);

    // Phase 4: Setup signal handlers
    log::info!("Phase 4: Setting up signal handlers");
    setup_signal_handlers()?;

    // Phase 5: Start services
    let mut services = match handoff {
        Some(h) => adopt_services(&config, h),
        None => {
            log::info!("Phase 5: Starting services");
            let services = start_services(&config)?;
            timeline::mark("services started");

            // seatd and the compositor are started back to back, and the
            // compositor connects to /run/seatd.sock the moment it starts.
            // seatd has not created it yet, so the first attempt always fails
            // -- and the restart backoff would then delay the compositor for
            // no better reason than that race.
            //
            // A real dependency system would express this properly; until
            // there is one, waiting for the socket is the honest version of
            // what "after seatd" means.
            wait_for_seat(&services);

            // Display welcome message
            print_welcome();
            services
        }
    };

    // Phase 6: Main loop - reap zombies and handle signals
    log::info!("Phase 6: Entering main loop");
    timeline::mark("main loop");
    loop {
        match main_loop(&mut services, &mut config)? {
            LoopExit::Shutdown => break,
            LoopExit::Reexec => {
                let snapshot =
                    reexec::Handoff::new(services.values().map(Service::snapshot).collect());
                // Only returns on failure, and on failure nothing has
                // happened: same image, same services, carry on.
                let Err(e) = reexec::handoff(&snapshot);
                log::error!("Re-exec failed: {:#}", e);
                log::error!("  Still running the previous raven-init; services untouched");
                REEXEC_REQUESTED.store(false, Ordering::SeqCst);
            }
        }
    }

    // Phase 7: Shutdown
    log::info!("Phase 7: Shutting down");
    shutdown_services(&mut services)?;

    // Leave no socket behind. A stale one makes the next raven-rc fail with
    // ECONNREFUSED rather than "not running", which reads like a broken
    // service manager instead of an absent one.
    fs::remove_file(control::SOCKET_PATH).ok();

    // Services are only part of what runs: the session under a getty -- the
    // compositor, its clients, shells -- belongs to no service, and each of
    // them can hold a file open for write on `/`. Left alive, they are why
    // the read-only remount below failed with EBUSY and every reboot came
    // back to a journal replay and a pile of orphan inodes.
    kill_remaining_processes();

    // Determine shutdown mode
    let mode = if REBOOT_REQUESTED.load(Ordering::SeqCst) {
        log::info!("Rebooting system...");
        RebootMode::RB_AUTOBOOT
    } else {
        log::info!("Powering off system...");
        RebootMode::RB_POWER_OFF
    };
    quiesce_filesystems();
    let Err(e) = reboot(mode);

    // reboot(2) returning at all is the failure. Returning from here would
    // end PID 1 and panic the kernel, which reports nothing; a shell on the
    // console at least says what went wrong and lets someone retry.
    log::error!("reboot({:?}) failed: {}", mode, e);
    emergency_shell();
}

/// Send every process except init SIGTERM, then SIGKILL what is left.
///
/// PID 1 only: `kill(-1, ...)` from anywhere else would take down whatever
/// the caller happens to be able to signal, which is why this is not part of
/// `shutdown_services`, which user mode shares.
///
/// Everything that survives its parent is reparented to init, so an ECHILD
/// from `waitpid` means there is nothing left to wait for. Kernel threads
/// ignore the signals and are not our children, so they never hold it up.
fn kill_remaining_processes() {
    use nix::errno::Errno;
    use nix::sys::signal::{kill, Signal};

    for (signal, grace) in [
        (Signal::SIGTERM, Duration::from_secs(5)),
        (Signal::SIGKILL, Duration::from_secs(2)),
    ] {
        match kill(Pid::from_raw(-1), signal) {
            // ESRCH: nobody to signal, so nobody to wait for.
            Err(Errno::ESRCH) => return,
            Err(e) => log::warn!("kill(-1, {:?}) failed: {}", signal, e),
            Ok(()) => log::info!("Sent {:?} to all remaining processes", signal),
        }

        let deadline = Instant::now() + grace;
        loop {
            match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
                Err(Errno::ECHILD) => return,
                Ok(WaitStatus::StillAlive) => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                // Reaped one; there may be more ready right now.
                Ok(_) | Err(Errno::EINTR) => {}
                Err(e) => {
                    log::warn!("waitpid during shutdown failed: {}", e);
                    break;
                }
            }
        }
    }
    log::warn!("Some processes survived SIGKILL; continuing shutdown");
}

/// Whether a re-exec should start this service, given what the hand-off left
/// of it.
///
/// `adopted` is `None` for a definition the previous image did not have -- one
/// added to init.toml or dropped into init.d since boot -- and starting those
/// is the whole point of the pass this answers for.
///
/// The rest is about not redoing the boot. A re-exec swaps the supervisor and
/// leaves every service process exactly where it is, so a service that already
/// had its turn this boot must not be given another one. Two kinds cannot tell
/// the supervisor that by state alone:
///
///   * A one-shot that finished. `snapshot` writes no pid for anything that is
///     not running, so `adopt` sees no pid and records `Stopped` -- the same
///     state as a service that never started. Every `raven-rc reexec` was
///     therefore re-running udev's coldplug, the console font and
///     `raven-dhcp --all -q`, which leaves a second dhcpcd on every wired
///     link; scripts/dev-install.sh runs a reexec after replacing the binary,
///     so this happened on every development install.
///   * A `restart = false` service that had already exited. It said in its
///     definition that it should not be started again after it stops, and a
///     re-exec is not a reason to disregard that.
///
/// The question is asked of `first_started_at`, which the hand-off carries, so
/// it survives the re-exec that is the whole problem. A service that is merely
/// between restarts -- `restart = true`, died, waiting out its backoff -- is
/// still started here, which is what brings it back after a re-exec that
/// landed inside that window.
fn wants_start_after_reexec(config: &ServiceConfig, adopted: Option<&Service>) -> bool {
    let Some(svc) = adopted else {
        return true;
    };

    if svc.is_running() || svc.is_manually_stopped() || svc.state() != ServiceState::Stopped {
        return false;
    }

    if svc.first_started_at().is_some() && (svc.is_oneshot() || !config.restart) {
        return false;
    }

    true
}

/// Take over the services a previous raven-init handed us, then start
/// whatever the configuration wants that is not already running.
///
/// The fresh config wins for definitions: a service found in both is adopted
/// under the new definition, so `restart` after a re-exec behaves as it would
/// after `reload`. A service the previous image knew and the config no longer
/// lists keeps its old definition, for the reason `reload` keeps orphans.
fn adopt_services(config: &InitConfig, handoff: reexec::Handoff) -> HashMap<String, Service> {
    let mut services: HashMap<String, Service> = HashMap::new();

    for snapshot in handoff.services {
        let name = snapshot.config.name.clone();
        let definition = config
            .services
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .unwrap_or_else(|| {
                log::warn!(
                    "Service {} is running but no longer configured; keeping it as-is",
                    name
                );
                snapshot.config.clone()
            });
        let pid = snapshot.pid;
        let svc = Service::adopt(snapshot, definition);
        if svc.is_running() {
            log::info!("Adopted {} (PID {})", name, pid.unwrap_or(0));
        } else if pid.is_some() {
            log::warn!("{} died during the hand-off; it will be restarted", name);
        }
        services.insert(name, svc);
    }

    // Anything enabled that the previous image was not running -- a service
    // added to init.toml since boot, or one it failed to start. Goes through
    // the operator's start path so `after` is honoured.
    let wanted: Vec<String> = config
        .services
        .iter()
        .filter(|c| c.enabled)
        // Except the demand-started ones, which "enabled and not running" is
        // the ordinary and correct state for. Without this, `raven-rc reexec`
        // would start every one of them -- the whole printing and bluetooth
        // stack on a machine with no printer -- which is the same mistake as
        // starting them at boot, arriving by a door nobody would think to
        // look at. They are reached instead by the walk through sysfs at the
        // top of `main_loop_at`, which starts exactly the ones whose device
        // is actually plugged in, whether this is a boot or a re-exec.
        .filter(|c| !c.is_demand_started())
        .filter(|c| wants_start_after_reexec(c, services.get(&c.name)))
        .map(|c| c.name.clone())
        .collect();
    for name in wanted {
        log::info!("Starting {} (not running before the re-exec)", name);
        let reply = control::start_service(&name, &mut services, config);
        // Line by line rather than on the whole reply: a start whose one-shot
        // dependency went wrong answers with a warning line before the result,
        // and reading only the first line would take a failed start for a
        // successful one. rc.rs:352 tests the same reply the same way.
        if reply.lines().any(|l| l.starts_with("error:")) {
            log::error!("  {}", reply.trim_end());
        }
    }

    services
}

/// Phases 1 through 2b: everything that makes a freshly booted kernel into a
/// system. Runs once per boot, never on a re-exec.
fn early_boot() -> Result<()> {
    // Phase 1: Early boot - mount essential filesystems
    log::info!("Phase 1: Mounting essential filesystems");
    mount_essential_filesystems()?;

    // Phase 1b: Everything else /etc/fstab asks for
    //
    // The initramfs mounts the root filesystem and nothing else, so on an
    // installed system this is what brings up /boot/efi, swap, and any extra
    // partition the user put in fstab. The live image has no fstab, and this
    // is a no-op there.
    log::info!("Phase 1b: Mounting /etc/fstab");
    mount_fstab();

    // Phase 2: Setup basic environment
    log::info!("Phase 2: Setting up environment");
    setup_environment()?;

    // Phase 2b: Hardware that gave up before the root was mounted
    //
    // A driver built into the kernel probes at ~1s, when the only filesystem
    // is the initramfs -- which ships no firmware. The blobs live in the real
    // root, mounted seconds later. rtw88_8821ce is the observed case: firmware
    // load ENOENT, probe fails with -22, and the WiFi card sits driverless
    // forever while its firmware sits on disk. Now that the root (and
    // /lib/firmware) is here, ask the kernel to try those devices again.
    //
    // Before that, hand the drivers their /etc/modprobe.d options: they are
    // built in, so modprobe never will, and the one that matters (rtw88_pci
    // disable_aspm) has to be set before the probe it fixes.
    log::info!("Phase 2b: Re-probing driverless PCI devices");
    apply_builtin_module_options(Path::new("/etc/modprobe.d"), Path::new("/sys/module"));
    reprobe_orphan_pci_devices();

    // Loopback is nobody's service, so nothing else brings it up -- and a down
    // `lo` quietly breaks everything that talks to 127.0.0.1.
    bring_loopback_up();

    // Let unprivileged ping work. This one write stays here, in the code,
    // rather than moving into a /usr/lib/sysctl.d fragment with the rest of
    // the kernel parameters below -- and it stays *above* the stage that
    // reads them, which is the part worth explaining.
    //
    // It is not policy. Every line in sysctl.d is somebody's opinion about how
    // this machine should behave and is meant to be argued with; this is a
    // repair for a property of the image itself, which ships a /sbin/ping with
    // no file capabilities because squashfs is built without xattrs. A machine
    // whose sysctl.d somebody emptied, or whose /usr is a different image's,
    // must still be able to ping, because "ping something" is the first thing
    // anybody types when the network looks wrong -- and a boot-time repair
    // that lives in a file that can be deleted is not a repair.
    //
    // Being above the stage is what makes it a default rather than a decree:
    // the fragments are applied afterwards, so /etc/sysctl.d/90-local.conf
    // saying `net.ipv4.ping_group_range = 0 0` closes the range again and
    // means it. Written the other way round, init would clobber the operator's
    // setting on every boot and nothing would say why.
    //
    // The mechanism, for completeness: the image's /sbin/ping has neither
    // setuid nor file capabilities (squashfs is not built with xattrs), so its raw
    // ICMP socket fails -- and the kernel's unprivileged ICMP datagram
    // fallback is disabled by default (ping_group_range is "1 0", an empty
    // range). Opening the range to every group is what systemd-based distros
    // ship, and the first thing anyone types on a machine with new networking
    // is ping.
    if let Err(e) = fs::write("/proc/sys/net/ipv4/ping_group_range", "0 2147483647") {
        log::warn!("Could not enable unprivileged ping: {}", e);
    }

    // Phase 2c: the kernel parameters the machine's policy asks for
    //
    // /usr/lib/sysctl.d and /etc/sysctl.d have existed on this image since the
    // skeleton first created them, and until now nothing read a line of
    // either. The result was a machine running with kernel.kptr_restrict = 0
    // and kernel.dmesg_restrict = 0 while a file in /usr/lib/sysctl.d sat
    // there asking for better, and no way to express a kernel policy short of
    // editing PID 1. See src/sysctl.rs for the format and for what is and is
    // not honoured of it.
    //
    // Here, rather than in a service, for two reasons. It needs /proc, which
    // Phase 1 mounted; and everything started after this point is entitled to
    // assume the kernel it is running on is the one the policy describes --
    // raven-udev coldplugs devices whose drivers read module parameters, the
    // network comes up with whatever rp_filter says, and a service that had
    // to wonder whether the sysctls had landed yet would be a service with a
    // race in it.
    //
    // It is not re-run when init re-executes itself: early_boot as a whole is
    // skipped there, and that is right rather than merely convenient. A sysctl
    // is state of the kernel, not of this process, so every value written here
    // is still in place after the exec -- while re-applying them would undo
    // whatever somebody had since changed by hand while chasing a problem.
    // The long version of that argument is on `sysctl::apply_boot_sysctls`.
    log::info!("Phase 2c: Applying kernel parameters from sysctl.d");
    sysctl::apply_boot_sysctls();
    timeline::mark("sysctl applied");

    Ok(())
}

/// The kernel filesystems that exist so the kernel can be asked what it is
/// doing: `(fstype, mount point, mount options)`.
///
/// A table rather than two more calls in the function below, because the mode
/// on debugfs is a security decision and a table is a thing a test can assert
/// about. See `the_introspection_filesystems_are_root_only_and_never_unmounted`.
///
/// securityfs carries no options because it needs none. It is a small
/// root-owned tree with nothing writable in it by default, and it is the only
/// way to find out at runtime which LSMs this kernel actually has: reading
/// /sys/kernel/security/lsm is the one check that confirms the CONFIG_LSM line
/// the kernel is built with took effect. Landlock's ABI version lives there
/// too, which is what a sandboxing helper has to read before it can know which
/// of its rules the running kernel will accept. Unmounted, that directory is
/// empty and the question simply cannot be asked.
///
/// debugfs is mounted `mode=0700`, and the judgement there is deliberate. It
/// is not that debugfs is harmless -- it is the largest unaudited surface the
/// kernel exposes, whole subsystems publish writable knobs into it with no
/// stability contract and in places no validation, which is why several
/// distributions leave it unmounted entirely. It is mounted because Raven's
/// own tuning needs it and has nowhere else to go:
/// /sys/kernel/debug/sched/sched_itmt_enabled is where the ITMT switch has
/// lived since 6.16, and the scheduler domain flags that say whether
/// asymmetric packing is actually on for a hybrid CPU are readable nowhere
/// else. 0700 on the mount's root directory is what keeps that surface to
/// root: a process that is already root can load a module, so debugfs hands it
/// nothing it did not have, while a process that is not root cannot even
/// traverse the mount point. If a kernel ever rejects the option the mount
/// fails and debugfs stays unmounted, which is the correct way round -- there
/// is no retry without the mode.
const INTROSPECTION_FILESYSTEMS: &[(&str, &str, &str)] = &[
    ("securityfs", "/sys/kernel/security", ""),
    ("debugfs", "/sys/kernel/debug", "mode=0700"),
];

fn mount_essential_filesystems() -> Result<()> {
    // Mount /proc
    mount_fs("proc", "/proc", "proc", MsFlags::empty(), "")?;

    // Mount /sys
    mount_fs("sysfs", "/sys", "sysfs", MsFlags::empty(), "")?;

    // Mount /dev (devtmpfs)
    mount_fs("devtmpfs", "/dev", "devtmpfs", MsFlags::empty(), "")?;

    // Create /dev subdirectories
    fs::create_dir_all("/dev/pts").ok();
    fs::create_dir_all("/dev/shm").ok();

    // Mount /dev/pts
    mount_fs(
        "devpts",
        "/dev/pts",
        "devpts",
        MsFlags::empty(),
        "gid=5,mode=620",
    )?;

    // Mount /dev/shm
    mount_fs("tmpfs", "/dev/shm", "tmpfs", MsFlags::empty(), "mode=1777")?;

    // Mount /run
    fs::create_dir_all("/run").ok();
    mount_fs("tmpfs", "/run", "tmpfs", MsFlags::empty(), "mode=755")?;

    // Mount /tmp
    mount_fs("tmpfs", "/tmp", "tmpfs", MsFlags::empty(), "mode=1777")?;

    // The X11 and ICE socket directories. Xwayland (via smithay) binds
    // /tmp/.X11-unix/X<n> and never creates the directory, so without this the
    // compositor logs "Could not find a free socket for the XServer" and every
    // X11-only client (Steam, for one) dies with "Unable to open a connection
    // to X". systemd-tmpfiles' x11.conf does this on other distros; here init
    // is the only thing that runs early enough. World-writable with the sticky
    // bit, as /tmp itself is, and created by root so no user owns it.
    for dir in ["/tmp/.X11-unix", "/tmp/.ICE-unix"] {
        if let Err(e) = fs::create_dir(dir) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                log::warn!("Failed to create {}: {}", dir, e);
                continue;
            }
        }
        if let Err(e) = fs::set_permissions(dir, fs::Permissions::from_mode(0o1777)) {
            log::warn!("Failed to set permissions on {}: {}", dir, e);
        }
    }

    // Mount cgroups if available
    if Path::new("/sys/fs/cgroup").exists() || fs::create_dir_all("/sys/fs/cgroup").is_ok() {
        mount_fs("cgroup2", "/sys/fs/cgroup", "cgroup2", MsFlags::empty(), "").ok();
    }

    // ...and give the services somewhere to live inside it. Done here, while
    // the filesystem that was just mounted is still the subject, so that the
    // one warning a machine without cgroup2 deserves is printed once at boot
    // rather than by whichever service happens to start first.
    //
    // This is not the only caller: `Cgroup::for_service` ensures the slice
    // again on every start, because `early_boot` -- and therefore this
    // function -- is skipped entirely after `raven-rc reexec`, and a
    // re-executed init that assumed the slice existed would put every service
    // started from then on into no cgroup at all, silently.
    cgroup::ensure_slice();

    // The two filesystems through which the kernel describes itself. Neither
    // is needed to reach the root or start a service, which is why they are
    // last, and neither was mounted at all until it was noticed that the
    // things they expose had become unreachable on a booted Raven machine.
    for (fstype, target, data) in INTROSPECTION_FILESYSTEMS {
        if let Err(e) = mount_fs(fstype, target, fstype, MsFlags::empty(), data) {
            // A kernel built without CONFIG_SECURITYFS or CONFIG_DEBUG_FS
            // answers ENODEV here. That is a fact about the kernel rather than
            // a fault of this machine's, so it is a debug line and not a
            // warning printed over somebody's console at every boot.
            log::debug!("Not mounting {} on {}: {:#}", fstype, target, e);
        }
    }

    log::info!("Essential filesystems mounted");
    timeline::mark("filesystems mounted");
    Ok(())
}

fn mount_fs(source: &str, target: &str, fstype: &str, flags: MsFlags, data: &str) -> Result<()> {
    // Create mount point if it doesn't exist
    fs::create_dir_all(target).ok();

    // Check if already mounted
    if is_mounted(target) {
        log::debug!("{} already mounted", target);
        return Ok(());
    }

    let data_opt: Option<&str> = if data.is_empty() { None } else { Some(data) };

    mount(Some(source), target, Some(fstype), flags, data_opt)
        .with_context(|| format!("Failed to mount {} on {}", fstype, target))?;

    log::debug!("Mounted {} on {}", fstype, target);
    Ok(())
}

/// Mount every auto entry in /etc/fstab, and enable every swap entry.
///
/// Deliberately forgiving: a bad fstab line should cost you that one mount, not
/// the boot. Everything here logs and carries on.
fn mount_fstab() {
    let content = match fs::read_to_string("/etc/fstab") {
        Ok(c) => c,
        Err(_) => {
            log::debug!("No /etc/fstab; nothing to mount");
            return;
        }
    };

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 {
            log::warn!("Ignoring malformed fstab line: {}", line);
            continue;
        }

        let (spec, target, fstype) = (fields[0], fields[1], fields[2]);
        let options = fields.get(3).copied().unwrap_or("defaults");

        if options.split(',').any(|o| o == "noauto") {
            continue;
        }

        if fstype == "swap" {
            enable_swap(spec);
            continue;
        }

        // The initramfs already mounted the root filesystem. Remounting it here
        // would at best be a no-op and at worst change its flags underneath a
        // running system.
        if target == "/" || target == "none" || target == "swap" {
            continue;
        }

        if is_mounted(target) {
            log::debug!("{} already mounted", target);
            continue;
        }

        mount_fstab_entry(spec, target, fstype, options);
    }
}

fn mount_fstab_entry(spec: &str, target: &str, fstype: &str, options: &str) {
    fs::create_dir_all(target).ok();

    let (flags, data) = parse_mount_options(options);

    if let Some(device) = resolve_fstab_spec(spec) {
        let data_opt = if data.is_empty() {
            None
        } else {
            Some(data.as_str())
        };
        let fstype_opt = if fstype == "auto" { None } else { Some(fstype) };

        match mount(Some(device.as_str()), target, fstype_opt, flags, data_opt) {
            Ok(()) => {
                log::info!("Mounted {} on {} ({})", device, target, fstype);
                return;
            }
            Err(e) => {
                log::warn!("Mounting {} on {} failed: {}", device, target, e);
            }
        }
    }

    // Fall back to mount(8). It links libblkid, so it resolves UUID= and LABEL=
    // by scanning the devices itself -- no /dev/disk/by-uuid, and therefore no
    // dependency on udev having started yet. Given a mount point alone it reads
    // the rest of the entry back out of fstab.
    match Command::new("/bin/mount")
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            log::info!("Mounted {} on {} (via mount(8))", spec, target);
        }
        Ok(status) => {
            log::warn!("mount {} failed with {}", target, status);
        }
        Err(e) => {
            log::warn!(
                "Could not mount {}: {} (and /bin/mount: {})",
                spec,
                target,
                e
            );
        }
    }
}

fn enable_swap(spec: &str) {
    let device = match resolve_fstab_spec(spec) {
        Some(d) => d,
        None => {
            // swapon(8) resolves UUID= the same way mount(8) does.
            match Command::new("/sbin/swapon")
                .arg(spec)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
            {
                Ok(status) if status.success() => log::info!("Enabled swap on {}", spec),
                _ => log::warn!("Could not enable swap on {}", spec),
            }
            return;
        }
    };

    let path = match std::ffi::CString::new(device.clone()) {
        Ok(p) => p,
        Err(_) => return,
    };

    // SAFETY: path is a valid NUL-terminated C string that outlives the call.
    let rc = unsafe { libc::swapon(path.as_ptr(), 0) };
    if rc == 0 {
        log::info!("Enabled swap on {}", device);
    } else {
        log::warn!(
            "swapon({}) failed: {}",
            device,
            std::io::Error::last_os_error()
        );
    }
}

/// Turn an fstab first field into a device path, or None when it needs a
/// blkid-style scan that only mount(8) and swapon(8) can do here.
fn resolve_fstab_spec(spec: &str) -> Option<String> {
    if spec.starts_with('/') {
        return Some(spec.to_string());
    }

    // These symlink trees only exist once udev has populated them, which at this
    // point in the boot it may not have. Returning None sends the caller to the
    // mount(8) fallback, which does not need them.
    let (dir, value) = if let Some(v) = spec.strip_prefix("UUID=") {
        ("/dev/disk/by-uuid", v)
    } else if let Some(v) = spec.strip_prefix("PARTUUID=") {
        ("/dev/disk/by-partuuid", v)
    } else if let Some(v) = spec.strip_prefix("LABEL=") {
        ("/dev/disk/by-label", v)
    } else {
        ("/dev/disk/by-partlabel", spec.strip_prefix("PARTLABEL=")?)
    };

    let link = format!("{}/{}", dir, value);
    if Path::new(&link).exists() {
        // Canonicalize so the log names the real device, not the symlink.
        return Some(
            fs::canonicalize(&link)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or(link),
        );
    }

    None
}

/// Split a comma-separated fstab option list into mount(2) flags and the
/// filesystem-specific data string that carries whatever is left.
fn parse_mount_options(options: &str) -> (MsFlags, String) {
    let mut flags = MsFlags::empty();
    let mut data: Vec<&str> = Vec::new();

    for opt in options.split(',') {
        match opt.trim() {
            "" | "defaults" | "rw" | "auto" | "exec" | "suid" | "dev" | "async" | "atime"
            | "diratime" | "nofail" => {}
            "ro" => flags |= MsFlags::MS_RDONLY,
            "noexec" => flags |= MsFlags::MS_NOEXEC,
            "nosuid" => flags |= MsFlags::MS_NOSUID,
            "nodev" => flags |= MsFlags::MS_NODEV,
            "noatime" => flags |= MsFlags::MS_NOATIME,
            "nodiratime" => flags |= MsFlags::MS_NODIRATIME,
            "relatime" => flags |= MsFlags::MS_RELATIME,
            "strictatime" => flags |= MsFlags::MS_STRICTATIME,
            "sync" => flags |= MsFlags::MS_SYNCHRONOUS,
            "dirsync" => flags |= MsFlags::MS_DIRSYNC,
            "remount" => flags |= MsFlags::MS_REMOUNT,
            "bind" => flags |= MsFlags::MS_BIND,
            other => data.push(other),
        }
    }

    (flags, data.join(","))
}

fn is_mounted(path: &str) -> bool {
    if let Ok(file) = File::open("/proc/mounts") {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == path {
                return true;
            }
        }
    }
    false
}

fn setup_environment() -> Result<()> {
    // Set hostname
    if let Ok(hostname) = fs::read_to_string("/etc/hostname") {
        let hostname = hostname.trim();
        if !hostname.is_empty() {
            nix::unistd::sethostname(hostname).ok();
            log::info!("Hostname set to: {}", hostname);
        }
    } else {
        nix::unistd::sethostname("raven-linux").ok();
    }

    // Set PATH
    std::env::set_var(
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    );

    // Set TERM
    std::env::set_var("TERM", "linux");

    // Ensure XDG_RUNTIME_DIR exists for Wayland/DBus consumers.
    let runtime_dir =
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/0".to_string());
    std::env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
    fs::create_dir_all(&runtime_dir).ok();
    let _ = fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700));

    // Create essential directories
    fs::create_dir_all("/var/log").ok();
    fs::create_dir_all("/var/run").ok();
    fs::create_dir_all("/var/tmp").ok();

    // Seed random number generator
    seed_random()?;

    // Set system clock from hardware clock if available
    set_system_clock();

    log::info!("Environment configured");
    Ok(())
}

fn seed_random() -> Result<()> {
    // Try to seed from saved random seed
    if Path::new("/var/lib/random-seed").exists() {
        if let Ok(seed) = fs::read("/var/lib/random-seed") {
            if let Ok(mut urandom) = File::options().write(true).open("/dev/urandom") {
                let _ = urandom.write_all(&seed);
            }
        }
    }
    Ok(())
}

fn set_system_clock() {
    // Try to set system clock from RTC
    let _ = Command::new("/sbin/hwclock")
        .args(["--hctosys", rtc::hwclock_flag()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn setup_signal_handlers() -> Result<()> {
    // We need to handle these signals:
    // SIGCHLD - Child process terminated (reap zombies)
    // SIGTERM - Shutdown request
    // SIGINT  - Ctrl+C (shutdown in emergency)
    // SIGUSR1 - Custom: power off
    // SIGUSR2 - Custom: reboot

    // For simplicity, we'll poll for signals in the main loop
    // using signal::sigprocmask

    Ok(())
}

fn start_services(config: &InitConfig) -> Result<HashMap<String, Service>> {
    let mut services: HashMap<String, Service> = HashMap::new();
    let mut pending: Vec<&ServiceConfig> = config.services.iter().filter(|s| s.enabled).collect();
    let mut unavailable: Vec<String> = Vec::new();

    // Services that are enabled, installed and perfectly startable, and that
    // this boot is deliberately not starting: the demand-triggered ones. Kept
    // apart from `unavailable` only so that a service ordered after one of
    // them gets an accurate reason -- "the thing you are ordered after is
    // started on demand" is a definition to go and fix, where "dependency
    // unavailable" sends somebody looking for a daemon that is not broken.
    //
    // The consequence itself is the same either way and is not a subtlety
    // worth hiding: a service ordered after a demand-started one does not
    // start at boot, and nothing starts it when the trigger fires, because a
    // trigger starts the service it names and that service's dependencies --
    // never its dependants. Ordering something after a service that may not
    // exist yet is a thing to think twice about, and the log line is where
    // that thinking gets prompted.
    let mut deferred: Vec<String> = Vec::new();

    // The one-shots whose `ready_timeout` has already been spent waiting in
    // this function, so that it is spent once per boot rather than once per
    // dependant. The wait below sits inside the loop over one service's
    // `after` list, and that loop is re-entered for every service ordered
    // after the same one-shot: init.toml has nine enabled services with
    // `after = ["udev"]`, so a coldplug wedged on a device probe charged the
    // boot udev's five seconds nine times over, in PID 1's only thread, and
    // printed nine identical warnings about it. Waiting again also cannot
    // learn anything the first wait did not: the one-shot is still running,
    // and what the dependants do about that -- start anyway -- does not
    // change.
    let mut oneshots_waited: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Resolve the small dependency graph as services are started. Configuration
    // order remains the tie-breaker, but `after` is authoritative.
    while !pending.is_empty() {
        let mut progressed = false;
        let mut index = 0;
        while index < pending.len() {
            // Look at the services already started before considering the next
            // one. The main loop is what watches ready paths, and it does not
            // exist yet: without this, every daemon that became ready while
            // the rest of the boot was still being started would be stamped
            // with the single moment this function returned, which is how
            // powerd, controlsd, timed and fprintd came to report the same
            // ready time to the millisecond in `raven-rc blame`.
            //
            // One stat per service that has a ready path and has not answered
            // yet, a dozen times over a boot. The residual error is the time
            // it takes to start one service, and the one case it cannot reach
            // is a file that appears while this function is blocked waiting
            // for somebody else's dependency below -- that service is stamped
            // when the wait ends rather than when its file appeared.
            control::observe_readiness(&mut services);

            let svc_config = pending[index];

            // Not started at boot, and that is the whole point of it. Checked
            // before the dependency resolution below rather than after,
            // because a demand-started service must not spend the boot
            // waiting for a dependency's ready path: it is not starting, so
            // nothing it is ordered after needs to have happened yet, and the
            // wait would charge every boot several seconds for a daemon that
            // is not going to run. Its `after` is honoured in full when the
            // trigger does fire, by `control::start_service`, which is the
            // same code `raven-rc start` goes through.
            if svc_config.is_demand_started() {
                log::info!(
                    "Service {} not started at boot: it starts on demand",
                    svc_config.name
                );
                deferred.push(svc_config.name.clone());
                pending.remove(index);
                progressed = true;
                continue;
            }
            if svc_config.after.iter().any(|d| deferred.contains(d)) {
                log::warn!(
                    "Service {} not started: it is ordered after {:?}, which starts on demand",
                    svc_config.name,
                    svc_config
                        .after
                        .iter()
                        .filter(|d| deferred.contains(d))
                        .collect::<Vec<_>>()
                );
                // Into `unavailable` rather than `deferred`: this service is
                // not demand-started, it is merely not started, and anything
                // ordered after *it* should be told the ordinary thing.
                unavailable.push(svc_config.name.clone());
                pending.remove(index);
                progressed = true;
                continue;
            }
            if svc_config.after.iter().any(|d| unavailable.contains(d)) {
                log::error!(
                    "Service {} skipped: dependency unavailable ({:?})",
                    svc_config.name,
                    svc_config.after
                );
                unavailable.push(svc_config.name.clone());
                pending.remove(index);
                progressed = true;
                continue;
            }
            if !svc_config.after.iter().all(|d| services.contains_key(d)) {
                index += 1;
                continue;
            }

            let mut dependencies_ready = true;
            for dependency in &svc_config.after {
                if let Some(dep_cfg) = config.services.iter().find(|s| &s.name == dependency) {
                    if dep_cfg.service_type == ServiceType::Oneshot {
                        // What `after = ["udev"]` was always meant to say, and
                        // could not. A one-shot has no ready path -- the thing
                        // it produces is a machine in a different state, not a
                        // socket -- so the loop below had nothing to wait on
                        // and every service ordered after the coldplug started
                        // the instant the coldplug had been *forked*. Which is
                        // to say the ordering was between two spawns, tens of
                        // microseconds apart, and the dependency it was
                        // written to express was satisfied by luck.
                        //
                        // Nothing here stops the dependant starting, whatever
                        // the answer is, and that is deliberate on all three
                        // counts:
                        //
                        //   * `after` is an ordering, the way systemd's
                        //     `After=` is. "Do not start me unless that
                        //     succeeded" is `Requires=`, which this supervisor
                        //     does not have and which nothing in init.toml has
                        //     ever asked for.
                        //   * a one-shot that failed has usually still done
                        //     most of its work -- a coldplug that could not
                        //     probe one device is not a machine with no
                        //     devices -- so the dependants would nearly always
                        //     have been fine, and skipping them would take out
                        //     dbus, cawd, powerd, controlsd, the network and
                        //     every one of their dependants over one non-zero
                        //     status from raven-udev.
                        //   * running out of `ready_timeout` is not a verdict
                        //     at all. The one-shot is still working; the
                        //     supervisor has merely stopped waiting, because
                        //     the alternative is a boot that a slow coldplug
                        //     can hang forever.
                        //
                        // A ready path that never appears is skipped below,
                        // and that is a different case: the socket the
                        // dependant is about to connect to does not exist, so
                        // starting it is starting it into a restart loop.
                        //
                        // The failure is not lost by carrying on. It is an
                        // ERROR in the log from `mark_exited`, "failed (status
                        // N)" in `raven-rc list`, and the NOTE on the service's
                        // own row in `raven-rc blame`.
                        // Zero once the deadline has been spent on this
                        // one-shot: `wait_until_finished` still reaps it and
                        // still reports where it got to, it simply does not
                        // sleep on it a second time.
                        let already_waited = !oneshots_waited.insert(dependency.clone());
                        let timeout = if already_waited {
                            Duration::ZERO
                        } else {
                            Duration::from_secs(dep_cfg.ready_timeout as u64)
                        };
                        match services
                            .get_mut(dependency)
                            .and_then(|dep| dep.wait_until_finished(timeout))
                        {
                            Some(OneshotOutcome::Completed) | None => {}
                            Some(OneshotOutcome::Failed(code)) => {
                                log::error!(
                                    "Service {} starting anyway: one-shot {} failed with status {}",
                                    svc_config.name,
                                    dependency,
                                    code
                                );
                            }
                            Some(OneshotOutcome::Running) if already_waited => {
                                // Said differently from the line below on
                                // purpose: nothing waited this time, so
                                // repeating "after 5s" would be a boot
                                // reporting forty-five seconds it did not
                                // spend.
                                log::warn!(
                                    "Service {} starting anyway: one-shot {} is still running",
                                    svc_config.name,
                                    dependency
                                );
                            }
                            Some(OneshotOutcome::Running) => {
                                log::warn!(
                                    "Service {} starting anyway: one-shot {} has not finished after {}s",
                                    svc_config.name,
                                    dependency,
                                    dep_cfg.ready_timeout
                                );
                            }
                            Some(OneshotOutcome::Unfinished) => {
                                log::warn!(
                                    "Service {} starting anyway: one-shot {} did not finish",
                                    svc_config.name,
                                    dependency
                                );
                            }
                        }
                        continue;
                    }

                    if let Some(path) = dep_cfg.ready_path.as_deref() {
                        // Blocks on an inotify watch rather than on a 50ms
                        // timer. The dependency chain on this machine is four
                        // deep in places, and a timer charged every link in it
                        // up to a tick of pure waiting on every boot even when
                        // the socket was there in a millisecond. The timeout
                        // and the meaning of the answer are unchanged.
                        let ready = readiness::wait_for_path(
                            path,
                            Duration::from_secs(dep_cfg.ready_timeout as u64),
                        );
                        if ready {
                            if let Some(dep) = services.get_mut(dependency) {
                                dep.mark_ready();
                            }
                        }
                        if !ready {
                            log::error!(
                                "Service {} skipped: {} did not become ready at {}",
                                svc_config.name,
                                dependency,
                                path
                            );
                            dependencies_ready = false;
                            break;
                        }
                    }
                }
            }
            if !dependencies_ready {
                unavailable.push(svc_config.name.clone());
                pending.remove(index);
                progressed = true;
                continue;
            }

            // A binary that is not installed is not a failure. A service can
            // be defined before its daemon exists -- a drop-in copied into
            // /etc/raven/init.d ahead of `rvn install`, or an entry for
            // software the owner removed -- and an ERROR on the console for
            // every absent daemon buries the login prompt under noise.
            // Skipped, not registered: `raven-rc start` falls back to the
            // config for services outside the running set, so the day the
            // binary appears it can be started by hand or by the next boot.
            if !std::path::Path::new(&svc_config.exec).exists() {
                log::info!(
                    "Service {} skipped: {} is not installed",
                    svc_config.name,
                    svc_config.exec
                );
                unavailable.push(svc_config.name.clone());
                pending.remove(index);
                progressed = true;
                continue;
            }
            match Service::start(svc_config) {
                Ok(svc) => {
                    log::info!("Started service: {}", svc_config.name);
                    services.insert(svc_config.name.clone(), svc);
                }
                Err(e) => {
                    log::error!("Failed to start {}: {:#}", svc_config.name, e);
                    if svc_config.critical {
                        return Err(e)
                            .context(format!("Critical service {} failed", svc_config.name));
                    }
                    unavailable.push(svc_config.name.clone());
                }
            }
            pending.remove(index);
            progressed = true;
        }

        if !progressed {
            for svc in pending.drain(..) {
                log::error!(
                    "Service {} not started: unresolved dependencies {:?}",
                    svc.name,
                    svc.after
                );
            }
            break;
        }
    }

    // Start default getty on tty1 if no services configured
    if services.is_empty() {
        let getty_config = ServiceConfig {
            name: "getty-tty1".to_string(),
            description: "Getty on tty1".to_string(),
            exec: "/bin/agetty".to_string(),
            args: vec![
                "--noclear".to_string(),
                "--skip-login".to_string(),
                "--login-program".to_string(),
                "/bin/raven-shell".to_string(),
                "tty1".to_string(),
                "linux".to_string(),
            ],
            restart: true,
            tty: Some("/dev/tty1".to_string()),
            ..ServiceConfig::default()
        };

        // Try agetty first, fall back to direct shell
        let svc = Service::start(&getty_config).or_else(|_| {
            let shell_config = ServiceConfig {
                name: "shell-tty1".to_string(),
                description: "Shell on tty1".to_string(),
                exec: "/bin/sh".to_string(),
                args: vec![],
                restart: true,
                tty: Some("/dev/tty1".to_string()),
                ..ServiceConfig::default()
            };
            Service::start(&shell_config)
        });

        if let Ok(s) = svc {
            log::info!("Started default getty/shell");
            services.insert("getty-tty1".to_string(), s);
        }
    }

    Ok(services)
}

fn print_welcome() {
    println!();
    println!("  =====================================");
    println!("  |       R A V E N   L I N U X       |");
    println!("  |         Init System v0.1         |");
    println!("  =====================================");
    println!();

    // Print OS release info if available
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                let name = line.trim_start_matches("PRETTY_NAME=").trim_matches('"');
                println!("  {}", name);
                break;
            }
        }
    }
    println!();
}

/// Where a supervisor's socket and status live, and which machine-level
/// duties it has. PID 1 has all of them; a session supervisor has none.
struct LoopPaths {
    socket: std::path::PathBuf,
    status_dir: std::path::PathBuf,
    machine: bool,
}

impl LoopPaths {
    fn system() -> Self {
        LoopPaths {
            socket: std::path::PathBuf::from(control::SOCKET_PATH),
            status_dir: std::path::PathBuf::from(control::STATUS_DIR),
            machine: true,
        }
    }
}

/// `raven-init --user`: the session supervisor's whole life. Nothing here
/// touches the machine; see usermode.rs for what is different from PID 1.
fn run_user() -> Result<()> {
    let paths = usermode::Paths::from_env()?;
    paths.prepare()?;
    // One supervisor per session. A second finds every service already
    // running and restart-loops against them, while the one it doubles —
    // wireplumber — fights the first over the default device. Checked before
    // anything starts, since failing to bind the socket later is not fatal.
    // A compositor restart running the session script again is the usual way
    // here.
    if control::is_live(&paths.socket) {
        eprintln!(
            "raven-init --user: a supervisor is already running for this session ({})",
            paths.socket.display()
        );
        return Ok(());
    }
    // Service logs go beside init's own; service.rs reads this.
    std::env::set_var("RAVEN_SERVICE_LOG_DIR", &paths.log_dir);
    init_logging_at(paths.log_dir.join("init.log"));
    timeline::mark("init started");
    control::set_user_mode(true);
    log::info!(
        "RavenInit starting for a session (uid {}); socket {}",
        nix::unistd::getuid(),
        paths.socket.display()
    );

    // SIGTERM and SIGINT end the session's services and this process; the
    // launcher, or a logout, is what sends them.
    extern "C" fn on_term(_: libc::c_int) {
        SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
    }
    // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
    unsafe {
        use nix::sys::signal::{signal, SigHandler, Signal};
        signal(Signal::SIGTERM, SigHandler::Handler(on_term)).context("SIGTERM handler")?;
        signal(Signal::SIGINT, SigHandler::Handler(on_term)).context("SIGINT handler")?;
        signal(Signal::SIGHUP, SigHandler::Handler(on_term)).context("SIGHUP handler")?;
    }

    let mut config = usermode::load_config(&paths);
    log::info!("Starting {} session services", config.services.iter().filter(|s| s.enabled).count());
    let mut services = start_services(&config)?;
    timeline::mark("services started");
    timeline::mark("main loop");

    let loop_paths = LoopPaths {
        socket: paths.socket.clone(),
        status_dir: paths.runtime.clone(),
        machine: false,
    };
    loop {
        match main_loop_at(&mut services, &mut config, &loop_paths)? {
            LoopExit::Shutdown => break,
            // Refused at dispatch in user mode; never reaches here.
            LoopExit::Reexec => {
                REEXEC_REQUESTED.store(false, Ordering::SeqCst);
            }
        }
    }
    log::info!("Session supervisor stopping");
    shutdown_services(&mut services)?;
    fs::remove_file(&paths.socket).ok();
    Ok(())
}

fn main_loop(services: &mut HashMap<String, Service>, config: &mut InitConfig) -> Result<LoopExit> {
    main_loop_at(services, config, &LoopPaths::system())
}

fn main_loop_at(
    services: &mut HashMap<String, Service>,
    config: &mut InitConfig,
    paths: &LoopPaths,
) -> Result<LoopExit> {
    // The control socket is how raven-rc asks about services and starts or
    // stops them. A failure to bind it is not fatal: PID 1 supervising
    // services matters more than PID 1 being controllable, and the
    // /run/raven-init.cmd fallback below still works.
    let control = match control::listen_at(paths.socket.to_str().unwrap_or(control::SOCKET_PATH)) {
        Ok(listener) => Some(listener),
        Err(e) => {
            log::warn!("Control socket unavailable: {:#}", e);
            log::warn!("  raven-rc service commands will not work this boot");
            None
        }
    };

    // The sleep marker, before anything can watch it. See power.rs.
    if paths.machine {
        power::publish_at_boot();
    }

    // What `raven-rc list` and `status` say, published for readers without
    // root. See control::StatusPublisher.
    let mut status = control::StatusPublisher::at(paths.status_dir.clone());
    status.publish(services, config);

    let child_wake = child_wakeup();
    if child_wake.is_none() {
        log::warn!("Could not set up the SIGCHLD wakeup; the main loop will tick every 100 ms");
    }

    // The third thing this loop sleeps on, beside the SIGCHLD pipe and the
    // control socket: a ready path appearing. See readiness.rs for why the
    // watch goes on the parent directory and why an event is only ever a
    // wakeup. `None` means no inotify, and the loop keeps the timer it had.
    let mut ready_watch = readiness::Watcher::new();

    // The fourth: the kernel's uevent broadcast, for services that are not
    // started at boot and wait for a device instead. See demand.rs, and in
    // particular the section of its module comment explaining that this is
    // demand-triggered start and NOT socket activation -- no descriptor is
    // passed to anything and no connection is ever held open.
    //
    // Not opened in user mode: the socket needs CAP_NET_ADMIN, a session
    // supervisor has none, and trying would print a warning about a
    // capability nobody expected it to have.
    let mut demand_watch = if paths.machine {
        demand::Monitor::open()
    } else {
        None
    };

    // What is demand-started and what will start it, said once, at the one
    // moment somebody reading a boot log is looking for a service that is not
    // there. Before the walk below, so the explanation precedes the actions.
    //
    // Said in user mode too, and deliberately: a session service kept out of
    // the boot by a `[services.demand]` block is kept out of it whether or not
    // anything is watching, so the one place that says what will bring it back
    // has to speak in both modes. What differs is the answer, which is what
    // `Watching` carries.
    demand::report_configuration(
        config,
        match (paths.machine, demand_watch.is_some()) {
            (true, true) => demand::Watching::Devices,
            (true, false) => demand::Watching::Nothing,
            (false, _) => demand::Watching::NotInThisMode,
        },
    );

    // The devices that were already plugged in when nobody was listening.
    //
    // This runs after the socket above is open, never before, and the
    // ordering is the same one readiness.rs argues at length for its watches:
    // a device that appears between a look and the arming of the thing that
    // would have noticed it is a device nothing ever notices. Opening first
    // and looking second means the worst case is seeing a device twice, which
    // costs one "is already running" and nothing else.
    if paths.machine {
        let wanted = demand::already_present(config);
        start_on_demand(&wanted, services, config);
    }

    // Log rotation, read once here rather than per sweep.
    //
    // `[system]` is read where it is needed and not re-applied on a `raven-rc
    // reload` -- reload reloads service definitions, which is the property
    // that makes it safe to run on a live machine -- so a changed log knob
    // takes effect at the next boot or `raven-rc reexec`, exactly like a
    // changed hostname. Reading it once also means an unreadable value is
    // complained about once, here, instead of once a minute forever.
    let (log_policy, log_policy_report) = logrotate::Policy::from_system(&config.system);
    for line in &log_policy_report.trouble {
        log::warn!("{}", line);
    }
    let mut log_sweep = logrotate::Schedule::due_now();
    // Both live in the same directory in both modes: /var/log/raven for the
    // machine, and $XDG_STATE_HOME/raven/log for a session, where `run_user`
    // sets $RAVEN_SERVICE_LOG_DIR before opening init.log beside the services'
    // logs. Asking `Service::log_dir` is therefore the one question that
    // answers both, and a session supervisor's logs are rotated by the same
    // sweep under the same policy.
    let log_dir = Service::log_dir();
    let init_log = log_dir.join("init.log");

    // How long to sleep with nothing pending. Bounded so a wake that was
    // missed (a signal before the handler was installed, a client that raced
    // the poll) costs at most this, and so the command-file fallback and the
    // published status still get looked at on a laptop that is doing nothing.
    const IDLE: Duration = Duration::from_secs(2);
    const BUSY: Duration = Duration::from_millis(100);

    log::info!("Entering main loop");

    loop {
        // Check for shutdown request
        if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
            log::info!("Shutdown requested, exiting main loop");
            return Ok(LoopExit::Shutdown);
        }
        if REEXEC_REQUESTED.load(Ordering::SeqCst) {
            log::info!("Re-exec requested, leaving main loop");
            return Ok(LoopExit::Reexec);
        }

        // Reap any zombie processes
        reap_zombies(services);

        // Check service status and restart if needed
        check_services(services, config);

        // Keep /var/log/raven from growing without bound. On the timer rather
        // than on every write: a size check per line written would be a stat
        // per line, and the supervisor does not see the writes in any case --
        // the services write to fds it handed over and has not looked at
        // since. Placed after `reap_zombies` so that a service which has just
        // died has already been reaped, and its log is a file with no writer
        // by the time the sweep reaches it.
        maintain_logs(&mut log_sweep, &log_policy, &log_dir, &init_log);

        // Arm a watch on the parent directory of every ready path still being
        // waited for, and drop the ones nothing waits on any more. This runs
        // before the look below and before the sleep, which is the ordering
        // the whole thing depends on: a watch armed after a check misses
        // exactly the services fast enough to answer in between.
        //
        // The answer is whether every waiting path is covered. It is not
        // always yes -- a daemon that creates its own runtime directory has no
        // directory to watch until it does -- and where it is no, the loop
        // keeps the short timer for that service rather than sleeping through
        // an event that will never come.
        let readiness_watched = match ready_watch.as_mut() {
            Some(watcher) => watcher.arm(
                services
                    .values()
                    .filter(|svc| svc.is_running() && svc.ready_at().is_none())
                    .filter_map(|svc| svc.ready_path()),
            ),
            None => false,
        };

        // Look once with the watch already armed, so a path that appeared
        // before this pass is recorded now rather than slept through.
        control::observe_readiness(services);

        // Sleep until something happens, or until pending work is due. The
        // SIGCHLD pipe is what makes IDLE safe: without it nothing wakes this
        // loop when a child dies, so a machine that could not set it up keeps
        // the short timer it had before there was anything to poll at all.
        let mut wait = if child_wake.is_none()
            || control::wants_quick_tick(services, readiness_watched)
        {
            BUSY
        } else {
            IDLE
        };
        // A pending restart is a deadline, not a reason to look often. Waking
        // on the deadline keeps the restart as punctual as the old 100ms tick
        // made it while a service waiting out the sixty-second backoff sleeps
        // through its own wait instead of holding the loop at ten wakes a
        // second for the whole of it. Floored at BUSY so that a deadline
        // already in the past -- between falling due and `check_services`
        // acting on it, or one left behind by a `stop` issued while a restart
        // was pending -- asks for the tick it always had rather than a
        // zero-length poll.
        if let Some(until) = control::next_retry_in(services, Instant::now()) {
            wait = wait.min(until.max(BUSY));
        }
        {
            use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
            let mut fds = Vec::with_capacity(4);
            if let Some(pipe) = &child_wake {
                fds.push(PollFd::new(pipe.as_fd(), PollFlags::POLLIN));
            }
            if let Some(l) = &control {
                fds.push(PollFd::new(l.as_fd(), PollFlags::POLLIN));
            }
            if let Some(watcher) = &ready_watch {
                fds.push(PollFd::new(watcher.as_fd(), PollFlags::POLLIN));
            }
            // A printer being plugged in wakes the loop here rather than
            // being found by a sweep, which is what makes a demand-started
            // service start within a few milliseconds of its trigger instead
            // of within the loop's two-second idle sleep.
            if let Some(monitor) = &demand_watch {
                fds.push(PollFd::new(monitor.as_fd(), PollFlags::POLLIN));
            }
            if fds.is_empty() {
                // No pipe, no socket, no inotify: nothing to wait on but the
                // clock. This is the degraded path the SIGCHLD warning above
                // describes, and it is why BUSY still exists.
                std::thread::sleep(BUSY);
            } else {
                // Clamped in milliseconds before the cast, not after. `as
                // u16` on its own wraps, so a wait longer than 65.5 seconds
                // would come out as a short one and the loop would spin at
                // whatever the remainder happened to be.
                let timeout = PollTimeout::from(
                    u16::try_from(wait.as_millis()).unwrap_or(u16::MAX),
                );
                match poll(&mut fds, timeout) {
                    Ok(_) | Err(nix::errno::Errno::EINTR) => {}
                    Err(e) => log::warn!("poll: {e}"),
                }
            }
        }
        // Drain whatever the handler wrote; the count is not the point.
        if let Some(pipe) = &child_wake {
            let mut buf = [0u8; 64];
            while nix::unistd::read(std::os::fd::AsRawFd::as_raw_fd(pipe), &mut buf)
                .is_ok_and(|n| n == buf.len())
            {}
        }
        // Likewise for the watch: a readable inotify fd that is never read
        // makes the next poll return immediately, forever.
        if let Some(watcher) = ready_watch.as_mut() {
            watcher.drain();
        }

        // Serve any waiting raven-rc clients.
        if let Some(ref listener) = control {
            match control::poll(listener, services, config) {
                control::Action::None => {}
                control::Action::Poweroff => {
                    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
                    REBOOT_REQUESTED.store(false, Ordering::SeqCst);
                }
                control::Action::Reboot => {
                    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
                    REBOOT_REQUESTED.store(true, Ordering::SeqCst);
                }
                // Inline, and the main loop stops here until the machine is
                // awake again. That is the correct behaviour rather than a
                // compromise: every service is frozen alongside us, so there
                // is nothing to supervise while we are asleep, and a suspend
                // that failed is one this loop should log and carry on from.
                control::Action::Suspend => {
                    if let Err(e) = power::suspend() {
                        log::error!("Suspend failed: {:#}", e);
                    }
                }
                // Inline for the same reason, and the error path matters more
                // here than it does above: `power::hibernate` refuses a
                // machine with no resume device rather than writing an image
                // nothing will read, and this is where that refusal has to be
                // said out loud. The reply has already gone out, so the log is
                // the only place left to say it.
                control::Action::Hibernate => {
                    if let Err(e) = power::hibernate() {
                        log::error!("Hibernate failed: {:#}", e);
                    }
                }
                // Not inline: the listener has to be gone before the exec so
                // the new image can bind the socket, and it is this
                // function's local. Returning drops it.
                control::Action::Reexec => {
                    REEXEC_REQUESTED.store(true, Ordering::SeqCst);
                }
            }
        }

        // Demand-triggered start. Read unconditionally rather than only when
        // the poll named this fd, for the same reason the readiness watch is
        // drained unconditionally: a netlink socket with an unread message
        // stays readable, so a pass that skipped the read would make every
        // subsequent poll return at once, forever.
        //
        // After `control::poll` on purpose. Starting a service can hold this
        // thread for as long as its dependencies take to become ready --
        // `ready_timeout`, five seconds by default, per dependency -- and a
        // raven-rc client that is already waiting on the socket should not be
        // behind a printer. It is the same bound `raven-rc start` has always
        // had on this thread; it is new only in that a device can now reach
        // it without anybody typing anything.
        if let Some(monitor) = demand_watch.as_mut() {
            let wanted = monitor.drain(config);
            if !wanted.is_empty() {
                start_on_demand(&wanted, services, config);
            }
        }

        // The event side of readiness. The poll above returned because a file
        // appeared in a watched directory (or because a child died, or a
        // client connected), and this is the stat that turns that into a
        // `ready_at`. Microseconds after the event rather than at whatever
        // moment the next tick happened to fall, which is the difference
        // between `blame` measuring a service and `blame` measuring itself.
        //
        // Still the only path for a service nobody lists in `after`, and still
        // the safety net for an event that was lost, coalesced or never
        // delivered because there was no inotify to deliver it.
        control::observe_readiness(services);

        // After poll, so a start or stop just requested is visible at once.
        status.publish(services, config);

        // Kept alongside the socket: one word in a file needs no client at all,
        // which is worth having when the socket is what is broken.
        if paths.machine {
            check_command_file()?;
        }
    }
}

/// Start the services a trigger has asked for, and say so.
///
/// The starting goes through `control::start_service` -- the same function
/// `raven-rc start` reaches -- and that is the entire reason this is three
/// lines rather than a fork and an exec. `after` ordering, the wait on a
/// dependency's ready path, the wait for a one-shot dependency to finish, the
/// fallback that reads a definition for a service that is not in the running
/// set, and the diagnostics that name /etc/raven/init.d and tell an operator
/// to run `raven-rc reload`: all of it already exists, and a second start path
/// written for hotplug would be a second answer to "what does starting a
/// service mean", differing from the first in ways nobody would find until a
/// printer was plugged into a machine whose dbus had not come up.
///
/// The reply is a string meant for a person at a terminal, so it is logged a
/// line at a time. An `error:` line is a warning, because it means a device
/// was plugged in and the daemon for it did not start -- somebody is standing
/// at the machine wondering why nothing happened, and the console is where
/// they will look.
fn start_on_demand(
    wanted: &[String],
    services: &mut HashMap<String, Service>,
    config: &InitConfig,
) {
    for name in demand::filter_startable(wanted, services, config) {
        let reply = control::start_service(&name, services, config);
        for line in reply.lines().filter(|l| !l.trim().is_empty()) {
            if line.starts_with("error:") || line.starts_with("warning:") {
                log::warn!("On-demand start of {}: {}", name, line);
            } else {
                log::info!("On-demand start of {}: {}", name, line);
            }
        }
    }
}

/// Wait briefly for seatd's socket, when something will need a seat.
///
/// Bounded: a seat that never appears is a warning, not a hang. Anything that
/// wanted one will fail and say so, which is more use than a stalled boot.
fn wait_for_seat(services: &HashMap<String, Service>) {
    // ravend counts: it starts a compositor of its own for the greeter, before
    // anybody has logged in, and that compositor needs the seat just as much
    // as a session's does.
    let needs_seat = services.contains_key("wayland-session")
        || services.contains_key("ravend");
    if !needs_seat || !services.contains_key("seatd") {
        return;
    }

    // The same watch every other ready path gets. This one is hardcoded
    // rather than read from a definition because seatd's socket path is fixed
    // by libseat, not by init's configuration -- but there is no reason for it
    // to have been the one wait on the machine still done with a timer, and it
    // sits directly in front of the compositor starting, where half a tick of
    // delay is half a tick the greeter is not on screen.
    if readiness::wait_for_path("/run/seatd.sock", Duration::from_secs(5)) {
        log::info!("seatd is ready on /run/seatd.sock");
        return;
    }

    log::warn!("seatd did not create /run/seatd.sock within 5s;");
    log::warn!("  the compositor will not be able to acquire a seat.");
}

fn reap_zombies(services: &mut HashMap<String, Service>) {
    loop {
        match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::Exited(pid, status)) => {
                log::debug!("Process {} exited with status {}", pid, status);
                // Update service state if this was a managed service
                for svc in services.values_mut() {
                    if svc.pid() == Some(pid) {
                        svc.mark_exited(status);
                    }
                }
            }
            Ok(WaitStatus::Signaled(pid, signal, _)) => {
                log::debug!("Process {} killed by signal {:?}", pid, signal);
                for svc in services.values_mut() {
                    if svc.pid() == Some(pid) {
                        svc.mark_signaled(signal);
                    }
                }
            }
            Ok(WaitStatus::StillAlive) | Err(_) => break,
            _ => {}
        }
    }
}

fn check_services(services: &mut HashMap<String, Service>, _config: &InitConfig) {
    for svc in services.values_mut() {
        // Signaled counts as died, not just Exited. A service killed by
        // SIGSEGV, SIGKILL or the OOM killer is the case `restart = true`
        // exists for -- checking only Exited meant a clean exit was restarted
        // while an actual crash was left lying where it fell.
        //
        // Safe against the operator path: `stop` marks the service manually
        // stopped, and should_restart() refuses those. Shutdown never reaches
        // here, because the loop breaks on SHUTDOWN_REQUESTED first.
        let died = matches!(svc.state(), ServiceState::Exited | ServiceState::Signaled);

        if died && svc.should_restart() {
            log::info!("Restarting service: {}", svc.name());
            if let Err(e) = svc.restart() {
                log::error!("Failed to restart {}: {:#}", svc.name(), e);
            }
        }
    }
}

fn check_command_file() -> Result<()> {
    let cmd_path = "/run/raven-init.cmd";
    if Path::new(cmd_path).exists() {
        if let Ok(cmd) = fs::read_to_string(cmd_path) {
            let cmd = cmd.trim();
            log::info!("Received command: {}", cmd);

            match cmd {
                "poweroff" | "halt" => {
                    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
                    REBOOT_REQUESTED.store(false, Ordering::SeqCst);
                }
                "reboot" => {
                    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
                    REBOOT_REQUESTED.store(true, Ordering::SeqCst);
                }
                "suspend" | "sleep" => {
                    // Removed below before we sleep, not after: the file is
                    // gone by the time the machine stops, so a resume cannot
                    // read the same word again and suspend straight back.
                    fs::remove_file(cmd_path).ok();
                    if let Err(e) = power::suspend() {
                        log::error!("Suspend failed: {:#}", e);
                    }
                    return Ok(());
                }
                "hibernate" => {
                    // Removed first for the same reason, and it matters more:
                    // a hibernation that came back would otherwise find the
                    // word still there and hibernate again, which on a machine
                    // whose resume works is an unbreakable loop.
                    fs::remove_file(cmd_path).ok();
                    if let Err(e) = power::hibernate() {
                        log::error!("Hibernate failed: {:#}", e);
                    }
                    return Ok(());
                }
                _ => {
                    log::warn!("Unknown command: {}", cmd);
                }
            }

            // Remove command file
            fs::remove_file(cmd_path).ok();
        }
    }
    Ok(())
}

fn shutdown_services(services: &mut HashMap<String, Service>) -> Result<()> {
    log::info!("Stopping services...");

    // Give services with a stop command the chance to leave cleanly. This runs
    // to completion before any signal is sent, because the whole point is to
    // let a daemon act while it is still alive -- cawd deauthenticating from
    // its AP is the case this exists for. /etc/raven/shutdown.d cannot serve:
    // those scripts run after everything here has already been killed.
    for (name, svc) in services.iter_mut() {
        if svc.has_stop_exec() {
            log::info!("Running stop command for: {}", name);
            svc.run_stop_exec();
        }
    }

    // Send SIGTERM to all services
    for (name, svc) in services.iter_mut() {
        log::info!("Stopping service: {}", name);
        svc.stop();
    }

    // Wait for services to stop (with timeout)
    let timeout = Duration::from_secs(10);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        reap_zombies(services);

        let all_stopped = services
            .values()
            .all(|s| s.state() != ServiceState::Running);
        if all_stopped {
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    // Force kill any remaining services
    for (name, svc) in services.iter_mut() {
        if svc.state() == ServiceState::Running {
            log::warn!("Force killing service: {}", name);
            svc.kill();
        }
    }

    // Run shutdown scripts
    run_shutdown_scripts();

    Ok(())
}

fn run_shutdown_scripts() {
    let shutdown_dir = "/etc/raven/shutdown.d";
    if Path::new(shutdown_dir).is_dir() {
        if let Ok(entries) = fs::read_dir(shutdown_dir) {
            let mut scripts: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            scripts.sort_by_key(|e| e.file_name());

            for entry in scripts {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(metadata) = path.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            log::info!("Running shutdown script: {:?}", path);
                            let _ = Command::new(&path)
                                .stdout(Stdio::null())
                                .stderr(Stdio::null())
                                .status();
                        }
                    }
                }
            }
        }
    }
}

/// Leave every filesystem in a state the imminent `reboot(2)` cannot corrupt.
///
/// The order is the whole content of this function:
///
/// 1. `sync` -- push dirty pages out while everything is still writable.
/// 2. unmount what can be unmounted, deepest first.
/// 3. remount the rest read-only, ending with `/`, which closes the journal.
/// 4. `sync` again -- step 3 can itself dirty metadata, and this is the last
///    chance to write it, since `reboot(2)` syncs nothing.
fn quiesce_filesystems() {
    sync_filesystems();
    unmount_filesystems();
    // init.log is itself a file open for write on `/`, so it would keep the
    // remount busy even with every other process gone.
    close_log_file();
    remount_readonly();
    sync_filesystems();
}

fn sync_filesystems() {
    log::info!("Syncing filesystems...");
    unsafe {
        libc::sync();
    }
}

/// Filesystem types the kernel owns. Unmounting them achieves nothing (they
/// hold no dirty data and vanish with the reboot), and remounting them
/// read-only can fail in ways that are noise rather than signal.
const VIRTUAL_FSTYPES: &[&str] = &[
    "proc",
    "sysfs",
    "devtmpfs",
    "devpts",
    "tmpfs",
    "ramfs",
    "cgroup",
    "cgroup2",
    "securityfs",
    "debugfs",
    "tracefs",
    "configfs",
    "fusectl",
    "bpf",
    "nsfs",
    "mqueue",
    "hugetlbfs",
    "pstore",
    "efivarfs",
    "autofs",
    "binfmt_misc",
];

/// One line of /proc/mounts: where it is mounted, and what kind it is.
fn read_mounts() -> Vec<(String, String)> {
    let Ok(file) = File::open("/proc/mounts") else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // device, mountpoint, fstype, ...
            (parts.len() >= 3).then(|| (parts[1].to_string(), parts[2].to_string()))
        })
        .collect()
}

/// Real filesystems from a mount list, deepest first.
///
/// Deepest first is what lets a nested mount go before the one it sits inside;
/// sorting by path length is enough for that, since a child's mount point is
/// always longer than its parent's. Pure so the ordering can be tested without
/// a real /proc/mounts -- getting it backwards is silent, and shows up only as
/// a filesystem that was still busy at reboot.
fn real_mounts_deepest_first(mounts: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = mounts
        .into_iter()
        .filter(|(_, fstype)| !VIRTUAL_FSTYPES.contains(&fstype.as_str()))
        .collect();
    out.sort_by_key(|(mount_point, _)| std::cmp::Reverse(mount_point.len()));
    out
}

fn unmount_filesystems() {
    log::info!("Unmounting filesystems...");

    // Deepest first, and more than once: a mount can be busy on the first pass
    // only because something nested inside it has not gone yet, and unmounting
    // that child frees the parent. Three passes settles any realistic nesting;
    // whatever is still held after that is handled by the read-only remount,
    // which does not require the mount to be idle.
    for _ in 0..3 {
        let mut progress = false;

        for (mount_point, _) in real_mounts_deepest_first(read_mounts()) {
            if mount_point == "/" {
                continue;
            }
            if nix::mount::umount(mount_point.as_str()).is_ok() {
                log::debug!("Unmounted {}", mount_point);
                progress = true;
            }
        }

        if !progress {
            break;
        }
    }
}

/// Remount every remaining real filesystem read-only, ending with `/`.
///
/// `sync()` alone is not enough to leave a filesystem clean. It pushes dirty
/// pages out, but the filesystem stays mounted read-write with an open journal,
/// so `reboot(2)` -- which does not unmount anything -- leaves it marked dirty.
/// The next boot then replays the journal, and `fsck` treats the filesystem as
/// having come from a crash. Any write that lands between the `sync` and the
/// reboot is lost outright.
///
/// A read-only remount is what closes that window: the kernel flushes the
/// journal and marks the filesystem clean, and nothing can dirty it afterwards.
/// It is the step `unmount_filesystems` cannot do for `/`, which can never be
/// unmounted while it is the root.
///
/// EBUSY is expected and not an error. Something may still hold a file open for
/// write -- a service that ignored SIGTERM, or init's own log -- and a reboot
/// with a dirty root is exactly the state this was already in, so it is logged
/// and stepped over rather than retried forever.
fn remount_readonly() {
    log::info!("Remounting filesystems read-only...");

    let flags = MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY;

    // Deepest first, so `/` is last: remounting it read-only while a real
    // filesystem below is still read-write would be the wrong order to leave
    // them in if a later remount fails.
    for (mount_point, _) in real_mounts_deepest_first(read_mounts()) {
        match mount(
            None::<&str>,
            mount_point.as_str(),
            None::<&str>,
            flags,
            None::<&str>,
        ) {
            Ok(()) => log::info!("Remounted {} read-only", mount_point),
            Err(e) => log::warn!("Could not remount {} read-only: {}", mount_point, e),
        }
    }
}

fn emergency_shell() -> ! {
    eprintln!();
    eprintln!("!!! EMERGENCY SHELL !!!");
    eprintln!("Init has failed. Dropping to emergency shell.");
    eprintln!("Type 'exit' to attempt to continue boot.");
    eprintln!();

    // Keep PID 1 alive: if the user exits the shell, re-open it.
    loop {
        let shells = ["/bin/bash", "/bin/sh"];
        let mut started = false;

        for shell in &shells {
            if !Path::new(shell).exists() {
                continue;
            }

            eprintln!("Starting emergency shell: {shell}");
            let start = Instant::now();
            match Command::new(shell).status() {
                Ok(status) => {
                    // If the shell immediately exits with 127, it's commonly an exec/linker failure
                    // (e.g., missing shared library symbol). Try the next shell.
                    if start.elapsed() < Duration::from_millis(200) && status.code() == Some(127) {
                        eprintln!("Shell {shell} failed to start (exit 127). Trying next...");
                        continue;
                    }

                    started = true;
                    eprintln!("Shell exited (status={status:?}). Returning to emergency mode...");
                    break;
                }
                Err(err) => {
                    eprintln!("Failed to exec {shell}: {err}. Trying next...");
                }
            }
        }

        if !started {
            eprintln!("No shell available. System halted.");
            std::thread::sleep(Duration::from_secs(1));
        } else {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // /etc/fstab options are a mix of two different things: flags that go to
    // mount(2) as bits, and filesystem-specific options that go through as a
    // string. Getting the split wrong means either a silently ignored option or
    // an EINVAL from the kernel, and neither says which one it was.
    #[test]
    fn mount_options_split_into_flags_and_data() {
        let (flags, data) = parse_mount_options("rw,relatime");
        assert_eq!(flags, MsFlags::MS_RELATIME);
        assert_eq!(data, "");

        // The ESP line raven-install writes. fmask/dmask are vfat's, not the
        // kernel's, and have to survive as data.
        let (flags, data) = parse_mount_options("rw,noatime,fmask=0077,dmask=0077");
        assert_eq!(flags, MsFlags::MS_NOATIME);
        assert_eq!(data, "fmask=0077,dmask=0077");

        let (flags, data) = parse_mount_options("rw,nosuid,nodev,mode=1777");
        assert_eq!(flags, MsFlags::MS_NOSUID | MsFlags::MS_NODEV);
        assert_eq!(data, "mode=1777");

        // "defaults" is not an option, it is the absence of any.
        let (flags, data) = parse_mount_options("defaults");
        assert_eq!(flags, MsFlags::empty());
        assert_eq!(data, "");

        let (flags, _) = parse_mount_options("ro");
        assert_eq!(flags, MsFlags::MS_RDONLY);
    }

    // A `sync` leaves a filesystem with its data written and its journal open,
    // so `reboot(2)` -- which unmounts nothing -- still marks it dirty. These
    // cover the ordering and filtering that the read-only remount depends on;
    // the remount itself needs a real mount table and a reboot to observe.

    #[test]
    fn kernel_filesystems_are_left_alone() {
        // Unmounting or remounting these achieves nothing: they hold no dirty
        // data and go away with the reboot. Only the real ones matter.
        let mounts = vec![
            ("/proc".to_string(), "proc".to_string()),
            ("/sys".to_string(), "sysfs".to_string()),
            ("/dev".to_string(), "devtmpfs".to_string()),
            ("/dev/shm".to_string(), "tmpfs".to_string()),
            ("/run".to_string(), "tmpfs".to_string()),
            ("/sys/fs/cgroup".to_string(), "cgroup2".to_string()),
            ("/".to_string(), "ext4".to_string()),
        ];
        let real = real_mounts_deepest_first(mounts);
        assert_eq!(
            real,
            vec![("/".to_string(), "ext4".to_string())],
            "only the real filesystem should survive the filter"
        );
    }

    /// The two filesystems mounted so the kernel can be interrogated have to
    /// stay root-only and have to survive shutdown's unmount sweep.
    ///
    /// There is no way to test the mount itself: securityfs and debugfs are
    /// not FS_USERNS_MOUNT, so not even a user namespace lets this suite mount
    /// one, and PID 1's own mount namespace is the machine's. What can be
    /// pinned down is the part that is a decision rather than a syscall --
    /// that debugfs is never handed out world-readable, and that neither entry
    /// is missing from the do-not-unmount list, which would have shutdown
    /// trying to remount a kernel filesystem read-only.
    #[test]
    fn the_introspection_filesystems_are_root_only_and_never_unmounted() {
        let types: Vec<&str> = INTROSPECTION_FILESYSTEMS
            .iter()
            .map(|(fstype, _, _)| *fstype)
            .collect();
        assert!(
            types.contains(&"securityfs"),
            "without securityfs there is no way to read which LSMs are active"
        );
        assert!(
            types.contains(&"debugfs"),
            "without debugfs the scheduler's runtime switches are unreachable"
        );

        for (fstype, target, data) in INTROSPECTION_FILESYSTEMS {
            assert!(
                VIRTUAL_FSTYPES.contains(fstype),
                "{fstype} is not in VIRTUAL_FSTYPES, so shutdown would try to remount it"
            );
            assert!(
                target.starts_with("/sys/kernel/"),
                "{target} is not where the kernel's own trees live"
            );
            if *fstype == "debugfs" {
                assert_eq!(
                    *data, "mode=0700",
                    "debugfs must not be reachable by anyone but root"
                );
            }
        }
    }

    #[test]
    fn the_root_is_remounted_last() {
        // `/` must come last. Remounting it read-only while a real filesystem
        // below it is still read-write leaves them inconsistent if a later
        // remount fails.
        let mounts = vec![
            ("/".to_string(), "ext4".to_string()),
            ("/home".to_string(), "ext4".to_string()),
            ("/boot".to_string(), "vfat".to_string()),
            ("/home/javan/data".to_string(), "xfs".to_string()),
        ];
        let order: Vec<String> = real_mounts_deepest_first(mounts)
            .into_iter()
            .map(|(mount_point, _)| mount_point)
            .collect();

        assert_eq!(order.last().map(String::as_str), Some("/"), "{order:?}");
        assert_eq!(
            order.first().map(String::as_str),
            Some("/home/javan/data"),
            "deepest should go first: {order:?}"
        );
        // A child must precede the parent it sits inside.
        let child = order.iter().position(|m| m == "/home/javan/data").unwrap();
        let parent = order.iter().position(|m| m == "/home").unwrap();
        assert!(child < parent, "nested mount must come first: {order:?}");
    }

    // Anything that is not an absolute path needs a blkid-style scan, which at
    // this point in the boot only mount(8) can do. Returning None is what sends
    // the caller down that fallback, so a wrong Some() here is a failed mount.
    #[test]
    fn only_absolute_paths_resolve_without_a_scan() {
        assert_eq!(
            resolve_fstab_spec("/dev/nvme0n1p3"),
            Some("/dev/nvme0n1p3".to_string())
        );

        // These must name nothing that can exist, because this test runs on
        // real machines too. `LABEL=RAVEN_ROOT` used to be here, and on an
        // installed RavenLinux it resolves -- raven-install labels the root
        // exactly that -- so the suite passed in a build container and failed
        // on the operating system it belongs to. A test whose result depends
        // on the disks in the machine running it is testing the machine.
        assert_eq!(resolve_fstab_spec("UUID=00000000-no-such-uuid"), None);
        assert_eq!(resolve_fstab_spec("LABEL=RAVEN_NO_SUCH_LABEL_9f3a"), None);
        assert_eq!(resolve_fstab_spec("PARTUUID=00000000-no-such-part"), None);

        // Not a recognised form at all.
        assert_eq!(resolve_fstab_spec("tmpfs"), None);
        assert_eq!(resolve_fstab_spec("proc"), None);
    }
}

#[cfg(test)]
mod builtin_module_options_tests {
    use super::*;

    /// A modprobe.d and a /sys/module with one built-in module (`rtw88_pci`,
    /// parameter `disable_aspm` = N) and nothing for `rtw89_pci`.
    fn fixture(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("raven-modopts-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let modprobe_d = root.join("modprobe.d");
        let sys_module = root.join("module");
        fs::create_dir_all(&modprobe_d).expect("mkdir");
        fs::create_dir_all(sys_module.join("rtw88_pci/parameters")).expect("mkdir");
        fs::write(sys_module.join("rtw88_pci/parameters/disable_aspm"), "N").expect("write");
        (modprobe_d, sys_module)
    }

    #[test]
    fn options_line_reaches_a_built_in_module() {
        // The shipped file, comments and all. The rtw89 line has no
        // /sys/module entry (loadable or absent) and must be left to modprobe
        // without tripping over anything.
        let (modprobe_d, sys_module) = fixture("shipped");
        fs::write(
            modprobe_d.join("rtw88.conf"),
            "# RavenLinux: Realtek rtw88 defaults\noptions rtw88_pci disable_aspm=1\n",
        )
        .expect("write");
        fs::write(
            modprobe_d.join("rtw89.conf"),
            "options rtw89_pci disable_aspm=1\n",
        )
        .expect("write");
        // Not a .conf: modprobe ignores it, so must we.
        fs::write(
            modprobe_d.join("rtw88.conf.bak"),
            "options rtw88_pci disable_aspm=0\n",
        )
        .expect("write");

        apply_builtin_module_options(&modprobe_d, &sys_module);

        let value =
            fs::read_to_string(sys_module.join("rtw88_pci/parameters/disable_aspm")).expect("read");
        assert_eq!(value, "1");
        assert!(!sys_module.join("rtw89_pci").exists());
    }

    #[test]
    fn dash_in_module_name_maps_to_the_kernel_spelling() {
        let (modprobe_d, sys_module) = fixture("dash");
        fs::write(
            modprobe_d.join("x.conf"),
            "options rtw88-pci disable_aspm=1 disable_msi=1\n",
        )
        .expect("write");

        apply_builtin_module_options(&modprobe_d, &sys_module);

        let value =
            fs::read_to_string(sys_module.join("rtw88_pci/parameters/disable_aspm")).expect("read");
        assert_eq!(value, "1");
    }
}

#[cfg(test)]
mod complaint_latch_tests {
    use super::*;

    /// The flood the latch exists to stop, in the form it actually arrives in.
    ///
    /// Comparing each complaint against the single previous one suppressed
    /// nothing in practice, for two independent reasons that are both
    /// permanent states rather than transients. A directory over its total cap
    /// reports the live byte count, which moves every time a service writes a
    /// line, so no two sweeps ever produced the same string; and a sweep that
    /// finds two faults leaves the *second* one remembered, so on the next
    /// sweep the first differs from it and both warn again. Either way a WARN
    /// reached the console every sixty seconds for the life of the machine --
    /// printed over whoever was logging in on tty1, which is the thing the
    /// doc comment above says must not happen.
    #[test]
    fn a_complaint_whose_numbers_move_is_still_the_same_complaint() {
        let mut latch = ComplaintLatch::new();

        assert!(latch.admit("/var/log/raven is 201.4M, over the 200M cap"));
        assert!(
            !latch.admit("/var/log/raven is 201.5M, over the 200M cap"),
            "a cap overshoot that grew by a hundred kilobytes is the same \
             permanent condition, said once"
        );
        assert!(!latch.admit("/var/log/raven is 214.0M, over the 200M cap"));
    }

    #[test]
    fn two_faults_in_one_sweep_are_each_said_once() {
        let mut latch = ComplaintLatch::new();
        let sweep = [
            "cannot rotate dbus.log: Read-only file system",
            "cannot rotate cawd.log: Read-only file system",
        ];

        for line in sweep {
            assert!(latch.admit(line), "the first sweep says both");
        }
        for line in sweep {
            assert!(
                !latch.admit(line),
                "and no later sweep says either of them again"
            );
        }

        // A genuinely new fault still reaches the console.
        assert!(latch.admit("cannot prune dbus.log.3: Read-only file system"));
    }

    /// The memory is bounded, because the input is filenames and this is PID 1.
    #[test]
    fn the_latch_remembers_a_bounded_number_of_complaints() {
        let mut latch = ComplaintLatch::new();
        for n in 0..COMPLAINTS_REMEMBERED {
            // Distinct in letters, not in digits, since digits are what the
            // key collapses.
            assert!(latch.admit(&format!("cannot rotate {}.log", "x".repeat(n + 1))));
        }
        assert_eq!(latch.said.len(), COMPLAINTS_REMEMBERED);

        assert!(latch.admit("cannot rotate newcomer.log"));
        assert_eq!(
            latch.said.len(),
            COMPLAINTS_REMEMBERED,
            "the oldest is dropped rather than the set growing"
        );
        assert!(
            !latch.admit("cannot rotate newcomer.log"),
            "and what is still remembered is what is still arriving"
        );
    }
}

#[cfg(test)]
mod reexec_start_filter_tests {
    use super::*;
    use crate::service::ServiceSnapshot;

    /// What a hand-off says about a service that is not running: a definition,
    /// no pid, and the times of the run it did have.
    fn snapshot_of(config: &ServiceConfig, ran: bool) -> ServiceSnapshot {
        let mono = timeline::instant_secs(Instant::now());
        ServiceSnapshot {
            config: config.clone(),
            pid: None,
            started_mono: ran.then_some(mono),
            ready_mono: ran.then_some(mono),
            first_started_mono: ran.then_some(mono),
            first_ready_mono: ran.then_some(mono),
            ..ServiceSnapshot::default()
        }
    }

    fn oneshot(name: &str) -> ServiceConfig {
        ServiceConfig {
            name: name.to_string(),
            exec: "/bin/true".to_string(),
            service_type: ServiceType::Oneshot,
            ..ServiceConfig::default()
        }
    }

    /// The defect: `raven-rc reexec` re-ran every one-shot that had already
    /// completed, because a finished one-shot and one that never started are
    /// both `Stopped` after `adopt`.
    #[test]
    fn a_completed_one_shot_is_not_re_run_by_a_re_exec() {
        let cfg = oneshot("udev");
        let svc = Service::adopt(snapshot_of(&cfg, true), cfg.clone());

        // The state the filter used to look at, and why it was not enough.
        assert_eq!(svc.state(), ServiceState::Stopped);
        assert_eq!(svc.oneshot_outcome(), Some(OneshotOutcome::Completed));

        assert!(
            !wants_start_after_reexec(&cfg, Some(&svc)),
            "a coldplug that already ran this boot must not run again"
        );
    }

    /// And the three things the same filter must keep doing.
    #[test]
    fn a_re_exec_still_starts_what_never_had_its_turn() {
        // A definition the previous supervisor did not have at all.
        let added = oneshot("console-font");
        assert!(wants_start_after_reexec(&added, None));

        // A one-shot the previous supervisor knew and never started -- it was
        // added by a reload, or its dependency was unavailable.
        let never = Service::adopt(snapshot_of(&added, false), added.clone());
        assert!(
            wants_start_after_reexec(&added, Some(&never)),
            "a one-shot with no run behind it still has one coming"
        );

        // A daemon waiting out its restart backoff when the re-exec landed.
        let daemon = ServiceConfig {
            name: "cawd".to_string(),
            exec: "/bin/true".to_string(),
            restart: true,
            ..ServiceConfig::default()
        };
        let waiting = Service::adopt(snapshot_of(&daemon, true), daemon.clone());
        assert!(
            wants_start_after_reexec(&daemon, Some(&waiting)),
            "a restartable daemon that is down is still brought back"
        );

        // The same service with `restart = false`, which asked not to be.
        let once = ServiceConfig {
            restart: false,
            ..daemon.clone()
        };
        let stopped = Service::adopt(snapshot_of(&once, true), once.clone());
        assert!(!wants_start_after_reexec(&once, Some(&stopped)));
    }
}

#[cfg(test)]
mod oneshot_wait_tests {
    use super::*;

    /// A one-shot that never finishes costs its `ready_timeout` once, not once
    /// per service ordered after it.
    ///
    /// The wait lives inside the loop over one service's `after` list, so it
    /// was re-entered with a fresh deadline for every dependant. init.toml has
    /// nine enabled services with `after = ["udev"]` and udev takes the
    /// default five-second timeout, so a coldplug wedged on a device probe
    /// spent forty-five seconds asleep in PID 1's only thread and printed nine
    /// warnings that all said "after 5s" -- a boot that reads as hung, for a
    /// dependency the supervisor had already decided to stop waiting for.
    ///
    /// Real processes rather than a mock, because what is being measured is
    /// wall-clock time spent inside `start_services` itself.
    #[test]
    fn a_hung_one_shot_is_waited_for_once_not_once_per_dependant() {
        let root = std::env::temp_dir().join(format!("raven-oneshot-wait-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");
        // A directory that is not a cgroup2 tree, and a log directory of this
        // test's own: nothing here may reach the machine's /sys or /var.
        std::env::set_var("RAVEN_CGROUP_ROOT", root.join("not-a-cgroup2-tree"));
        std::env::set_var("RAVEN_SERVICE_LOG_DIR", &root);

        let hang = ServiceConfig {
            name: "coldplug".to_string(),
            exec: "/bin/sleep".to_string(),
            args: vec!["30".to_string()],
            service_type: ServiceType::Oneshot,
            // The smallest the field can express, so the test costs one second
            // rather than five.
            ready_timeout: 1,
            ..ServiceConfig::default()
        };
        let dependants: Vec<ServiceConfig> = ["dbus", "powerd", "network"]
            .iter()
            .map(|name| ServiceConfig {
                name: name.to_string(),
                exec: "/bin/sleep".to_string(),
                args: vec!["30".to_string()],
                after: vec!["coldplug".to_string()],
                ..ServiceConfig::default()
            })
            .collect();

        let mut services = vec![hang];
        services.extend(dependants);
        let config = InitConfig {
            services,
            ..InitConfig::default()
        };

        let started_at = Instant::now();
        let started = start_services(&config).expect("nothing here is critical");
        let spent = started_at.elapsed();

        // Kill and reap before asserting. A test that fails with four sleeps
        // still holding cargo's stdout pipe open hangs the harness instead of
        // reporting the failure.
        let mut names: Vec<&String> = started.keys().collect();
        names.sort();
        let names: Vec<String> = names.into_iter().cloned().collect();
        for name in &names {
            if let Some(pid) = started[name].pid() {
                unsafe {
                    libc::kill(pid.as_raw(), libc::SIGKILL);
                    let mut status = 0;
                    libc::waitpid(pid.as_raw(), &mut status, 0);
                }
            }
        }
        let _ = fs::remove_dir_all(&root);

        assert_eq!(
            names,
            vec![
                "coldplug".to_string(),
                "dbus".to_string(),
                "network".to_string(),
                "powerd".to_string()
            ],
            "every service still starts; the wait is an ordering, not a gate"
        );
        assert!(
            spent >= Duration::from_millis(900),
            "the first dependant must still wait the one-shot out: {spent:?}"
        );
        assert!(
            spent < Duration::from_millis(2500),
            "the other two must not each pay for it again: {spent:?}"
        );
    }
}
