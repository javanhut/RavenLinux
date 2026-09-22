//! raven-rc - Control utility for RavenInit
//!
//! Commands:
//!   list             - List every service and its state
//!   status [NAME]    - System status, or one service in detail
//!   blame            - Boot timeline: when each service started and was ready, slowest first
//!   logs NAME [-f]   - Show the tail of a service's log, and follow it with -f
//!   start NAME       - Start a stopped service
//!   stop NAME        - Stop a running service, and keep it stopped
//!   restart NAME     - Stop then start a service
//!   enable NAME      - Start this service at boot (persists to init.toml)
//!   disable NAME     - Do not start it at boot (persists to init.toml)
//!   reload           - Re-read init.toml and /etc/raven/init.d without a reboot
//!   suspend          - Suspend the machine to RAM
//!   hibernate        - Write the session to swap and power off
//!   poweroff         - Shut down the system
//!   reboot           - Reboot the system
//!   halt             - Halt the system
//!
//! `raven-rc --user <command>` talks to the session's own raven-init (see
//! usermode.rs) at $XDG_RUNTIME_DIR/raven-init/ctl instead: the daemons that
//! belong to the person logged in rather than to the machine. The power
//! verbs are not available there.
//!
//! Service commands go over the control socket at /run/raven-init.sock and
//! need raven-init to be PID 1. Shutdown commands fall back to the command
//! file and then to the reboot syscall, so they still work when it is not.
//!
//! The socket is root-only. `list` and `status` do not need it: raven-init
//! publishes their text under /run/raven-init, world-readable, and this tool
//! reads that when it is not running as root. Everything else needs sudo.
//!
//! `logs` needs neither. A service's output is a plain file that init opened
//! on its behalf, so `logs` opens that file and reads it, and works on a
//! machine whose raven-init has died, is being re-exec'd, or was never PID 1
//! at all. It is also the reason `logs -f` follows here rather than over the
//! socket: init answers a request on PID 1's own thread with a 200ms client
//! timeout (control.rs), and a connection held open for as long as somebody
//! watches a log would be a connection holding the supervisor still.

use std::collections::VecDeque;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, BufReader, ErrorKind, Read, Seek, SeekFrom, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Where raven-init listens. Must match control::SOCKET_PATH.
const SOCKET_PATH: &str = "/run/raven-init.sock";

/// Which raven-init this invocation talks to. Set once in main from
/// `--user`; the system one until then.
struct Target {
    socket: String,
    status_dir: String,
    user: bool,
}

static TARGET: std::sync::OnceLock<Target> = std::sync::OnceLock::new();

fn rc_target() -> &'static Target {
    TARGET.get_or_init(|| Target {
        socket: SOCKET_PATH.to_string(),
        status_dir: STATUS_DIR.to_string(),
        user: false,
    })
}

/// The session supervisor's socket: under XDG_RUNTIME_DIR, or nowhere.
fn user_target() -> Result<Target, String> {
    let runtime = env::var("XDG_RUNTIME_DIR")
        .map_err(|_| "XDG_RUNTIME_DIR is not set; there is no session raven-init to talk to".to_string())?;
    Ok(Target {
        socket: format!("{runtime}/raven-init/ctl"),
        status_dir: format!("{runtime}/raven-init"),
        user: true,
    })
}

/// Where raven-init publishes the text of `list` (`status`), `status NAME`
/// (`services/NAME`) and `blame` (`blame`), mode 0644. Must match
/// control::STATUS_DIR.
const STATUS_DIR: &str = "/run/raven-init";

/// Where init puts a system service's output. Must match `Service::log_dir`'s
/// fallback in service.rs.
const SYSTEM_LOG_DIR: &str = "/var/log/raven";

/// How many lines `logs` shows when it is not following.
///
/// A screenful and a bit. The number that matters is not this one but the fact
/// that it is bounded at all: a service log is capped at 10MB by
/// `crate::logrotate` and a daemon that has been talking since boot will have
/// most of that, so printing the file would mean a person who typed
/// `raven-rc logs dbus` to see why dbus just died watching ten megabytes go
/// past instead. The last fifty lines are where the reason is; `-f` is there
/// for the rest of it, and the file is named in the error paths for anyone who
/// wants to bring their own pager.
const TAIL_LINES: usize = 50;

/// How often a follow looks for new output.
///
/// A poll rather than an inotify watch, deliberately. raven-init watches for
/// files with inotify because it is PID 1 and cannot afford to wake up for
/// nothing (readiness.rs); this is a short-lived program a person is sitting
/// in front of, and five stat()s a second for as long as they watch costs less
/// than the shell that launched it. Two tenths of a second is below the delay
/// a person reads as lag, and it bounds how much of a truncation this can
/// miss -- see [`follow_step`].
const FOLLOW_INTERVAL: Duration = Duration::from_millis(200);

/// How much of the end of a log to read at a time when looking for its last
/// lines. One of these almost always holds fifty lines of daemon output.
const TAIL_CHUNK: u64 = 64 * 1024;

/// How much a follow moves in one read.
const READ_CHUNK: usize = 64 * 1024;

/// How far back through the rotated generations the tail is willing to walk.
///
/// Must match `logrotate::MAX_KEEP`, which is the largest `log_keep` init will
/// honour: past that there is nothing to find. The walk stops at the first
/// generation that is not there anyway, so this is a backstop against a log
/// directory somebody has filled with `<name>.log.<n>` files by hand rather
/// than a limit anything reaches.
const MAX_GENERATIONS: u32 = 64;

/// The decompressor for a rotated `.gz` generation. Same binary
/// `crate::logrotate` compresses with, and absent on the same machines.
const GZIP: &str = "/bin/gzip";

/// Connection and write operations should fail quickly when PID 1 is absent.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);

/// A restart may spend up to five seconds waiting for the old process to exit,
/// then verify the replacement. The former one-second read timeout made a
/// successful slow restart look like raven-rc could not execute the command.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

/// How many operands a verb takes.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Arity {
    /// `raven-rc list`
    None,
    /// `raven-rc status [NAME]`
    Optional,
    /// `raven-rc start NAME`
    Required,
}

