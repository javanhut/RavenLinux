//! Keyboard input handling

use crate::config::KeyboardConfig;

pub struct KeyboardHandler {
    config: KeyboardConfig,
}

impl KeyboardHandler {
    pub fn new(config: KeyboardConfig) -> Self {
        Self { config }
    }

    pub fn layout(&self) -> &str {
        &self.config.layout
    }

    pub fn variant(&self) -> Option<&str> {
        self.config.variant.as_deref()
    }

    pub fn repeat_rate(&self) -> u32 {
        self.config.repeat_rate
    }

    pub fn repeat_delay(&self) -> u32 {
        self.config.repeat_delay
    }
}
