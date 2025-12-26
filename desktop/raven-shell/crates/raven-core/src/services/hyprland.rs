use async_channel::Sender;
use compact_str::CompactString;
use hyprland::data::Clients;
use hyprland::dispatch::{Dispatch, DispatchType, WindowIdentifier, WorkspaceIdentifierWithSpecial};
use hyprland::event_listener::EventListener;
use hyprland::shared::{Address, HyprData};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::DockItem;
use crate::messages::{ShellCommand, ShellEvent};

/// Async service for Hyprland IPC communication
pub struct HyprlandService {
    event_tx: Sender<ShellEvent>,
    command_rx: mpsc::Receiver<ShellCommand>,
}

impl HyprlandService {
    pub fn new(event_tx: Sender<ShellEvent>, command_rx: mpsc::Receiver<ShellCommand>) -> Self {
        Self {
            event_tx,
            command_rx,
        }
    }

    /// Check if Hyprland is running by looking for its socket
    fn is_hyprland_running() -> bool {
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            if let Ok(sig) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
                let socket_path = format!("{}/hypr/{}/.socket.sock", runtime_dir, sig);
                return std::path::Path::new(&socket_path).exists();
            }
            // Also check for any hypr directory
            let hypr_dir = format!("{}/hypr", runtime_dir);
            if std::path::Path::new(&hypr_dir).exists() {
                if let Ok(entries) = std::fs::read_dir(&hypr_dir) {
                    for entry in entries.flatten() {
                        let socket = entry.path().join(".socket.sock");
                        if socket.exists() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Main run loop - syncs initial state, starts event listener, handles commands
    pub async fn run(mut self) -> anyhow::Result<()> {
        info!("Starting Hyprland service");

        // Wait for Hyprland to be running
        loop {
            if Self::is_hyprland_running() {
                match self.try_connect().await {
                    Ok(()) => break,
                    Err(e) => {
                        warn!("Failed to connect to Hyprland: {}, retrying in 2s", e);
                    }
                }
            } else {
                warn!("Hyprland not running, waiting...");
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        // Spawn event listener in background
        let event_tx = self.event_tx.clone();
        let listener_handle = tokio::spawn(async move {
            loop {
                if let Err(e) = Self::run_event_listener(event_tx.clone()).await {
                    error!("Event listener error: {}, reconnecting...", e);
                    let _ = event_tx.send(ShellEvent::HyprlandDisconnected).await;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        });

        // Handle commands from GTK
        while let Some(cmd) = self.command_rx.recv().await {
            if let Err(e) = self.handle_command(cmd).await {
                error!("Failed to execute command: {}", e);
            }
        }

        listener_handle.abort();
        Ok(())
    }

    /// Attempt to connect and sync initial window state
    async fn try_connect(&self) -> anyhow::Result<()> {
        debug!("Syncing initial window state from Hyprland");

        // Get all current windows
        let clients = Clients::get_async().await?;

        for client in clients {
            if !DockItem::should_track(&client.class) {
                continue;
            }

            let _ = self
                .event_tx
                .send(ShellEvent::WindowOpened {
                    address: client.address.to_string().into(),
                    class: client.class.into(),
                    title: client.title.into(),
                    pid: client.pid as u32,
                })
                .await;

            // Check if minimized (in special workspace)
            if client.workspace.name.starts_with("special:") {
                let _ = self
                    .event_tx
                    .send(ShellEvent::WindowMoved {
                        address: client.address.to_string().into(),
                        workspace: client.workspace.id,
                        is_special: true,
                    })
                    .await;
            }
        }

        let _ = self.event_tx.send(ShellEvent::HyprlandConnected).await;
        info!("Connected to Hyprland IPC");

        Ok(())
    }

    /// Run the event listener (blocks until error)
    async fn run_event_listener(tx: Sender<ShellEvent>) -> anyhow::Result<()> {
        let mut listener = EventListener::new();

        // Window opened
        let tx1 = tx.clone();
        listener.add_window_open_handler(move |data| {
            let event = ShellEvent::WindowOpened {
                address: data.window_address.to_string().into(),
                class: data.window_class.into(),
                title: data.window_title.into(),
                pid: 0, // PID not available in event, will be updated if needed
            };
            let _ = tx1.send_blocking(event);
        });

        // Window closed
        let tx2 = tx.clone();
        listener.add_window_close_handler(move |addr| {
            let event = ShellEvent::WindowClosed {
                address: addr.to_string().into(),
            };
            let _ = tx2.send_blocking(event);
        });

        // Active window changed (focus)
        let tx3 = tx.clone();
        listener.add_active_window_change_handler(move |data| {
            if let Some(data) = data {
                let event = ShellEvent::WindowFocused {
                    address: data.window_address.to_string().into(),
                };
                let _ = tx3.send_blocking(event);
            }
        });

        // Window moved (workspace change, including minimize)
        let tx4 = tx.clone();
        listener.add_window_moved_handler(move |data| {
            // Parse workspace ID from name, default to 0
            let workspace_id = data.workspace_name.parse::<i32>().unwrap_or(0);
            let event = ShellEvent::WindowMoved {
                address: data.window_address.to_string().into(),
                workspace: workspace_id,
                is_special: data.workspace_name.starts_with("special:"),
            };
            let _ = tx4.send_blocking(event);
        });

        // Window title changed
        let tx5 = tx.clone();
        listener.add_window_title_change_handler(move |addr| {
            let event = ShellEvent::WindowTitleChanged {
                address: addr.to_string().into(),
                title: "".into(), // Title not provided in event, would need to query
            };
            let _ = tx5.send_blocking(event);
        });

        debug!("Starting Hyprland event listener");
        listener.start_listener_async().await?;

        Ok(())
    }

    /// Handle a command from the GTK UI
    async fn handle_command(&self, cmd: ShellCommand) -> anyhow::Result<()> {
        match cmd {
            ShellCommand::FocusWindow(addr) => {
                debug!("Focusing window: {}", addr);
                Dispatch::call_async(DispatchType::FocusWindow(WindowIdentifier::Address(
                    Address::new(&addr),
                )))
                .await?;
            }

            ShellCommand::CloseWindow(addr) => {
                debug!("Closing window: {}", addr);
                Dispatch::call_async(DispatchType::CloseWindow(WindowIdentifier::Address(
                    Address::new(&addr),
                )))
                .await?;
            }

            ShellCommand::MinimizeWindow(addr) => {
                debug!("Minimizing window: {}", addr);
                Dispatch::call_async(DispatchType::MoveToWorkspaceSilent(
                    WorkspaceIdentifierWithSpecial::Special(Some("minimized".into())),
                    Some(WindowIdentifier::Address(Address::new(&addr))),
                ))
                .await?;
            }

            ShellCommand::RestoreWindow(addr) => {
                debug!("Restoring window: {}", addr);
                // Move back to current workspace
                Dispatch::call_async(DispatchType::MoveToWorkspaceSilent(
                    WorkspaceIdentifierWithSpecial::Relative(0),
                    Some(WindowIdentifier::Address(Address::new(&addr))),
                ))
                .await?;
                // Focus it
                Dispatch::call_async(DispatchType::FocusWindow(WindowIdentifier::Address(
                    Address::new(&addr),
                )))
                .await?;
            }

            ShellCommand::Logout => {
                info!("Logout requested");
                Dispatch::call_async(DispatchType::Exit).await?;
            }

            // These are handled by ProcessService or locally
            _ => {}
        }

        Ok(())
    }
}
