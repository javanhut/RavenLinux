use gtk4::prelude::*;
use gtk4::Label;
use glib::ControlFlow;
use std::time::Duration;

/// Clock widget that updates every second
pub struct Clock {
    label: Label,
}

impl Clock {
    pub fn new() -> Self {
        let label = Label::new(None);
        label.add_css_class("clock");

        // Set initial time
        Self::update_label(&label);

        // Schedule updates every second
        let label_clone = label.clone();
        glib::timeout_add_local(Duration::from_secs(1), move || {
            Self::update_label(&label_clone);
            ControlFlow::Continue
        });

        Self { label }
    }

    fn update_label(label: &Label) {
        let now = chrono::Local::now();
        // Format: Mon Jan 2  3:04 PM
        let formatted = now.format("%a %b %-d  %-I:%M %p").to_string();
        label.set_text(&formatted);
    }

    pub fn widget(&self) -> &Label {
        &self.label
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}
