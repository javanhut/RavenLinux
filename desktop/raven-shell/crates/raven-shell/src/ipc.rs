use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use raven_core::ComponentId;

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

/// Send show command
pub fn send_show(id: ComponentId) -> anyhow::Result<()> {
    let cmd = format!("show {}", component_name(id));
    let response = send_command(&cmd)?;
    println!("{}", response.trim());
    Ok(())
}

/// Send hide command
pub fn send_hide(id: ComponentId) -> anyhow::Result<()> {
    let cmd = format!("hide {}", component_name(id));
    let response = send_command(&cmd)?;
    println!("{}", response.trim());
    Ok(())
}

/// Send toggle command
pub fn send_toggle(id: ComponentId) -> anyhow::Result<()> {
    let cmd = format!("toggle {}", component_name(id));
    let response = send_command(&cmd)?;
    println!("{}", response.trim());
    Ok(())
}

/// Send reload config command
pub fn send_reload_config() -> anyhow::Result<()> {
    let response = send_command("reload-config")?;
    println!("{}", response.trim());
    Ok(())
}

/// Show status of all components
pub fn show_status() -> anyhow::Result<()> {
    let response = send_command("status")?;
    println!("{}", response.trim());
    Ok(())
}

/// Get component name for IPC
fn component_name(id: ComponentId) -> &'static str {
    match id {
        ComponentId::Panel => "panel",
        ComponentId::Desktop => "desktop",
        ComponentId::Menu => "menu",
        ComponentId::Power => "power",
        ComponentId::Settings => "settings",
        ComponentId::Keybindings => "keybindings",
        ComponentId::FileManager => "files",
        ComponentId::WiFi => "wifi",
        ComponentId::Usb => "usb",
        ComponentId::Installer => "installer",
    }
}
