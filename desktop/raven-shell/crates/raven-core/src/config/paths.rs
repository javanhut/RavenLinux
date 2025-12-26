use std::path::PathBuf;

/// Configuration paths for all Raven components
pub struct ConfigPaths {
    pub dock_config: PathBuf,
    pub raven_settings: PathBuf,
    pub icon_cache_dir: PathBuf,
    pub desktop_icons: PathBuf,
    pub keybindings: PathBuf,
}

impl ConfigPaths {
    pub fn new() -> Self {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        Self {
            dock_config: config_dir.join("raven-shell/dock.json"),
            raven_settings: home_dir.join(".config/raven/settings.json"),
            icon_cache_dir: config_dir.join("raven-shell/icons"),
            desktop_icons: config_dir.join("raven/pinned-apps.json"),
            keybindings: home_dir.join(".config/hypr/hyprland.conf"),
        }
    }

    /// Get the raven-shell config directory
    pub fn shell_config_dir(&self) -> PathBuf {
        self.dock_config.parent().unwrap_or(&PathBuf::from(".")).to_path_buf()
    }

    /// Get the raven config directory
    pub fn raven_config_dir(&self) -> PathBuf {
        self.raven_settings.parent().unwrap_or(&PathBuf::from(".")).to_path_buf()
    }
}

impl Default for ConfigPaths {
    fn default() -> Self {
        Self::new()
    }
}
