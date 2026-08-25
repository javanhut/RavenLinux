//! Service control protocol tests.
//!
//! These drive `control::dispatch` against real child processes rather than a
//! mock, because the behaviour worth protecting is about process lifetime: that
//! a stopped service stays stopped, and that a restarted one comes back.
//!
//! No socket is involved. `dispatch` is deliberately split from the I/O so the
//! interesting half can be tested without PID 1.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[path = "../src/config.rs"]
mod config;
#[path = "../src/service.rs"]
mod service;

// control.rs refers to its siblings as `crate::config` / `crate::service`; in a
// test binary the crate root is this file, so those paths resolve here.
use config as _config_for_control;
use service as _service_for_control;
#[path = "../src/control.rs"]
mod control;

use config::{InitConfig, ServiceConfig, SystemConfig};
use control::Action;
use service::Service;

/// A service that sits there until something kills it.
fn sleeper(name: &str) -> ServiceConfig {
    ServiceConfig {
        name: name.to_string(),
        description: format!("test service {}", name),
        exec: "/bin/sleep".to_string(),
        args: vec!["300".to_string()],
        restart: true,
        enabled: true,
        critical: false,
        environment: HashMap::new(),
        tty: None,
        runtime_dirs: Vec::new(),
        stop_exec: None,
        stop_args: Vec::new(),
        stop_timeout: 5,
    }
}

fn config_with(services: Vec<ServiceConfig>) -> InitConfig {
    InitConfig {
        system: SystemConfig::default(),
        services,
        mounts: Vec::new(),
        source_path: None,
    }
}

