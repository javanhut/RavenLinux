//! Configuration structures for RavenInit

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
                    exec: "/sbin/agetty".to_string(),
                    args: vec![
                        "--noclear".to_string(),
                        "--autologin".to_string(),
                        "root".to_string(),
                        "tty1".to_string(),
                        "linux".to_string(),
                    ],
                    restart: true,
                    enabled: true,
                    critical: false,
                    environment: HashMap::new(),
                },
            ],
            mounts: Vec::new(),
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
