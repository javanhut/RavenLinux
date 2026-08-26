//! Control socket for RavenInit
//!
//! Gives raven-rc a way to ask PID 1 about services and to start, stop and
//! restart them. Before this existed the only channel was `/run/raven-init.cmd`
//! -- a file holding one word, with no reply -- which was enough for "power
//! off" and nothing else.
//!
//! The protocol is one line of request, then response text until the server
//! closes the stream. Text rather than a serialisation format because the
//! consumers are raven-rc and whoever is holding a shell at three in the
//! morning; `socat - UNIX-CONNECT:/run/raven-init.sock` should be a usable
//! client.
//!
//! # Running inside PID 1
//!
//! Everything here is written on the assumption that a badly behaved client
//! must not be able to wedge init:
//!
//!   * the listener is non-blocking, and the poll drains ready connections and
//!     returns rather than waiting for one;
//!   * every accepted stream carries read and write timeouts, so a client that
//!     connects and says nothing costs one timeout, not a hung machine;
//!   * requests are length-capped before they are parsed.
//!
//! The socket is mode 0600. It starts and stops services, so it is root-only.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::config::InitConfig;
use crate::service::{Service, ServiceState};

/// Where raven-rc looks for us.
pub const SOCKET_PATH: &str = "/run/raven-init.sock";

/// Longest request we will read. Generous for `restart some-service-name`.
const MAX_REQUEST: usize = 1024;

/// How long a single client is allowed to take. Deliberately short: this runs
/// on PID 1's thread, and the main loop still has services to supervise.
const CLIENT_TIMEOUT: Duration = Duration::from_millis(200);

/// What the caller should do after a request is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nothing further; the request was served in full.
    None,
    /// Begin an orderly shutdown and power off.
    Poweroff,
    /// Begin an orderly shutdown and reboot.
    Reboot,
}

/// Create the control socket at [`SOCKET_PATH`], replacing any stale one.
pub fn listen() -> Result<UnixListener> {
    listen_at(SOCKET_PATH)
}

/// Create the control socket at an arbitrary path.
///
/// Takes the path so tests can bind somewhere writable; PID 1 always uses
/// [`listen`].
pub fn listen_at(path: &str) -> Result<UnixListener> {
    // A socket file left by a previous boot would make bind() fail with
    // EADDRINUSE. Nothing else owns this path, so removing it is safe.
    if Path::new(path).exists() {
        std::fs::remove_file(path).ok();
    }

    let listener = UnixListener::bind(path).with_context(|| format!("Failed to bind {}", path))?;

    listener
        .set_nonblocking(true)
        .context("Failed to set the control socket non-blocking")?;

    // Root only: this interface starts and stops system services.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).ok();

    log::info!("Control socket listening on {}", path);
    Ok(listener)
}

/// Serve whatever clients are already waiting, then return.
///
/// Called once per main-loop tick. Never blocks: an empty accept queue returns
/// `Action::None` immediately.
pub fn poll(
    listener: &UnixListener,
    services: &mut HashMap<String, Service>,
    config: &mut InitConfig,
) -> Action {
    let mut action = Action::None;

    loop {
        match listener.accept() {
            Ok((stream, _)) => match serve_one(stream, services, &mut *config) {
                Ok(Action::None) => {}
                Ok(other) => action = other,
                Err(e) => log::warn!("Control connection failed: {}", e),
            },
            // Nothing else queued.
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => {
                log::warn!("Control socket accept failed: {}", e);
                break;
            }
        }
    }

    action
}

fn serve_one(
    stream: UnixStream,
    services: &mut HashMap<String, Service>,
    config: &mut InitConfig,
) -> Result<Action> {
    stream.set_read_timeout(Some(CLIENT_TIMEOUT)).ok();
    stream.set_write_timeout(Some(CLIENT_TIMEOUT)).ok();

    let mut reader = BufReader::new(stream.try_clone().context("Failed to clone stream")?);
    let mut line = String::new();

    // take() caps the request before it reaches the parser, so a client that
    // opens a connection and streams forever cannot grow init's memory.
    {
        let mut limited = (&mut reader).take(MAX_REQUEST as u64);
        limited
            .read_line(&mut line)
            .context("Failed to read request")?;
    }

    let (reply, action) = dispatch(line.trim(), services, config);

    let mut out = stream;
    out.write_all(reply.as_bytes()).ok();
    out.flush().ok();
    // Dropping `out` closes the stream, which is what ends the response.

    Ok(action)
}

