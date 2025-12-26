pub mod common;
pub mod panel;
pub mod desktop;
pub mod menu;
pub mod power;
pub mod settings;
pub mod keybindings;
pub mod file_manager;

pub use common::{BoxedComponent, Component, ComponentContext, LayerConfig, LayerWindow, MessageBus};
pub use panel::{start_panel_events, Clock, Dock, PanelComponent};
pub use desktop::{DesktopComponent, DesktopContextMenu, DesktopIcon, IconGrid};
pub use menu::{AppCategory, AppDatabase, AppEntry, MenuComponent};
pub use power::PowerComponent;
pub use keybindings::KeybindingsComponent;
pub use settings::SettingsComponent;
pub use file_manager::FileManagerComponent;
