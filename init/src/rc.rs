//! raven-rc - Control utility for RavenInit
//!
//! Commands:
//!   list             - List every service and its state
//!   status [NAME]    - System status, or one service in detail
//!   start NAME       - Start a stopped service
//!   stop NAME        - Stop a running service, and keep it stopped
//!   restart NAME     - Stop then start a service
//!   enable NAME      - Start this service at boot (persists to init.toml)
//!   disable NAME     - Do not start it at boot (persists to init.toml)
//!   poweroff         - Shut down the system
//!   reboot           - Reboot the system
//!   halt             - Halt the system
//!
//! Service commands go over the control socket at /run/raven-init.sock and
//! need raven-init to be PID 1. Shutdown commands fall back to the command
//! file and then to the reboot syscall, so they still work when it is not.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process;
use std::time::Duration;

/// Where raven-init listens. Must match control::SOCKET_PATH.
const SOCKET_PATH: &str = "/run/raven-init.sock";

/// Bound on how long we wait for init. Short: raven-rc is interactive, and an
/// init too busy to answer in a second is a fact worth reporting rather than
/// hiding behind a hang.
const TIMEOUT: Duration = Duration::from_secs(1);

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

    // Everything after the verb, for commands that take a service name.
    let operand = if program == "raven-rc" {
        args.get(2).map(|s| s.as_str())
    } else {
        args.get(1).map(|s| s.as_str())
    };

    match command {
        "poweroff" | "halt" => do_poweroff(),
        "reboot" => do_reboot(),
        "status" => do_status(operand),
        "list" => do_ask("list"),
        "start" | "stop" | "restart" | "enable" | "disable" => match operand {
            Some(name) => do_ask(&format!("{} {}", command, name)),
            None => {
                eprintln!("{}: needs a service name", command);
                eprintln!(
                    "try: {} {} <service>   (or `{} list`)",
                    program, command, program
                );
                process::exit(1);
            }
        },
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

/// Send one request to init and return its reply.
fn ask(request: &str) -> Result<String, String> {
    let mut stream = UnixStream::connect(SOCKET_PATH).map_err(|e| {
        format!(
            "cannot reach raven-init on {}: {}\n\
             (is raven-init PID 1? on the live ISO it is not -- \
             see /proc/1/comm)",
            SOCKET_PATH, e
        )
    })?;

    stream.set_read_timeout(Some(TIMEOUT)).ok();
    stream.set_write_timeout(Some(TIMEOUT)).ok();

    writeln!(stream, "{}", request).map_err(|e| format!("cannot send to raven-init: {}", e))?;
    stream.flush().ok();
    // Tell init we are done writing, so it stops waiting for more request.
    stream.shutdown(std::net::Shutdown::Write).ok();

    let mut reply = String::new();
    stream
        .read_to_string(&mut reply)
        .map_err(|e| format!("cannot read from raven-init: {}", e))?;

    Ok(reply)
}

/// Send a request, print the reply, and exit non-zero if init reported an error.
fn do_ask(request: &str) {
    match ask(request) {
        Ok(reply) => {
            print!("{}", reply);
            // init prefixes failures with "error:", which is what makes this
            // usable from a script.
            if reply.lines().any(|l| l.starts_with("error:")) {
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <command> [SERVICE]", program);
    eprintln!();
    eprintln!("Services:");
    eprintln!("  list             - List every service and its state");
    eprintln!("  status [NAME]    - System status, or one service in detail");
    eprintln!("  start NAME       - Start a stopped service");
    eprintln!("  stop NAME        - Stop a service, and keep it stopped");
    eprintln!("  restart NAME     - Stop then start a service");
    eprintln!("  enable NAME      - Start at boot      (writes init.toml)");
    eprintln!("  disable NAME     - Do not start at boot (writes init.toml)");
    eprintln!();
    eprintln!("System:");
    eprintln!("  poweroff         - Power off the system");
    eprintln!("  reboot           - Reboot the system");
    eprintln!("  halt             - Halt the system");
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
    // Preferred path: the control socket, which init answers synchronously so
    // we learn whether the request actually landed.
    if let Ok(reply) = ask(cmd) {
        print!("{}", reply);
        return;
    }

    let cmd_path = "/run/raven-init.cmd";

    // The command file is only meaningful if raven-init is the thing reading
    // it. On the live ISO PID 1 is a shell script, and writing here *succeeds*
    // while nothing ever acts on it -- a silent no-op is the worst outcome for
    // a shutdown command, so go straight to the syscall instead.
    if !init_is_raven() {
        eprintln!("raven-init is not PID 1; asking the kernel directly.");
        direct_reboot(cmd);
    }

    // Write command to control file
    if let Err(e) = fs::write(cmd_path, cmd) {
        eprintln!("Failed to send command to init: {}", e);

        // Fall back to direct syscall if we can't communicate with init
        eprintln!("Attempting direct system call...");
        direct_reboot(cmd);
    }

    println!("Command sent to init.");
}

/// True when raven-init is PID 1.
fn init_is_raven() -> bool {
    fs::read_to_string("/proc/1/comm")
        .map(|c| c.trim() == "raven-init")
        .unwrap_or(false)
}

/// Ask the kernel to reboot or power off, with no init involved.
fn direct_reboot(cmd: &str) -> ! {
    use nix::sys::reboot::{reboot, RebootMode};

    // Sync filesystems first
    unsafe {
        libc::sync();
    }

    let mode = if cmd == "reboot" {
        RebootMode::RB_AUTOBOOT
    } else {
        RebootMode::RB_POWER_OFF
    };

    match reboot(mode) {
        Ok(_) => process::exit(0),
        Err(e) => {
            eprintln!("Reboot syscall failed: {}", e);
            eprintln!("You may need root privileges.");
            process::exit(1);
        }
    }
}

fn do_status(target: Option<&str>) {
    // `status <service>` is a question for init, not for /proc.
    if let Some(name) = target {
        do_ask(&format!("status {}", name));
        return;
    }

    do_system_status();

    // The service table is the part only init can answer. Absent init, say so
    // once and plainly rather than printing nothing and looking healthy.
    println!();
    match ask("list") {
        Ok(reply) => {
            println!("Services:");
            for line in reply.lines() {
                println!("  {}", line);
            }
        }
        Err(e) => {
            println!("Services: unavailable -- {}", e);
        }
    }
}

fn do_system_status() {
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
