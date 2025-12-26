// WiFi manager tool - ported from raven-wifi

mod backend;
mod widget;

pub use backend::{Backend, Network, WiFiManager};
pub use widget::WiFiWidget;

use gtk4::prelude::*;
use raven_core::ComponentId;

/// WiFi Tool wrapper for integration with raven-shell
pub struct WiFiTool {
    widget: Option<WiFiWidget>,
}

impl WiFiTool {
    pub fn new() -> Self {
        Self { widget: None }
    }

    pub fn id() -> ComponentId {
        ComponentId::WiFi
    }

    pub fn show(&mut self) {
        if self.widget.is_none() {
            self.widget = Some(WiFiWidget::new());
        }
        if let Some(ref widget) = self.widget {
            widget.show();
        }
    }

    pub fn hide(&self) {
        if let Some(ref widget) = self.widget {
            widget.hide();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.widget.as_ref().map(|w| w.window().is_visible()).unwrap_or(false)
    }

    pub fn toggle(&mut self) {
        if self.is_visible() {
            self.hide();
        } else {
            self.show();
        }
    }
}

impl Default for WiFiTool {
    fn default() -> Self {
        Self::new()
    }
}
