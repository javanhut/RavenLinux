mod widget;
mod dock;
mod clock;
pub mod menus;

pub use widget::{start_panel_events, PanelComponent};
pub use dock::Dock;
pub use clock::Clock;
