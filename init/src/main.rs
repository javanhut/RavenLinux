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

mod config;
mod control;
mod service;

use config::{InitConfig, ServiceConfig};
use service::{Service, ServiceState};

/// Global flag for shutdown request
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static REBOOT_REQUESTED: AtomicBool = AtomicBool::new(false);

fn main() {
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
    file: std::sync::Mutex<Option<std::fs::File>>,
}

impl log::Log for DualLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!("[raven-init] {}: {}\n", record.level(), record.args());
        // The console shows only what needs a human: WARN and ERROR. INFO
        // lines -- every "Started service", every clean exit -- go to the log
        // file alone, because after the gettys are up the console belongs to
        // whoever is logging in, and init chatter printed over a login prompt
        // reads as a broken boot to exactly the person it should reassure.
        if record.level() <= log::Level::Warn {
            eprint!("{}", line);
        }
        if let Ok(mut guard) = self.file.lock() {
            // Opened lazily: /var/log may not be writable until the root is
            // mounted rw, and boot must not wait on it.
            if guard.is_none() {
                std::fs::create_dir_all("/var/log/raven").ok();
                *guard = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/var/log/raven/init.log")
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

fn init_logging() {
    let logger = Box::new(DualLogger {
        file: std::sync::Mutex::new(None),
    });
    if log::set_boxed_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
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
         running copy still holds will fail to start and be restarted until the\n\
         restart budget is spent.\n\
         \n\
         To manage the running system, use raven-rc.\n\
         To test raven-init anyway, set RAVEN_INIT_ALLOW_NONPID1=1.",
        pid1,
        std::process::id()
    );
}

fn run_init() -> Result<()> {
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
    log::info!("Phase 2b: Re-probing driverless PCI devices");
    reprobe_orphan_pci_devices();

    // Loopback is nobody's service, so nothing else brings it up -- and a down
    // `lo` quietly breaks everything that talks to 127.0.0.1.
    bring_loopback_up();

    // Let unprivileged ping work. The image's /sbin/ping has neither setuid
    // nor file capabilities (squashfs is not built with xattrs), so its raw
    // ICMP socket fails -- and the kernel's unprivileged ICMP datagram
    // fallback is disabled by default (ping_group_range is "1 0", an empty
    // range). Opening the range to every group is what systemd-based distros
    // ship, and the first thing anyone types on a machine with new networking
    // is ping.
    if let Err(e) = fs::write("/proc/sys/net/ipv4/ping_group_range", "0 2147483647") {
        log::warn!("Could not enable unprivileged ping: {}", e);
    }

    // Phase 3: Load configuration
    log::info!("Phase 3: Loading configuration");
    let mut config = load_config()?;
    apply_kernel_cmdline_overrides(&mut config)?;
    fixup_getty_login_programs(&mut config);

    // Phase 4: Setup signal handlers
    log::info!("Phase 4: Setting up signal handlers");
    setup_signal_handlers()?;

    // Phase 5: Start services
    log::info!("Phase 5: Starting services");
    let mut services = start_services(&config)?;

    // seatd and the compositor are started back to back, and the compositor
    // connects to /run/seatd.sock the moment it starts. seatd has not created
    // it yet, so the first attempt always fails -- and the restart budget can
    // be spent on that race before seatd is ever ready.
    //
    // A real dependency system would express this properly; until there is one,
    // waiting for the socket is the honest version of what "after seatd" means.
    wait_for_seat(&services);

    // Display welcome message
    print_welcome();

    // Phase 6: Main loop - reap zombies and handle signals
    log::info!("Phase 6: Entering main loop");
    main_loop(&mut services, &mut config)?;

    // Phase 7: Shutdown
    log::info!("Phase 7: Shutting down");
    shutdown_services(&mut services)?;

    // Leave no socket behind. A stale one makes the next raven-rc fail with
    // ECONNREFUSED rather than "not running", which reads like a broken
    // service manager instead of an absent one.
    fs::remove_file(control::SOCKET_PATH).ok();

    // Determine shutdown mode
    if REBOOT_REQUESTED.load(Ordering::SeqCst) {
        log::info!("Rebooting system...");
        sync_filesystems();
        unmount_filesystems();
        let _ = reboot(RebootMode::RB_AUTOBOOT);
    } else {
        log::info!("Powering off system...");
        sync_filesystems();
        unmount_filesystems();
        let _ = reboot(RebootMode::RB_POWER_OFF);
    }

    Ok(())
}

/// Find a system program, trying the paths things actually get installed to.
///
/// Hardcoding one path is how seatd ended up unstartable: the synthesized
/// service named /bin/seatd while the image installs /sbin/seatd, so seatd
/// never ran, and the compositor failed with "Failed to open session: No such
/// file or directory" -- an error about a missing socket, which reads like a
/// seat/permission problem rather than a daemon that was never started. The
/// same mistake had already been made with unix_chkpwd and with login.
fn find_program(name: &str) -> Option<String> {
    find_program_in(&SYSTEM_BIN_DIRS, name)
}

/// Where system programs get installed, in search order.
const SYSTEM_BIN_DIRS: [&str; 5] = ["/sbin", "/usr/sbin", "/bin", "/usr/bin", "/usr/local/bin"];

/// The searchable half of [`find_program`], split out so the ordering can be
/// tested without depending on what the build host happens to have installed.
fn find_program_in(dirs: &[&str], name: &str) -> Option<String> {
    dirs.iter()
        .map(|dir| format!("{}/{}", dir, name))
        .find(|path| Path::new(path).is_file())
}

fn fixup_getty_login_programs(config: &mut InitConfig) {
    if Path::new("/bin/raven-shell").exists() {
        return;
    }

    for svc in &mut config.services {
        if !svc.exec.ends_with("agetty") {
            continue;
        }
        let mut idx = 0;
        while idx + 1 < svc.args.len() {
            if svc.args[idx] == "--login-program" && svc.args[idx + 1] == "/bin/raven-shell" {
                svc.args[idx + 1] = "/bin/sh".to_string();
            }
            idx += 1;
        }
    }
}

fn apply_kernel_cmdline_overrides(config: &mut InitConfig) -> Result<()> {
    let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    let graphics = cmdline
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("raven.graphics="));
    let wayland_choice = cmdline
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("raven.wayland="));

    if graphics != Some("wayland") {
        return Ok(());
    }

    log::info!("Kernel cmdline requested Wayland graphics");

    // Disable tty1 getty by default to avoid fighting for the tty.
    for svc in &mut config.services {
        if svc.name == "getty-tty1" {
            svc.enabled = false;
        }
    }

    // Avoid starting both a compositor and the session wrapper at once.
    for svc in &mut config.services {
        if svc.name == "raven-compositor" || svc.name == "wayland-session" {
            svc.enabled = false;
        }
    }

    // Ensure runtime dirs for root session exist.
    fs::create_dir_all("/run/user/0").ok();
    let _ = fs::set_permissions("/run/user/0", fs::Permissions::from_mode(0o700));

    // Without seatd there is no seat, and without a seat the compositor cannot
    // take DRM master -- so if it is missing, say so here rather than leaving
    // the compositor to fail in a restart loop with a misleading message.
    let seatd_path = find_program("seatd");
    if seatd_path.is_none() {
        log::warn!("seatd not found; a Wayland session will not be able to acquire a seat");
    }

    ensure_service(
        &mut config.services,
        ServiceConfig {
            name: "seatd".to_string(),
            description: "Seat management daemon".to_string(),
            exec: seatd_path.unwrap_or_else(|| "/sbin/seatd".to_string()),
            args: vec!["-g".to_string(), "video".to_string()],
            restart: true,
            enabled: true,
            critical: false,
            environment: HashMap::new(),
            tty: None,
            runtime_dirs: Vec::new(),
            after: vec!["udev".to_string()],
            ready_path: Some("/run/seatd.sock".to_string()),
            ready_timeout: 5,
            stop_exec: None,
            stop_args: Vec::new(),
            stop_timeout: 5,
        },
    );

    let mut compositor_env = HashMap::new();
    compositor_env.insert("XDG_RUNTIME_DIR".to_string(), "/run/user/0".to_string());
    compositor_env.insert("LIBSEAT_BACKEND".to_string(), "seatd".to_string());

    let session_path = find_program("raven-wayland-session");
    if let Some(ref session_exec) = session_path {
        let mut env = compositor_env;
        env.insert(
            "RAVEN_WAYLAND_COMPOSITOR".to_string(),
            wayland_choice.unwrap_or("raven-compositor").to_string(),
        );

        ensure_service(
            &mut config.services,
            ServiceConfig {
                name: "wayland-session".to_string(),
                description: "Raven Wayland session".to_string(),
                exec: session_exec.clone(),
                args: Vec::new(),
                restart: true,
                enabled: true,
                critical: false,
                environment: env,
                tty: None,
                runtime_dirs: Vec::new(),
                after: vec!["udev".to_string(), "seatd".to_string()],
                ready_path: None,
                ready_timeout: 5,
                stop_exec: None,
                stop_args: Vec::new(),
                stop_timeout: 5,
            },
        );
    } else {
        // Fallback to raven-compositor directly
        ensure_service(
            &mut config.services,
            ServiceConfig {
                name: "raven-compositor".to_string(),
                description: "Raven Wayland compositor".to_string(),
                exec: "/bin/raven-compositor".to_string(),
                args: vec!["--backend".to_string(), "udev".to_string()],
                restart: true,
                enabled: true,
                critical: false,
                environment: compositor_env,
                tty: None,
                runtime_dirs: Vec::new(),
                after: vec!["udev".to_string(), "seatd".to_string()],
                ready_path: None,
                ready_timeout: 5,
                stop_exec: None,
                stop_args: Vec::new(),
                stop_timeout: 5,
            },
        );
    }

    Ok(())
}

