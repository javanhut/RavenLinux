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
//!
//! # Reading without root
//!
//! `list` and `status NAME` are questions, not orders, and an operator with
//! a plain shell should not need sudo to ask them. Rather than open the
//! socket to everyone and sort callers by uid -- which would still hand an
//! unprivileged session a socket into PID 1, the thing ARCHITECTURE.md says
//! it must never hold -- init publishes the *answers*: [`StatusPublisher`]
//! writes the text `list` and `status NAME` would return to files under
//! [`STATUS_DIR`], mode 0644, and rewrites each one only when its text
//! changes. raven-rc reads those when it is not root. Nothing flows the other
//! way: the files are output, the socket is the only input, and it stays
//! root-only.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::config::InitConfig;
use crate::service::{Service, ServiceState};

/// Where raven-rc looks for us.
pub const SOCKET_PATH: &str = "/run/raven-init.sock";

/// Set by `raven-init --user`: this supervisor owns a session, not the
/// machine, so the verbs that act on the machine are refused with a pointer
/// to the system raven-rc rather than answered with a lie.
static USER_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_user_mode(on: bool) {
    USER_MODE.store(on, std::sync::atomic::Ordering::SeqCst);
}

pub fn user_mode() -> bool {
    USER_MODE.load(std::sync::atomic::Ordering::SeqCst)
}

/// Where init publishes the text of `list` and `status NAME` for readers
/// without root: `status` holds the list, `services/NAME` one service each,
/// `blame` the boot timeline.
/// Must match the constant of the same name in rc.rs.
pub const STATUS_DIR: &str = "/run/raven-init";

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
    /// Suspend to RAM, and carry on once the machine is awake again.
    ///
    /// Unlike the two above this is not a shutdown: the main loop performs it
    /// inline and then keeps supervising the same services it had before. The
    /// reply is written and the connection closed *before* it happens, because
    /// a client blocked on a socket read for the length of a suspend would look
    /// like a hang and, worse, would still be waiting on a machine that may
    /// never come back.
    Suspend,
    /// Replace PID 1 with the raven-init binary on disk, keeping every
    /// service running. See `crate::reexec`.
    ///
    /// Like `Suspend`, the reply goes out before anything happens: the exec
    /// closes the control socket, and a client still waiting on it would see
    /// a hang where the operation had actually succeeded.
    Reexec,
}

/// Whether a raven-init is answering on the control socket at `path`.
///
/// A connection is enough to tell: the server accepts it, reads an empty
/// request and closes, all inside one poll tick, so asking costs it nothing.
/// A leftover socket file with no listener behind it refuses the connection.
pub fn is_live(path: &Path) -> bool {
    UnixStream::connect(path).is_ok()
}

