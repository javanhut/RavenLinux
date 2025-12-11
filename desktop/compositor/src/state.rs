//! Compositor state management

use crate::config::Config;
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

        // Placeholder event loop
        while self.running {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }

        Ok(())
    }

    fn run_native(&mut self) -> Result<()> {
        // TODO: Initialize DRM/libinput backend for native hardware
        // This runs directly on the GPU/display hardware

        info!("Native mode not yet implemented");
        info!("Requires running from a TTY without another compositor");

        // Placeholder event loop
        while self.running {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }

        Ok(())
    }

    pub fn quit(&mut self) {
        info!("Quit requested");
        self.running = false;
    }
}
