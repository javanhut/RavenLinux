// USB creator tool - ported from raven-usb

mod backend;
mod widget;

pub use backend::{UsbDevice, UsbManager};
pub use widget::UsbWidget;

use gtk4::prelude::*;
use raven_core::ComponentId;

/// USB Creator Tool wrapper for integration with raven-shell
pub struct UsbTool {
    widget: Option<UsbWidget>,
}

impl UsbTool {
    pub fn new() -> Self {
        Self { widget: None }
    }

    pub fn id() -> ComponentId {
        ComponentId::Usb
    }

    pub fn show(&mut self) {
        if self.widget.is_none() {
            self.widget = Some(UsbWidget::new());
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

impl Default for UsbTool {
    fn default() -> Self {
        Self::new()
    }
}