/// Every service verb, its arity, and its one-line help.
///
/// One table because the dispatcher and `print_usage` used to be separate
/// lists 120 lines apart, and they drifted the first time a verb was added:
/// `reload` reached the help text and not the `match`, so it was advertised
/// and then rejected with "Unknown command". Anything listed here is
/// dispatchable by construction, and `verbs_are_dispatchable` fails the build
/// if that ever stops being true.
const SERVICE_VERBS: &[(&str, Arity, &str)] = &[
    ("list", Arity::None, "List every service and its state"),
    (
        "status",
        Arity::Optional,
        "System status, or one service in detail",
    ),
    (
        "blame",
        Arity::None,
        "Boot timeline: service start and ready times, slowest first",
    ),
    // Required, because `logs` without a service name has no sensible
    // meaning -- and because the flag is not an operand. `Arity` says how
    // many *names* a verb takes, which is what the dispatcher and the help
    // text need from it; `-f` is parsed by `parse_logs_args` out of
    // everything after the verb, the same way `status` formats its own reply
    // without the table having to know that it does. Teaching `Arity` about
    // flags would mean every other verb carrying a shape it does not use, to
    // describe the one thing about `logs` that this table is not for.
    (
        "logs",
        Arity::Required,
        "Show a service's log; -f follows it",
    ),
    ("start", Arity::Required, "Start a stopped service"),
    (
        "stop",
        Arity::Required,
        "Stop a service, and keep it stopped",
    ),
    ("restart", Arity::Required, "Stop then start a service"),
    (
        "enable",
        Arity::Required,
        "Start at boot      (writes init.toml)",
    ),
    (
        "disable",
        Arity::Required,
        "Do not start at boot (writes init.toml)",
    ),
    (
        "reload",
        Arity::None,
        "Re-read config; picks up newly installed services",
    ),
    (
        "reexec",
        Arity::None,
        "Swap PID 1 for the raven-init on disk; services keep running",
    ),
];

fn arity_of(verb: &str) -> Option<Arity> {
    SERVICE_VERBS
        .iter()
        .find(|(name, _, _)| *name == verb)
        .map(|(_, arity, _)| *arity)
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
    // `--user` selects the session's raven-init. Removed from argv so the
    // rest of the parsing sees the same shape it always has.
    if args.get(1).is_some_and(|a| a == "--user") {
        args.remove(1);
        match user_target() {
            Ok(t) => {
                let _ = TARGET.set(t);
            }
            Err(e) => {
                eprintln!("raven-rc: {e}");
                process::exit(1);
            }
        }
    }
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
    // Only `logs` looks past the first of them, for its `-f`; every other verb
    // takes one operand and ignores the rest, as it always has.
    let operands: &[String] = if program == "raven-rc" {
        args.get(2..).unwrap_or(&[])
    } else {
        args.get(1..).unwrap_or(&[])
    };
    let operand = operands.first().map(|s| s.as_str());

    if rc_target().user
        && matches!(
            command,
            "poweroff" | "halt" | "reboot" | "suspend" | "sleep" | "hibernate" | "reexec"
        )
    {
        eprintln!("raven-rc --user: '{command}' acts on the machine; run it without --user");
        process::exit(1);
    }

    match command {
        "poweroff" | "halt" => do_poweroff(),
        "reboot" => do_reboot(),
        "suspend" | "sleep" => do_suspend(),
        "hibernate" => do_hibernate(),
        "help" | "--help" | "-h" => {
            print_usage(program);
            process::exit(0);
        }
        // `status` formats its own reply, so it does not go through do_ask.
        "status" => do_status(operand),
        // `logs` answers itself entirely: the data is a file on this machine,
        // not something init has to be asked for. It is in SERVICE_VERBS all
        // the same, so it is advertised and so `every_advertised_verb_has_an_arity`
        // still covers it.
        "logs" => do_logs(operands),
        verb => match arity_of(verb) {
            Some(Arity::None) => do_ask(verb),
            Some(Arity::Optional) => match operand {
                Some(name) => do_ask(&format!("{} {}", verb, name)),
                None => do_ask(verb),
            },
            Some(Arity::Required) => match operand {
                Some(name) => do_ask(&format!("{} {}", verb, name)),
                None => {
                    eprintln!("{}: needs a service name", verb);
                    eprintln!(
                        "try: {} {} <service>   (or `{} list`)",
                        program, verb, program
                    );
                    process::exit(1);
                }
            },
            None => {
                eprintln!("Unknown command: {}", verb);
                print_usage(program);
                process::exit(1);
            }
        },
    }
}

