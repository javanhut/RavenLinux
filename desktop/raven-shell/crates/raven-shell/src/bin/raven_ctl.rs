use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "raven-ctl")]
#[command(about = "Control utility for Raven Shell")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
    /// Focus a window by address
    Focus {
        /// Window address
        address: String,
    },
    /// Close a window by address
    Close {
        /// Window address
        address: String,
    },
    /// Launch an application
    Launch {
        /// Application command
        command: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let cmd = match cli.command {
        Command::Show { component } => format!("show {}", component),
        Command::Hide { component } => format!("hide {}", component),
        Command::Toggle { component } => format!("toggle {}", component),
        Command::ReloadConfig => "reload-config".to_string(),
        Command::Status => "status".to_string(),
        Command::Focus { address } => format!("focus {}", address),
        Command::Close { address } => format!("close {}", address),
        Command::Launch { command } => format!("launch {}", command),
    };

    let response = send_command(&cmd)?;
    println!("{}", response.trim());

    Ok(())
}

/// Get the IPC socket path
fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir).join("raven-shell.sock")
}

/// Send a command to the daemon via IPC
fn send_command(cmd: &str) -> anyhow::Result<String> {
    let path = socket_path();

    if !path.exists() {
        return Err(anyhow::anyhow!(
            "Raven Shell daemon is not running (socket not found at {:?})",
            path
        ));
    }

    let mut stream = UnixStream::connect(&path)?;
    stream.write_all(cmd.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    Ok(response)
}
