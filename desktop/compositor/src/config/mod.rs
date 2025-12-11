//! Configuration management for raven-compositor

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub appearance: AppearanceConfig,
    pub input: InputConfig,
    pub keybindings: HashMap<String, String>,
    pub workspaces: WorkspaceConfig,
    pub window_rules: Vec<WindowRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub focus_follows_mouse: bool,
    pub cursor_theme: String,
    pub cursor_size: u32,
    pub xwayland: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    pub gaps_inner: u32,
    pub gaps_outer: u32,
    pub border_width: u32,
    pub border_radius: u32,
    pub border_color_active: String,
    pub border_color_inactive: String,
    pub shadow_enabled: bool,
    pub shadow_size: u32,
    pub animation_duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    pub keyboard: KeyboardConfig,
    pub mouse: MouseConfig,
    pub touchpad: TouchpadConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardConfig {
    pub layout: String,
    pub variant: Option<String>,
    pub repeat_rate: u32,
    pub repeat_delay: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseConfig {
    pub natural_scroll: bool,
    pub acceleration: f64,
    pub scroll_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchpadConfig {
    pub natural_scroll: bool,
    pub tap_to_click: bool,
    pub two_finger_scroll: bool,
    pub disable_while_typing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub count: usize,
    pub names: Vec<String>,
    pub default_layout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRule {
    pub matches: WindowMatch,
    pub actions: Vec<WindowAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowMatch {
    pub app_id: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WindowAction {
    Float,
    Tile,
    Workspace { index: usize },
    Opacity { value: f32 },
    Size { width: u32, height: u32 },
}

impl Default for Config {
    fn default() -> Self {
        let mut keybindings = HashMap::new();

        // Default keybindings
        keybindings.insert("Super+Return".into(), "spawn:raven-term".into());
        keybindings.insert("Super+Space".into(), "spawn:raven-launcher".into());
        keybindings.insert("Super+Q".into(), "close".into());
        keybindings.insert("Super+F".into(), "fullscreen".into());
        keybindings.insert("Super+T".into(), "toggle-tiling".into());
        keybindings.insert("Super+Tab".into(), "window-switcher".into());

        // Workspace bindings
        for i in 1..=9 {
            keybindings.insert(format!("Super+{}", i), format!("workspace:{}", i));
            keybindings.insert(format!("Super+Shift+{}", i), format!("move-to-workspace:{}", i));
        }

        // Focus movement (vim-style)
        keybindings.insert("Super+H".into(), "focus:left".into());
        keybindings.insert("Super+J".into(), "focus:down".into());
        keybindings.insert("Super+K".into(), "focus:up".into());
        keybindings.insert("Super+L".into(), "focus:right".into());

        // Window movement
        keybindings.insert("Super+Shift+H".into(), "move:left".into());
        keybindings.insert("Super+Shift+J".into(), "move:down".into());
        keybindings.insert("Super+Shift+K".into(), "move:up".into());
        keybindings.insert("Super+Shift+L".into(), "move:right".into());

        // Resize
        keybindings.insert("Super+Ctrl+H".into(), "resize:shrink-width".into());
        keybindings.insert("Super+Ctrl+J".into(), "resize:grow-height".into());
        keybindings.insert("Super+Ctrl+K".into(), "resize:shrink-height".into());
        keybindings.insert("Super+Ctrl+L".into(), "resize:grow-width".into());

        Self {
            general: GeneralConfig {
                focus_follows_mouse: false,
                cursor_theme: "raven-cursors".into(),
                cursor_size: 24,
                xwayland: true,
            },
            appearance: AppearanceConfig {
                gaps_inner: 8,
                gaps_outer: 8,
                border_width: 2,
                border_radius: 8,
                border_color_active: "#58a6ff".into(),
                border_color_inactive: "#30363d".into(),
                shadow_enabled: true,
                shadow_size: 12,
                animation_duration_ms: 200,
            },
            input: InputConfig {
                keyboard: KeyboardConfig {
                    layout: "us".into(),
                    variant: None,
                    repeat_rate: 25,
                    repeat_delay: 600,
                },
                mouse: MouseConfig {
                    natural_scroll: false,
                    acceleration: 0.0,
                    scroll_factor: 1.0,
                },
                touchpad: TouchpadConfig {
                    natural_scroll: true,
                    tap_to_click: true,
                    two_finger_scroll: true,
                    disable_while_typing: true,
                },
            },
            keybindings,
            workspaces: WorkspaceConfig {
                count: 9,
                names: (1..=9).map(|i| i.to_string()).collect(),
                default_layout: "tiling".into(),
            },
            window_rules: vec![
                WindowRule {
                    matches: WindowMatch {
                        app_id: Some("raven-launcher".into()),
                        title: None,
                    },
                    actions: vec![WindowAction::Float],
                },
            ],
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        xdg::BaseDirectories::with_prefix("raven")
            .map(|dirs| dirs.get_config_home().join("compositor.toml"))
            .unwrap_or_else(|_| PathBuf::from("~/.config/raven/compositor.toml"))
    }
}
