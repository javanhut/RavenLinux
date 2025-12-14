//! Pointer (mouse/touchpad) input handling

use crate::config::{MouseConfig, TouchpadConfig};

pub struct PointerHandler {
    mouse_config: MouseConfig,
    touchpad_config: TouchpadConfig,
}

impl PointerHandler {
    pub fn new(mouse_config: MouseConfig, touchpad_config: TouchpadConfig) -> Self {
        Self {
            mouse_config,
            touchpad_config,
        }
    }

    pub fn mouse_natural_scroll(&self) -> bool {
        self.mouse_config.natural_scroll
    }

    pub fn mouse_acceleration(&self) -> f64 {
        self.mouse_config.acceleration
    }

    pub fn mouse_scroll_factor(&self) -> f64 {
        self.mouse_config.scroll_factor
    }

    pub fn touchpad_natural_scroll(&self) -> bool {
        self.touchpad_config.natural_scroll
    }

    pub fn touchpad_tap_to_click(&self) -> bool {
        self.touchpad_config.tap_to_click
    }

    pub fn touchpad_two_finger_scroll(&self) -> bool {
        self.touchpad_config.two_finger_scroll
    }

    pub fn touchpad_disable_while_typing(&self) -> bool {
        self.touchpad_config.disable_while_typing
    }
}