/// Send one request to init and return its reply.
fn ask(request: &str) -> Result<String, String> {
    // A question from an unprivileged user is answered from what init has
    // published, never from the socket. Root keeps the live path, and so does
    // anyone on a raven-init too old to publish, who then gets the socket's
    // own permission-denied diagnosis below.
    // The session's socket is the user's own, so it is always the live path.
    if !is_root() && !rc_target().user {
        if let Some(reply) = read_published(request) {
            return Ok(reply);
        }
    }

    let mut stream = UnixStream::connect(&rc_target().socket).map_err(|e| {
        // These two failures look alike and mean opposite things, so name
        // them. ECONNREFUSED in particular says the socket file is still
        // there while nothing is listening -- a raven-init that died without
        // cleaning up -- which reads like a broken service manager rather
        // than an absent one.
        let diagnosis = match e.kind() {
            ErrorKind::NotFound if rc_target().user => format!(
                "there is no socket at {}, so no raven-init is supervising this session.\n\
                 The session launcher starts one; `raven-init --user &` does by hand.",
                rc_target().socket
            ),
            ErrorKind::NotFound => format!(
                "there is no socket at {}, so raven-init is not running.\n\
                 PID 1 is '{}', not raven-init, so service commands are not available.",
                rc_target().socket,
                pid1_name()
            ),
            ErrorKind::ConnectionRefused => format!(
                "the socket at {} exists but nothing is listening: raven-init\n\
                 exited without cleaning up. PID 1 is '{}'.\n\
                 Remove the stale socket, or reboot.",
                rc_target().socket,
                pid1_name()
            ),
            ErrorKind::PermissionDenied => format!(
                "permission denied on {}. The control socket is root-only;\n\
                 try again with sudo. (`list` and `status` work without root once\n\
                 raven-init publishes {}; this one has not.)",
                rc_target().socket, rc_target().status_dir
            ),
            _ => format!("cannot reach raven-init on {}: {}", rc_target().socket, e),
        };
        format!("raven-rc: {}", diagnosis)
    })?;

    stream.set_read_timeout(Some(RESPONSE_TIMEOUT)).ok();
    stream.set_write_timeout(Some(CONNECT_TIMEOUT)).ok();

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

fn is_root() -> bool {
    nix::unistd::geteuid().is_root()
}

/// The published answer to a read-only request, or None when the request is
/// not one (`start`, `stop`, ...) or nothing has been published.
///
/// A missing `services/NAME` under a present directory is reported the way
/// init would report it over the socket, so scripts see the same text.
fn read_published(request: &str) -> Option<String> {
    let mut parts = request.split_whitespace();
    let path = match (parts.next(), parts.next(), parts.next()) {
        (Some("list"), None, _) | (Some("status"), None, _) => format!("{}/status", STATUS_DIR),
        (Some("blame"), None, _) => format!("{}/blame", STATUS_DIR),
        (Some("status"), Some(name), None) => {
            if name.contains('/') || name.starts_with('.') {
                return Some(format!("error: no such service '{}'\n", name));
            }
            match fs::read_to_string(format!("{}/services/{}", STATUS_DIR, name)) {
                Ok(text) => return Some(text),
                Err(e) if e.kind() == ErrorKind::NotFound && Path::new(STATUS_DIR).is_dir() => {
                    return Some(format!("error: no such service '{}'\n", name));
                }
                Err(_) => return None,
            }
        }
        _ => return None,
    };
    fs::read_to_string(path).ok()
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
    eprintln!("Usage: {} [--user] <command> [SERVICE]", program);
    eprintln!("  --user           - Talk to this session's raven-init (its own daemons, no power verbs)");
    eprintln!();
    eprintln!("Services:");
    for (verb, arity, help) in SERVICE_VERBS {
        let operand = match arity {
            Arity::None => "",
            Arity::Optional => " [NAME]",
            Arity::Required => " NAME",
        };
        eprintln!("  {:<16} - {}", format!("{verb}{operand}"), help);
    }
    eprintln!();
    eprintln!("System:");
    eprintln!("  suspend          - Suspend to RAM (also: sleep)");
    eprintln!("  hibernate        - Write the session to swap and power off");
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

/// Ask init to sleep, and say so if it could not.
///
/// Deliberately not routed through [`send_command`]: that one falls back to
/// `/run/raven-init.cmd` and then to `reboot(2)`, and a fallback chain that
/// ends in "power off the machine instead" is not one a suspend should be on.
/// The fallback here is to perform the same write init would have performed,
/// which is the right answer when raven-init is not PID 1 -- in a container,
/// on a rescue system, or under another init entirely.
fn do_suspend() {
    match ask("suspend") {
        Ok(reply) => {
            print!("{}", reply);
        }
        Err(e) => {
            eprintln!("{}", e);
            eprintln!("Suspending directly instead.");
            direct_suspend();
        }
    }
}

/// Ask init to hibernate, and say so if it could not.
///
/// Unlike [`do_suspend`] there is no direct fallback, and that is the point
/// rather than an omission. The check that makes hibernation safe -- that
/// something will actually read the image back on the next boot -- lives in
/// init, in `power::hibernate`. A fallback here would either have to duplicate
/// that check, which is how the two drift apart, or skip it, which is how a
/// machine with no resume device powers off and loses the session. An init
/// that cannot be reached is a machine that does not hibernate.
fn do_hibernate() {
    match ask("hibernate") {
        Ok(reply) => {
            print!("{}", reply);
        }
        Err(e) => {
            eprintln!("{}", e);
            eprintln!(
                "Hibernation is init's to perform: it is the only thing that checks \
                 this machine can resume. Not hibernating."
            );
            process::exit(1);
        }
    }
}

/// Write the sleep state ourselves. Needs root, and blocks until we resume.
fn direct_suspend() {
    const STATE_PATH: &str = "/sys/power/state";

    let offered = match fs::read_to_string(STATE_PATH) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("raven-rc: cannot read {}: {}", STATE_PATH, e);
            eprintln!("  This kernel has no suspend support.");
            process::exit(1);
        }
    };

    // Same order as init's: suspend-to-RAM, then the software-only fallback.
    let Some(state) = ["mem", "freeze"]
        .into_iter()
        .find(|s| offered.split_whitespace().any(|o| o == *s))
    else {
        eprintln!(
            "raven-rc: {} offers '{}'; none of it is a sleep state we use.",
            STATE_PATH,
            offered.trim()
        );
        process::exit(1);
    };

    if let Err(e) = fs::write(STATE_PATH, state) {
        eprintln!("raven-rc: could not suspend: {}", e);
        if e.kind() == ErrorKind::PermissionDenied {
            eprintln!("  {} is root-only; try again with sudo.", STATE_PATH);
        }
        process::exit(1);
    }
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
    // it. Writing while another PID 1 is active succeeds but nobody acts on
    // it, so go straight to the syscall instead.
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

/// The binary PID 1 was started from, resolved as far as this process is
/// allowed to look.
///
/// /proc/1/exe is the authoritative answer and is root-only: resolving that
/// link needs PTRACE_MODE_READ, so for an ordinary user every operation on it
/// -- including Path::exists() -- fails with EACCES. /proc/1/cmdline is
/// world-readable, so fall back to argv[0] and canonicalize it: the kernel
/// boots /sbin/init, which is a symlink onto the real binary.
fn pid1_exe() -> Option<PathBuf> {
    if let Ok(exe) = fs::read_link("/proc/1/exe") {
        return Some(exe);
    }

    let cmdline = fs::read_to_string("/proc/1/cmdline").ok()?;
    let argv0 = Path::new(cmdline.split('\0').next()?);
    if !argv0.is_absolute() {
        return None;
    }
    fs::canonicalize(argv0).ok()
}

/// What PID 1 actually is, for diagnostics.
///
/// The binary's name rather than comm: the kernel sets comm from the basename
/// of the path it exec'd and it exec's /sbin/init, so a running raven-init
/// calls itself "init" and never "raven-init". comm is the last resort, for
/// when neither /proc/1/exe nor /proc/1/cmdline can be read.
fn pid1_name() -> String {
    if let Some(name) = pid1_exe()
        .as_deref()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
    {
        return name;
    }

    fs::read_to_string("/proc/1/comm")
        .map(|c| c.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// True when raven-init is PID 1.
///
/// Answering "no" when the answer is really "cannot tell" is the safe way to
/// be wrong: it sends a shutdown to the reboot syscall instead of through
/// init. Answering "yes" wrongly would hand the command file to a supervisor
/// that never reads it.
fn init_is_raven() -> bool {
    pid1_name() == "raven-init"
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

    if !rc_target().user {
        do_system_status();
    }

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

    // Something is always PID 1 or this process would not be running, so the
    // question is not whether init is there but which init it is. Name the
    // binary: on a system whose PID 1 is not raven-init, that is the first
    // thing worth knowing, and it explains every "unavailable" below it.
    match pid1_name().as_str() {
        "raven-init" => println!("Init process: raven-init (PID 1)"),
        "unknown" => println!("Init process: unknown -- /proc/1 is unreadable"),
        other => println!("Init process: {} (PID 1) -- not raven-init", other),
    }

    if let Ok(cmdline) = fs::read_to_string("/proc/1/cmdline") {
        let cmd = cmdline.replace('\0', " ");
        println!("Init command: {}", cmd.trim());
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

// ---------------------------------------------------------------------------
// raven-rc logs
//
// Per-service logs have always been written; until this verb existed they
// were only reachable by somebody who remembered that they live in
// /var/log/raven and are named after the service. Everything below is
// client-side: it opens files, and never asks init for anything.
// ---------------------------------------------------------------------------

/// What `raven-rc logs ...` was asked for.
#[derive(Debug, PartialEq)]
struct LogsRequest {
    name: String,
    follow: bool,
}

/// Split `logs`'s operands into a service name and the follow flag.
///
/// The flag may come before or after the name, because `raven-rc logs -f dbus`
/// and `raven-rc logs dbus -f` are both things people type and neither is
/// wrong. Anything beginning with `-` is an option: a service whose name
/// starts with a hyphen is not a service init can be asked about either, since
/// every other verb would read it as an option too.
fn parse_logs_args(operands: &[String]) -> Result<LogsRequest, String> {
    let mut name: Option<&str> = None;
    let mut follow = false;

    for operand in operands {
        match operand.as_str() {
            "-f" | "--follow" => follow = true,
            other if other.starts_with('-') => {
                return Err(format!(
                    "logs: unknown option '{other}'\n\
                     try: raven-rc logs <service> [-f]"
                ));
            }
            other => {
                if let Some(first) = name {
                    return Err(format!(
                        "logs: one service at a time ('{first}' and then '{other}')\n\
                         try: raven-rc logs {first}"
                    ));
                }
                name = Some(other);
            }
        }
    }

    let Some(name) = name else {
        return Err("logs: needs a service name\n\
                    try: raven-rc logs <service> [-f]   (or `raven-rc list`)"
            .to_string());
    };

    // The same rule read_published applies, and for the same reason: this name
    // becomes a path, and a name that is not a plain file name in the log
    // directory is not a service.
    if name.contains('/') || name.starts_with('.') {
        return Err(format!("logs: no such service '{name}'"));
    }

    Ok(LogsRequest {
        name: name.to_string(),
        follow,
    })
}

/// Where this invocation's service logs are.
fn log_dir() -> Result<PathBuf, String> {
    resolve_log_dir(
        rc_target().user,
        env::var_os("RAVEN_SERVICE_LOG_DIR"),
        env::var_os("XDG_STATE_HOME"),
        env::var_os("HOME"),
    )
}

/// The log directory for a target, from the environment that decides it.
///
/// Takes its environment as arguments rather than reading it, so the tests can
/// ask what a session's directory would be without setting a process-global
/// variable that the rest of `cargo test` is running in.
///
/// The system answer is `/var/log/raven` and nothing else -- deliberately NOT
/// `$RAVEN_SERVICE_LOG_DIR`, although that is the variable `Service::log_dir`
/// honours inside init. A session's raven-init exports it (main.rs, run_user)
/// so that the services it starts log under the user's state directory, which
/// means every terminal opened from that session has it set. Reading it here
/// would make `raven-rc logs dbus` in such a terminal quietly answer with the
/// session's dbus instead of the machine's, on a machine where the two exist
/// side by side and it matters which one is being debugged. `--user` is how
/// you say you meant the session's, and under `--user` the variable is exactly
/// what the supervisor set and is believed.
fn resolve_log_dir(
    user: bool,
    service_log_dir: Option<OsString>,
    xdg_state: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, String> {
    if !user {
        return Ok(PathBuf::from(SYSTEM_LOG_DIR));
    }
    if let Some(dir) = service_log_dir {
        return Ok(PathBuf::from(dir));
    }
    // usermode::Paths::from_env computes the same thing; that copy is the one
    // init writes through, this one is the one that reads.
    if let Some(state) = xdg_state {
        return Ok(PathBuf::from(state).join("raven/log"));
    }
    match home {
        Some(home) => Ok(PathBuf::from(home).join(".local/state/raven/log")),
        None => Err(
            "logs: neither XDG_STATE_HOME nor HOME is set, so there is nowhere\n\
             a session's logs could be. Name the directory in RAVEN_SERVICE_LOG_DIR."
                .to_string(),
        ),
    }
}

/// `raven-rc logs <service> [-f]`.
fn do_logs(operands: &[String]) {
    let request = match parse_logs_args(operands) {
        Ok(request) => request,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    let dir = match log_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    let path = dir.join(format!("{}.log", request.name));

    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("{}", explain_unreadable_log(&request.name, &dir, &path, &e));
            process::exit(1);
        }
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();

    let tail = match read_tail(&mut file, &path, TAIL_LINES) {
        Ok(tail) => tail,
        Err(e) => {
            eprintln!("raven-rc: cannot read {}: {}", path.display(), e);
            process::exit(1);
        }
    };
    // Trouble reading the *history* is not a reason to print nothing: the tail
    // of the live log is still the answer to most questions, and the missing
    // part is older than it.
    for line in &tail.trouble {
        eprintln!("raven-rc: {line}");
    }
    emit(&mut out, &tail.bytes);

    if !request.follow {
        let _ = out.flush();
        return;
    }

    // Resume from where the tail stopped reading rather than from the end as
    // it is now: a line written while the tail was being assembled belongs in
    // the output, not in the gap between the two.
    if let Err(e) = file.seek(SeekFrom::Start(tail.end)) {
        eprintln!("raven-rc: cannot follow {}: {}", path.display(), e);
        process::exit(1);
    }
    // On stderr, so that `raven-rc logs dbus -f | grep something` still pipes
    // nothing but the log. A quiet service otherwise looks like a verb that
    // did not work.
    eprintln!("raven-rc: following {} -- ^C to stop", path.display());
    follow(&path, &mut file, &mut out);
}

/// Why a log could not be opened, said in terms of the service rather than of
/// the file.
///
/// The three cases mean three different things and an errno distinguishes none
/// of them: a service that does not exist, a service that exists and has never
/// run, and a log that is there but not ours to read.
fn explain_unreadable_log(name: &str, dir: &Path, path: &Path, e: &io::Error) -> String {
    match e.kind() {
        ErrorKind::PermissionDenied => format!(
            "raven-rc: permission denied on {}.\n\
             A service's log is written by init as root and carries whatever\n\
             the umask gave it; try again with sudo.",
            path.display()
        ),
        // The hint is only worth giving to somebody who did not already say
        // --user: told to a session, "try --user" is advice they have taken.
        ErrorKind::NotFound if !dir.is_dir() && rc_target().user => format!(
            "raven-rc: there is no log directory at {}, so this session's\n\
             raven-init has not started anything that wrote a log.",
            dir.display()
        ),
        ErrorKind::NotFound if !dir.is_dir() => format!(
            "raven-rc: there is no log directory at {}, so no service has\n\
             written anything yet. A session daemon's log is not there:\n\
             `raven-rc --user logs {}` reads the session's instead.",
            dir.display(),
            name
        ),
        // Init publishes one file per service it knows about, so its absence
        // is a good guess at "no such service" -- but only a guess, which is
        // why the wording does not claim more than it checked.
        ErrorKind::NotFound
            if Path::new(&format!("{}/services/{}", rc_target().status_dir, name)).exists() =>
        {
            format!(
                "raven-rc: {} exists but has no log at {}.\n\
                 It has not been started since this raven-init came up; the log is\n\
                 created by the start, so `raven-rc {}start {}` is what makes one.",
                name,
                path.display(),
                if rc_target().user { "--user " } else { "" },
                name
            )
        }
        ErrorKind::NotFound => format!(
            "raven-rc: no log for '{}' at {}.\n\
             `raven-rc list` names the services init knows about; a service that\n\
             has never been started has never had a log written for it.",
            name,
            path.display()
        ),
        _ => format!("raven-rc: cannot open {}: {}", path.display(), e),
    }
}

/// The last lines of a service's log, taken across the rotation boundary.
struct Tail {
    /// The bytes to print, oldest line first.
    bytes: Vec<u8>,
    /// How many lines they are.
    lines: usize,
    /// How long the current log was when it was read. Where a follow resumes.
    end: u64,
    /// What could not be read of the older generations, for stderr.
    trouble: Vec<String>,
}

/// Read the last `want` lines of a log, continuing into the rotated
/// generations when the current file is shorter than that.
///
/// A rotation is not an event a person reading a log cares about. It happens
/// when a file passes 10MB, which for a quiet service can be months apart and
/// for a noisy one can be minutes, and a service that has just been rotated
/// would otherwise answer `raven-rc logs` with the four lines it has written
/// since -- which is exactly the moment somebody is asking, because a service
/// that has just filled 10MB is a service that is doing something. So the tail
/// walks back: `<name>.log`, then `<name>.log.1` (or `.1.gz`), and so on until
/// it has enough lines or runs out of generations.
///
/// No separator is printed between them. The generations are one stream that
/// was cut into files for the disk's sake, and a reader wants the service's
/// last fifty lines rather than a lesson in how they are stored; the files are
/// named in the error paths for anyone who needs to know.
fn read_tail(file: &mut File, path: &Path, want: usize) -> io::Result<Tail> {
    let (bytes, lines, end) = tail_of_file(file, want)?;
    let mut tail = Tail {
        bytes,
        lines,
        end,
        trouble: Vec::new(),
    };

    let mut n = 1;
    while tail.lines < want && n <= MAX_GENERATIONS {
        let plain = generation(path, n);
        let gz = compressed(&plain);
        let still_want = want - tail.lines;

        let older = if plain.exists() {
            File::open(&plain)
                .and_then(|mut f| tail_of_file(&mut f, still_want).map(|(b, l, _)| (b, l)))
                .map_err(|e| format!("cannot read {}: {}", plain.display(), e))
        } else if gz.exists() {
            tail_of_gzip(&gz, still_want)
                .map_err(|e| format!("cannot read {}: {}", gz.display(), e))
        } else {
            // A gap in the numbering is the end of the history. Nothing writes
            // one, and walking past it would mean reading files that are not
            // this log's just because they are named like it.
            break;
        };

        let (mut bytes, lines) = match older {
            Ok(older) => older,
            Err(e) => {
                tail.trouble.push(e);
                break;
            }
        };

        // A generation whose last line was cut off by the rotation would
        // otherwise run into the first line of the newer file.
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        bytes.extend_from_slice(&tail.bytes);
        tail.bytes = bytes;
        tail.lines += lines;
        n += 1;
    }

    Ok(tail)
}

/// The last `want` lines of an open file, and how long the file was.
///
/// Read backwards from the end in [`TAIL_CHUNK`] steps rather than forwards
/// from the start: the file may be 10MB and the answer is in the last few
/// hundred bytes of it.
fn tail_of_file(file: &mut File, want: usize) -> io::Result<(Vec<u8>, usize, u64)> {
    let end = file.seek(SeekFrom::End(0))?;
    if want == 0 || end == 0 {
        return Ok((Vec::new(), 0, end));
    }

    let mut pos = end;
    let mut buf: Vec<u8> = Vec::new();
    loop {
        let chunk = TAIL_CHUNK.min(pos);
        pos -= chunk;
        let mut head = vec![0u8; chunk as usize];
        file.seek(SeekFrom::Start(pos))?;
        file.read_exact(&mut head)?;
        head.extend_from_slice(&buf);
        buf = head;
        // Strictly more than `want`, so that the partial line at the front of
        // the buffer -- the one this chunk cut in half -- is one we can drop
        // rather than one we have to print half of.
        if pos == 0 || count_lines(&buf) > want {
            break;
        }
    }

    let (bytes, lines) = last_lines(buf, want);
    Ok((bytes, lines, end))
}

/// The last `want` lines of a gzipped generation.
///
/// Forwards, through /bin/gzip, because a gzip stream cannot be read from the
/// end -- which is also why this keeps a ring of the last `want` lines instead
/// of the decompressed file: a generation is up to 10MB and there is no reason
/// for any of it but the tail to exist in this process at once.
///
/// Shelling out rather than linking a decompressor follows what init already
/// does: `crate::logrotate` writes these files with the same binary, and a
/// machine without it has uncompressed generations for this to read instead.
fn tail_of_gzip(path: &Path, want: usize) -> io::Result<(Vec<u8>, usize)> {
    let mut child = Command::new(GZIP)
        .arg("-cd")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let mut kept: VecDeque<Vec<u8>> = VecDeque::new();
    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    kept.push_back(line);
                    if kept.len() > want {
                        kept.pop_front();
                    }
                }
                Err(e) => {
                    let _ = child.wait();
                    return Err(e);
                }
            }
        }
    }

    // Reaped here and not left to the exit: this process may be following a
    // log for hours, and a zombie decompressor sitting in the table for all of
    // it is untidy in exactly the way an init system should not be. It is also
    // how a failed decompression is noticed at all, since stderr went to
    // /dev/null to keep it out of the log the person is reading.
    let status = child.wait()?;
    if !status.success() {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("{GZIP} exited with {status}"),
        ));
    }

    let lines = kept.len();
    let mut bytes = Vec::new();
    for line in kept {
        bytes.extend_from_slice(&line);
    }
    Ok((bytes, lines))
}

