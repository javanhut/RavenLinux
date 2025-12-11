//! raven-rc - Control utility for RavenInit
//!
//! Commands:
//!   poweroff  - Shut down the system
//!   reboot    - Reboot the system
//!   halt      - Halt the system
//!   status    - Show init status

use std::env;
use std::fs;
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = Path::new(&args[0])
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("raven-rc");

    // Handle symlinked commands (poweroff, reboot, halt)
    let command = match program {
        "poweroff" => "poweroff",
        "reboot" => "reboot",
        "halt" => "halt",
        "shutdown" => {
            // Parse shutdown arguments
            if args.len() > 1 {
                match args[1].as_str() {
                    "-h" | "-P" => "poweroff",
                    "-r" => "reboot",
                    "-H" => "halt",
                    "now" => "poweroff",
                    _ => "poweroff",
                }
            } else {
                "poweroff"
            }
        }
        _ => {
            // raven-rc <command>
            if args.len() < 2 {
                print_usage(program);
                process::exit(1);
            }
            args[1].as_str()
        }
    };

    match command {
        "poweroff" | "halt" => do_poweroff(),
        "reboot" => do_reboot(),
        "status" => do_status(),
        "help" | "--help" | "-h" => {
            print_usage(program);
            process::exit(0);
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage(program);
            process::exit(1);
        }
    }
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <command>", program);
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  poweroff  - Power off the system");
    eprintln!("  reboot    - Reboot the system");
    eprintln!("  halt      - Halt the system");
    eprintln!("  status    - Show system status");
    eprintln!();
    eprintln!("This utility can also be invoked as:");
    eprintln!("  poweroff, reboot, halt, shutdown");
}

fn do_poweroff() {
    println!("Initiating system power off...");
    send_command("poweroff");
}

fn do_reboot() {
    println!("Initiating system reboot...");
    send_command("reboot");
}

fn send_command(cmd: &str) {
    let cmd_path = "/run/raven-init.cmd";

    // Write command to control file
    if let Err(e) = fs::write(cmd_path, cmd) {
        eprintln!("Failed to send command to init: {}", e);

        // Fall back to direct syscall if we can't communicate with init
        eprintln!("Attempting direct system call...");

        use nix::sys::reboot::{reboot, RebootMode};

        // Sync filesystems first
        unsafe { libc::sync(); }

        let mode = if cmd == "reboot" {
            RebootMode::RB_AUTOBOOT
        } else {
            RebootMode::RB_POWER_OFF
        };

        if let Err(e) = reboot(mode) {
            eprintln!("Reboot syscall failed: {}", e);
            eprintln!("You may need root privileges.");
            process::exit(1);
        }
    }

    println!("Command sent to init.");
}

fn do_status() {
    println!("RavenLinux Init Status");
    println!("======================");
    println!();

    // Check if init is running
    if Path::new("/proc/1/exe").exists() {
        println!("Init process: Running (PID 1)");

        // Try to read init's cmdline
        if let Ok(cmdline) = fs::read_to_string("/proc/1/cmdline") {
            let cmd = cmdline.replace('\0', " ");
            println!("Init command: {}", cmd.trim());
        }
    } else {
        println!("Init process: Unknown");
    }

    // System uptime
    if let Ok(uptime) = fs::read_to_string("/proc/uptime") {
        if let Some(secs) = uptime.split_whitespace().next() {
            if let Ok(secs) = secs.parse::<f64>() {
                let hours = (secs / 3600.0) as u64;
                let mins = ((secs % 3600.0) / 60.0) as u64;
                let secs = (secs % 60.0) as u64;
                println!("Uptime: {}h {}m {}s", hours, mins, secs);
            }
        }
    }

    // Hostname
    if let Ok(hostname) = fs::read_to_string("/etc/hostname") {
        println!("Hostname: {}", hostname.trim());
    }

    // Load average
    if let Ok(loadavg) = fs::read_to_string("/proc/loadavg") {
        let parts: Vec<&str> = loadavg.split_whitespace().collect();
        if parts.len() >= 3 {
            println!("Load average: {} {} {}", parts[0], parts[1], parts[2]);
        }
    }

    // Memory info
    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        let mut total = 0u64;
        let mut available = 0u64;

        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kb) = parse_meminfo_line(line) {
                    total = kb;
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(kb) = parse_meminfo_line(line) {
                    available = kb;
                }
            }
        }

        if total > 0 {
            let used = total - available;
            let percent = (used as f64 / total as f64) * 100.0;
            println!(
                "Memory: {} MB used / {} MB total ({:.1}%)",
                used / 1024,
                total / 1024,
                percent
            );
        }
    }

    println!();

    // List running services (simple: look for processes)
    println!("Processes:");
    if let Ok(entries) = fs::read_dir("/proc") {
        let mut count = 0;
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if name.chars().all(|c| c.is_ascii_digit()) {
                    count += 1;
                }
            }
        }
        println!("  Total: {}", count);
    }
}

fn parse_meminfo_line(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse().ok()
    } else {
        None
    }
}
