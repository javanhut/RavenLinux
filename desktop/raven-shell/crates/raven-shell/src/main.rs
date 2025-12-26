mod daemon;
mod ipc;

use clap::{Parser, Subcommand};
use tracing::info;

use raven_core::ComponentId;

const APP_ID: &str = "org.ravenlinux.shell";

#[derive(Parser)]
#[command(name = "raven-shell")]
#[command(about = "Unified Raven Desktop shell for Hyprland")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run as daemon (default)
    Daemon,
    /// Show a component
    Show {
        /// Component name (menu, power, settings, keybindings, files, wifi)
        component: String,
    },
    /// Hide a component
    Hide {
        /// Component name
        component: String,
    },
    /// Toggle a component
    Toggle {
        /// Component name
        component: String,
    },
    /// Reload configuration
    ReloadConfig,
    /// Show status of all components
    Status,
}

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("raven_shell=info".parse()?)
                .add_directive("raven_core=info".parse()?)
                .add_directive("raven_components=info".parse()?)
                .add_directive("hyprland=warn".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Daemon) | None => {
            info!("Starting Raven Shell daemon");
            daemon::run()
        }
        Some(Command::Show { component }) => {
            let id = parse_component(&component)?;
            ipc::send_show(id)
        }
        Some(Command::Hide { component }) => {
            let id = parse_component(&component)?;
            ipc::send_hide(id)
        }
        Some(Command::Toggle { component }) => {
            let id = parse_component(&component)?;
            ipc::send_toggle(id)
        }
        Some(Command::ReloadConfig) => ipc::send_reload_config(),
        Some(Command::Status) => ipc::show_status(),
    }
}

fn parse_component(name: &str) -> anyhow::Result<ComponentId> {
    ComponentId::from_str(name)
        .ok_or_else(|| anyhow::anyhow!("Unknown component: {}", name))
}