fn ensure_service(services: &mut Vec<ServiceConfig>, desired: ServiceConfig) {
    let Some(existing) = services.iter_mut().find(|s| s.name == desired.name) else {
        services.push(desired);
        return;
    };

    existing.description = desired.description;
    existing.exec = desired.exec;
    existing.args = desired.args;
    existing.restart = desired.restart;
    existing.enabled = desired.enabled;
    existing.critical = desired.critical;
    existing.environment = desired.environment;
}

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

    // Mount cgroups if available
    if Path::new("/sys/fs/cgroup").exists() || fs::create_dir_all("/sys/fs/cgroup").is_ok() {
        mount_fs("cgroup2", "/sys/fs/cgroup", "cgroup2", MsFlags::empty(), "").ok();
    }

    log::info!("Essential filesystems mounted");
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
    } else if let Some(v) = spec.strip_prefix("PARTLABEL=") {
        ("/dev/disk/by-partlabel", v)
    } else {
        return None;
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
        .args(["--hctosys", "--utc"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn load_config() -> Result<InitConfig> {
    let config_paths = ["/etc/raven/init.toml", "/etc/init.toml"];

    for path in &config_paths {
        if Path::new(path).exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(mut config) = toml::from_str::<InitConfig>(&content) {
                    log::info!("Loaded configuration from {}", path);
                    // Remembered so enable/disable know what to rewrite.
                    config.source_path = Some(std::path::PathBuf::from(path));
                    return Ok(config);
                }
            }
        }
    }

    log::info!("Using default configuration");
    Ok(InitConfig::default())
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
    let mut services = HashMap::new();
    let mut pending: Vec<&ServiceConfig> = config.services.iter().filter(|s| s.enabled).collect();
    let mut unavailable: Vec<String> = Vec::new();

    // Resolve the small dependency graph as services are started. Configuration
    // order remains the tie-breaker, but `after` is authoritative.
    while !pending.is_empty() {
        let mut progressed = false;
        let mut index = 0;
        while index < pending.len() {
            let svc_config = pending[index];
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
                    if let Some(path) = dep_cfg.ready_path.as_deref() {
                        if !wait_for_ready_path(path, dep_cfg.ready_timeout) {
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

            // A binary that is not installed is not a failure. Half of this
            // list is software the owner may add later -- init.toml ships an
            // sshd entry precisely so that `rvn install openssh` makes the
            // next boot start it -- and an ERROR on the console for every
            // absent optional daemon buries the login prompt under noise.
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
            enabled: true,
            critical: false,
            environment: HashMap::new(),
            tty: Some("/dev/tty1".to_string()),
            stop_exec: None,
            stop_args: Vec::new(),
            stop_timeout: 5,
            runtime_dirs: Vec::new(),
            after: Vec::new(),
            ready_path: None,
            ready_timeout: 5,
        };

        // Try agetty first, fall back to direct shell
        let svc = Service::start(&getty_config).or_else(|_| {
            let shell_config = ServiceConfig {
                name: "shell-tty1".to_string(),
                description: "Shell on tty1".to_string(),
                exec: "/bin/sh".to_string(),
                args: vec![],
                restart: true,
                enabled: true,
                critical: false,
                environment: HashMap::new(),
                tty: Some("/dev/tty1".to_string()),
                stop_exec: None,
                stop_args: Vec::new(),
                stop_timeout: 5,
                runtime_dirs: Vec::new(),
                after: Vec::new(),
                ready_path: None,
                ready_timeout: 5,
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

fn wait_for_ready_path(path: &str, timeout: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(timeout as u64);
    while Instant::now() < deadline {
        if Path::new(path).exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Path::new(path).exists()
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

fn main_loop(services: &mut HashMap<String, Service>, config: &mut InitConfig) -> Result<()> {
    // The control socket is how raven-rc asks about services and starts or
    // stops them. A failure to bind it is not fatal: PID 1 supervising
    // services matters more than PID 1 being controllable, and the
    // /run/raven-init.cmd fallback below still works.
    let control = match control::listen() {
        Ok(listener) => Some(listener),
        Err(e) => {
            log::warn!("Control socket unavailable: {:#}", e);
            log::warn!("  raven-rc service commands will not work this boot");
            None
        }
    };

    log::info!("Entering main loop");

    loop {
        // Check for shutdown request
        if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
            log::info!("Shutdown requested, exiting main loop");
            break;
        }

        // Reap any zombie processes
        reap_zombies(services);

        // Check service status and restart if needed
        check_services(services, config);

        // Sleep briefly to avoid busy-waiting
        std::thread::sleep(Duration::from_millis(100));

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
            }
        }

        // Kept alongside the socket: one word in a file needs no client at all,
        // which is worth having when the socket is what is broken.
        check_command_file()?;
    }

    Ok(())
}

/// Wait briefly for seatd's socket, when something will need a seat.
///
/// Bounded: a seat that never appears is a warning, not a hang. Anything that
/// wanted one will fail and say so, which is more use than a stalled boot.
fn wait_for_seat(services: &HashMap<String, Service>) {
    let needs_seat =
        services.contains_key("wayland-session") || services.contains_key("raven-compositor");
    if !needs_seat || !services.contains_key("seatd") {
        return;
    }

    let socket = Path::new("/run/seatd.sock");
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline {
        if socket.exists() {
            log::info!("seatd is ready on /run/seatd.sock");
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
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

fn sync_filesystems() {
    log::info!("Syncing filesystems...");
    unsafe {
        libc::sync();
    }
}

fn unmount_filesystems() {
    log::info!("Unmounting filesystems...");

    // Read current mounts
    let mounts: Vec<String> = if let Ok(file) = File::open("/proc/mounts") {
        let reader = BufReader::new(file);
        reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
            .collect()
    } else {
        return;
    };

    // Unmount in reverse order, skipping essential ones
    let skip = ["/proc", "/sys", "/dev", "/run", "/"];
    for mount_point in mounts.iter().rev() {
        if !skip.contains(&mount_point.as_str()) {
            log::debug!("Unmounting {}", mount_point);
            let _ = nix::mount::umount(mount_point.as_str());
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

    // Anything that is not an absolute path needs a blkid-style scan, which at
    // this point in the boot only mount(8) can do. Returning None is what sends
    // the caller down that fallback, so a wrong Some() here is a failed mount.
    #[test]
    fn only_absolute_paths_resolve_without_a_scan() {
        assert_eq!(
            resolve_fstab_spec("/dev/nvme0n1p3"),
            Some("/dev/nvme0n1p3".to_string())
        );

        // No /dev/disk/by-uuid in a test environment, so these fall through.
        assert_eq!(resolve_fstab_spec("UUID=1234-5678-90ab"), None);
        assert_eq!(resolve_fstab_spec("LABEL=RAVEN_ROOT"), None);
        assert_eq!(resolve_fstab_spec("PARTUUID=deadbeef-01"), None);

        // Not a recognised form at all.
        assert_eq!(resolve_fstab_spec("tmpfs"), None);
        assert_eq!(resolve_fstab_spec("proc"), None);
    }
}

#[cfg(test)]
mod path_resolution_tests {
    use super::*;

    /// A directory layout mirroring the image: seatd and login in /sbin only.
    fn fixture(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("raven-find-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in ["sbin", "bin"] {
            fs::create_dir_all(root.join(d)).expect("mkdir");
        }
        fs::write(root.join("sbin/seatd"), "#!/bin/sh\n").expect("write");
        fs::write(root.join("bin/sh"), "#!/bin/sh\n").expect("write");
        root
    }

    #[test]
    fn finds_a_program_that_lives_only_in_sbin() {
        // The bug this exists for: seatd, login and unix_chkpwd all install to
        // /sbin, and each was looked up at a hardcoded /bin path. The service
        // then failed with an error about a missing socket or a broken auth
        // stack rather than a program that was never run.
        let root = fixture("sbin");
        let dirs: Vec<String> = ["sbin", "bin"]
            .iter()
            .map(|d| root.join(d).display().to_string())
            .collect();
        let refs: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();

        let found = find_program_in(&refs, "seatd").expect("seatd must be found in sbin");
        assert!(found.ends_with("/sbin/seatd"), "{found}");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_programs_report_absent_rather_than_guessing_a_path() {
        let root = fixture("missing");
        let dirs: Vec<String> = ["sbin", "bin"]
            .iter()
            .map(|d| root.join(d).display().to_string())
            .collect();
        let refs: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();

        assert_eq!(find_program_in(&refs, "definitely-not-installed"), None);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_order_prefers_sbin_over_bin() {
        let root = fixture("order");
        // Same name in both; /sbin is where system daemons belong.
        fs::write(root.join("sbin/dup"), "s").expect("write");
        fs::write(root.join("bin/dup"), "b").expect("write");

        let dirs: Vec<String> = ["sbin", "bin"]
            .iter()
            .map(|d| root.join(d).display().to_string())
            .collect();
        let refs: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();

        let found = find_program_in(&refs, "dup").expect("found");
        assert!(found.contains("/sbin/"), "{found}");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_real_search_path_covers_where_the_image_installs_things() {
        // Guards the constant itself: seatd, login and unix_chkpwd all land in
        // /sbin in this image, so dropping it from the list would silently
        // reintroduce every one of those bugs.
        assert!(SYSTEM_BIN_DIRS.contains(&"/sbin"));
        assert!(SYSTEM_BIN_DIRS.contains(&"/usr/bin"));
        assert!(SYSTEM_BIN_DIRS.contains(&"/bin"));
    }
}
