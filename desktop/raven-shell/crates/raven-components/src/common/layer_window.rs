use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use tracing::debug;

use raven_core::Orientation;

/// Layer shell window configuration
#[derive(Debug, Clone)]
pub struct LayerConfig {
    /// Layer to place the window on
    pub layer: Layer,
    /// Edges to anchor to
    pub anchors: Vec<Edge>,
    /// Exclusive zone (auto, none, or specific size)
    pub exclusive_zone: ExclusiveZone,
    /// Keyboard mode
    pub keyboard_mode: KeyboardMode,
    /// Margins from edges
    pub margins: LayerMargins,
    /// Namespace for the surface
    pub namespace: String,
}

/// Exclusive zone configuration
#[derive(Debug, Clone, Copy)]
pub enum ExclusiveZone {
    /// Auto-calculate based on window size
    Auto,
    /// No exclusive zone
    None,
    /// Fixed size in pixels
    Fixed(i32),
}

/// Margins for layer shell window
#[derive(Debug, Clone, Copy, Default)]
pub struct LayerMargins {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Default for LayerConfig {
    fn default() -> Self {
        Self {
            layer: Layer::Top,
            anchors: vec![],
            exclusive_zone: ExclusiveZone::None,
            keyboard_mode: KeyboardMode::None,
            margins: LayerMargins::default(),
            namespace: "raven-shell".to_string(),
        }
    }
}

impl LayerConfig {
    /// Create config for a panel at given orientation
    pub fn panel(orientation: Orientation) -> Self {
        let anchors = match orientation {
            Orientation::Top => vec![Edge::Top, Edge::Left, Edge::Right],
            Orientation::Bottom => vec![Edge::Bottom, Edge::Left, Edge::Right],
            Orientation::Left => vec![Edge::Left, Edge::Top, Edge::Bottom],
            Orientation::Right => vec![Edge::Right, Edge::Top, Edge::Bottom],
        };

        Self {
            layer: Layer::Top,
            anchors,
            exclusive_zone: ExclusiveZone::Auto,
            keyboard_mode: KeyboardMode::None,
            namespace: "raven-panel".to_string(),
            margins: LayerMargins::default(),
        }
    }

    /// Create config for a desktop background
    pub fn desktop() -> Self {
        Self {
            layer: Layer::Background,
            anchors: vec![Edge::Top, Edge::Bottom, Edge::Left, Edge::Right],
            exclusive_zone: ExclusiveZone::None,
            keyboard_mode: KeyboardMode::None,
            namespace: "raven-desktop".to_string(),
            margins: LayerMargins::default(),
        }
    }

    /// Create config for an overlay popup (menu, power, etc.)
    pub fn overlay() -> Self {
        Self {
            layer: Layer::Overlay,
            anchors: vec![],
            exclusive_zone: ExclusiveZone::None,
            keyboard_mode: KeyboardMode::Exclusive,
            namespace: "raven-overlay".to_string(),
            margins: LayerMargins::default(),
        }
    }

    /// Create config for a centered overlay
    pub fn centered_overlay() -> Self {
        Self {
            layer: Layer::Overlay,
            anchors: vec![],
            exclusive_zone: ExclusiveZone::None,
            keyboard_mode: KeyboardMode::Exclusive,
            namespace: "raven-overlay".to_string(),
            margins: LayerMargins::default(),
        }
    }

    /// Create config for a top-left anchored overlay (menu)
    pub fn menu_overlay() -> Self {
        Self {
            layer: Layer::Overlay,
            anchors: vec![Edge::Top, Edge::Left],
            exclusive_zone: ExclusiveZone::None,
            keyboard_mode: KeyboardMode::Exclusive,
            namespace: "raven-menu".to_string(),
            margins: LayerMargins { top: 40, left: 8, ..Default::default() },
        }
    }
}

/// Wrapper for layer-shell enabled windows
pub struct LayerWindow {
    window: ApplicationWindow,
}

impl LayerWindow {
    /// Create a new layer-shell window with the given configuration
    pub fn new(app: &Application, config: LayerConfig) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .build();

        // Initialize layer shell
        window.init_layer_shell();

        // Set layer
        window.set_layer(config.layer);

        // Set anchors
        for edge in &config.anchors {
            window.set_anchor(*edge, true);
        }

        // Set margins
        window.set_margin(Edge::Top, config.margins.top);
        window.set_margin(Edge::Right, config.margins.right);
        window.set_margin(Edge::Bottom, config.margins.bottom);
        window.set_margin(Edge::Left, config.margins.left);

        // Set exclusive zone
        match config.exclusive_zone {
            ExclusiveZone::Auto => window.auto_exclusive_zone_enable(),
            ExclusiveZone::None => window.set_exclusive_zone(0),
            ExclusiveZone::Fixed(size) => window.set_exclusive_zone(size),
        }

        // Set keyboard mode
        window.set_keyboard_mode(config.keyboard_mode);

        // Set namespace
        window.set_namespace(&config.namespace);

        debug!(
            "Created layer window: layer={:?}, anchors={:?}, namespace={}",
            config.layer, config.anchors, config.namespace
        );

        Self { window }
    }

    /// Get the underlying GTK window
    pub fn window(&self) -> &ApplicationWindow {
        &self.window
    }

    /// Set the window content
    pub fn set_child(&self, child: Option<&impl IsA<gtk4::Widget>>) {
        self.window.set_child(child);
    }

    /// Present the window
    pub fn present(&self) {
        self.window.present();
    }

    /// Hide the window
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// Show the window
    pub fn show(&self) {
        self.window.set_visible(true);
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// Close the window
    pub fn close(&self) {
        self.window.close();
    }

    /// Update anchors (for panel position changes)
    pub fn set_anchors(&self, anchors: &[Edge]) {
        // First clear all anchors
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            self.window.set_anchor(edge, false);
        }
        // Then set new ones
        for edge in anchors {
            self.window.set_anchor(*edge, true);
        }
    }

    /// Update margins
    pub fn set_margins(&self, margins: LayerMargins) {
        self.window.set_margin(Edge::Top, margins.top);
        self.window.set_margin(Edge::Right, margins.right);
        self.window.set_margin(Edge::Bottom, margins.bottom);
        self.window.set_margin(Edge::Left, margins.left);
    }
}

impl std::ops::Deref for LayerWindow {
    type Target = ApplicationWindow;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

impl AsRef<gtk4::Window> for LayerWindow {
    fn as_ref(&self) -> &gtk4::Window {
        self.window.upcast_ref()
    }
}
