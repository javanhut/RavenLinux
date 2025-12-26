use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::config::ConfigPaths;
use crate::messages::{ShellCommand, ShellEvent};
use super::{ConfigWatcher, HyprlandService, ProcessService};

/// Central hub for managing all async services
pub struct ServiceHub {
    /// Event sender for broadcasting to GTK
    event_tx: async_channel::Sender<ShellEvent>,
    /// Event receiver for GTK components
    event_rx: async_channel::Receiver<ShellEvent>,
    /// Command sender for GTK to use
    command_tx: mpsc::Sender<ShellCommand>,
    /// Tokio runtime
    runtime: Arc<Runtime>,
}

impl ServiceHub {
    /// Create a new ServiceHub with its own tokio runtime
    pub fn new() -> anyhow::Result<Self> {
        let runtime = Runtime::new()?;
        let runtime = Arc::new(runtime);

        // Create communication channels
        let (event_tx, event_rx) = async_channel::bounded::<ShellEvent>(64);
        let (command_tx, command_rx) = mpsc::channel::<ShellCommand>(64);

        // Create service-specific command channels
        let (hyprland_tx, hyprland_rx) = mpsc::channel::<ShellCommand>(64);
        let (process_tx, process_rx) = mpsc::channel::<ShellCommand>(64);

        // Spawn command router
        let hyprland_sender = hyprland_tx;
        let process_sender = process_tx;
        runtime.spawn(Self::route_commands(command_rx, hyprland_sender, process_sender));

        // Spawn services
        let event_tx_hyprland = event_tx.clone();
        runtime.spawn(async move {
            let service = HyprlandService::new(event_tx_hyprland, hyprland_rx);
            if let Err(e) = service.run().await {
                error!("Hyprland service error: {}", e);
            }
        });

        runtime.spawn(async move {
            let service = ProcessService::new(process_rx);
            if let Err(e) = service.run().await {
                error!("Process service error: {}", e);
            }
        });

        let event_tx_config = event_tx.clone();
        runtime.spawn(async move {
            let paths = ConfigPaths::new();
            let watcher = ConfigWatcher::new(paths, event_tx_config);
            if let Err(e) = watcher.run().await {
                error!("Config watcher error: {}", e);
            }
        });

        info!("ServiceHub initialized with all services");

        Ok(Self {
            event_tx,
            event_rx,
            command_tx,
            runtime,
        })
    }

    /// Route commands to appropriate services
    async fn route_commands(
        mut rx: mpsc::Receiver<ShellCommand>,
        hyprland_tx: mpsc::Sender<ShellCommand>,
        process_tx: mpsc::Sender<ShellCommand>,
    ) {
        while let Some(cmd) = rx.recv().await {
            match &cmd {
                // Hyprland window commands
                ShellCommand::FocusWindow(_)
                | ShellCommand::CloseWindow(_)
                | ShellCommand::MinimizeWindow(_)
                | ShellCommand::RestoreWindow(_)
                | ShellCommand::Logout => {
                    let _ = hyprland_tx.send(cmd).await;
                }

                // Process commands
                ShellCommand::LaunchApp(_)
                | ShellCommand::Lock
                | ShellCommand::Reboot
                | ShellCommand::Shutdown
                | ShellCommand::Suspend
                | ShellCommand::Hibernate => {
                    let _ = process_tx.send(cmd).await;
                }

                // UI-only commands (handled locally by components)
                ShellCommand::SetPanelPosition(_)
                | ShellCommand::PinApp { .. }
                | ShellCommand::SaveDockConfig
                | ShellCommand::ShowComponent(_)
                | ShellCommand::HideComponent(_)
                | ShellCommand::ToggleComponent(_)
                | ShellCommand::SetWallpaper(_)
                | ShellCommand::SaveConfig
                | ShellCommand::ReloadConfig
                | ShellCommand::ScanNetworks
                | ShellCommand::ConnectNetwork { .. }
                | ShellCommand::DisconnectNetwork
                | ShellCommand::ForgetNetwork(_) => {
                    // These are handled by the panel/components directly
                }
            }
        }
    }

    /// Get a clone of the event receiver for a component
    pub fn event_receiver(&self) -> async_channel::Receiver<ShellEvent> {
        self.event_rx.clone()
    }

    /// Get a clone of the command sender for a component
    pub fn command_sender(&self) -> mpsc::Sender<ShellCommand> {
        self.command_tx.clone()
    }

    /// Get a clone of the event sender (for internal use)
    pub fn event_sender(&self) -> async_channel::Sender<ShellEvent> {
        self.event_tx.clone()
    }

    /// Enter the runtime context (for GTK callbacks)
    pub fn enter_runtime(&self) -> tokio::runtime::EnterGuard<'_> {
        self.runtime.enter()
    }

    /// Get a reference to the runtime
    pub fn runtime(&self) -> &Arc<Runtime> {
        &self.runtime
    }

    /// Broadcast an event to all listeners
    pub fn broadcast_event(&self, event: ShellEvent) {
        let _ = self.event_tx.send_blocking(event);
    }
}

impl Default for ServiceHub {
    fn default() -> Self {
        Self::new().expect("Failed to create ServiceHub")
    }
}
