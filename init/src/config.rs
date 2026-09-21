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

impl InitConfig {
    /// No services and no system settings: the starting point for a session
    /// supervisor, which must not inherit the getty the system default has.
    pub fn default_empty() -> Self {
        Self {
            system: SystemConfig {
                hostname: String::new(),
                ..SystemConfig::default()
            },
            services: Vec::new(),
            mounts: Vec::new(),
            source_path: None,
        }
    }
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
                    tty: Some("/dev/tty1".to_string()),
                    ..ServiceConfig::default()
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

    /// Scheduling priority, -20 (most favoured) to 19 (least), as `nice(1)`
    /// spells it.
    ///
    /// Zero, the default, is what a child of init inherits and what almost
    /// every service should keep: a daemon that is idle most of the time costs
    /// nothing at any priority, and one that is busy is usually busy because
    /// somebody is waiting for it. The field exists for the handful that are
    /// neither -- a package indexer, a thumbnailer, anything that will happily
    /// eat every core it is given for work nobody is watching -- where a
    /// positive value keeps the desktop responsive while it runs.
    ///
    /// Negative values need CAP_SYS_NICE, which init has and the service's
    /// account may not; they are applied before any `user =` drop for exactly
    /// that reason. Use them sparingly: a daemon at -5 competes with the
    /// compositor, and a compositor that misses its frame deadline is more
    /// visible than anything a negative nice value was meant to fix.
    #[serde(default)]
    pub nice: i8,

    /// The kernel's out-of-memory killer score adjustment, -1000 to 1000.
    ///
    /// When memory runs out the kernel picks a victim by score, and the score
    /// is dominated by how much memory the process is using -- which means the
    /// thing that dies is usually the largest innocent bystander. This is the
    /// thumb on that scale: a positive value volunteers a service as the first
    /// to be killed, a negative one asks the kernel to look elsewhere.
    ///
    /// Zero, the default, leaves a service exactly where the kernel's own
    /// accounting puts it, and that is the right answer for nearly everything.
    /// The cases worth setting it for are the two extremes: a cache or an
    /// indexer that can be killed and restarted without anyone noticing
    /// (positive), and a daemon whose death takes the session with it
    /// (negative). Note that a service with `memory_max` is usually better
    /// served by that, because it is killed for its own consumption instead of
    /// being chosen when something else exhausts the machine.
    ///
    /// Lowering the score below zero needs privilege, so like `nice` it is
    /// applied before the process drops to its account.
    #[serde(default)]
    pub oom_score_adj: i32,

    /// Hard memory limit for the service and everything it forks, written as
    /// a size: "512M", "2G", or "max" for no limit.
    ///
    /// This is a cgroup limit, not an rlimit, and the difference is the whole
    /// point: RLIMIT_AS makes one process's `mmap` fail, which most daemons
    /// respond to by crashing in whatever way their least-tested error path
    /// crashes. `memory.max` counts the service and its children together and
    /// makes the kernel reclaim first and OOM-kill second, so a daemon that
    /// slowly leaks is killed and restarted by the supervisor instead of
    /// taking the machine's last free page with it.
    ///
    /// `None` -- the default -- means no limit, which is correct for a service
    /// whose working set nobody has measured. A limit guessed too low is worse
    /// than none: it turns a daemon that works into one that is killed under
    /// exactly the load it was installed to handle, and the evidence is a
    /// SIGKILL with no message.
    #[serde(default)]
    pub memory_max: Option<String>,

    /// Relative share of CPU time when the machine is oversubscribed, 1 to
    /// 10000. The kernel's default is 100.
    ///
    /// A weight is not a cap. A service at 50 is not limited to half a core --
    /// it gets every idle cycle it asks for, exactly as it would with no
    /// setting at all, and the number only decides who yields when two
    /// services want the same core at the same moment. That is almost always
    /// the behaviour wanted from a supervisor: capping a daemon that is not
    /// competing with anything wastes the machine.
    ///
    /// `None` leaves the service at the kernel's 100, which is what every
    /// service that has not been deliberately ranked should have.
    #[serde(default)]
    pub cpu_weight: Option<u32>,

    /// Relative share of disk bandwidth, 1 to 10000, under the same rules as
    /// `cpu_weight`.
    ///
    /// Only effective where the kernel has a weight-capable I/O policy for the
    /// device (BFQ, or iocost with a cost model); on a machine using
    /// mq-deadline the `io.weight` file does not exist and the setting is
    /// reported once and ignored. It is kept as a field anyway because the
    /// failure is a warning in a log rather than a service that does not
    /// start, and because the case it is for -- an indexer starving the rest
    /// of the machine of disk -- is real on the hardware that has BFQ.
    #[serde(default)]
    pub io_weight: Option<u32>,

