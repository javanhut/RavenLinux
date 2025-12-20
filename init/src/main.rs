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
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use nix::mount::{mount, MsFlags};
use nix::sys::reboot::{reboot, RebootMode};
use nix::sys::signal::{self, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{self, Pid};

mod config;
mod service;

use config::{InitConfig, ServiceConfig};
use service::{Service, ServiceState};

/// Global flag for shutdown request
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static REBOOT_REQUESTED: AtomicBool = AtomicBool::new(false);

fn main() {
    // Check if we're actually PID 1
    let pid = std::process::id();
    if pid != 1 {
        eprintln!("raven-init: Warning: Not running as PID 1 (pid={})", pid);
        eprintln!("raven-init: This is intended to run as the init process");
        // Continue anyway for testing purposes
    }

    // Initialize logging
    init_logging();

    log::info!("RavenInit starting...");

    // Run the init sequence
    if let Err(e) = run_init() {
        log::error!("Init failed: {}", e);
        // Try to drop to emergency shell
        emergency_shell();
    }
}

fn init_logging() {
    // Simple stderr logging for early boot
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| writeln!(buf, "[raven-init] {}: {}", record.level(), record.args()))
        .init();
}

fn run_init() -> Result<()> {
    // Phase 1: Early boot - mount essential filesystems
    log::info!("Phase 1: Mounting essential filesystems");
    mount_essential_filesystems()?;

    // Phase 2: Setup basic environment
    log::info!("Phase 2: Setting up environment");
    setup_environment()?;

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

    // Display welcome message
    print_welcome();

    // Phase 6: Main loop - reap zombies and handle signals
    log::info!("Phase 6: Entering main loop");
    main_loop(&mut services, &config)?;

    // Phase 7: Shutdown
    log::info!("Phase 7: Shutting down");
    shutdown_services(&mut services)?;

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

    ensure_service(
        &mut config.services,
        ServiceConfig {
            name: "seatd".to_string(),
            description: "Seat management daemon".to_string(),
            exec: "/bin/seatd".to_string(),
            args: vec!["-g".to_string(), "video".to_string()],
            restart: true,
            enabled: true,
            critical: false,
            environment: HashMap::new(),
            tty: None,
        },
    );

    let mut compositor_env = HashMap::new();
    compositor_env.insert("XDG_RUNTIME_DIR".to_string(), "/run/user/0".to_string());
    compositor_env.insert("LIBSEAT_BACKEND".to_string(), "seatd".to_string());

    let session_path = Path::new("/bin/raven-wayland-session");
    if session_path.exists() {
        let mut env = compositor_env;
        env.insert(
            "RAVEN_WAYLAND_COMPOSITOR".to_string(),
            wayland_choice.unwrap_or("raven").to_string(),
        );

        ensure_service(
            &mut config.services,
            ServiceConfig {
                name: "wayland-session".to_string(),
                description: "Raven Wayland session".to_string(),
                exec: "/bin/raven-wayland-session".to_string(),
                args: Vec::new(),
                restart: true,
                enabled: true,
                critical: false,
                environment: env,
                tty: None,
            },
        );
    } else {
        // Always use raven-compositor
        ensure_service(
            &mut config.services,
            ServiceConfig {
                name: "raven-compositor".to_string(),
                description: "Raven Wayland compositor".to_string(),
                exec: "/bin/raven-compositor".to_string(),
                args: Vec::new(),
                restart: true,
                enabled: true,
                critical: false,
                environment: compositor_env,
                tty: None,
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
                if let Ok(config) = toml::from_str(&content) {
                    log::info!("Loaded configuration from {}", path);
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

    // Start configured services
    for svc_config in &config.services {
        if svc_config.enabled {
            match Service::start(svc_config) {
                Ok(svc) => {
                    log::info!("Started service: {}", svc_config.name);
                    services.insert(svc_config.name.clone(), svc);
                }
                Err(e) => {
                    log::error!("Failed to start {}: {}", svc_config.name, e);
                    if svc_config.critical {
                        return Err(e)
                            .context(format!("Critical service {} failed", svc_config.name));
                    }
                }
            }
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

fn main_loop(services: &mut HashMap<String, Service>, config: &InitConfig) -> Result<()> {
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

        // Check for signals via /run/raven-init.cmd
        check_command_file()?;
    }

    Ok(())
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

fn check_services(services: &mut HashMap<String, Service>, config: &InitConfig) {
    for svc in services.values_mut() {
        if svc.state() == ServiceState::Exited && svc.should_restart() {
            log::info!("Restarting service: {}", svc.name());
            if let Err(e) = svc.restart() {
                log::error!("Failed to restart {}: {}", svc.name(), e);
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
