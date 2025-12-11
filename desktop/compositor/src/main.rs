//! raven-compositor - RavenDE Wayland Compositor
//!
//! A modern Wayland compositor built with Smithay, designed for developers.

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod config;
mod input;
mod render;
mod shell;
mod state;
mod window;
mod workspace;

use state::RavenState;

fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting raven-compositor v{}", env!("CARGO_PKG_VERSION"));

    // Parse command line arguments
    let args = Args::parse();

    // Load configuration
    let config = config::Config::load()?;
    info!("Configuration loaded");

    // Initialize compositor state
    let mut state = RavenState::new(config, args.nested)?;

    // Run the compositor
    info!("Entering event loop");
    state.run()?;

    info!("raven-compositor shutting down");
    Ok(())
}

#[derive(Debug)]
struct Args {
    nested: bool,
}

impl Args {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        Self {
            nested: args.iter().any(|a| a == "--nested" || a == "-n"),
        }
    }
}