/// How many lines a buffer holds. A final line without a newline on the end is
/// still a line -- that is what a log being written to right now looks like.
fn count_lines(buf: &[u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }
    let newlines = buf.iter().filter(|b| **b == b'\n').count();
    if buf.ends_with(b"\n") {
        newlines
    } else {
        newlines + 1
    }
}

/// Keep the last `want` lines of a buffer, and say how many that turned out to
/// be.
fn last_lines(buf: Vec<u8>, want: usize) -> (Vec<u8>, usize) {
    let total = count_lines(&buf);
    if total <= want {
        return (buf, total);
    }

    // The newline that terminates the final line is not a boundary between two
    // lines, so it is not one to count.
    let mut end = buf.len();
    if buf.ends_with(b"\n") {
        end -= 1;
    }

    let mut seen = 0usize;
    let mut start = 0usize;
    for i in (0..end).rev() {
        if buf[i] == b'\n' {
            seen += 1;
            if seen == want {
                start = i + 1;
                break;
            }
        }
    }

    (buf[start..].to_vec(), want)
}

/// `<dir>/<name>.log` -> `<dir>/<name>.log.<n>`. Must match
/// `logrotate::generation`.
fn generation(path: &Path, n: u32) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{}.{}", name, n))
}

/// The same path with `.gz` on the end. Must match `logrotate::compressed`.
fn compressed(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{}.gz", name))
}

