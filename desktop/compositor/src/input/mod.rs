//! Input handling for keyboard, mouse, and touchpad

pub mod keyboard;
pub mod pointer;

use crate::config::InputConfig;

pub struct InputHandler {
    config: InputConfig,
}

impl InputHandler {
    pub fn new(config: InputConfig) -> Self {
        Self { config }
    }
}