/// Turn one request line into a reply and an action.
///
/// Split out from the I/O so it can be tested without a socket.
pub fn dispatch(
    request: &str,
    services: &mut HashMap<String, Service>,
    config: &mut InitConfig,
) -> (String, Action) {
    let mut parts = request.split_whitespace();
    let verb = parts.next().unwrap_or("");
    let target = parts.next();

    match verb {
        "list" => (list_services(services, config), Action::None),

        "status" => match target {
            Some(name) => (status_one(name, services, config), Action::None),
            // Bare `status` means the whole system, which is what an operator
            // asking "what is going on" wants.
            None => (list_services(services, config), Action::None),
        },

        "enable" => with_target(target, verb, |name| set_enabled(name, true, config)),
        "disable" => with_target(target, verb, |name| set_enabled(name, false, config)),

        "start" => with_target(target, verb, |name| start_service(name, services, config)),
        "stop" => with_target(target, verb, |name| stop_service(name, services)),
        "restart" => with_target(target, verb, |name| restart_service(name, services, config)),

        "poweroff" | "halt" => ("Powering off\n".to_string(), Action::Poweroff),
        "reboot" => ("Rebooting\n".to_string(), Action::Reboot),

        "" => ("error: empty request\n".to_string(), Action::None),
        other => (
            format!(
                "error: unknown command '{}'\n\
                 commands: list, status [NAME], start NAME, stop NAME, restart NAME, \
                 enable NAME, disable NAME, poweroff, reboot, halt\n",
                other
            ),
            Action::None,
        ),
    }
}

/// Shared "this verb needs a service name" handling.
fn with_target<F>(target: Option<&str>, verb: &str, f: F) -> (String, Action)
where
    F: FnOnce(&str) -> String,
{
    match target {
        Some(name) => (f(name), Action::None),
        None => (
            format!("error: {} needs a service name\n", verb),
            Action::None,
        ),
    }
}

/// How a service should be described to an operator.
///
/// `ServiceState` alone is not enough: a service an operator stopped and one
/// that exited on its own are both `Exited`, and calling the first "exited"
/// invites the question of why it has not come back.
fn describe_state(svc: &Service) -> &'static str {
    if svc.is_running() {
        "running"
    } else if svc.is_manually_stopped() {
        "stopped (by request)"
    } else {
        match svc.state() {
            ServiceState::Running => "running",
            ServiceState::Exited => "exited",
            ServiceState::Signaled => "killed",
            ServiceState::Stopped => "stopped",
            ServiceState::Failed => "failed",
        }
    }
}

fn list_services(services: &HashMap<String, Service>, config: &InitConfig) -> String {
    // STATE is what the service is doing now; BOOT is what the config says
    // should happen next time. Those were one column before, which made
    // "disabled" mean both "not running" and "not started at boot" and left
    // `enable` with nothing visible to change.
    let mut out =
        String::from("SERVICE              STATE                 PID      BOOT      DESCRIPTION\n");

    // Config order, not HashMap order: an operator reading this twice should
    // see the same list in the same sequence.
    let mut seen = Vec::new();
    for cfg in &config.services {
        seen.push(cfg.name.as_str());
        let boot = if cfg.enabled { "enabled" } else { "disabled" };
        match services.get(&cfg.name) {
            Some(svc) => out.push_str(&format_row(
                &cfg.name,
                describe_state(svc),
                svc.pid().map(|p| p.as_raw()),
                boot,
                svc.description(),
            )),
            None => out.push_str(&format_row(
                &cfg.name,
                "stopped",
                None,
                boot,
                &cfg.description,
            )),
        }
    }

    // Anything running that the config does not mention -- the fallback getty
    // start_services() synthesises when no service is configured.
    let mut extras: Vec<_> = services
        .iter()
        .filter(|(name, _)| !seen.contains(&name.as_str()))
        .collect();
    extras.sort_by_key(|(name, _)| name.to_string());
    for (name, svc) in extras {
        out.push_str(&format_row(
            name,
            describe_state(svc),
            svc.pid().map(|p| p.as_raw()),
            "-",
            svc.description(),
        ));
    }

    out
}

