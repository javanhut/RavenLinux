//! Compositor state management

use crate::config::Config;
use crate::native;
use crate::workspace::WorkspaceManager;
use anyhow::Result;
use tracing::info;

/// Main compositor state
pub struct RavenState {
    pub config: Config,
    pub workspaces: WorkspaceManager,
    pub running: bool,
    nested: bool,
}

impl RavenState {
    pub fn new(config: Config, nested: bool) -> Result<Self> {
        let workspace_count = config.workspaces.count;

        Ok(Self {
            config,
            workspaces: WorkspaceManager::new(workspace_count),
            running: true,
            nested,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        if self.nested {
            info!("Running in nested mode (Winit backend)");
            self.run_nested()
        } else {
            info!("Running on native hardware (DRM backend)");
            self.run_native()
        }
    }

    fn run_nested(&mut self) -> Result<()> {
        // TODO: Initialize Winit backend for nested sessions
        // This allows running the compositor inside another Wayland/X11 session

        info!("Nested mode not yet implemented");
        info!("To test, use: WAYLAND_DISPLAY=wayland-1 ./raven-compositor");
        anyhow::bail!("nested backend not implemented yet")
    }

    fn run_native(&mut self) -> Result<()> {
        info!("Starting native backend (DRM/KMS)");
        native::run_native(&self.config)
    }

    pub fn quit(&mut self) {
        info!("Quit requested");
        self.running = false;
    }
}
