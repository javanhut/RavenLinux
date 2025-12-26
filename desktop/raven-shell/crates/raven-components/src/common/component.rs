use std::sync::Arc;
use gtk4::Application;
use parking_lot::RwLock;
use tokio::sync::mpsc;

use raven_core::{
    ComponentId, ConfigPaths, RavenSettings, ServiceHub, ShellCommand, ShellEvent,
};

/// Context provided to all components
pub struct ComponentContext {
    /// GTK application reference
    pub app: Application,
    /// Event receiver for this component
    pub event_rx: async_channel::Receiver<ShellEvent>,
    /// Command sender for this component
    pub command_tx: mpsc::Sender<ShellCommand>,
    /// Shared configuration
    pub config: Arc<RwLock<RavenSettings>>,
    /// Configuration paths
    pub paths: ConfigPaths,
    /// Service hub reference
    pub services: Arc<ServiceHub>,
}

impl ComponentContext {
    /// Create a new context for a component
    pub fn new(
        app: &Application,
        services: &Arc<ServiceHub>,
        config: Arc<RwLock<RavenSettings>>,
    ) -> Self {
        Self {
            app: app.clone(),
            event_rx: services.event_receiver(),
            command_tx: services.command_sender(),
            config,
            paths: ConfigPaths::new(),
            services: services.clone(),
        }
    }

    /// Send a command
    pub fn send_command(&self, cmd: ShellCommand) {
        let tx = self.command_tx.clone();
        glib::spawn_future_local(async move {
            let _ = tx.send(cmd).await;
        });
    }

    /// Get current settings
    pub fn settings(&self) -> RavenSettings {
        self.config.read().clone()
    }
}

impl Clone for ComponentContext {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            event_rx: self.event_rx.clone(),
            command_tx: self.command_tx.clone(),
            config: self.config.clone(),
            paths: ConfigPaths::new(),
            services: self.services.clone(),
        }
    }
}

/// Trait that all shell components must implement
/// Note: Components are not Send since they contain GTK widgets that must be used on the main thread
pub trait Component {
    /// Get the component's identifier
    fn id(&self) -> ComponentId;

    /// Initialize the component with context
    fn init(&mut self, ctx: ComponentContext);

    /// Show the component window
    fn show(&self);

    /// Hide the component window
    fn hide(&self);

    /// Check if currently visible
    fn is_visible(&self) -> bool;

    /// Toggle visibility
    fn toggle(&self) {
        if self.is_visible() {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Handle an event from the message bus
    fn handle_event(&self, event: &ShellEvent);

    /// Cleanup before shutdown
    fn shutdown(&self) {}

    /// Whether this component should always be visible (panel, desktop)
    fn is_always_visible(&self) -> bool {
        false
    }

    /// Get the component's GTK window (for layer-shell setup)
    fn window(&self) -> Option<&gtk4::Window> {
        None
    }
}

/// Boxed component type for storage
pub type BoxedComponent = Box<dyn Component>;