/// What one pass of a follow did.
#[derive(Debug, PartialEq)]
enum Step {
    /// Bytes were copied out. How many is not worth carrying: the caller's
    /// only question is whether to sleep before looking again.
    Wrote,
    /// Nothing new since last time.
    Idle,
    /// The file is shorter than where we had got to: it was rotated, and this
    /// pass has gone back to the start of it.
    Rotated,
}

/// Copy whatever is new out of a log, once.
///
/// This follows the *inode*, not the path, and that is the whole reason a
/// follow survives a rotation without reopening anything. `crate::logrotate`
/// rotates a service log by copying its contents to `<name>.log.1` and
/// truncating the original in place -- it has to, because the service is
/// holding an O_APPEND fd on that inode and a rename would leave it writing
/// into the rotated file forever. The same property is what this relies on
/// from the reading side: the file handle opened before the rotation is still
/// the file the service is writing to afterwards, so there is no reopen-by-path
/// dance here and no window in which the follower is watching a file nobody
/// writes to any more.
///
/// The one thing it has to notice is the truncation itself, which it does by
/// comparing where it had got to against how long the file now is. A log
/// deleted rather than rotated is not noticed at all, and is not meant to be:
/// this keeps reading the unlinked inode the service still has open, which is
/// where that service's output is still going. `tail -F` reopens by name
/// instead; that is a different promise, and the one it makes is the wrong one
/// here, because it would silently start following a file that the running
/// service is not writing to.
///
/// The gap: a service that writes more bytes than the follower is behind by,
/// within one [`FOLLOW_INTERVAL`] of a truncation, makes the file longer than
/// our offset again before this looks, and those lines are missed. It is the
/// same window `tail -f` has and it costs a fifth of a second of output from a
/// service that has just filled its 10MB log -- the rotated generation has
/// them, and `raven-rc logs` prints from it.
fn follow_step(file: &mut File, out: &mut dyn Write) -> io::Result<Step> {
    let mut buf = [0u8; READ_CHUNK];
    match file.read(&mut buf) {
        Ok(0) => {
            let pos = file.stream_position()?;
            let len = file.metadata()?.len();
            if len < pos {
                file.seek(SeekFrom::Start(0))?;
                return Ok(Step::Rotated);
            }
            Ok(Step::Idle)
        }
        Ok(n) => {
            out.write_all(&buf[..n])?;
            out.flush()?;
            Ok(Step::Wrote)
        }
        // A signal landing mid-read is not the end of the log.
        Err(e) if e.kind() == ErrorKind::Interrupted => Ok(Step::Idle),
        Err(e) => Err(e),
    }
}

