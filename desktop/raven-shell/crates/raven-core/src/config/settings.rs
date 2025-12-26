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

/// Raven settings (settings.json) - shared with all raven components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RavenSettings {
    // Panel settings
    #[serde(default)]
    pub panel_position: Orientation,
    #[serde(default = "default_panel_height")]
    pub panel_height: i32,

    // Appearance
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

    // Desktop
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallpaper_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallpaper_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_desktop_icons: Option<bool>,

    // Panel features
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_clock: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_workspaces: Option<bool>,

    // Window management
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_size: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_follows_mouse: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titlebar_buttons: Option<String>,

    // Input
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard_layout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse_speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub touchpad_natural_scroll: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub touchpad_tap_to_click: Option<bool>,

    // Power
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_timeout: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspend_timeout: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lid_close_action: Option<String>,

    // Sound
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
    /// Get the default settings path
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/raven/settings.json")
    }

    pub fn load(path: &PathBuf) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default()
    }

    pub fn save_to(&self, path: &PathBuf) -> anyhow::Result<()> {
        let dir = path.parent().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
        std::fs::create_dir_all(dir)?;
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Save to the default settings path
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&Self::default_path())
    }

    /// Get show_clock with default
    pub fn show_clock(&self) -> bool {
        self.show_clock.unwrap_or(true)
    }

    /// Get clock_format with default
    pub fn clock_format(&self) -> &str {
        self.clock_format.as_deref().unwrap_or("%H:%M")
    }

    /// Get enable_animations with default
    pub fn enable_animations(&self) -> bool {
        self.enable_animations.unwrap_or(true)
    }

    /// Get show_desktop_icons with default
    pub fn show_desktop_icons(&self) -> bool {
        self.show_desktop_icons.unwrap_or(true)
    }

    // Accessors with defaults for settings pages
    pub fn theme(&self) -> &str {
        self.theme.as_deref().unwrap_or("dark")
    }

    pub fn accent_color(&self) -> &str {
        self.accent_color.as_deref().unwrap_or("#009688")
    }

    pub fn font_size(&self) -> i32 {
        self.font_size.unwrap_or(14)
    }

    pub fn panel_opacity(&self) -> f64 {
        self.panel_opacity.unwrap_or(75.0)
    }

    pub fn wallpaper_path(&self) -> &str {
        self.wallpaper_path.as_deref().unwrap_or("")
    }

    pub fn wallpaper_mode(&self) -> &str {
        self.wallpaper_mode.as_deref().unwrap_or("fill")
    }

    pub fn panel_position_str(&self) -> &str {
        match self.panel_position {
            Orientation::Top => "top",
            Orientation::Bottom => "bottom",
            Orientation::Left => "left",
            Orientation::Right => "right",
        }
    }

    pub fn show_workspaces(&self) -> bool {
        self.show_workspaces.unwrap_or(true)
    }

    pub fn border_width(&self) -> i32 {
        self.border_width.unwrap_or(2)
    }

    pub fn gap_size(&self) -> i32 {
        self.gap_size.unwrap_or(8)
    }

    pub fn focus_follows_mouse(&self) -> bool {
        self.focus_follows_mouse.unwrap_or(false)
    }

    pub fn keyboard_layout(&self) -> &str {
        self.keyboard_layout.as_deref().unwrap_or("us")
    }

    pub fn mouse_speed(&self) -> f64 {
        self.mouse_speed.unwrap_or(50.0)
    }

    pub fn touchpad_natural_scroll(&self) -> bool {
        self.touchpad_natural_scroll.unwrap_or(false)
    }

    pub fn touchpad_tap_to_click(&self) -> bool {
        self.touchpad_tap_to_click.unwrap_or(true)
    }

    pub fn screen_timeout(&self) -> i32 {
        self.screen_timeout.unwrap_or(300)
    }

    pub fn suspend_timeout(&self) -> i32 {
        self.suspend_timeout.unwrap_or(900)
    }

    pub fn lid_close_action(&self) -> &str {
        self.lid_close_action.as_deref().unwrap_or("suspend")
    }

    pub fn master_volume(&self) -> i32 {
        self.master_volume.unwrap_or(75)
    }

    pub fn mute_on_lock(&self) -> bool {
        self.mute_on_lock.unwrap_or(false)
    }

    // Setters that set Option values
    pub fn set_theme(&mut self, val: &str) {
        self.theme = Some(val.to_string());
    }

    pub fn set_accent_color(&mut self, val: &str) {
        self.accent_color = Some(val.to_string());
    }

    pub fn set_font_size(&mut self, val: i32) {
        self.font_size = Some(val);
    }

    pub fn set_panel_opacity(&mut self, val: f64) {
        self.panel_opacity = Some(val);
    }

    pub fn set_enable_animations(&mut self, val: bool) {
        self.enable_animations = Some(val);
    }

    pub fn set_wallpaper_path(&mut self, val: &str) {
        self.wallpaper_path = Some(val.to_string());
    }

    pub fn set_wallpaper_mode(&mut self, val: &str) {
        self.wallpaper_mode = Some(val.to_string());
    }

    pub fn set_show_desktop_icons(&mut self, val: bool) {
        self.show_desktop_icons = Some(val);
    }

    pub fn set_panel_position(&mut self, val: &str) {
        self.panel_position = match val {
            "bottom" => Orientation::Bottom,
            "left" => Orientation::Left,
            "right" => Orientation::Right,
            _ => Orientation::Top,
        };
    }

    pub fn set_panel_height(&mut self, val: i32) {
        self.panel_height = val;
    }

    pub fn set_show_clock(&mut self, val: bool) {
        self.show_clock = Some(val);
    }

    pub fn set_clock_format(&mut self, val: &str) {
        self.clock_format = Some(val.to_string());
    }

    pub fn set_show_workspaces(&mut self, val: bool) {
        self.show_workspaces = Some(val);
    }

    pub fn set_border_width(&mut self, val: i32) {
        self.border_width = Some(val);
    }

    pub fn set_gap_size(&mut self, val: i32) {
        self.gap_size = Some(val);
    }

    pub fn set_focus_follows_mouse(&mut self, val: bool) {
        self.focus_follows_mouse = Some(val);
    }

    pub fn set_keyboard_layout(&mut self, val: &str) {
        self.keyboard_layout = Some(val.to_string());
    }

    pub fn set_mouse_speed(&mut self, val: f64) {
        self.mouse_speed = Some(val);
    }

    pub fn set_touchpad_natural_scroll(&mut self, val: bool) {
        self.touchpad_natural_scroll = Some(val);
    }

    pub fn set_touchpad_tap_to_click(&mut self, val: bool) {
        self.touchpad_tap_to_click = Some(val);
    }

    pub fn set_screen_timeout(&mut self, val: i32) {
        self.screen_timeout = Some(val);
    }

    pub fn set_suspend_timeout(&mut self, val: i32) {
        self.suspend_timeout = Some(val);
    }

    pub fn set_lid_close_action(&mut self, val: &str) {
        self.lid_close_action = Some(val.to_string());
    }

    pub fn set_master_volume(&mut self, val: i32) {
        self.master_volume = Some(val);
    }

    pub fn set_mute_on_lock(&mut self, val: bool) {
        self.mute_on_lock = Some(val);
    }
}
