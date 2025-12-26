use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::mpsc;

use raven_core::{ComponentId, ShellCommand, ShellEvent};

/// Message bus for inter-component communication
pub struct MessageBus {
    /// Event sender (broadcasts to all subscribers)
    event_tx: async_channel::Sender<ShellEvent>,
    /// Event receiver (components clone this)
    event_rx: async_channel::Receiver<ShellEvent>,
    /// Command sender (components use this to send commands)
    command_tx: mpsc::Sender<ShellCommand>,
    /// Component visibility state
    visibility: Arc<RwLock<HashMap<ComponentId, bool>>>,
}

impl MessageBus {
    /// Create a new message bus
    pub fn new(
        event_tx: async_channel::Sender<ShellEvent>,
        event_rx: async_channel::Receiver<ShellEvent>,
        command_tx: mpsc::Sender<ShellCommand>,
    ) -> Self {
        Self {
            event_tx,
            event_rx,
            command_tx,
            visibility: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a clone of the event receiver for a component
    pub fn subscribe(&self) -> async_channel::Receiver<ShellEvent> {
        self.event_rx.clone()
    }

    /// Get a clone of the command sender
    pub fn command_sender(&self) -> mpsc::Sender<ShellCommand> {
        self.command_tx.clone()
    }

    /// Broadcast an event to all subscribers
    pub fn broadcast(&self, event: ShellEvent) {
        let _ = self.event_tx.send_blocking(event);
    }

    /// Send a command
    pub fn send_command(&self, cmd: ShellCommand) {
        let tx = self.command_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(cmd).await;
        });
    }

    /// Set component visibility
    pub fn set_visible(&self, id: ComponentId, visible: bool) {
        self.visibility.write().insert(id, visible);
    }

    /// Get component visibility
    pub fn is_visible(&self, id: ComponentId) -> bool {
        *self.visibility.read().get(&id).unwrap_or(&false)
    }

    /// Show a component
    pub fn show_component(&self, id: ComponentId) {
        self.broadcast(ShellEvent::ShowComponent(id));
    }

    /// Hide a component
    pub fn hide_component(&self, id: ComponentId) {
        self.broadcast(ShellEvent::HideComponent(id));
    }

    /// Toggle a component
    pub fn toggle_component(&self, id: ComponentId) {
        self.broadcast(ShellEvent::ToggleComponent(id));
    }
}

impl Clone for MessageBus {
    fn clone(&self) -> Self {
        Self {
            event_tx: self.event_tx.clone(),
            event_rx: self.event_rx.clone(),
            command_tx: self.command_tx.clone(),
            visibility: self.visibility.clone(),
        }
    }
}