/// True once the process is dead. Polls, because SIGTERM is asynchronous.
///
/// Reads the state field from /proc rather than using `kill(pid, 0)`: nothing
/// reaps children in a test binary, so a terminated service lingers as a
/// zombie and `kill(pid, 0)` keeps reporting it alive. A zombie is dead.
fn wait_gone(pid: i32, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            // Reaped and gone.
            Err(_) => return true,
            Ok(stat) => {
                // comm can contain spaces and parens; state is the first field
                // after the closing paren.
                if let Some((_, rest)) = stat.rsplit_once(')') {
                    if rest.split_whitespace().next() == Some("Z") {
                        return true;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn list_reports_running_and_disabled_services() {
    let running = sleeper("alive");
    let mut disabled = sleeper("never-started");
    disabled.enabled = false;

    let mut cfg = config_with(vec![running.clone(), disabled]);
    let mut services = HashMap::new();
    services.insert(
        "alive".to_string(),
        Service::start(&running).expect("starts"),
    );

    let (reply, action) = control::dispatch("list", &mut services, &mut cfg);
    assert_eq!(action, Action::None);

    assert!(reply.contains("alive"), "{reply}");
    assert!(reply.contains("running"), "{reply}");
    // A service in the config that was never started must still be listed --
    // otherwise `list` cannot tell you what you could start.
    assert!(reply.contains("never-started"), "{reply}");
    assert!(reply.contains("disabled"), "{reply}");

    services.get_mut("alive").unwrap().kill();
}

#[test]
fn stop_keeps_the_service_stopped() {
    // The whole point of the manually_stopped flag: without it the supervisor
    // sees restart = true and starts the service straight back up.
    let cfg_svc = sleeper("stoppable");
    let mut cfg = config_with(vec![cfg_svc.clone()]);

    let mut services = HashMap::new();
    let svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("has a pid").as_raw();
    services.insert("stoppable".to_string(), svc);

    let (reply, _) = control::dispatch("stop stoppable", &mut services, &mut cfg);
    assert!(reply.contains("Stopping"), "{reply}");

    assert!(
        wait_gone(pid, Duration::from_secs(5)),
        "process should exit"
    );

    let svc = services.get_mut("stoppable").unwrap();
    assert!(
        svc.is_manually_stopped(),
        "stop must record operator intent"
    );
    assert!(
        !svc.should_restart(),
        "a service stopped on request must not be auto-restarted"
    );
}

#[test]
fn restart_actually_brings_the_service_back() {
    // Regression: stop_by_request only *sends* SIGTERM, so is_running() was
    // still true when start_by_request ran. It returned early, the process then
    // died, and the manually_stopped flag kept it dead -- restart was a stop.
    let cfg_svc = sleeper("restartable");
    let mut cfg = config_with(vec![cfg_svc.clone()]);

    let mut services = HashMap::new();
    let svc = Service::start(&cfg_svc).expect("starts");
    let first_pid = svc.pid().expect("has a pid").as_raw();
    services.insert("restartable".to_string(), svc);

    let (reply, action) = control::dispatch("restart restartable", &mut services, &mut cfg);
    assert_eq!(action, Action::None);
    assert!(reply.contains("Restarted"), "{reply}");

    let svc = services.get(&"restartable".to_string()).unwrap();
    assert!(svc.is_running(), "must be running again after restart");
    assert!(
        !svc.is_manually_stopped(),
        "restart must clear the operator-stopped flag"
    );

    let second_pid = svc.pid().expect("has a pid").as_raw();
    assert_ne!(
        first_pid, second_pid,
        "restart must be a new process, not the old one still lingering"
    );

    services.get_mut("restartable").unwrap().kill();
}

#[test]
fn start_can_bring_up_a_service_disabled_at_boot() {
    // enabled = false means "not automatically", not "never".
    let mut disabled = sleeper("on-demand");
    disabled.enabled = false;
    let mut cfg = config_with(vec![disabled]);

    let mut services = HashMap::new();
    let (reply, _) = control::dispatch("start on-demand", &mut services, &mut cfg);
    assert!(reply.contains("Started"), "{reply}");

    let svc = services.get(&"on-demand".to_string()).expect("now tracked");
    assert!(svc.is_running());

    services.get_mut("on-demand").unwrap().kill();
}

#[test]
fn stopped_service_can_be_started_again() {
    let cfg_svc = sleeper("cycle");
    let mut cfg = config_with(vec![cfg_svc.clone()]);

    let mut services = HashMap::new();
    let svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("pid").as_raw();
    services.insert("cycle".to_string(), svc);

    control::dispatch("stop cycle", &mut services, &mut cfg);
    assert!(wait_gone(pid, Duration::from_secs(5)));

    let (reply, _) = control::dispatch("start cycle", &mut services, &mut cfg);
    assert!(reply.contains("Started"), "{reply}");
    assert!(services.get(&"cycle".to_string()).unwrap().is_running());

    services.get_mut("cycle").unwrap().kill();
}

#[test]
fn status_of_one_service_names_its_state_and_pid() {
    let cfg_svc = sleeper("inspectable");
    let mut cfg = config_with(vec![cfg_svc.clone()]);

    let mut services = HashMap::new();
    let svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("pid").as_raw();
    services.insert("inspectable".to_string(), svc);

    let (reply, _) = control::dispatch("status inspectable", &mut services, &mut cfg);
    assert!(reply.contains("inspectable"), "{reply}");
    assert!(reply.contains("running"), "{reply}");
    assert!(reply.contains(&pid.to_string()), "{reply}");
    assert!(reply.contains("/bin/sleep"), "{reply}");

    services.get_mut("inspectable").unwrap().kill();
}

#[test]
fn shutdown_verbs_return_actions_not_replies_alone() {
    let mut cfg = config_with(vec![]);
    let mut services = HashMap::new();

    assert_eq!(
        control::dispatch("poweroff", &mut services, &mut cfg).1,
        Action::Poweroff
    );
    assert_eq!(
        control::dispatch("halt", &mut services, &mut cfg).1,
        Action::Poweroff
    );
    assert_eq!(
        control::dispatch("reboot", &mut services, &mut cfg).1,
        Action::Reboot
    );
}

#[test]
fn bad_requests_are_reported_not_guessed_at() {
    let mut cfg = config_with(vec![sleeper("real")]);
    let mut services = HashMap::new();

    for (request, expect) in [
        ("", "empty request"),
        ("frobnicate", "unknown command"),
        ("start", "needs a service name"),
        ("stop", "needs a service name"),
        ("restart", "needs a service name"),
        ("status nonexistent", "no such service"),
        ("stop nonexistent", "no such service"),
    ] {
        let (reply, action) = control::dispatch(request, &mut services, &mut cfg);
        assert_eq!(action, Action::None, "{request} must not act");
        assert!(
            reply.contains(expect),
            "request {request:?} should mention {expect:?}, got: {reply}"
        );
    }
}

// ---------------------------------------------------------------------------
// Socket round-trip
// ---------------------------------------------------------------------------
// The tests above exercise `dispatch` directly, which skips the half that runs
// inside PID 1: accepting, reading a capped request, replying, closing. These
// cover that, including the property that matters most there -- a client that
// connects and then says nothing must not wedge the server.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

fn temp_socket(name: &str) -> String {
    format!(
        "{}/raven-init-test-{}-{}.sock",
        std::env::temp_dir().display(),
        name,
        std::process::id()
    )
}

/// One request/response exchange against a listener, driven by a real client.
fn round_trip(path: &str, request: &str) -> String {
    let mut client = UnixStream::connect(path).expect("connects");
    client.set_read_timeout(Some(Duration::from_secs(2))).ok();
    writeln!(client, "{}", request).expect("writes");
    client.flush().ok();
    client.shutdown(std::net::Shutdown::Write).ok();

    let mut reply = String::new();
    client.read_to_string(&mut reply).expect("reads");
    reply
}

#[test]
fn socket_serves_a_real_client() {
    let path = temp_socket("roundtrip");
    let listener = control::listen_at(&path).expect("binds");

    let cfg_svc = sleeper("socket-svc");
    let mut cfg = config_with(vec![cfg_svc.clone()]);
    let mut services = HashMap::new();
    services.insert(
        "socket-svc".to_string(),
        Service::start(&cfg_svc).expect("starts"),
    );

    // The client runs on another thread because poll() only serves what is
    // already queued -- exactly how the main loop calls it.
    let client_path = path.clone();
    let client = std::thread::spawn(move || round_trip(&client_path, "list"));

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if control::poll(&listener, &mut services, &mut cfg) == Action::None {
            // poll returns None both for "nothing queued" and "served a
            // non-shutdown request", so keep ticking until the client is done.
        }
        if client.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let reply = client.join().expect("client thread");
    assert!(reply.contains("socket-svc"), "{reply}");
    assert!(reply.contains("running"), "{reply}");

    services.get_mut("socket-svc").unwrap().kill();
    std::fs::remove_file(&path).ok();
}

#[test]
fn poll_returns_immediately_when_nothing_is_waiting() {
    // The main loop calls this every tick; if it ever blocked on an empty
    // accept queue, PID 1 would stop supervising services.
    let path = temp_socket("empty");
    let listener = control::listen_at(&path).expect("binds");
    let mut cfg = config_with(vec![]);
    let mut services = HashMap::new();

    let start = Instant::now();
    for _ in 0..100 {
        assert_eq!(
            control::poll(&listener, &mut services, &mut cfg),
            Action::None
        );
    }
    assert!(
        start.elapsed() < Duration::from_millis(200),
        "100 empty polls took {:?}; poll must not block",
        start.elapsed()
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn a_silent_client_does_not_wedge_the_server() {
    // A connection that never sends a request must cost one read timeout and
    // no more. Anything else is a denial of service against PID 1.
    let path = temp_socket("silent");
    let listener = control::listen_at(&path).expect("binds");
    let mut cfg = config_with(vec![]);
    let mut services = HashMap::new();

    let held = UnixStream::connect(&path).expect("connects");

    let start = Instant::now();
    control::poll(&listener, &mut services, &mut cfg);
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "a silent client blocked poll for {elapsed:?}"
    );

    drop(held);
    std::fs::remove_file(&path).ok();
}

/// The supervisor's restart decision, mirroring main.rs::check_services.
///
/// Duplicated rather than imported because main.rs is a binary root, not a
/// library; the assertions below are what keep the two honest.
fn supervisor_would_restart(svc: &mut Service) -> bool {
    let died = matches!(
        svc.state(),
        service::ServiceState::Exited | service::ServiceState::Signaled
    );
    died && svc.should_restart()
}

#[test]
fn a_crashed_service_is_restarted_but_a_stopped_one_is_not() {
    // Regression: check_services only tested for ServiceState::Exited, so a
    // service killed by a signal -- SIGSEGV, SIGKILL, the OOM killer, every
    // real crash -- was never restarted despite restart = true.
    let cfg_svc = sleeper("crasher");
    let mut cfg = config_with(vec![cfg_svc.clone()]);

    let mut services = HashMap::new();
    let svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("pid").as_raw();
    services.insert("crasher".to_string(), svc);

    // Crash it from outside: nothing marked this service as stopped.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
    assert!(wait_gone(pid, Duration::from_secs(5)));

    let svc = services.get_mut("crasher").unwrap();
    svc.poll_exit();
    assert_eq!(svc.state(), service::ServiceState::Signaled);
    assert!(
        supervisor_would_restart(svc),
        "a crashed service with restart = true must be restarted"
    );

    // The operator path must still win over that.
    let cfg_svc2 = sleeper("quiet");
    let mut services2 = HashMap::new();
    services2.insert(
        "quiet".to_string(),
        Service::start(&cfg_svc2).expect("starts"),
    );
    let pid2 = services2["quiet"].pid().expect("pid").as_raw();

    control::dispatch(
        "stop quiet",
        &mut services2,
        &mut config_with(vec![cfg_svc2]),
    );
    assert!(wait_gone(pid2, Duration::from_secs(5)));

    let svc2 = services2.get_mut("quiet").unwrap();
    svc2.poll_exit();
    assert!(
        !supervisor_would_restart(svc2),
        "an operator-stopped service must stay stopped even though SIGTERM signals it"
    );
}

// ---------------------------------------------------------------------------
// enable / disable
// ---------------------------------------------------------------------------

/// An init.toml with the kind of commentary the shipped one carries.
const ANNOTATED_CONFIG: &str = r#"# RavenLinux Init Configuration
# /etc/raven/init.toml

[system]
hostname = "raven-linux"   # trailing comment
log_level = "info"

[[services]]
name = "cawd"
description = "CAW wireless daemon"
exec = "/usr/bin/cawd"
args = []
restart = true
enabled = true
critical = false

# iwd must stay disabled: it fights cawd for the wiphy.
[[services]]
name = "iwd"
description = "iNet Wireless Daemon"
exec = "/usr/libexec/iwd"
args = []
restart = true
enabled = false
critical = false
"#;

fn temp_config(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("raven-init-cfg-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("init.toml");
    std::fs::write(&path, ANNOTATED_CONFIG).expect("write");
    path
}

/// Config parsed from the annotated file, with source_path set as init does.
fn loaded_config(path: &std::path::Path) -> InitConfig {
    let text = std::fs::read_to_string(path).expect("read");
    let mut cfg: InitConfig = toml::from_str(&text).expect("parses");
    cfg.source_path = Some(path.to_path_buf());
    cfg
}

#[test]
fn disable_persists_and_keeps_the_comments() {
    let path = temp_config("disable");
    let mut cfg = loaded_config(&path);
    let mut services = HashMap::new();

    let (reply, action) = control::dispatch("disable cawd", &mut services, &mut cfg);
    assert_eq!(action, Action::None);
    assert!(reply.contains("Disabled cawd"), "{reply}");
    // enable/disable are about boot, and the reply must not imply otherwise.
    assert!(reply.contains("boot"), "{reply}");

    // In memory.
    assert!(
        !cfg.services
            .iter()
            .find(|s| s.name == "cawd")
            .unwrap()
            .enabled
    );

    // On disk, and still parseable.
    let after = std::fs::read_to_string(&path).expect("read back");
    let reparsed: InitConfig = toml::from_str(&after).expect("still valid TOML");
    assert!(
        !reparsed
            .services
            .iter()
            .find(|s| s.name == "cawd")
            .unwrap()
            .enabled
    );
    // The other service is untouched.
    assert!(
        !reparsed
            .services
            .iter()
            .find(|s| s.name == "iwd")
            .unwrap()
            .enabled
    );

    // The point of toml_edit: a serde round-trip would have eaten these.
    assert!(
        after.contains("# RavenLinux Init Configuration"),
        "header comment lost:\n{after}"
    );
    assert!(
        after.contains("# iwd must stay disabled"),
        "explanatory comment lost:\n{after}"
    );
    assert!(
        after.contains("# trailing comment"),
        "inline comment lost:\n{after}"
    );

    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn enable_persists_and_survives_a_reload() {
    let path = temp_config("enable");
    let mut cfg = loaded_config(&path);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("enable iwd", &mut services, &mut cfg);
    assert!(reply.contains("Enabled iwd"), "{reply}");

    // The check that matters: a fresh load -- what the next boot does -- sees it.
    let reloaded = loaded_config(&path);
    assert!(
        reloaded
            .services
            .iter()
            .find(|s| s.name == "iwd")
            .unwrap()
            .enabled,
        "enable must survive a reload, or it did not persist"
    );

    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn enabling_an_already_enabled_service_is_not_an_error() {
    let path = temp_config("idempotent");
    let mut cfg = loaded_config(&path);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("enable cawd", &mut services, &mut cfg);
    assert!(!reply.starts_with("error:"), "{reply}");
    assert!(reply.contains("already"), "{reply}");
    assert!(
        cfg.services
            .iter()
            .find(|s| s.name == "cawd")
            .unwrap()
            .enabled
    );

    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn a_failed_write_does_not_leave_memory_lying() {
    // If the config cannot be written, the in-memory flag must go back: a
    // `status` that says "disabled" while the next boot enables it is worse
    // than a plain refusal.
    let path = temp_config("readonly");
    let mut cfg = loaded_config(&path);
    let mut services = HashMap::new();

    // Point at a file that cannot exist, standing in for a read-only rootfs.
    cfg.source_path = Some(std::path::PathBuf::from("/nonexistent-dir/init.toml"));

    let (reply, _) = control::dispatch("disable cawd", &mut services, &mut cfg);
    assert!(reply.starts_with("error:"), "{reply}");
    assert!(
        cfg.services
            .iter()
            .find(|s| s.name == "cawd")
            .unwrap()
            .enabled,
        "a failed persist must roll the in-memory flag back"
    );

    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn enable_without_a_config_file_is_refused_not_silently_dropped() {
    // Built-in defaults, no file: there is nothing to persist to, and
    // pretending otherwise would lose the change at reboot with no warning.
    let mut cfg = config_with(vec![sleeper("orphan")]);
    assert!(cfg.source_path.is_none());
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("disable orphan", &mut services, &mut cfg);
    assert!(reply.starts_with("error:"), "{reply}");
    assert!(reply.contains("no"), "{reply}");
    assert!(
        cfg.services
            .iter()
            .find(|s| s.name == "orphan")
            .unwrap()
            .enabled,
        "refused request must not change memory either"
    );
}

#[test]
fn enable_reports_unknown_services() {
    let path = temp_config("unknown");
    let mut cfg = loaded_config(&path);
    let mut services = HashMap::new();

    for request in ["enable nope", "disable nope"] {
        let (reply, _) = control::dispatch(request, &mut services, &mut cfg);
        assert!(reply.contains("no such service"), "{request}: {reply}");
    }
    // A rejected request must not have touched the file.
    let after = std::fs::read_to_string(&path).expect("read");
    assert_eq!(
        after, ANNOTATED_CONFIG,
        "file changed on a rejected request"
    );

    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn list_separates_runtime_state_from_boot_state() {
    // The two used to share a column, which is why `enable` had nothing
    // visible to change.
    let path = temp_config("columns");
    let mut cfg = loaded_config(&path);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("list", &mut services, &mut cfg);
    assert!(reply.contains("STATE"), "{reply}");
    assert!(reply.contains("BOOT"), "{reply}");

    // cawd: enabled at boot, not currently running.
    let cawd = reply
        .lines()
        .find(|l| l.starts_with("cawd"))
        .expect("cawd row");
    assert!(cawd.contains("stopped"), "{cawd}");
    assert!(cawd.contains("enabled"), "{cawd}");

    let iwd = reply
        .lines()
        .find(|l| l.starts_with("iwd"))
        .expect("iwd row");
    assert!(iwd.contains("disabled"), "{iwd}");

    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn no_temp_file_is_left_behind() {
    // write_atomic works through a temp file in the same directory; a leftover
    // would be shipped alongside init.toml and confuse the next reader.
    let path = temp_config("debris");
    let mut cfg = loaded_config(&path);
    let mut services = HashMap::new();

    control::dispatch("disable cawd", &mut services, &mut cfg);

    let dir = path.parent().unwrap();
    let leftovers: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n != "init.toml")
        .collect();
    assert!(leftovers.is_empty(), "left behind: {leftovers:?}");

    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_crash_looping_service_is_given_up_on_once_not_every_tick() {
    // Regression: should_restart() was a pure query that logged from inside
    // itself and recorded nothing. A crash-looping service therefore printed
    // "restarting too frequently, disabling restart" on every 100ms supervisor
    // tick -- roughly ten lines a second, forever -- while never actually
    // disabling anything: five seconds later it restarted and began again.
    // The flood made the console unusable, which is how it surfaced.
    let mut cfg_svc = sleeper("flapper");
    cfg_svc.exec = "/bin/false".to_string(); // exits immediately, every time
    cfg_svc.args = vec![];
    let mut cfg = config_with(vec![cfg_svc.clone()]);

    let mut services = HashMap::new();
    services.insert(
        "flapper".to_string(),
        Service::start(&cfg_svc).expect("starts"),
    );

    let svc = services.get_mut("flapper").unwrap();

    // Drive the supervisor by hand: let it die, decide, restart, repeat.
    let mut restarts = 0;
    for _ in 0..40 {
        svc.wait_for_exit(Duration::from_secs(2));
        if svc.should_restart() {
            let _ = svc.restart();
            restarts += 1;
        } else {
            break;
        }
    }

    assert!(
        restarts <= 5,
        "gave up after {restarts} restarts; the budget is 5"
    );
    assert!(!svc.should_restart(), "must stay given up");

    // The decision must be stable and silent from here: a hundred more ticks
    // must not change it, which is what stops the flood.
    for _ in 0..100 {
        assert!(!svc.should_restart());
    }
    assert_eq!(svc.state(), service::ServiceState::Failed);

    // An explicit start is the operator saying the cause is fixed. This one
    // is not -- /bin/false still exits at once -- and the reply must say so
    // rather than claiming success for a process that is already gone.
    let (reply, _) = control::dispatch("start flapper", &mut services, &mut cfg);
    assert!(
        reply.contains("exited with status 1 immediately"),
        "start must report an instant death, not claim success: {reply}"
    );
    let svc = services.get_mut("flapper").unwrap();
    svc.wait_for_exit(Duration::from_secs(2));
    assert!(
        svc.should_restart(),
        "an operator start must refill the restart budget"
    );

    services.get_mut("flapper").unwrap().kill();
}

#[test]
fn runtime_dirs_are_created_and_output_goes_to_the_log() {
    // dbus's death spiral: /run is a fresh tmpfs each boot, dbus-daemon does
    // not mkdir its own socket directory, and its complaints -- like every
    // other daemon's -- printed over the console. Services now get their
    // runtime_dirs created and their stdout/stderr sent to a per-service log.
    let root = std::env::temp_dir().join(format!("raven-svclog-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let logs = root.join("logs");
    let rundir = root.join("run/dbus-like");

    std::env::set_var("RAVEN_SERVICE_LOG_DIR", &logs);

    let cfg_svc = ServiceConfig {
        name: "chatty".to_string(),
        description: "prints and exits".to_string(),
        exec: "/bin/sh".to_string(),
        args: vec![
            "-c".to_string(),
            "echo to-stdout; echo to-stderr >&2".to_string(),
        ],
        restart: false,
        enabled: true,
        critical: false,
        environment: HashMap::new(),
        tty: None,
        runtime_dirs: vec![rundir.display().to_string()],
        stop_exec: None,
        stop_args: Vec::new(),
        stop_timeout: 5,
    };

    let mut svc = Service::start(&cfg_svc).expect("starts");
    svc.wait_for_exit(Duration::from_secs(5));
    std::env::remove_var("RAVEN_SERVICE_LOG_DIR");

    assert!(
        rundir.is_dir(),
        "runtime_dirs must exist before the service runs"
    );

    let log = logs.join("chatty.log");
    let text = std::fs::read_to_string(&log).expect("log file written");
    assert!(text.contains("to-stdout"), "{text}");
    assert!(
        text.contains("to-stderr"),
        "stderr must reach the log too: {text}"
    );

    std::fs::remove_dir_all(&root).ok();
}
