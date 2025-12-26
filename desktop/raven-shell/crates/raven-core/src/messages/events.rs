use compact_str::CompactString;
use smallvec::SmallVec;
use std::path::PathBuf;

use crate::config::{PanelConfig, RavenSettings};
use super::ComponentId;

/// Events FROM async services TO GTK (updates UI)
#[derive(Debug, Clone)]
pub enum ShellEvent {
    // =========== Hyprland Window Events ===========

    /// A new window was opened
    WindowOpened {
        address: CompactString,
        class: CompactString,
        title: CompactString,
        pid: u32,
    },

    /// A window was closed
    WindowClosed {
        address: CompactString,
    },

    /// Active window changed (for focus indication)
    WindowFocused {
        address: CompactString,
    },

    /// Window moved to different workspace (including special/minimized)
    WindowMoved {
        address: CompactString,
        workspace: i32,
        is_special: bool,
    },

    /// Window title changed
    WindowTitleChanged {
        address: CompactString,
        title: CompactString,
    },

    /// Batched updates for rapid events
    BatchUpdate(SmallVec<[WindowChange; 8]>),

    // =========== Connection Events ===========

    /// Successfully connected to Hyprland IPC
    HyprlandConnected,

    /// Lost connection to Hyprland IPC
    HyprlandDisconnected,

    // =========== Configuration Events ===========

    /// Dock config (pinned apps) was reloaded
    ConfigReloaded(PanelConfig),

    /// Raven settings were reloaded
    SettingsReloaded(RavenSettings),

    // =========== Clock Events ===========

    /// Clock tick (for updating time display)
    ClockTick,

    // =========== Component Visibility Events ===========

    /// Request to show a component
    ShowComponent(ComponentId),

    /// Request to hide a component
    HideComponent(ComponentId),

    /// Request to toggle a component
    ToggleComponent(ComponentId),

    // =========== Desktop Events ===========

    /// Wallpaper was changed
    WallpaperChanged(PathBuf),

    /// Desktop icons configuration changed
    DesktopIconsChanged,

    // =========== Network Events (for WiFi tool) ===========

    /// Network scan completed
    NetworkScanComplete(Vec<NetworkInfo>),

    /// Connected to a network
    NetworkConnected(String),

    /// Disconnected from network
    NetworkDisconnected,

    /// Network operation error
    NetworkError(String),

    // =========== Power/Battery Events ===========

    /// Battery status changed
    BatteryChanged {
        level: u8,
        charging: bool,
    },
}

/// Individual window change for batched updates
#[derive(Debug, Clone)]
pub struct WindowChange {
    pub address: CompactString,
    pub change_type: ChangeType,
}

/// Type of change that occurred to a window
#[derive(Debug, Clone)]
pub enum ChangeType {
    Opened {
        class: CompactString,
        title: CompactString,
        pid: u32,
    },
    Closed,
    Focused,
    Unfocused,
    Minimized,
    Restored,
    TitleChanged(CompactString),
}

/// Network information for WiFi tool
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub ssid: String,
    pub signal_strength: i32,
    pub connected: bool,
    pub secured: bool,
    pub known: bool,
}