fn format_row(name: &str, state: &str, pid: Option<i32>, boot: &str, description: &str) -> String {
    let pid = pid
        .map(|p| p.to_string())
        .unwrap_or_else(|| "-".to_string());
    format!(
        "{:<20} {:<21} {:<8} {:<9} {}\n",
        name, state, pid, boot, description
    )
}

fn status_one(name: &str, services: &HashMap<String, Service>, config: &InitConfig) -> String {
    let cfg = config.services.iter().find(|c| c.name == name);

    let Some(svc) = services.get(name) else {
        return match cfg {
            Some(cfg) => format!(
                "{}\n  state        stopped\n  boot         {}\n  description  {}\n  exec         {}\n",
                name,
                if cfg.enabled { "enabled" } else { "disabled" },
                cfg.description,
                cfg.exec
            ),
            None => format!("error: no such service '{}'\n", name),
        };
    };

    let mut out = format!("{}\n", name);
    out.push_str(&format!("  state        {}\n", describe_state(svc)));
    out.push_str(&format!(
        "  boot         {}\n",
        match cfg {
            Some(c) if c.enabled => "enabled",
            Some(_) => "disabled",
            // Running but absent from the config: the synthesised fallback getty.
            None => "not in config",
        }
    ));
    out.push_str(&format!("  description  {}\n", svc.description()));

    match svc.pid() {
        Some(pid) => out.push_str(&format!("  pid          {}\n", pid)),
        None => out.push_str("  pid          -\n"),
    }

    if let Some(code) = svc.exit_status() {
        if !svc.is_running() {
            out.push_str(&format!("  exit status  {}\n", code));
        }
    }

    out.push_str(&format!(
        "  restarts     {}{}\n",
        svc.restart_count(),
        if svc.restart_configured() {
            ""
        } else {
            " (restart disabled in config)"
        }
    ));

    if let Some(cfg) = cfg {
        out.push_str(&format!("  exec         {}\n", cfg.exec));
    }

    out
}

fn start_service(
    name: &str,
    services: &mut HashMap<String, Service>,
    config: &InitConfig,
) -> String {
    let Some(service_config) = config.services.iter().find(|c| c.name == name).cloned() else {
        return format!("error: no such service '{}'\n", name);
    };

    // Starting a unit manually must honor the same direct dependencies as
    // boot. Previously `raven-rc start polkitd` could report success while
    // D-Bus was stopped, leaving the new process to fail immediately.
    for dependency in &service_config.after {
        let running = services
            .get_mut(dependency)
            .map(|svc| {
                svc.poll_exit();
                svc.is_running()
            })
            .unwrap_or(false);
        if !running {
            let reply = start_service_raw(dependency, services, config);
            if reply.starts_with("error:") {
                return format!(
                    "error: cannot start {}: dependency {}: {}",
                    name, dependency, reply
                );
            }
        }

        if let Some(dep_cfg) = config.services.iter().find(|c| &c.name == dependency) {
            if let Some(path) = dep_cfg.ready_path.as_deref() {
                let deadline =
                    std::time::Instant::now() + Duration::from_secs(dep_cfg.ready_timeout as u64);
                while std::time::Instant::now() < deadline && !std::path::Path::new(path).exists() {
                    std::thread::sleep(Duration::from_millis(20));
                }
                if !std::path::Path::new(path).exists() {
                    return format!(
                        "error: cannot start {}: dependency {} was not ready at {}\n",
                        name, dependency, path
                    );
                }
            }
        }
    }

    start_service_raw(name, services, config)
}