    /// Per-process resource limits, applied with `setrlimit(2)` between fork
    /// and exec. See [`ResourceLimits`].
    #[serde(default)]
    pub limits: ResourceLimits,
}

/// The `setrlimit(2)` limits a service definition can ask for.
///
/// Written as its own table rather than as flat `limit_nofile`-style keys so
/// that a definition reads as `[services.limits]` with the relevant lines
/// under it, and so that adding a limit later does not add another prefix to
/// the top level of every service block.
///
/// Deliberately short. These four are the ones that have actually mattered on
/// this system; a daemon that needs RLIMIT_STACK or RLIMIT_RTPRIO set from
/// outside is rare enough that the field can be added when it appears, and a
/// full table of every rlimit the kernel has would be sixteen fields, fifteen
/// of which nobody would ever set.
///
/// Each limit sets the soft and the hard value together -- see
/// `crate::cgroup::rlimits_for` for why.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum open file descriptors (RLIMIT_NOFILE), as a count.
    ///
    /// The one limit worth knowing about. The kernel's default soft limit is
    /// 1024, which is fine for a daemon holding a few sockets and completely
    /// wrong for one holding a descriptor per client; the symptom is `accept`
    /// returning EMFILE and a daemon that stops answering without dying, which
    /// looks like a hang rather than a limit. `None` keeps whatever init
    /// inherited from the kernel.
    #[serde(default)]
    pub nofile: Option<u64>,

    /// Maximum processes and threads for this service's *user* (RLIMIT_NPROC),
    /// as a count.
    ///
    /// Note the "for this user" part, which is the trap in this limit and the
    /// reason it is rarely the right tool: RLIMIT_NPROC is counted per uid
    /// across the whole system, not per service, so setting it on a service
    /// that runs as root counts every root process on the machine. It is
    /// useful on a service with an account of its own -- and misleading
    /// anywhere else. `pids.current` in the service's cgroup is the number
    /// that actually describes the service.
    #[serde(default)]
    pub nproc: Option<u64>,

    /// Maximum locked (unswappable) memory, as a size such as "64M".
    ///
    /// For daemons that lock secrets into memory so they cannot be written to
    /// swap. The default is small -- 8MB on most kernels -- and a daemon that
    /// needs more fails its `mlock` and usually carries on with the secret in
    /// swappable memory, which is the failure nobody notices.
    #[serde(default)]
    pub memlock: Option<String>,

    /// Maximum core dump size, as a size; "0" disables core dumps entirely.
    ///
    /// Worth setting to 0 on any service that handles credentials, because a
    /// core dump is a copy of everything it had in memory written to disk by
    /// the kernel, with no say from the program. Worth setting high on a
    /// daemon that is being debugged, which is the other half of why this is a
    /// per-service setting rather than a global one.
    #[serde(default)]
    pub core: Option<String>,
}

/// Every field at the value `#[serde(default)]` would give it.
///
/// This exists so that the places that build a `ServiceConfig` in code --
/// init's fallback getty, the services `overrides.rs` synthesizes from the
/// kernel command line, and the fixtures in the tests -- can name the three or
/// four fields they care about and inherit the rest. Before it, every one of
/// those sites listed all eighteen fields by hand, so adding a field to this
/// struct was a compile error in eight files at once and an invitation to fix
/// it by copying a neighbouring value rather than by thinking about what the
/// new field should be.
///
/// It must agree with serde, field for field, or a service built in code and
/// the same service written out in TOML would behave differently -- and the
/// one that differs would be the one nobody can read. `enabled` is the case
/// that matters: it defaults to *true* here, as `default_true` makes it in a
/// file, because a service block somebody wrote is a service they want.
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            exec: String::new(),
            args: Vec::new(),
            restart: false,
            enabled: true,
            critical: false,
            environment: HashMap::new(),
            pre_exec: Vec::new(),
            tty: None,
            user: None,
            runtime_dirs: Vec::new(),
            after: Vec::new(),
            ready_path: None,
            ready_timeout: default_ready_timeout(),
            stop_exec: None,
            stop_args: Vec::new(),
            stop_timeout: default_stop_timeout(),
            nice: 0,
            oom_score_adj: 0,
            memory_max: None,
            cpu_weight: None,
            io_weight: None,
            limits: ResourceLimits::default(),
        }
    }
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
            log::info!(
                "Service '{}' defined by drop-in {}",
                svc.name,
                path.display()
            );
            config.services.push(svc);
        }
    }
}
