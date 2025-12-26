// System installer tool - ported from raven-installer

mod backend;
mod widget;

pub use backend::{Disk, InstallConfig, InstallerManager, Partition};
pub use widget::InstallerWidget;

use gtk4::prelude::*;
use raven_core::ComponentId;

/// System Installer Tool wrapper for integration with raven-shell
pub struct InstallerTool {
    widget: Option<InstallerWidget>,
}

impl InstallerTool {
    pub fn new() -> Self {
        Self { widget: None }
    }

    pub fn id() -> ComponentId {
        ComponentId::Installer
    }

    pub fn show(&mut self) {
        if self.widget.is_none() {
            self.widget = Some(InstallerWidget::new());
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

impl Default for InstallerTool {
    fn default() -> Self {
        Self::new()
    }
}
