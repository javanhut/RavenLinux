use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
