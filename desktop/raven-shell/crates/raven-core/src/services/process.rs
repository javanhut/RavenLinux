use compact_str::CompactString;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::messages::ShellCommand;

/// Service for handling process launching and power commands
pub struct ProcessService {
    command_rx: mpsc::Receiver<ShellCommand>,
}

impl ProcessService {
    pub fn new(command_rx: mpsc::Receiver<ShellCommand>) -> Self {
        Self { command_rx }
    }

    /// Run the process service (blocks forever)
    pub async fn run(mut self) -> anyhow::Result<()> {
        info!("Starting process service");

        while let Some(cmd) = self.command_rx.recv().await {
            match cmd {
                ShellCommand::LaunchApp(command) => {
                    Self::launch_app(&command).await;
                }

                ShellCommand::Lock => {
                    Self::lock_screen().await;
                }

                ShellCommand::Reboot => {
                    Self::reboot().await;
                }

                ShellCommand::Shutdown => {
                    Self::shutdown().await;
                }

                ShellCommand::Suspend => {
                    Self::suspend().await;
                }

                ShellCommand::Hibernate => {
                    Self::hibernate().await;
                }

                // Other commands are handled by HyprlandService or locally
                _ => {}
            }
        }

        Ok(())
    }

    /// Launch an application asynchronously
    async fn launch_app(command: &CompactString) {
        debug!("Launching app: {}", command);

        // Spawn in background - don't wait for result
        let cmd = command.to_string();
        tokio::spawn(async move {
            let result = tokio::process::Command::new("sh")
                .args(["-c", &cmd])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();

            if let Err(e) = result {
                error!("Failed to launch '{}': {}", cmd, e);
            }
        });
    }

    /// Lock the screen using available screen locker
    async fn lock_screen() {
        info!("Locking screen");

        // Try various screen lockers in order of preference
        let lockers = ["swaylock", "hyprlock", "loginctl lock-session"];

        for locker in lockers {
            let result = tokio::process::Command::new("sh")
                .args(["-c", locker])
                .spawn();

            if result.is_ok() {
                debug!("Started screen locker: {}", locker);
                return;
            }
        }

        error!("No screen locker found");
    }

    /// Reboot the system
    async fn reboot() {
        info!("Rebooting system");

        // Try various reboot methods
        let methods = [
            "systemctl reboot",
            "reboot",
            "raven-powerctl reboot",
        ];

        for method in methods {
            let result = tokio::process::Command::new("sh")
                .args(["-c", method])
                .spawn();

            if result.is_ok() {
                debug!("Initiated reboot via: {}", method);
                return;
            }
        }

        error!("Failed to initiate reboot");
    }

    /// Shutdown the system
    async fn shutdown() {
        info!("Shutting down system");

        // Try various shutdown methods
        let methods = [
            "systemctl poweroff",
            "poweroff",
            "raven-powerctl poweroff",
        ];

        for method in methods {
            let result = tokio::process::Command::new("sh")
                .args(["-c", method])
                .spawn();

            if result.is_ok() {
                debug!("Initiated shutdown via: {}", method);
                return;
            }
        }

        error!("Failed to initiate shutdown");
    }

    /// Suspend the system
    async fn suspend() {
        info!("Suspending system");

        let methods = [
            "systemctl suspend",
            "loginctl suspend",
        ];

        for method in methods {
            let result = tokio::process::Command::new("sh")
                .args(["-c", method])
                .spawn();

            if result.is_ok() {
                debug!("Initiated suspend via: {}", method);
                return;
            }
        }

        error!("Failed to initiate suspend");
    }

    /// Hibernate the system
    async fn hibernate() {
        info!("Hibernating system");

        let methods = [
            "systemctl hibernate",
            "loginctl hibernate",
        ];

        for method in methods {
            let result = tokio::process::Command::new("sh")
                .args(["-c", method])
                .spawn();

            if result.is_ok() {
                debug!("Initiated hibernate via: {}", method);
                return;
            }
        }

        error!("Failed to initiate hibernate");
    }
}

/// Helper for running a command with fallbacks
pub async fn run_with_fallbacks(commands: &[&str]) -> bool {
    for cmd in commands {
        let result = tokio::process::Command::new("sh")
            .args(["-c", cmd])
            .spawn();

        if result.is_ok() {
            debug!("Successfully ran: {}", cmd);
            return true;
        }
    }
    false
}
