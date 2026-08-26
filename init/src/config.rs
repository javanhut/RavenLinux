//! Configuration structures for RavenInit, and the loader that fills them.
//!
//! Loading lives here rather than in `main.rs` because it has two callers with
//! equal claim: init at boot, and `raven-rc reload` on a live system. Reload
//! must produce exactly what a boot would -- same search order, same drop-in
//! merge, same precedence -- and the only way to guarantee that is for both to
//! run this code. A second loader written for reload would be a second answer
//! to "what is configured", and the one that only runs on reload is the one
//! that rots unnoticed.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Main init configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitConfig {
    /// System configuration
    #[serde(default)]
    pub system: SystemConfig,

    /// Services to start
    #[serde(default)]
    pub services: Vec<ServiceConfig>,

    /// Mount points
    #[serde(default)]
    pub mounts: Vec<MountConfig>,

    /// The file this configuration was read from.
    ///
    /// Not part of the file format -- `enable`/`disable` need somewhere to
    /// write back to, and "whichever path load_config happened to find" is
    /// knowledge that was previously thrown away the moment parsing succeeded.
    /// `None` means the built-in defaults are in use and there is no file to
    /// edit.
    #[serde(skip)]
    pub source_path: Option<std::path::PathBuf>,
}

impl Default for InitConfig {
    fn default() -> Self {
        Self {
            system: SystemConfig::default(),
            services: vec![
                // Default getty service
                ServiceConfig {
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
            pre_exec: Vec::new(),
                    tty: Some("/dev/tty1".to_string()),
                    user: None,
                    stop_exec: None,
                    stop_args: Vec::new(),
                    stop_timeout: 5,
                    runtime_dirs: Vec::new(),
                    after: Vec::new(),
                    ready_path: None,
                    ready_timeout: 5,
                },
            ],
            mounts: Vec::new(),
            source_path: None,
        }
    }
}

/// System-wide configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Hostname
    #[serde(default = "default_hostname")]
    pub hostname: String,

    /// Default runlevel/target
    #[serde(default = "default_runlevel")]
    pub default_runlevel: String,

    /// Shutdown timeout in seconds
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout: u32,

    /// Enable kernel module loading
    #[serde(default = "default_true")]
    pub load_modules: bool,

    /// Enable udev/eudev
    #[serde(default = "default_true")]
    pub enable_udev: bool,

    /// Enable network
    #[serde(default = "default_true")]
    pub enable_network: bool,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            hostname: default_hostname(),
            default_runlevel: default_runlevel(),
            shutdown_timeout: default_shutdown_timeout(),
            load_modules: true,
            enable_udev: true,
            enable_network: true,
            log_level: default_log_level(),
        }
    }
}

fn default_hostname() -> String {
    "raven-linux".to_string()
}

fn default_runlevel() -> String {
    "default".to_string()
}

fn default_shutdown_timeout() -> u32 {
    10
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Service configuration
///
/// `PartialEq` is what `raven-rc reload` compares to tell a definition that
/// actually changed from one that was merely re-read: without it every reload
/// would report every service as updated, and the report is the whole point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name (identifier)
    pub name: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Executable path
    pub exec: String,

    /// Command line arguments
    #[serde(default)]
    pub args: Vec<String>,

    /// Whether to restart on exit
    #[serde(default)]
    pub restart: bool,

    /// Whether service is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether service is critical (failure = boot failure)
    #[serde(default)]
    pub critical: bool,

    /// Environment variables
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// A command run to completion before each start, for setup the daemon
    /// will not do itself -- sshd's `ssh-keygen -A` generating missing host
    /// keys is the motivating case. First element is the program, the rest
    /// its arguments. It must be idempotent: it runs on every start,
    /// including supervisor restarts.
    #[serde(default)]
    pub pre_exec: Vec<String>,

    /// TTY device for this service (e.g., "/dev/tty1")
    /// If set, the service will be spawned with proper session and job control
    #[serde(default)]
    pub tty: Option<String>,

    /// Account this service runs as. `None` keeps it as root.
    ///
    /// Init is PID 1 and therefore root, and every service it started
    /// inherited that -- which for a daemon that needs a raw socket or DRM
    /// master is right, and for a desktop session is not. The graphical
    /// session is the case this exists for: without it the compositor and
    /// every application launched from its dock ran as uid 0, which also made
    /// the `video`/`render`/`input` membership the installer sets up
    /// meaningless, because root bypasses all of it.
    ///
    /// The name is resolved against `/etc/passwd` at start time rather than
    /// being stored as a uid, so a definition stays correct if the account is
    /// recreated with a different number, and a drop-in may name a user that
    /// does not exist yet without being wrong until it does.
    ///
    /// A name that cannot be resolved fails the start. Falling back to root
    /// would hand a service more privilege than its definition asked for,
    /// which is the one outcome nobody writing `user =` wants.
    #[serde(default)]
    pub user: Option<String>,

    /// Directories to create before the service starts.
    ///
    /// /run is a tmpfs, so anything under it exists only if something creates
    /// it each boot. dbus is the motivating case: dbus-daemon binds
    /// /run/dbus/system_bus_socket but does not create /run/dbus, so on a
    /// system where nothing else made the directory it exited with "Failed to
    /// bind socket ... No such file or directory" and burned its whole restart
    /// budget on a missing mkdir.
    #[serde(default)]
    pub runtime_dirs: Vec<String>,

    /// Services which must be started before this service.
    #[serde(default)]
    pub after: Vec<String>,

    /// Optional filesystem object that proves this service is ready, such as
    /// a control socket. Dependants wait for it instead of racing spawn().
    #[serde(default)]
    pub ready_path: Option<String>,

    /// Maximum time to wait for `ready_path`, in seconds.
    #[serde(default = "default_ready_timeout")]
    pub ready_timeout: u32,

    /// Command run to stop this service cleanly, before any signal is sent.
    ///
    /// Some daemons cannot be stopped by SIGTERM alone. `cawd` is the case
    /// this exists for: it holds a wireless association, and the only way it
    /// can leave the air politely is a request on its own control socket
    /// (`caw shutdown`), because a signal handler would need `signalfd` and
    /// the crate forbids unsafe. systemd calls this `ExecStop=`.
    ///
    /// Failure is not fatal, and neither is a timeout: SIGTERM follows either
    /// way. See `stop_timeout`.
    #[serde(default)]
    pub stop_exec: Option<String>,

    /// Arguments for [`ServiceConfig::stop_exec`]
    #[serde(default)]
    pub stop_args: Vec<String>,

    /// How long to wait for `stop_exec` before moving on to SIGTERM, seconds.
    #[serde(default = "default_stop_timeout")]
    pub stop_timeout: u32,
}

