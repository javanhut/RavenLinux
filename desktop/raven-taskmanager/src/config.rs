use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Panel orientation/position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl Orientation {
    pub fn is_vertical(&self) -> bool {
        matches!(self, Orientation::Left | Orientation::Right)
    }
}

/// A dock item representing an application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockItem {
    pub id: CompactString,
    pub name: CompactString,
    pub command: CompactString,
    pub icon: CompactString,
    #[serde(default)]
    pub pinned: bool,

    // Runtime state (not serialized)
    #[serde(skip)]
    pub running: bool,
    #[serde(skip)]
    pub minimized: bool,
    #[serde(skip)]
    pub focused: bool,
    #[serde(skip)]
    pub pid: Option<u32>,
    #[serde(skip)]
    pub address: Option<CompactString>,
    #[serde(skip)]
    pub workspace_id: Option<i32>,
}

impl DockItem {
    pub fn new_running(
        address: CompactString,
        class: CompactString,
        title: CompactString,
        pid: u32,
    ) -> Self {
        let (name, icon) = Self::get_app_info(&class, &title);

        Self {
            id: format!("hypr-{}", address).into(),
            name,
            command: class,
            icon,
            pinned: false,
            running: true,
            minimized: false,
            focused: false,
            pid: Some(pid),
            address: Some(address),
            workspace_id: None,
        }
    }

    /// Get display name and icon for a window class
    fn get_app_info(class: &str, title: &str) -> (CompactString, CompactString) {
        match class {
            // Raven apps
            "raven-terminal" => ("Terminal".into(), "utilities-terminal".into()),
            "raven-wifi" => ("WiFi".into(), "network-wireless".into()),
            "raven-menu" => ("Menu".into(), "application-menu".into()),
            "raven-settings" => ("Settings".into(), "preferences-system".into()),
            "raven-files" => ("Files".into(), "system-file-manager".into()),
            "raven-editor" => ("Editor".into(), "text-editor".into()),
            "raven-launcher" => ("Launcher".into(), "system-search".into()),
            "raven-installer" => ("Installer".into(), "system-software-install".into()),

            // Common terminals
            "kitty" | "Alacritty" | "foot" => ("Terminal".into(), "utilities-terminal".into()),

            // Browsers
            "firefox" | "Firefox" => ("Firefox".into(), "firefox".into()),
            "chromium" | "Chromium" => ("Chromium".into(), "chromium".into()),

            // File managers
            "org.gnome.Nautilus" | "nautilus" => ("Files".into(), "system-file-manager".into()),
            "thunar" | "Thunar" => ("Files".into(), "system-file-manager".into()),

            // Editors
            "code" | "Code" | "code-oss" => ("VS Code".into(), "visual-studio-code".into()),

            // Default: use title or class
            _ => {
                let name = if title.is_empty() {
                    class.into()
                } else if title.len() > 20 {
                    format!("{}...", &title[..17]).into()
                } else {
                    title.into()
                };
                (name, "application-x-executable".into())
            }
        }
    }

    /// Check if this window class should be shown in dock
    pub fn should_track(class: &str) -> bool {
        !matches!(
            class,
            "" | "raven-shell" | "raven-desktop" | "raven-panel"
        )
    }
}

/// Dock configuration (dock.json) - pinned apps
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PanelConfig {
    #[serde(default)]
    pub pinned_apps: Vec<DockItem>,
}

impl PanelConfig {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
        let dir = path.parent().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
        std::fs::create_dir_all(dir)?;
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

/// Raven settings (settings.json) - shared with other raven components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RavenSettings {
    #[serde(default)]
    pub panel_position: Orientation,
    #[serde(default = "default_panel_height")]
    pub panel_height: i32,

    // Other settings (preserved but not used directly by panel)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel_opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_animations: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallpaper_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallpaper_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_desktop_icons: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_clock: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_workspaces: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_size: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_follows_mouse: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titlebar_buttons: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard_layout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse_speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub touchpad_natural_scroll: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub touchpad_tap_to_click: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_timeout: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspend_timeout: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lid_close_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_volume: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute_on_lock: Option<bool>,
}

fn default_panel_height() -> i32 {
    38
}

impl Default for RavenSettings {
    fn default() -> Self {
        Self {
            panel_position: Orientation::Top,
            panel_height: 38,
            theme: None,
            accent_color: None,
            font_size: None,
            icon_theme: None,
            cursor_theme: None,
            panel_opacity: None,
            enable_animations: None,
            wallpaper_path: None,
            wallpaper_mode: None,
            show_desktop_icons: None,
            show_clock: None,
            clock_format: None,
            show_workspaces: None,
            border_width: None,
            gap_size: None,
            focus_follows_mouse: None,
            titlebar_buttons: None,
            keyboard_layout: None,
            mouse_speed: None,
            touchpad_natural_scroll: None,
            touchpad_tap_to_click: None,
            screen_timeout: None,
            suspend_timeout: None,
            lid_close_action: None,
            master_volume: None,
            mute_on_lock: None,
        }
    }
}

impl RavenSettings {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
        let dir = path.parent().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
        std::fs::create_dir_all(dir)?;
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

/// Configuration paths
pub struct ConfigPaths {
    pub dock_config: PathBuf,
    pub raven_settings: PathBuf,
    pub icon_cache_dir: PathBuf,
}

impl ConfigPaths {
    pub fn new() -> Self {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        Self {
            dock_config: config_dir.join("raven-shell/dock.json"),
            raven_settings: home_dir.join(".config/raven/settings.json"),
            icon_cache_dir: config_dir.join("raven-shell/icons"),
        }
    }
}

impl Default for ConfigPaths {
    fn default() -> Self {
        Self::new()
    }
}
