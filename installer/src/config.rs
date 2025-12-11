//! Installation configuration

use serde::{Deserialize, Serialize};

/// Installation profile
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InstallProfile {
    Minimal,
    #[default]
    Standard,
    Full,
}

/// Partition scheme
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PartitionScheme {
    #[default]
    Auto,
    Manual,
}

/// Filesystem type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Filesystem {
    #[default]
    Ext4,
    Btrfs,
    Xfs,
}

/// Disk configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskConfig {
    pub device: String,
    pub scheme: PartitionScheme,
    pub filesystem: Filesystem,
    pub encrypt: bool,
    pub encryption_password: Option<String>,
    pub partitions: Vec<Partition>,
}

/// Partition definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub mount_point: String,
    pub size_mb: u64,
    pub filesystem: Filesystem,
}

/// User configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserConfig {
    pub username: String,
    pub full_name: String,
    pub password: String,
    pub is_admin: bool,
    pub autologin: bool,
}

/// System configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub hostname: String,
    pub timezone: String,
    pub locale: String,
    pub keyboard_layout: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            hostname: "raven".to_string(),
            timezone: "UTC".to_string(),
            locale: "en_US.UTF-8".to_string(),
            keyboard_layout: "us".to_string(),
        }
    }
}

/// Complete installation configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstallConfig {
    pub profile: InstallProfile,
    pub disk: DiskConfig,
    pub user: UserConfig,
    pub system: SystemConfig,
}

impl InstallConfig {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.disk.device.is_empty() {
            errors.push("No disk selected".to_string());
        }

        if self.user.username.is_empty() {
            errors.push("Username is required".to_string());
        }

        if self.user.password.is_empty() {
            errors.push("Password is required".to_string());
        }

        if self.disk.encrypt && self.disk.encryption_password.is_none() {
            errors.push("Encryption password required when encryption is enabled".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