/// Long enough for a deauthentication to reach the AP, short enough that a
/// wedged stop command does not hold up a reboot.
fn default_stop_timeout() -> u32 {
    5
}

fn default_ready_timeout() -> u32 {
    5
}

/// Mount point configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountConfig {
    /// Source device or filesystem
    pub source: String,

    /// Mount point path
    pub target: String,

    /// Filesystem type
    pub fstype: String,

    /// Mount options
    #[serde(default)]
    pub options: String,

    /// Mount at boot
    #[serde(default = "default_true")]
    pub mount_at_boot: bool,
}

pub fn load() -> Result<InitConfig> {
    let config_paths = ["/etc/raven/init.toml", "/etc/init.toml"];

    for path in &config_paths {
        if Path::new(path).exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(mut config) = toml::from_str::<InitConfig>(&content) {
                    log::info!("Loaded configuration from {}", path);
                    // Remembered so enable/disable know what to rewrite.
                    config.source_path = Some(std::path::PathBuf::from(path));
                    load_dropins(&mut config);
                    return Ok(config);
                }
            }
        }
    }

    log::info!("Using default configuration");
    let mut config = InitConfig::default();
    load_dropins(&mut config);
    Ok(config)
}

/// Folds /etc/raven/init.d/*.toml into the service list.
///
/// The base image ships only what Raven itself provides; daemons arrive later
/// through `rvn install`, and a freshly installed daemon needs a service
/// definition without anyone hand-editing init.toml. Each drop-in is a file of
/// `[[services]]` blocks in exactly init.toml's schema, so a definition can be
/// moved between the two verbatim.
///
/// init.toml wins a name collision: the operator's main config outranks a file
/// a package (or a copy-paste) dropped in. Files are read in sorted order so
/// the outcome does not depend on directory enumeration.
fn load_dropins(config: &mut InitConfig) {
    let dir = std::env::var_os("RAVEN_INIT_DROPIN_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/etc/raven/init.d"));

    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };

    let mut paths: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();

    for path in paths {
        let Ok(content) = fs::read_to_string(&path) else {
            log::warn!("Cannot read drop-in {}", path.display());
            continue;
        };
        // Parsed as a full InitConfig so the schema is identical, but only the
        // services are taken -- a drop-in must not be able to change the
        // hostname or shutdown timeout as a side effect.
        let parsed: InitConfig = match toml::from_str(&content) {
            Ok(parsed) => parsed,
            Err(e) => {
                // A daemon's definition being broken must not take the boot
                // with it; the service just does not exist until it is fixed.
                log::warn!("Ignoring drop-in {}: {}", path.display(), e);
                continue;
            }
        };
        for svc in parsed.services {
            if config.services.iter().any(|s| s.name == svc.name) {
                log::warn!(
                    "Drop-in {} redefines '{}'; keeping the init.toml definition",
                    path.display(),
                    svc.name
                );
                continue;
            }
            log::info!("Service '{}' defined by drop-in {}", svc.name, path.display());
            config.services.push(svc);
        }
    }
}