fn start_service_raw(
    name: &str,
    services: &mut HashMap<String, Service>,
    config: &InitConfig,
) -> String {
    if let Some(svc) = services.get_mut(name) {
        // Reap first: without this a start issued inside the reaper's 100ms
        // window sees a corpse as a running service and declines to act.
        svc.poll_exit();

        if svc.is_running() {
            return format!("{} is already running\n", name);
        }
        return match svc.start_by_request() {
            Ok(()) => confirm_started(name, svc),
            Err(e) => format!("error: failed to start {}: {:#}\n", name, e),
        };
    }

    // Not in the running set: a service the config disables at boot. Starting
    // it on request is exactly what `enabled = false` should permit -- it means
    // "not automatically", not "never".
    let Some(cfg) = config.services.iter().find(|c| c.name == name) else {
        // The base image defines no services for software it does not ship,
        // but it does carry templates for the daemons people install first.
        // "No such service" with the fix in hand beats making them hunt.
        // rvn copies the template in automatically when it installs the
        // matching binary, so landing here usually means the software is not
        // installed yet -- or was put on disk by something other than rvn.
        let template = format!("/usr/share/raven/services/{}.toml", name);
        if std::path::Path::new(&template).exists() {
            return format!(
                "error: no such service '{}'\n\
                 Its software is not installed. `rvn install` sets the service\n\
                 up with it; installed some other way, copy the definition:\n\
                 \x20 cp {} /etc/raven/init.d/\n",
                name, template
            );
        }
        return format!("error: no such service '{}'\n", name);
    };

    match Service::start(cfg) {
        Ok(mut svc) => {
            let reply = confirm_started(name, &mut svc);
            services.insert(name.to_string(), svc);
            reply
        }
        Err(e) => format!("error: failed to start {}: {:#}\n", name, e),
    }
}

/// How long to watch a just-started service before calling it started.
///
/// spawn() succeeding only means fork/exec worked; a daemon that finds its
/// socket already held exits milliseconds later. Reporting "Started cawd" and
/// then watching the supervisor restart-loop it into the ground is the worst
/// of both -- the operator is told it worked and the log says otherwise.
const START_GRACE: Duration = Duration::from_millis(200);

fn confirm_started(name: &str, svc: &mut Service) -> String {
    let deadline = std::time::Instant::now() + START_GRACE;
    while std::time::Instant::now() < deadline {
        svc.poll_exit();
        if !svc.is_running() {
            let how = match svc.exit_status() {
                Some(code) => format!("exited with status {}", code),
                None => "was killed".to_string(),
            };
            return format!(
                "error: {} started but {} immediately.\n\
                 Its own output above says why; `raven-rc status {}` has the rest.\n",
                name, how, name
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    format!("Started {}\n", name)
}

fn stop_service(name: &str, services: &mut HashMap<String, Service>) -> String {
    let Some(svc) = services.get_mut(name) else {
        return format!("error: no such service '{}'\n", name);
    };

    svc.poll_exit();

    if !svc.is_running() {
        return format!("{} is not running\n", name);
    }

    svc.stop_by_request();
    // Deliberately not waiting for the process to die: SIGTERM is asynchronous
    // and PID 1 has services to supervise. `status` will show it settle.
    format!("Stopping {}\n", name)
}

fn restart_service(
    name: &str,
    services: &mut HashMap<String, Service>,
    config: &InitConfig,
) -> String {
    let Some(svc) = services.get_mut(name) else {
        // Restarting something that is not running is a start.
        return start_service(name, services, config);
    };

    if svc.is_running() {
        svc.stop_by_request();
        // Must actually be gone before the replacement starts -- see
        // Service::wait_for_exit. Costs PID 1 up to this long on an explicit
        // operator command, which is the one case where that is acceptable.
        svc.wait_for_exit(Duration::from_secs(5));
    }

    match svc.start_by_request() {
        Ok(()) => format!("Restarted {}\n", name),
        Err(e) => format!("error: failed to restart {}: {:#}\n", name, e),
    }
}

// ---------------------------------------------------------------------------
// enable / disable
// ---------------------------------------------------------------------------
// These change what happens at *boot*, not what is running now -- the same
// split systemd draws, and for the same reason: "start it now" and "start it
// every time" are different decisions and conflating them surprises people in
// both directions.

/// Flip a service's `enabled` flag, in memory and on disk.
fn set_enabled(name: &str, enabled: bool, config: &mut InitConfig) -> String {
    let verb = if enabled { "enable" } else { "disable" };

    let Some(svc) = config.services.iter_mut().find(|c| c.name == name) else {
        return format!("error: no such service '{}'\n", name);
    };

    let was = svc.enabled;
    svc.enabled = enabled;

    let Some(path) = config.source_path.clone() else {
        // Restore: refusing the request but leaving the flag flipped would make
        // the in-memory state a lie the next `status` would tell.
        if let Some(svc) = config.services.iter_mut().find(|c| c.name == name) {
            svc.enabled = was;
        }
        return format!(
            "error: cannot {} {}: running on built-in defaults, there is no \
             config file to write\n",
            verb, name
        );
    };

    let past = if enabled { "Enabled" } else { "Disabled" };

    match persist_enabled_wherever_defined(&path, name, enabled) {
        Ok(()) => {
            let note = if was == enabled {
                format!(" (was already {}d)", verb)
            } else {
                String::new()
            };
            format!(
                "{} {}{}\nTakes effect at boot; use `start`/`stop` to change it now.\n",
                past, name, note
            )
        }
        Err(e) => {
            if let Some(svc) = config.services.iter_mut().find(|c| c.name == name) {
                svc.enabled = was;
            }
            format!("error: cannot {} {}: {:#}\n", verb, name, e)
        }
    }
}

/// Rewrite a service's `enabled` key in whichever file defines it.
///
/// init.toml is tried first; a service it does not name was folded in from a
/// drop-in under /etc/raven/init.d, so the rewrite goes to the drop-in that
/// defines it. Writing the flag into init.toml instead would work once and
/// then leave two files disagreeing about the same service.
fn persist_enabled_wherever_defined(
    main: &std::path::Path,
    name: &str,
    enabled: bool,
) -> std::io::Result<()> {
    match persist_enabled(main, name, enabled) {
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        other => return other,
    }

    let dir = std::env::var_os("RAVEN_INIT_DROPIN_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/etc/raven/init.d"));

    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
                .collect()
        })
        .unwrap_or_default();
    paths.sort();

    for path in paths {
        match persist_enabled(&path, name, enabled) {
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) if e.kind() == ErrorKind::InvalidData => continue,
            other => return other,
        }
    }

    Err(std::io::Error::new(
        ErrorKind::NotFound,
        format!(
            "'{}' is not defined in {} or any drop-in under {}",
            name,
            main.display(),
            dir.display()
        ),
    ))
}

