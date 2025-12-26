use compact_str::CompactString;
use std::path::PathBuf;

use crate::config::Orientation;

/// Component identifiers for visibility control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentId {
    Panel,
    Desktop,
    Menu,
    Power,
    Settings,
    Keybindings,
    FileManager,
    WiFi,
    Usb,
    Installer,
}

impl ComponentId {
    /// Parse from string (for IPC commands)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "panel" => Some(Self::Panel),
            "desktop" => Some(Self::Desktop),
            "menu" => Some(Self::Menu),
            "power" => Some(Self::Power),
            "settings" => Some(Self::Settings),
            "keybindings" => Some(Self::Keybindings),
            "filemanager" | "file-manager" | "files" => Some(Self::FileManager),
            "wifi" => Some(Self::WiFi),
            "usb" => Some(Self::Usb),
            "installer" => Some(Self::Installer),
            _ => None,
        }
    }

    /// Get component name for display
    pub fn name(&self) -> &'static str {
        match self {
            Self::Panel => "Panel",
            Self::Desktop => "Desktop",
            Self::Menu => "Menu",
            Self::Power => "Power",
            Self::Settings => "Settings",
            Self::Keybindings => "Keybindings",
            Self::FileManager => "File Manager",
            Self::WiFi => "WiFi",
            Self::Usb => "USB",
            Self::Installer => "Installer",
        }
    }

    /// Check if component is always visible (no toggle)
    pub fn is_always_visible(&self) -> bool {
        matches!(self, Self::Panel | Self::Desktop)
    }

    /// Check if component is a tool (requires separate privilege handling)
    pub fn is_tool(&self) -> bool {
        matches!(self, Self::WiFi | Self::Usb | Self::Installer)
    }
}

/// Commands FROM GTK TO async services
#[derive(Debug, Clone)]
pub enum ShellCommand {
    // =========== Hyprland Window Commands ===========

    /// Focus a window by address
    FocusWindow(CompactString),

    /// Close a window by address
    CloseWindow(CompactString),

    /// Minimize window to special workspace
    MinimizeWindow(CompactString),

    /// Restore window from special workspace
    RestoreWindow(CompactString),

    // =========== Process Commands ===========

    /// Launch an application
    LaunchApp(CompactString),

    // =========== Power Commands ===========

    /// Logout (exit Hyprland)
    Logout,

    /// Lock the screen
    Lock,

    /// Reboot the system
    Reboot,

    /// Shutdown the system
    Shutdown,

    /// Suspend the system
    Suspend,

    /// Hibernate the system
    Hibernate,

    // =========== Panel/Dock Commands ===========

    /// Change panel position
    SetPanelPosition(Orientation),

    /// Pin or unpin an app
    PinApp {
        id: CompactString,
        pinned: bool,
    },

    /// Save current dock config
    SaveDockConfig,

    // =========== Component Commands ===========

    /// Show a component
    ShowComponent(ComponentId),

    /// Hide a component
    HideComponent(ComponentId),

    /// Toggle a component's visibility
    ToggleComponent(ComponentId),

    // =========== Settings Commands ===========

    /// Set wallpaper
    SetWallpaper(PathBuf),

    /// Save all configuration
    SaveConfig,

    /// Reload configuration
    ReloadConfig,

    // =========== Network Commands (for WiFi tool) ===========

    /// Scan for networks
    ScanNetworks,

    /// Connect to a network
    ConnectNetwork {
        ssid: String,
        password: Option<String>,
    },

    /// Disconnect from current network
    DisconnectNetwork,

    /// Forget a saved network
    ForgetNetwork(String),
}
