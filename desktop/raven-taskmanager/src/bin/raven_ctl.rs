use anyhow::{anyhow, Result};
use hyprland::data::{Client, Clients};
use hyprland::dispatch::{Dispatch, DispatchType, WindowIdentifier, WorkspaceIdentifierWithSpecial};
use hyprland::shared::{HyprData, HyprDataActiveOptional};
use std::env;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return Ok(());
    }

    match args[1].as_str() {
        "focus" => {
            let pid = parse_pid(&args, 2)?;
            focus_window(pid)?;
        }
        "minimize" => {
            let pid = parse_pid(&args, 2)?;
            minimize_window(pid)?;
        }
        "restore" => {
            let pid = parse_pid(&args, 2)?;
            restore_window(pid)?;
        }
        "close" => {
            let pid = parse_pid(&args, 2)?;
            close_window(pid)?;
        }
        "list" => {
            list_windows()?;
        }
        "active" => {
            active_window()?;
        }
        "version" | "-v" | "--version" => {
            println!("raven-ctl {}", VERSION);
        }
        "help" | "-h" | "--help" => {
            print_help();
        }
        cmd => {
            eprintln!("Unknown command: {}", cmd);
            print_help();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        r#"raven-ctl - Window control utility for Raven Desktop

USAGE:
    raven-ctl <command> [arguments]

COMMANDS:
    focus <pid>      Focus the window belonging to a process
    minimize <pid>   Minimize a window to special workspace
    restore <pid>    Restore a minimized window
    close <pid>      Close a window gracefully
    list             List all windows
    active           Get the currently focused window
    version          Print version information
    help             Print this help message

EXAMPLES:
    raven-ctl focus 12345
    raven-ctl minimize 12345
    raven-ctl list
"#
    );
}

fn parse_pid(args: &[String], index: usize) -> Result<i32> {
    args.get(index)
        .ok_or_else(|| anyhow!("Missing PID argument"))?
        .parse::<i32>()
        .map_err(|_| anyhow!("Invalid PID"))
}

fn find_client_by_pid(pid: i32) -> Result<Client> {
    let clients = Clients::get()?;

    clients
        .into_iter()
        .find(|c| c.pid == pid)
        .ok_or_else(|| anyhow!("No window found for PID {}", pid))
}

fn focus_window(pid: i32) -> Result<()> {
    let client = find_client_by_pid(pid)?;

    // If minimized, restore first
    if client.workspace.name.starts_with("special:") {
        Dispatch::call(DispatchType::MoveToWorkspaceSilent(
            WorkspaceIdentifierWithSpecial::Relative(0),
            Some(WindowIdentifier::Address(client.address.clone())),
        ))?;
    }

    Dispatch::call(DispatchType::FocusWindow(WindowIdentifier::Address(
        client.address,
    )))?;

    Ok(())
}

fn minimize_window(pid: i32) -> Result<()> {
    let client = find_client_by_pid(pid)?;

    Dispatch::call(DispatchType::MoveToWorkspaceSilent(
        WorkspaceIdentifierWithSpecial::Special(Some("minimized".into())),
        Some(WindowIdentifier::Address(client.address)),
    ))?;

    Ok(())
}

fn restore_window(pid: i32) -> Result<()> {
    let client = find_client_by_pid(pid)?;

    Dispatch::call(DispatchType::MoveToWorkspaceSilent(
        WorkspaceIdentifierWithSpecial::Relative(0),
        Some(WindowIdentifier::Address(client.address.clone())),
    ))?;

    Dispatch::call(DispatchType::FocusWindow(WindowIdentifier::Address(
        client.address,
    )))?;

    Ok(())
}

fn close_window(pid: i32) -> Result<()> {
    let client = find_client_by_pid(pid)?;

    Dispatch::call(DispatchType::CloseWindow(WindowIdentifier::Address(
        client.address,
    )))?;

    Ok(())
}

fn list_windows() -> Result<()> {
    let clients = Clients::get()?;

    for client in clients {
        let status = if client.workspace.name.starts_with("special:") {
            "[minimized]"
        } else if client.focus_history_id == 0 {
            "[focused]"
        } else {
            ""
        };

        println!(
            "{}\t{}\t{} {}",
            client.pid, client.class, client.title, status
        );
    }

    Ok(())
}

fn active_window() -> Result<()> {
    let active = hyprland::data::Client::get_active()?;

    if let Some(client) = active {
        println!(
            "{}\t{}\t{}",
            client.pid, client.class, client.title
        );
    } else {
        println!("No active window");
    }

    Ok(())
}
