mod config;
mod css;
mod messages;
mod services;
mod state;
mod ui;

use gtk4::prelude::*;
use gtk4::Application;
use tokio::runtime::Runtime;
use tracing::{error, info};

use crate::config::ConfigPaths;
use crate::messages::{PanelCommand, PanelEvent};
use crate::services::{ConfigWatcher, HyprlandService, ProcessService};
use crate::ui::Panel;

const APP_ID: &str = "org.ravenlinux.shell";

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("raven_taskmanager=info".parse()?)
                .add_directive("hyprland=warn".parse()?),
        )
        .init();

    info!("Starting Raven Taskmanager");

    // Create tokio runtime for async services
    let runtime = Runtime::new()?;
    let _guard = runtime.enter();

    // Create communication channels
    // Events: services -> GTK (async-channel for glib compatibility)
    let (event_tx, event_rx) = async_channel::bounded::<PanelEvent>(64);

    // Commands: GTK -> services (tokio mpsc)
    let (hyprland_tx, hyprland_rx) = tokio::sync::mpsc::channel::<PanelCommand>(64);
    let (process_tx, process_rx) = tokio::sync::mpsc::channel::<PanelCommand>(64);

    // Combined command sender that broadcasts to both services
    let command_tx = CommandBroadcaster::new(hyprland_tx, process_tx);

    // Spawn async services on tokio runtime
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

    // Run GTK application
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_activate(move |app| {
        // Load CSS
        css::load_css();

        // Create and present panel
        let panel = Panel::new(app, event_rx.clone(), command_tx.clone());
        panel.present();

        info!("Panel window presented");
    });

    // Run GTK main loop (blocks)
    let exit_code = app.run();

    info!("Raven Shell exiting");

    // Cleanup
    drop(runtime);

    std::process::exit(exit_code.into());
}

/// Broadcasts commands to multiple services
#[derive(Clone)]
struct CommandBroadcaster {
    hyprland_tx: tokio::sync::mpsc::Sender<PanelCommand>,
    process_tx: tokio::sync::mpsc::Sender<PanelCommand>,
}

impl CommandBroadcaster {
    fn new(
        hyprland_tx: tokio::sync::mpsc::Sender<PanelCommand>,
        process_tx: tokio::sync::mpsc::Sender<PanelCommand>,
    ) -> tokio::sync::mpsc::Sender<PanelCommand> {
        // Create a channel that we'll use for the panel
        let (tx, mut rx) = tokio::sync::mpsc::channel::<PanelCommand>(64);

        let hyprland = hyprland_tx;
        let process = process_tx;

        // Spawn a task to forward commands to appropriate services
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match &cmd {
                    // Hyprland window commands
                    PanelCommand::FocusWindow(_)
                    | PanelCommand::CloseWindow(_)
                    | PanelCommand::MinimizeWindow(_)
                    | PanelCommand::RestoreWindow(_)
                    | PanelCommand::Logout => {
                        let _ = hyprland.send(cmd).await;
                    }

                    // Process commands
                    PanelCommand::LaunchApp(_)
                    | PanelCommand::Lock
                    | PanelCommand::Reboot
                    | PanelCommand::Shutdown => {
                        let _ = process.send(cmd).await;
                    }

                    // UI-only commands (handled locally)
                    PanelCommand::SetPanelPosition(_)
                    | PanelCommand::PinApp { .. }
                    | PanelCommand::SaveDockConfig => {
                        // These are handled by the panel itself via the event loop
                    }
                }
            }
        });

        tx
    }
}