/// Follow a log until the person stops watching. Never returns: ^C is the exit.
fn follow(path: &Path, file: &mut File, out: &mut dyn Write) -> ! {
    loop {
        match follow_step(file, out) {
            // More may already be waiting, so do not sleep on the way back.
            Ok(Step::Wrote) => continue,
            Ok(Step::Rotated) => {
                eprintln!(
                    "raven-rc: {} was rotated; following it from the start",
                    path.display()
                );
            }
            Ok(Step::Idle) => {}
            Err(e) if e.kind() == ErrorKind::BrokenPipe => process::exit(0),
            Err(e) => {
                eprintln!("raven-rc: cannot follow {}: {}", path.display(), e);
                process::exit(1);
            }
        }
        std::thread::sleep(FOLLOW_INTERVAL);
    }
}

/// Write log bytes out, treating a closed pipe as the end of the job.
///
/// `raven-rc logs dbus | head` closes the pipe after ten lines, and the
/// `println!` this would otherwise have been panics on that -- which in a
/// binary built with `panic = "abort"` means raven-rc dying with a signal and
/// a shell reporting it, for a person who got exactly what they asked for.
fn emit(out: &mut dyn Write, bytes: &[u8]) {
    match out.write_all(bytes) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::BrokenPipe => process::exit(0),
        Err(e) => {
            eprintln!("raven-rc: cannot write output: {e}");
            process::exit(1);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression this table exists to prevent.
    ///
    /// `reload` was added to the help text and to the server's dispatch, but
    /// not to the client's `match`, so `raven-rc reload` printed
    /// "Unknown command: reload" and then a menu listing `reload` three lines
    /// below it. The verb never reached the socket.
    #[test]
    fn reload_is_dispatchable() {
        assert_eq!(
            arity_of("reload"),
            Some(Arity::None),
            "raven-rc reload must dispatch, and must not demand a service name"
        );
    }

    /// Help text and dispatch now read the same table, so the only way they can
    /// disagree again is a verb that is handled outside it. `status` and `logs`
    /// are the two deliberate cases -- the first formats its own reply, the
    /// second answers from the filesystem without asking init at all -- and
    /// both are in the table so that they are still advertised. Anything else
    /// added to the `match` without a table entry would be advertised nowhere.
    #[test]
    fn every_advertised_verb_has_an_arity() {
        for (verb, _, help) in SERVICE_VERBS {
            assert!(
                arity_of(verb).is_some(),
                "{verb} is advertised but not dispatchable"
            );
            assert!(!help.is_empty(), "{verb} has no help text");
        }
    }

    /// `logs` is dispatched outside the `match arity_of(verb)` arm, like
    /// `status`, so the table is the only thing that advertises it. It must
    /// still demand a name: `raven-rc logs` with no service is a question with
    /// no subject, and print_usage's " NAME" comes from this entry.
    #[test]
    fn logs_is_advertised_and_wants_a_service_name() {
        assert_eq!(arity_of("logs"), Some(Arity::Required));
        assert!(
            SERVICE_VERBS
                .iter()
                .any(|(verb, _, help)| *verb == "logs" && help.contains("-f")),
            "the help for logs must mention -f; the operand column cannot show it"
        );
    }

    #[test]
    fn the_table_has_no_duplicates() {
        let mut seen: Vec<&str> = SERVICE_VERBS.iter().map(|(v, _, _)| *v).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(before, seen.len(), "duplicate verb in SERVICE_VERBS");
    }

    /// A service verb that needs a name must not be silently sent without one:
    /// init would answer "unknown command", which reads like the verb is wrong
    /// rather than the invocation.
    #[test]
    fn service_verbs_require_a_name() {
        for verb in ["start", "stop", "restart", "enable", "disable"] {
            assert_eq!(arity_of(verb), Some(Arity::Required), "{verb}");
        }
        for verb in ["list", "reload", "reexec"] {
            assert_eq!(arity_of(verb), Some(Arity::None), "{verb}");
        }
    }

    // -----------------------------------------------------------------------
    // logs
    // -----------------------------------------------------------------------

    /// A log directory of our own, so no test needs /var/log and no two tests
    /// can see each other's files. Same shape as logrotate.rs's `dir`, and for
    /// the same reason: an environment variable is process-global and `cargo
    /// test` runs these in threads.
    fn dir(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("raven-rc-logs-{}-{}", tag, process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");
        root
    }

    fn operands(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    /// Both spellings people actually type. `tail -f logfile` puts the flag
    /// first and `journalctl -u foo -f` puts it last, and a person who has
    /// just been using one of those should not be told off by the other.
    #[test]
    fn the_follow_flag_may_come_before_or_after_the_service() {
        let expected = LogsRequest {
            name: "dbus".to_string(),
            follow: true,
        };
        assert_eq!(parse_logs_args(&operands(&["dbus", "-f"])), Ok(expected));
        assert_eq!(
            parse_logs_args(&operands(&["-f", "dbus"])),
            Ok(LogsRequest {
                name: "dbus".to_string(),
                follow: true
            })
        );
        assert_eq!(
            parse_logs_args(&operands(&["--follow", "dbus"])),
            Ok(LogsRequest {
                name: "dbus".to_string(),
                follow: true
            })
        );
        assert_eq!(
            parse_logs_args(&operands(&["dbus"])),
            Ok(LogsRequest {
                name: "dbus".to_string(),
                follow: false
            })
        );
    }

    /// Each of these is a different mistake and each gets its own sentence,
    /// because "usage:" on its own does not tell somebody which half of what
    /// they typed was wrong.
    #[test]
    fn a_malformed_logs_invocation_says_which_part_is_wrong() {
        let no_name = parse_logs_args(&operands(&["-f"])).expect_err("no service named");
        assert!(no_name.contains("needs a service name"), "{no_name}");

        let unknown = parse_logs_args(&operands(&["dbus", "-n", "5"])).expect_err("no -n yet");
        assert!(unknown.contains("unknown option '-n'"), "{unknown}");

        let two = parse_logs_args(&operands(&["dbus", "cawd"])).expect_err("two services");
        assert!(two.contains("one service at a time"), "{two}");

        // The name becomes a path; a name that leaves the log directory is not
        // a service, and saying "no such service" is both true and the same
        // answer init gives over the socket.
        for escape in ["../../etc/shadow", "/etc/shadow", ".hidden"] {
            let refused = parse_logs_args(&operands(&[escape])).expect_err("a name that escapes the log directory is refused");
            assert!(refused.contains("no such service"), "{refused}");
        }
    }

    /// The trap this function exists to avoid: a session's raven-init exports
    /// RAVEN_SERVICE_LOG_DIR to everything it starts, terminals included, so
    /// reading it for the system target would make `raven-rc logs dbus` in a
    /// desktop terminal answer about the session's dbus rather than the
    /// machine's.
    #[test]
    fn the_system_log_directory_is_never_taken_from_the_session() {
        let resolved = resolve_log_dir(
            false,
            Some(OsString::from("/home/somebody/.local/state/raven/log")),
            Some(OsString::from("/home/somebody/.local/state")),
            Some(OsString::from("/home/somebody")),
        );
        assert_eq!(resolved, Ok(PathBuf::from(SYSTEM_LOG_DIR)));
    }

    /// Under --user the same variable IS believed: there it is what the
    /// session's supervisor set, which is the authoritative answer. Failing
    /// that, the same XDG computation usermode::Paths::from_env does.
    #[test]
    fn a_session_log_directory_follows_the_supervisor_then_xdg() {
        assert_eq!(
            resolve_log_dir(true, Some(OsString::from("/run/user/1000/logs")), None, None),
            Ok(PathBuf::from("/run/user/1000/logs"))
        );
        assert_eq!(
            resolve_log_dir(
                true,
                None,
                Some(OsString::from("/home/somebody/.local/state")),
                Some(OsString::from("/home/somebody"))
            ),
            Ok(PathBuf::from("/home/somebody/.local/state/raven/log"))
        );
        assert_eq!(
            resolve_log_dir(true, None, None, Some(OsString::from("/home/somebody"))),
            Ok(PathBuf::from("/home/somebody/.local/state/raven/log"))
        );
        assert!(resolve_log_dir(true, None, None, None).is_err());
    }

    #[test]
    fn a_log_shorter_than_the_tail_is_printed_whole() {
        let root = dir("short");
        let log = root.join("quiet.log");
        fs::write(&log, "one\ntwo\n").expect("write");

        let mut file = File::open(&log).expect("open");
        let (bytes, lines, end) = tail_of_file(&mut file, TAIL_LINES).expect("tail");
        assert_eq!(bytes, b"one\ntwo\n");
        assert_eq!(lines, 2);
        assert_eq!(end, 8);

        let _ = fs::remove_dir_all(&root);
    }

    /// The point of reading backwards. A log is capped at 10MB and the answer
    /// to "why did it just die" is at the end of it.
    #[test]
    fn only_the_last_lines_of_a_long_log_are_read() {
        let root = dir("long");
        let log = root.join("chatty.log");
        let body: String = (0..5_000).map(|n| format!("line {n}\n")).collect();
        fs::write(&log, &body).expect("write");

        let mut file = File::open(&log).expect("open");
        let (bytes, lines, _) = tail_of_file(&mut file, TAIL_LINES).expect("tail");
        assert_eq!(lines, TAIL_LINES);
        let text = String::from_utf8(bytes).expect("utf8");
        assert!(text.starts_with("line 4950\n"), "{}", &text[..20]);
        assert!(text.ends_with("line 4999\n"));
        // Whole lines only: the chunk boundary must never be printed as half
        // a line.
        assert_eq!(text.lines().count(), TAIL_LINES);

        let _ = fs::remove_dir_all(&root);
    }

    /// A log being written to right now ends mid-line, and that half-written
    /// line is the most recent thing the service said.
    #[test]
    fn a_line_with_no_newline_yet_is_still_shown() {
        let root = dir("partial");
        let log = root.join("writing.log");
        fs::write(&log, "done\nin progress").expect("write");

        let mut file = File::open(&log).expect("open");
        let (bytes, lines, _) = tail_of_file(&mut file, TAIL_LINES).expect("tail");
        assert_eq!(bytes, b"done\nin progress");
        assert_eq!(lines, 2);

        let _ = fs::remove_dir_all(&root);
    }

    /// The case the rotation created: a service that has just filled its log
    /// has almost nothing in the current file, and everything a person is
    /// asking about is in the generation beside it.
    #[test]
    fn the_tail_continues_into_the_rotated_generation() {
        let root = dir("generations");
        let log = root.join("noisy.log");
        fs::write(root.join("noisy.log.1"), "a\nb\nc\n").expect("write");
        fs::write(&log, "d\ne\n").expect("write");

        let mut file = File::open(&log).expect("open");
        let tail = read_tail(&mut file, &log, 4).expect("tail");
        assert!(tail.trouble.is_empty(), "{:?}", tail.trouble);
        assert_eq!(tail.bytes, b"b\nc\nd\ne\n");
        assert_eq!(tail.lines, 4);
        // `end` is the current file, not the history: it is where a follow
        // picks up.
        assert_eq!(tail.end, 4);

        // And it stops at the first missing generation rather than reading
        // files that only happen to be named like this log's.
        fs::write(root.join("noisy.log.3"), "older\n").expect("write");
        let mut file = File::open(&log).expect("open");
        let tail = read_tail(&mut file, &log, TAIL_LINES).expect("tail");
        assert_eq!(tail.bytes, b"a\nb\nc\nd\ne\n");

        let _ = fs::remove_dir_all(&root);
    }

    /// Skipped, rather than failed, where there is no /bin/gzip: a machine
    /// without it is a supported configuration -- logrotate leaves its
    /// generations uncompressed there -- and a test that fails on one is
    /// testing the build host.
    #[test]
    fn a_compressed_generation_is_read_through_gzip() {
        if !Path::new(GZIP).exists() {
            return;
        }
        let root = dir("gz");
        let log = root.join("verbose.log");
        let older = root.join("verbose.log.1");
        fs::write(&older, "a\nb\nc\n").expect("write");
        fs::write(&log, "d\n").expect("write");

        let status = Command::new(GZIP)
            .arg(&older)
            .status()
            .expect("run gzip");
        assert!(status.success());
        assert!(root.join("verbose.log.1.gz").exists());

        let mut file = File::open(&log).expect("open");
        let tail = read_tail(&mut file, &log, 3).expect("tail");
        assert!(tail.trouble.is_empty(), "{:?}", tail.trouble);
        assert_eq!(tail.bytes, b"b\nc\nd\n");

        let _ = fs::remove_dir_all(&root);
    }

    /// THE property `logs -f` depends on, from the reading side.
    ///
    /// logrotate rotates a service log by copying it away and truncating the
    /// original in place, because the service is holding an O_APPEND fd on
    /// that inode. This asserts the follower survives that: the handle it
    /// opened before the rotation is still the file the service writes to
    /// after it, and the only thing the follower has to notice is that the
    /// file got shorter. If anybody ever turns the rotation into a rename,
    /// this is the test that says the follow broke with it.
    #[test]
    fn a_follow_survives_a_rotation_because_the_inode_does() {
        use std::os::unix::fs::MetadataExt as _;

        let root = dir("follow");
        let log = root.join("busy.log");
        let body: String = (0..200).map(|n| format!("before {n}\n")).collect();
        fs::write(&log, &body).expect("write");

        // Exactly what open_log hands a child: an O_APPEND handle on the path.
        let mut held = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .expect("service handle");
        let mut follower = File::open(&log).expect("follower");
        let inode_before = follower.metadata().expect("stat").ino();

        // Catch up to the end, the way do_logs does before it follows.
        let mut seen: Vec<u8> = Vec::new();
        while follow_step(&mut follower, &mut seen).expect("step") != Step::Idle {}
        assert_eq!(seen.len(), body.len());

        // Rotate it the way logrotate does: copy the contents out, then empty
        // the file in place. No rename, and no new inode.
        fs::copy(&log, root.join("busy.log.1")).expect("copy");
        let truncating = fs::OpenOptions::new()
            .write(true)
            .open(&log)
            .expect("open for truncate");
        truncating.set_len(0).expect("truncate");

        // The service, which never noticed any of that, carries on writing.
        held.write_all(b"after the rotation\n").expect("append");

        assert_eq!(
            follow_step(&mut follower, &mut seen).expect("step"),
            Step::Rotated,
            "a file shorter than where we had got to is a rotation"
        );
        assert_eq!(
            follow_step(&mut follower, &mut seen).expect("step"),
            Step::Wrote
        );
        assert!(
            String::from_utf8_lossy(&seen).ends_with("after the rotation\n"),
            "the follow must keep reading the same inode the service writes to"
        );
        assert_eq!(
            fs::metadata(&log).expect("stat").ino(),
            inode_before,
            "the rotation must not replace the file"
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// The naming scheme is logrotate's, redeclared here because rc.rs is a
    /// separate binary and there is no shared crate to put it in. If these
    /// ever disagree, `raven-rc logs` silently stops finding the history.
    #[test]
    fn generation_names_match_logrotates() {
        let log = Path::new("/var/log/raven/dbus.log");
        assert_eq!(
            generation(log, 1),
            PathBuf::from("/var/log/raven/dbus.log.1")
        );
        assert_eq!(
            compressed(&generation(log, 2)),
            PathBuf::from("/var/log/raven/dbus.log.2.gz")
        );
    }
}