/// Create the control socket at an arbitrary path.
///
/// Takes the path so tests can bind somewhere writable; PID 1 always uses
/// [`listen`].
pub fn listen_at(path: &str) -> Result<UnixListener> {
    // A socket file left by a previous boot, or by a supervisor that has
    // since exited, would make bind() fail with EADDRINUSE. Nothing answers
    // on it, so removing it is safe. One that still answers is another
    // raven-init's: taking the path would leave that one running but
    // unreachable, so it is refused instead.
    if Path::new(path).exists() {
        if is_live(Path::new(path)) {
            anyhow::bail!("{} is held by a running raven-init", path);
        }
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

    if user_mode()
        && matches!(
            verb,
            "poweroff" | "halt" | "reboot" | "suspend" | "sleep" | "reexec"
        )
    {
        return (
            format!("error: '{verb}' acts on the machine, and this raven-init supervises a session; use raven-rc without --user\n"),
            Action::None,
        );
    }

    match verb {
        "list" => (list_services(services, config), Action::None),

        "status" => match target {
            // A question asked over the socket is asked once, by a person
            // waiting for the answer, so it is worth reading the live
            // resource counters for -- unlike the published copy below.
            Some(name) => (
                status_one(name, services, config, Counters::Read),
                Action::None,
            ),
            // Bare `status` means the whole system, which is what an operator
            // asking "what is going on" wants.
            None => (list_services(services, config), Action::None),
        },

        "enable" => with_target(target, verb, |name| set_enabled(name, true, config)),
        "disable" => with_target(target, verb, |name| set_enabled(name, false, config)),

        "blame" => (
            blame_services(
                services,
                config,
                crate::timeline::first_milestone("main loop"),
            ),
            Action::None,
        ),

        "reload" => (reload_config(services, config), Action::None),

        "start" => with_target(target, verb, |name| start_service(name, services, config)),
        "stop" => with_target(target, verb, |name| stop_service(name, services)),
        "restart" => with_target(target, verb, |name| restart_service(name, services, config)),

        "poweroff" | "halt" => ("Powering off\n".to_string(), Action::Poweroff),
        "reboot" => ("Rebooting\n".to_string(), Action::Reboot),
        // `sleep` as well, because half the world's laptops call it that and
        // an operator guessing wrong at three in the morning should still get
        // the machine to sleep.
        "suspend" | "sleep" => ("Suspending\n".to_string(), Action::Suspend),

        // Checked here, while there is still a client to tell. Once the main
        // loop acts on it the only place a failure can be reported is the
        // log, and "raven-rc reexec said OK and nothing happened" is exactly
        // the report an operator cannot act on.
        "reexec" => match crate::reexec::target() {
            Ok(path) => (
                format!("Re-executing {}\n", path.display()),
                Action::Reexec,
            ),
            Err(e) => (format!("error: cannot re-exec: {:#}\n", e), Action::None),
        },

        "" => ("error: empty request\n".to_string(), Action::None),
        other => (
            format!(
                "error: unknown command '{}'\n\
                 commands: list, status [NAME], blame, start NAME, stop NAME, restart NAME, \
                 enable NAME, disable NAME, reload, reexec, suspend, poweroff, reboot, halt\n",
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

/// Whether the main loop has time-driven work pending: a running service
/// whose ready path has not appeared yet and cannot be watched for, or a dead
/// one with a restart due. When neither, the loop can sleep until a child
/// exits, a client connects or a ready path appears, instead of waking ten
/// times a second to look.
///
/// `readiness_watched` is the main loop's answer to "is every ready path I am
/// still waiting on covered by an inotify watch" (see readiness.rs). When it
/// is, waiting for a ready path is no longer time-driven work at all: the
/// kernel wakes the poll the moment the file appears, so the loop can go idle
/// instead of spinning at ten looks a second for the whole of boot. When it is
/// not -- no inotify, or a daemon that has not created its own runtime
/// directory yet -- this falls back to exactly the condition it always had,
/// because a service whose readiness nothing can notify us about must still be
/// looked at on a timer.
pub fn wants_quick_tick(services: &HashMap<String, Service>, readiness_watched: bool) -> bool {
    services.values().any(|svc| {
        (!readiness_watched
            && svc.is_running()
            && svc.ready_path().is_some()
            && svc.ready_at().is_none())
            || (!svc.is_running() && svc.retry_at().is_some())
    })
}

/// Look at every running service's ready path once and record the ones that
/// have appeared. Called twice per main-loop pass -- once with the watches
/// freshly armed and once after the sleep they woke from -- so a service
/// nobody waits on (nothing lists it in `after`) still gets a ready time, and
/// so does one whose file appeared while the watch was being set up.
///
/// This stays the only caller of `note_ready_if_present`, and that function
/// stays the only place `ready_at` is written. An inotify event is a reason to
/// call this; it is never itself the evidence that a service is ready.
pub fn observe_readiness(services: &mut HashMap<String, Service>) {
    for (name, svc) in services.iter_mut() {
        if svc.note_ready_if_present() {
            log::info!(
                "Service {} is ready at {}",
                name,
                svc.ready_path().unwrap_or("?")
            );
        }
    }
}

/// The boot timeline: when init got where, and when each service first
/// started and first became ready, slowest first. Seconds since the kernel
/// started, so the numbers line up with dmesg.
///
/// Every service time here is its *first* in this boot. A service that has
/// restarted since -- after a resume, after a crash, after `raven-rc restart`
/// -- keeps the times it had at boot, because this table answers "why was the
/// machine slow to come up" and a restart eight hours later is not an answer
/// to it. How often a service has come back, and when it last did, belong to
/// `raven-rc status NAME`, and that is where they are.
///
/// `boot_done` is when init reached its main loop, if it has, and is taken as
/// an argument rather than read here so the boundary can be driven from a test
/// without writing into the process-wide milestone list -- the same reason
/// `Service::should_restart_at` takes a clock. Rows that first started after
/// it are shown, because they are part of what is running, but they are left
/// out of the summary: a daemon somebody started by hand at lunchtime is not
/// part of how long the machine took to boot.
pub(crate) fn blame_services(
    services: &HashMap<String, Service>,
    config: &InitConfig,
    boot_done: Option<f64>,
) -> String {
    let mut out = String::new();
    out.push_str("Seconds since the kernel started. Each service is timed from its first\n");
    out.push_str("start in this boot; restarts since are in `raven-rc status NAME`.\n");
    out.push_str("Milestones cross a re-exec: a name that appears twice marks one, and\n");
    out.push_str("the rows above the second `init started` are the original boot's.\n\n");

    for (name, at) in crate::timeline::milestones() {
        out.push_str(&format!("  {:<22} {:>9.3}\n", name, at));
    }
    out.push('\n');

    struct Row {
        name: String,
        started: Option<f64>,
        ready: Option<f64>,
        /// What goes in the READY column, which is not always a number: see
        /// where it is built below.
        ready_text: String,
        took: Option<f64>,
        /// This service first started after init reached its main loop, so it
        /// is not part of the boot the summary measures.
        after_boot: bool,
        note: String,
    }

    let mut rows: Vec<Row> = Vec::new();
    // Config order for the base set, then anything running the config does
    // not mention, the same set `list` shows.
    let mut names: Vec<String> = config.services.iter().map(|c| c.name.clone()).collect();
    for name in services.keys() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    for name in names {
        let Some(svc) = services.get(&name) else {
            continue;
        };
        let started = svc.first_started_at().map(crate::timeline::instant_secs);
        let ready = svc.first_ready_at().map(crate::timeline::instant_secs);
        let took = match (started, ready) {
            (Some(s), Some(r)) => Some((r - s).max(0.0)),
            _ => None,
        };
        // A service with a ready path that has never been reached is not the
        // same thing as a service with no ready path, and the two used to
        // print an identical bare dash in this column and be told apart only
        // by a note at the end of a wide line. A dash now means the definition
        // names nothing to wait for; `waiting` means it does and the file has
        // not appeared yet; `never` means it does, the file never appeared,
        // and the service is not running any more to make it.
        let ready_text = match (svc.ready_path(), ready) {
            (_, Some(at)) => format!("{at:.3}"),
            (Some(_), None) if svc.is_running() => "waiting".to_string(),
            (Some(_), None) => "never".to_string(),
            (None, None) => "-".to_string(),
        };
        let mut note = match (svc.ready_path(), ready, svc.is_running()) {
            (None, _, _) => "no ready path",
            (Some(_), Some(_), _) => "",
            (Some(_), None, true) => "not ready yet",
            (Some(_), None, false) => "exited before ready",
        }
        .to_string();
        let after_boot = matches!((started, boot_done), (Some(s), Some(done)) if s > done);
        if after_boot {
            // Without this the row is simply baffling: one line reading 2751
            // in a table whose other rows are all under ten, and nothing
            // saying why.
            note = if note.is_empty() {
                "started after boot".to_string()
            } else {
                format!("started after boot, {note}")
            };
        }
        rows.push(Row {
            name,
            started,
            ready,
            ready_text,
            took,
            after_boot,
            note,
        });
    }

    // Slowest to become ready first; then the ones still waiting; then the
    // rest in start order. The top line is the culprit.
    rows.sort_by(|a, b| match (a.took, b.took) {
        (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a
            .started
            .partial_cmp(&b.started)
            .unwrap_or(std::cmp::Ordering::Equal),
    });

    out.push_str("SERVICE              STARTED    READY      TOOK       NOTE\n");
    let fmt = |v: Option<f64>| {
        v.map(|x| format!("{:.3}", x))
            .unwrap_or_else(|| "-".to_string())
    };
    for r in &rows {
        out.push_str(&format!(
            "{:<20} {:>9}  {:>9}  {:>9}  {}\n",
            r.name,
            fmt(r.started),
            r.ready_text,
            fmt(r.took),
            r.note
        ));
    }

    // First start to last ready, over this boot and nothing else. Both halves
    // of that were wrong: the times were the current run's, so one obexd
    // restart after a resume reported `span 97613.069s` on a machine that
    // booted in seven seconds, and the fold read `ready.or(started)`, which
    // let a service that never became ready contribute its start time as
    // though it were a ready time and quietly overstate the boot.
    let boot_rows = || rows.iter().filter(|r| !r.after_boot);
    let first = boot_rows()
        .filter_map(|r| r.started)
        .fold(f64::INFINITY, f64::min);
    let last = boot_rows()
        .filter_map(|r| r.ready)
        .fold(f64::NEG_INFINITY, f64::max);
    if first.is_finite() && last.is_finite() {
        out.push_str(&format!(
            "\nservices: first start {:.3}, last ready {:.3}, span {:.3}s\n",
            first,
            last,
            last - first
        ));
    } else if first.is_finite() {
        // Nothing has been ready: either no service names a ready path or
        // none has reached one yet. A span folded over start times alone
        // would be a measurement of nothing, printed to three decimal places.
        out.push_str(&format!(
            "\nservices: first start {first:.3}, nothing ready yet\n"
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

/// Whether a status rendering may read the service's live resource counters.
///
/// This exists because of the status publisher, not because of the status
/// command. [`StatusPublisher`] re-renders this text on every main-loop tick
/// and writes the file whenever the text differs from last time, which is what
/// makes a quiet machine cost no I/O at all. `memory.current` is not quiet: a
/// daemon that is merely alive moves it by a page here and there, so a status
/// text carrying it would differ on nearly every tick and turn an idle machine
/// into a process that writes several files a second to /run, forever, to
/// record that nothing happened.
///
/// Rounding the numbers until they stop moving was the alternative and it is
/// not one: rounding hard enough to be stable (whole gigabytes, whole minutes
/// of CPU) leaves a figure too coarse to answer the question anybody asks it,
/// and rounding gently enough to be useful still churns under load -- exactly
/// when a supervisor should be writing least.
///
/// So the counters are read on demand, for a request that came over the
/// socket, and left out of the published copy. The visible consequence is that
/// `raven-rc status NAME` shows memory and CPU for root, who is served from
/// the socket, and not for an unprivileged caller, who is served from
/// /run/raven-init/services/NAME (see `read_published` in rc.rs). The
/// alternative was to make every idle machine pay for a number almost nobody
/// reads.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Counters {
    /// Read `memory.current`, `cpu.stat` and `pids.current` from the cgroup.
    Read,
    /// Leave them out, because this text is about to be written to a file.
    Skip,
}

fn status_one(
    name: &str,
    services: &HashMap<String, Service>,
    config: &InitConfig,
    counters: Counters,
) -> String {
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

    // The run that is happening now, which is not the number `blame` prints.
    // `blame` reports a service's first start in this boot and holds it still
    // through every restart, because it is describing the boot; this is the
    // current run, and on a service that has restarted the two differ by
    // however long the machine has been up. Between them they answer "when
    // did this come up" and "when did it last come back", which used to be
    // one number pretending to be both.
    if svc.is_running() {
        if let Some(at) = svc.started_at() {
            out.push_str(&format!("  started      {}\n", boot_clock(at, counters)));
        }
    }

    if counters == Counters::Read {
        out.push_str(&resource_usage(name));
    }

    if let Some(code) = svc.exit_status() {
        if !svc.is_running() {
            out.push_str(&format!("  exit status  {}\n", code));
        }
    }

    // The restart count is the number that turns "this service is running"
    // into "this service has been dying and coming back all afternoon", which
    // is a different machine to be standing in front of.
    out.push_str(&format!(
        "  restarts     {}{}\n",
        svc.restart_count(),
        if svc.restart_configured() {
            ""
        } else {
            " (restart disabled in config)"
        }
    ));

    // The count says a service has been coming back; this says whether that
    // was all through the morning or in the last minute, which is the
    // difference between a problem somebody already fixed and one that is
    // happening now. It is here rather than in `blame` because a restart is a
    // fact about a service's life, not about the boot -- `blame` reports the
    // boot and says so.
    if let Some(at) = svc.last_restart_at() {
        out.push_str(&format!("  last restart {}\n", boot_clock(at, counters)));
    }

    if let Some(at) = svc.retry_at() {
        if !svc.is_running() {
            let wait = at.saturating_duration_since(std::time::Instant::now());
            out.push_str(&format!("  next restart in {}s\n", wait.as_secs()));
        }
    }

    if let Some(cfg) = cfg {
        out.push_str(&format!("  exec         {}\n", cfg.exec));
    }

    out
}

/// What the service's cgroup can say about it right now, as status lines.
///
/// Empty for a service with no cgroup, which is not an error and not worth a
/// line saying so: a machine with no cgroup2 would otherwise repeat the same
/// apology in every status it printed, having already said it once at boot.
/// It is likewise empty for a service that has exited, because the reaper
/// removes the directory -- except where something the service forked outlived
/// it, and then these lines are the only place that shows it.
fn resource_usage(name: &str) -> String {
    let Some(cgroup) = crate::cgroup::Cgroup::existing(name) else {
        return String::new();
    };

    let mut out = String::new();

    if let Some(bytes) = cgroup.memory_current() {
        out.push_str(&format!("  memory       {}\n", format_bytes(bytes)));
    }

    if let Some(cpu) = cgroup.cpu_stat() {
        // Total first, because that is the number being looked for, and the
        // split after it, because a daemon burning its time in the kernel is
        // a different problem from one burning it in its own code -- and the
        // two are told apart by nothing else raven-rc prints.
        out.push_str(&format!(
            "  cpu          {} ({} user, {} system)\n",
            format_cpu(cpu.usage_usec),
            format_cpu(cpu.user_usec),
            format_cpu(cpu.system_usec)
        ));
    }

    if let Some(count) = cgroup.pids_current() {
        // Not "children": this counts the leader too, and it counts the ones
        // that daemonised out of the process group, which is the reason the
        // number is worth printing at all.
        out.push_str(&format!("  processes    {}\n", count));
    }

    out
}

/// A byte count as an operator would write it.
///
/// Binary multiples, matching `cgroup::parse_size` and the kernel: a status
/// line reading 256M for a limit written as `memory_max = "256M"` is the
/// whole point, and a decimal megabyte would make the two disagree by 5% for
/// no reason anybody could see.
fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let value = bytes as f64;
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if value < KIB * KIB {
        format!("{:.1}K", value / KIB)
    } else if value < KIB * KIB * KIB {
        format!("{:.1}M", value / (KIB * KIB))
    } else {
        format!("{:.2}G", value / (KIB * KIB * KIB))
    }
}

/// A moment, on the boot clock, and -- when this text is not about to be
/// written to a file -- how long ago it was.
///
/// The absolute half is the stable one: seconds since the kernel started,
/// which is what `blame` prints and what a kernel log line carries, and which
/// does not change no matter how often this text is re-rendered. The relative
/// half is a live reading in exactly the sense [`Counters`] is about -- it
/// differs on every tick simply because time passed -- so it is left out of
/// the published copy, where it would make the supervisor rewrite a file every
/// second for every service that has ever restarted, on an idle machine, to
/// record that nothing had happened.
fn boot_clock(at: std::time::Instant, counters: Counters) -> String {
    let secs = crate::timeline::instant_secs(at);
    match counters {
        Counters::Read => format!(
            "{:.3}s into this boot ({} ago)",
            secs,
            format_ago(at.elapsed())
        ),
        Counters::Skip => format!("{secs:.3}s into this boot"),
    }
}

/// How long ago, as somebody standing at the machine would say it.
///
/// Coarse on purpose and coarser the further back it goes: the question this
/// answers is "was that just now?", and three decimal places on a restart that
/// happened last Tuesday would answer a question nobody asked. The exact
/// second is on the same line, on the boot clock, for anyone reading this
/// against a log.
fn format_ago(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else if secs < 86_400 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d{:02}h", secs / 86_400, (secs % 86_400) / 3600)
    }
}

/// CPU microseconds as seconds, to the millisecond.
///
/// The same three decimal places `blame` prints, so the two tables can be read
/// against each other, and enough resolution to see that a one-shot used 40ms
/// rather than "0s".
fn format_cpu(usec: u64) -> String {
    format!("{:.3}s", usec as f64 / 1_000_000.0)
}

pub(crate) fn start_service(
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
                // The third of the three readiness waits this codebase had,
                // and the last one still spending a timer on it. It was a 20ms
                // sleep loop; it is now the same inotify wait boot uses, which
                // matters more here than it looks: this runs on PID 1's own
                // thread while a raven-rc client holds the socket, so every
                // millisecond of it is a millisecond the supervisor is not
                // reaping children or answering anyone else.
                //
                // `ready_timeout` still bounds it and the error below is still
                // reached on exactly the same schedule.
                if !crate::readiness::wait_for_path(
                    path,
                    Duration::from_secs(dep_cfg.ready_timeout as u64),
                ) {
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
        // A definition already on disk means init simply has not read it:
        // drop-ins are loaded once, at boot. Telling the operator to copy a
        // file that is already there is how this message used to send people
        // in a circle.
        let dropin = format!("/etc/raven/init.d/{}.toml", name);
        if std::path::Path::new(&dropin).exists() {
            return format!(
                "error: no such service '{}'\n\
                 Its definition exists at {} but has not been loaded --\n\
                 drop-ins are read at boot. Load it now with:\n\
                 \x20 raven-rc reload\n",
                name, dropin
            );
        }

        let template = format!("/usr/share/raven/services/{}.toml", name);
        if std::path::Path::new(&template).exists() {
            return format!(
                "error: no such service '{}'\n\
                 Its software is not installed. `rvn install` sets the service\n\
                 up with it; installed some other way, copy the definition and\n\
                 load it:\n\
                 \x20 cp {} /etc/raven/init.d/\n\
                 \x20 raven-rc reload\n",
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
                 Its output is in {}; `raven-rc status {}` has the rest.\n",
                name,
                how,
                Service::log_dir().join(format!("{}.log", name)).display(),
                name
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
/// Re-read the configuration from disk and reconcile it with what is running.
///
/// This exists because `/etc/raven/init.d` was read exactly once, at boot. A
/// daemon installed afterwards -- which is the normal case, since the base
/// image ships no daemons -- left `rvn` printing "service 'sshd' is now
/// available: `raven-rc start sshd`" while that command answered "no such
/// service". The definition was on disk and PID 1 had never looked again.
///
/// # What reload does and does not do
///
/// It reloads *definitions*, never processes. A running service keeps running
/// across a reload, keeps its pid, and does not notice one happened. That is
/// the property that makes it safe to run on a live machine, and it is why a
/// changed definition for a running service is reported as pending rather than
/// applied: the process on the other end was started from the old definition
/// and still matches it. `restart` is how the operator opts into the new one.
///
/// Only `[[services]]` is taken. `system` and `mounts` are deliberately
/// ignored: re-running mounts or renaming the host underneath a running system
/// is not a configuration reload, it is a boot, and the honest way to ask for
/// one is `reboot`. This mirrors the rule `load_dropin_services` already
/// applies to drop-ins for the same reason.
///
/// # Why it calls the boot path
///
/// `crate::config::load()` is the function init itself uses at startup, search
/// order, drop-in merge, `source_path` and all. Reload deliberately reuses it
/// rather than reimplementing the search: two loaders would be two answers to
/// "what is configured", and the one that only runs on reload is the one that
/// would rot unnoticed.
fn reload_config(services: &mut HashMap<String, Service>, config: &mut InitConfig) -> String {
    let mut fresh = match crate::config::load() {
        Ok(fresh) => fresh,
        Err(e) => {
            // The live configuration is untouched -- nothing has been merged
            // yet at this point, which is why parsing happens before any
            // mutation rather than field by field.
            return format!(
                "error: could not reload configuration: {e}\n\
                 the running configuration is unchanged\n"
            );
        }
    };

    // The other half of the boot path. Without it a reload sees only what is
    // written in a file, and everything synthesized from the kernel command
    // line and the installed binaries -- seatd, and whichever of `ravend` and
    // `wayland-session` this machine has -- has no incoming definition to be
    // matched against. The retain pass below then reads that absence as "its
    // file is gone": a running one was demoted to "removed but still running",
    // and a stopped one was dropped outright, so `raven-rc start ravend`
    // answered "no such service" until the next reboot.
    //
    // Running it here also makes reload the way a newly installed login screen
    // is picked up, which is the same promise reload already makes for a
    // drop-in: install RavenLogin, reload, and `ravend` is reported as added.
    //
    // Still before any mutation of `config`, so a failure leaves the running
    // configuration exactly as it was.
    if let Err(e) = crate::overrides::apply_kernel_cmdline_overrides(&mut fresh) {
        return format!(
            "error: could not reload configuration: {e}\n\
             the running configuration is unchanged\n"
        );
    }
    crate::overrides::fixup_getty_login_programs(&mut fresh);
    let fresh = fresh;

    let running = |name: &str| services.get(name).is_some_and(|svc| svc.is_running());

    let mut added: Vec<String> = Vec::new();
    let mut updated: Vec<String> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();
    let mut orphaned: Vec<String> = Vec::new();

    for incoming in &fresh.services {
        match config.services.iter_mut().find(|s| s.name == incoming.name) {
            Some(current) => {
                if current == incoming {
                    continue;
                }
                let was_running = running(&incoming.name);
                *current = incoming.clone();
                if was_running {
                    pending.push(incoming.name.clone());
                } else {
                    updated.push(incoming.name.clone());
                }
            }
            None => {
                config.services.push(incoming.clone());
                added.push(incoming.name.clone());
            }
        }
    }

    // Definitions whose file is gone. A stopped one is simply forgotten; a
    // running one keeps its definition, because dropping it would leave a
    // process on the system that `raven-rc stop` could no longer name.
    config.services.retain(|svc| {
        if fresh.services.iter().any(|f| f.name == svc.name) {
            return true;
        }
        if running(&svc.name) {
            orphaned.push(svc.name.clone());
            true
        } else {
            dropped.push(svc.name.clone());
            false
        }
    });

    config.source_path = fresh.source_path;

    if added.is_empty()
        && updated.is_empty()
        && pending.is_empty()
        && dropped.is_empty()
        && orphaned.is_empty()
    {
        return "Configuration reloaded; nothing changed\n".to_string();
    }

    let mut out = String::from("Configuration reloaded\n");
    let mut section = |label: &str, names: &[String], note: &str| {
        if names.is_empty() {
            return;
        }
        out.push_str(&format!("  {label}: {}", names.join(", ")));
        if note.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&format!(" ({note})\n"));
        }
    };

    section("added", &added, "not started; `raven-rc start NAME`");
    section("updated", &updated, "");
    section(
        "changed while running",
        &pending,
        "`raven-rc restart NAME` to apply",
    );
    section("removed", &dropped, "");
    section(
        "removed but still running",
        &orphaned,
        "`raven-rc stop NAME` to retire",
    );
    out
}

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
/// with comments explaining what each service is for and why it is set as it is,
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

/// Publishes what `list` and `status NAME` would say to [`STATUS_DIR`].
///
/// Called once per main-loop tick. Renders the same text the socket would
/// return and writes a file only when its text differs from what was last
/// written, so a quiet system costs a few string comparisons per tick and no
/// I/O. Files are written whole and installed by rename, so a reader never
/// sees a partial one. A service that leaves the configuration has its file
/// removed.
///
/// "The same text the socket would return" has one deliberate exception: the
/// live resource counters are left out here, because a number that moves by
/// itself would defeat the comparison above and make an idle machine write to
/// /run several times a second. [`Counters`] has the full argument.
pub struct StatusPublisher {
    dir: PathBuf,
    list: Option<String>,
    blame: Option<String>,
    services: HashMap<String, String>,
    warned: bool,
}

impl StatusPublisher {
    /// A publisher for [`STATUS_DIR`].
    pub fn new() -> Self {
        Self::at(STATUS_DIR)
    }

    /// A publisher for an arbitrary directory, so tests can use a temporary
    /// one; PID 1 always uses [`StatusPublisher::new`].
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        StatusPublisher {
            dir: dir.into(),
            list: None,
            blame: None,
            services: HashMap::new(),
            warned: false,
        }
    }

    /// Bring the published files up to date with `services`.
    ///
    /// Never fails the caller: a /run that cannot be written is logged once
    /// and the supervisor carries on, the same policy as an unbindable
    /// control socket.
    pub fn publish(&mut self, services: &HashMap<String, Service>, config: &InitConfig) {
        if let Err(e) = self.publish_inner(services, config) {
            if !self.warned {
                log::warn!("Cannot publish service status under {}: {:#}", self.dir.display(), e);
                log::warn!("  `raven-rc list` and `status` will need root this boot");
                self.warned = true;
            }
        }
    }

    fn publish_inner(&mut self, services: &HashMap<String, Service>, config: &InitConfig) -> Result<()> {
        let services_dir = self.dir.join("services");
        if !services_dir.is_dir() {
            std::fs::create_dir_all(&services_dir)
                .with_context(|| format!("Cannot create {}", services_dir.display()))?;
            std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o755)).ok();
            std::fs::set_permissions(&services_dir, std::fs::Permissions::from_mode(0o755)).ok();
        }

        let list = list_services(services, config);
        if self.list.as_deref() != Some(list.as_str()) {
            write_published(&self.dir.join("status"), &list)?;
            self.list = Some(list);
        }

        let blame = blame_services(
            services,
            config,
            crate::timeline::first_milestone("main loop"),
        );
        if self.blame.as_deref() != Some(blame.as_str()) {
            write_published(&self.dir.join("blame"), &blame)?;
            self.blame = Some(blame);
        }

        // Config order first, then anything running that the config does not
        // mention, the same set `list` shows.
        let mut names: Vec<String> = config.services.iter().map(|c| c.name.clone()).collect();
        for name in services.keys() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }

        for name in &names {
            // A name is a file name here; anything that could leave the
            // directory is not published rather than trusted.
            if name.is_empty() || name.contains('/') || name.starts_with('.') {
                continue;
            }
            // Counters::Skip: this text is compared against the last one and
            // written when it differs, so anything in it that moves on its own
            // is a write to /run on every tick. See `Counters`.
            let text = status_one(name, services, config, Counters::Skip);
            if self.services.get(name) != Some(&text) {
                write_published(&services_dir.join(name), &text)?;
                self.services.insert(name.clone(), text);
            }
        }

        let gone: Vec<String> = self
            .services
            .keys()
            .filter(|name| !names.contains(name))
            .cloned()
            .collect();
        for name in gone {
            std::fs::remove_file(services_dir.join(&name)).ok();
            self.services.remove(&name);
        }

        Ok(())
    }
}

impl Default for StatusPublisher {
    fn default() -> Self {
        Self::new()
    }
}

/// Write `text` to `path` whole, world-readable, installed by rename.
fn write_published(path: &Path, text: &str) -> Result<()> {
    let tmp = path.with_extension("new");
    std::fs::write(&tmp, text).with_context(|| format!("Cannot write {}", tmp.display()))?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644)).ok();
    std::fs::rename(&tmp, path).with_context(|| format!("Cannot install {}", path.display()))?;
    Ok(())
}
