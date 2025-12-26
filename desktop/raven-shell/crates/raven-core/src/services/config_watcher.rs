use async_channel::Sender;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::config::{ConfigPaths, PanelConfig, RavenSettings};
use crate::messages::ShellEvent;

/// Service that watches config files for changes using inotify
pub struct ConfigWatcher {
    paths: ConfigPaths,
    event_tx: Sender<ShellEvent>,
}

impl ConfigWatcher {
    pub fn new(paths: ConfigPaths, event_tx: Sender<ShellEvent>) -> Self {
        Self { paths, event_tx }
    }

    /// Run the config watcher (blocks forever)
    pub async fn run(self) -> anyhow::Result<()> {
        info!("Starting config watcher");

        // Ensure config directories exist
        if let Some(parent) = self.paths.dock_config.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Some(parent) = self.paths.raven_settings.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        // Channel for file events
        let (notify_tx, mut notify_rx) = mpsc::channel::<std::path::PathBuf>(32);

        // Create file watcher
        let notify_tx_clone = notify_tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    // Only react to modifications and creations
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_)
                    ) {
                        for path in event.paths {
                            let _ = notify_tx_clone.blocking_send(path);
                        }
                    }
                }
            },
            notify::Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        // Watch config files (watch parent dirs since files might not exist yet)
        if let Some(parent) = self.paths.dock_config.parent() {
            if parent.exists() {
                watcher.watch(parent, RecursiveMode::NonRecursive)?;
                debug!("Watching directory: {:?}", parent);
            }
        }

        if let Some(parent) = self.paths.raven_settings.parent() {
            if parent.exists() {
                watcher.watch(parent, RecursiveMode::NonRecursive)?;
                debug!("Watching directory: {:?}", parent);
            }
        }

        // Debounce timer - wait for rapid changes to settle
        let mut debounce_deadline: Option<tokio::time::Instant> = None;
        let mut pending_dock_reload = false;
        let mut pending_settings_reload = false;

        loop {
            tokio::select! {
                // New file change notification
                Some(path) = notify_rx.recv() => {
                    if path == self.paths.dock_config {
                        pending_dock_reload = true;
                        debounce_deadline = Some(tokio::time::Instant::now() + Duration::from_millis(100));
                    } else if path == self.paths.raven_settings {
                        pending_settings_reload = true;
                        debounce_deadline = Some(tokio::time::Instant::now() + Duration::from_millis(100));
                    }
                }

                // Debounce timer expired - reload configs
                _ = async {
                    if let Some(deadline) = debounce_deadline {
                        tokio::time::sleep_until(deadline).await;
                    } else {
                        // No deadline set, wait forever
                        std::future::pending::<()>().await;
                    }
                } => {
                    debounce_deadline = None;

                    if pending_dock_reload {
                        pending_dock_reload = false;
                        self.reload_dock_config().await;
                    }

                    if pending_settings_reload {
                        pending_settings_reload = false;
                        self.reload_settings().await;
                    }
                }
            }
        }
    }

    /// Reload dock config and send event
    async fn reload_dock_config(&self) {
        debug!("Reloading dock config: {:?}", self.paths.dock_config);

        match tokio::fs::read(&self.paths.dock_config).await {
            Ok(data) => match serde_json::from_slice::<PanelConfig>(&data) {
                Ok(config) => {
                    info!("Dock config reloaded with {} pinned apps", config.pinned_apps.len());
                    let _ = self.event_tx.send(ShellEvent::ConfigReloaded(config)).await;
                }
                Err(e) => {
                    warn!("Failed to parse dock config: {}", e);
                }
            },
            Err(e) => {
                // File might not exist yet, that's OK
                debug!("Could not read dock config: {}", e);
            }
        }
    }

    /// Reload raven settings and send event
    async fn reload_settings(&self) {
        debug!("Reloading raven settings: {:?}", self.paths.raven_settings);

        match tokio::fs::read(&self.paths.raven_settings).await {
            Ok(data) => match serde_json::from_slice::<RavenSettings>(&data) {
                Ok(settings) => {
                    info!("Raven settings reloaded, panel position: {:?}", settings.panel_position);
                    let _ = self.event_tx.send(ShellEvent::SettingsReloaded(settings)).await;
                }
                Err(e) => {
                    warn!("Failed to parse raven settings: {}", e);
                }
            },
            Err(e) => {
                debug!("Could not read raven settings: {}", e);
            }
        }
    }
}
