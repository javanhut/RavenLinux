use std::process::{Command, Stdio};
use tracing::{debug, error};

/// Spawn a detached process that won't be killed when the parent exits
pub fn spawn_detached(command: &str) -> bool {
    debug!("Spawning detached: {}", command);

    let result = Command::new("sh")
        .args(["-c", command])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match result {
        Ok(_) => {
            debug!("Successfully spawned: {}", command);
            true
        }
        Err(e) => {
            error!("Failed to spawn '{}': {}", command, e);
            false
        }
    }
}

/// Spawn with fallbacks - tries each command until one succeeds
pub fn spawn_with_fallbacks(commands: &[&str]) -> bool {
    for cmd in commands {
        if spawn_detached(cmd) {
            return true;
        }
    }
    false
}
