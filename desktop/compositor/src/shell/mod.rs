//! Shell integration (panels, launchers, etc.)

/// Layer shell surfaces (panels, overlays, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Background,
    Bottom,
    Top,
    Overlay,
}

/// Anchor position for layer surfaces
#[derive(Debug, Clone, Copy)]
pub struct Anchor {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl Anchor {
    pub fn top() -> Self {
        Self {
            top: true,
            bottom: false,
            left: true,
            right: true,
        }
    }

    pub fn bottom() -> Self {
        Self {
            top: false,
            bottom: true,
            left: true,
            right: true,
        }
    }
}

pub struct LayerSurface {
    pub layer: Layer,
    pub anchor: Anchor,
    pub exclusive_zone: i32,
    pub keyboard_interactivity: bool,
}
