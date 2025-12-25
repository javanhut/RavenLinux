use compact_str::CompactString;
use smallvec::SmallVec;
use crate::config::{PanelConfig, RavenSettings, Orientation};

/// Events FROM async services TO GTK (updates UI)
#[derive(Debug, Clone)]
pub enum PanelEvent {
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

    /// Dock config (pinned apps) was reloaded
    ConfigReloaded(PanelConfig),

    /// Raven settings were reloaded
    SettingsReloaded(RavenSettings),

    /// Successfully connected to Hyprland IPC
    HyprlandConnected,

    /// Lost connection to Hyprland IPC
    HyprlandDisconnected,

    /// Clock tick (for updating time display)
    ClockTick,
}

/// Commands FROM GTK TO async services
#[derive(Debug, Clone)]
pub enum PanelCommand {
    /// Focus a window by address
    FocusWindow(CompactString),

    /// Close a window by address
    CloseWindow(CompactString),

    /// Minimize window to special workspace
    MinimizeWindow(CompactString),

    /// Restore window from special workspace
    RestoreWindow(CompactString),

    /// Launch an application
    LaunchApp(CompactString),

    /// Change panel position
    SetPanelPosition(Orientation),

    /// Pin or unpin an app
    PinApp {
        id: CompactString,
        pinned: bool,
    },

    /// Save current dock config
    SaveDockConfig,

    /// Power commands
    Logout,
    Lock,
    Reboot,
    Shutdown,
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
