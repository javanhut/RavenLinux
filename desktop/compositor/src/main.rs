//! raven-compositor - RavenDE Wayland Compositor
//!
//! A modern Wayland compositor built with Smithay, designed for developers.

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod config;
mod input;
mod native;
mod render;
mod shell;
mod state;
mod window;
mod workspace;

use state::RavenState;

fn main() -> Result<()> {
    // Raw debug output for serial console
    eprintln!("=== RAVEN-COMPOSITOR STARTING ===");
    eprintln!("PID: {}", std::process::id());
    eprintln!("WAYLAND_DISPLAY: {:?}", std::env::var("WAYLAND_DISPLAY").ok());
    eprintln!("XDG_RUNTIME_DIR: {:?}", std::env::var("XDG_RUNTIME_DIR").ok());

    // Initialize logging with stderr writer
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)  // Enable DEBUG level for more info
        .with_writer(std::io::stderr)   // Explicitly write to stderr
        .with_ansi(false)               // No ANSI colors for serial
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    eprintln!("=== LOGGING INITIALIZED ===");
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