/// Rewrite one service's `enabled` key in the config file.
///
/// Uses toml_edit rather than re-serialising the parsed config: init.toml ships
/// with comments explaining why iwd is disabled and what each service is for,
/// and a round-trip through serde would silently delete all of it.
///
/// The write is atomic -- temp file in the same directory, then rename. A
/// half-written init.toml is not a cosmetic problem: it fails to parse at next
/// boot, load_config falls back to built-in defaults, and the machine comes up
/// with the wrong services and no explanation.
fn persist_enabled(path: &std::path::Path, name: &str, enabled: bool) -> std::io::Result<()> {
    let text = std::fs::read_to_string(path)?;

    let mut doc = text.parse::<toml_edit::DocumentMut>().map_err(|e| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("{} is not valid TOML: {}", path.display(), e),
        )
    })?;

    let services = doc
        .get_mut("services")
        .and_then(|s| s.as_array_of_tables_mut())
        .ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                format!("{} has no [[services]] array", path.display()),
            )
        })?;

    let mut found = false;
    for table in services.iter_mut() {
        if table.get("name").and_then(|n| n.as_str()) == Some(name) {
            table["enabled"] = toml_edit::value(enabled);
            found = true;
            break;
        }
    }

    if !found {
        return Err(std::io::Error::new(
            ErrorKind::NotFound,
            format!(
                "no [[services]] entry named '{}' in {}",
                name,
                path.display()
            ),
        ));
    }

    write_atomic(path, doc.to_string().as_bytes())
}

/// Replace a file's contents without ever leaving it partially written.
fn write_atomic(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;

    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("/"));
    // Same directory, so the rename below is within one filesystem and
    // therefore atomic.
    let tmp = dir.join(format!(
        ".{}.raven-init.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("config")
    ));

    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(contents)?;
        // Durability before visibility: rename is atomic, but without this the
        // rename can land while the contents are still only in page cache.
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();

    if result.is_err() {
        // Do not leave debris behind on a read-only or full filesystem.
        std::fs::remove_file(&tmp).ok();
    }

    result
}
