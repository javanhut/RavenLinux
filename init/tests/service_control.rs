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
// service.rs resolves a `user =` account through `crate::user`, so that module
// has to exist under this test binary's crate root too.
#[path = "../src/user.rs"]
mod user;
#[path = "../src/service.rs"]
mod service;
// reload re-runs the boot-time transforms, so control.rs names `crate::overrides`
// as well. It in turn reaches for `crate::config` and `crate::user`, both above.
#[path = "../src/overrides.rs"]
mod overrides;
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
        pre_exec: Vec::new(),
        tty: None,
        user: None,
        runtime_dirs: Vec::new(),
        after: Vec::new(),
        ready_path: None,
        ready_timeout: 5,
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
fn start_brings_up_direct_dependencies_first() {
    let mut dependency = sleeper("dependency");
    dependency.enabled = false;
    let mut dependent = sleeper("dependent");
    dependent.enabled = false;
    dependent.after = vec!["dependency".to_string()];

    let mut cfg = config_with(vec![dependency, dependent]);
    let mut services = HashMap::new();
    let (reply, _) = control::dispatch("start dependent", &mut services, &mut cfg);

    assert!(reply.contains("Started dependent"), "{reply}");
    assert!(services.get("dependency").is_some_and(Service::is_running));
    assert!(services.get("dependent").is_some_and(Service::is_running));

    services.get_mut("dependent").unwrap().kill();
    services.get_mut("dependency").unwrap().kill();
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

/// Suspend is not a shutdown, and the difference is the whole reason it has
/// its own action: init performs it inline and goes back to supervising the
/// same services, rather than tearing the system down.
#[test]
fn suspend_is_an_action_of_its_own() {
    let mut services = HashMap::new();
    let mut cfg = config_with(vec![]);

    assert_eq!(
        control::dispatch("suspend", &mut services, &mut cfg).1,
        control::Action::Suspend
    );
    // The spelling half the world's laptops use.
    assert_eq!(
        control::dispatch("sleep", &mut services, &mut cfg).1,
        control::Action::Suspend
    );
    // And it is not confused with either shutdown verb.
    assert_ne!(
        control::dispatch("suspend", &mut services, &mut cfg).1,
        control::Action::Poweroff
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

# getty-ttyS0 stays disabled: nothing is on the serial port.
[[services]]
name = "getty-ttyS0"
description = "Serial console getty"
exec = "/sbin/agetty"
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
            .find(|s| s.name == "getty-ttyS0")
            .unwrap()
            .enabled
    );

    // The point of toml_edit: a serde round-trip would have eaten these.
    assert!(
        after.contains("# RavenLinux Init Configuration"),
        "header comment lost:\n{after}"
    );
    assert!(
        after.contains("# getty-ttyS0 stays disabled"),
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

    let (reply, _) = control::dispatch("enable getty-ttyS0", &mut services, &mut cfg);
    assert!(reply.contains("Enabled getty-ttyS0"), "{reply}");

    // The check that matters: a fresh load -- what the next boot does -- sees it.
    let reloaded = loaded_config(&path);
    assert!(
        reloaded
            .services
            .iter()
            .find(|s| s.name == "getty-ttyS0")
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

    let serial = reply
        .lines()
        .find(|l| l.starts_with("getty-ttyS0"))
        .expect("getty-ttyS0 row");
    assert!(serial.contains("disabled"), "{serial}");

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
        pre_exec: Vec::new(),
        tty: None,
        user: None,
        runtime_dirs: vec![rundir.display().to_string()],
        after: Vec::new(),
        ready_path: None,
        ready_timeout: 5,
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

/// The failure from the openssh screenshot: `raven-rc start sshd` on a system
/// where openssh was never installed replied `failed to start sshd: Failed to
/// start sshd`. The outer context was formatted with `{}`, which prints only
/// the top of an anyhow chain, and the context it printed said nothing the
/// operator did not already know.
#[test]
fn starting_a_service_whose_binary_is_missing_names_the_binary() {
    let mut missing = sleeper("sshd");
    missing.exec = "/usr/bin/definitely-not-installed".to_string();
    missing.args = Vec::new();
    // Not in the running set, exactly like a daemon boot skipped as absent.
    let mut cfg = config_with(vec![missing]);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("start sshd", &mut services, &mut cfg);

    assert!(reply.starts_with("error:"), "{reply}");
    assert!(
        reply.contains("/usr/bin/definitely-not-installed"),
        "the reply must name the missing path: {reply}"
    );
    assert!(
        reply.contains("not installed"),
        "the reply must say why: {reply}"
    );
    // The old doubled message must not come back.
    assert!(
        !reply.contains("Failed to start"),
        "context must add information, not repeat the prefix: {reply}"
    );
    // A service that could not start must not be recorded as running.
    assert!(!services.contains_key("sshd"), "{reply}");
}

/// The same check on the other branch of `start`: a service that is in the
/// running set but stopped, which is where `raven-rc start` lands after a stop
/// or a crash.
#[test]
fn restarting_onto_a_deleted_binary_names_the_binary() {
    let path = std::env::temp_dir().join("raven-init-vanishing-sleeper");
    std::fs::write(&path, "#!/bin/sh\nexec /bin/sleep 300\n").expect("writes");
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("chmods");

    let mut cfg_svc = sleeper("vanishing");
    cfg_svc.exec = path.display().to_string();
    cfg_svc.args = Vec::new();
    let mut cfg = config_with(vec![cfg_svc.clone()]);

    let mut services = HashMap::new();
    let svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("has a pid").as_raw();
    services.insert("vanishing".to_string(), svc);

    let (reply, _) = control::dispatch("stop vanishing", &mut services, &mut cfg);
    assert!(reply.contains("Stopping"), "{reply}");
    assert!(wait_gone(pid, Duration::from_secs(5)), "process should exit");

    // The package is removed while the service is stopped.
    std::fs::remove_file(&path).expect("removes");

    let (reply, _) = control::dispatch("start vanishing", &mut services, &mut cfg);
    assert!(reply.contains(&path.display().to_string()), "{reply}");
    assert!(reply.contains("not installed"), "{reply}");
}

/// A binary that exists but is not executable is a different mistake and must
/// read as one -- `spawn` reports both as a bare errno.
#[test]
fn starting_a_non_executable_binary_says_so() {
    let path = std::env::temp_dir().join("raven-init-not-executable");
    std::fs::write(&path, "#!/bin/sh\ntrue\n").expect("writes");
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o644))
        .expect("chmods");

    let mut svc = sleeper("chmodless");
    svc.exec = path.display().to_string();
    svc.args = Vec::new();
    let mut cfg = config_with(vec![svc]);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("start chmodless", &mut services, &mut cfg);

    assert!(reply.contains("not executable"), "{reply}");
    assert!(reply.contains("0644"), "the mode belongs in the message: {reply}");

    let _ = std::fs::remove_file(&path);
}

/// The base image ships no ssh, so its services arrive as drop-ins. A service
/// defined by /etc/raven/init.d/*.toml must be startable and, critically,
/// enable/disable must rewrite the drop-in that defines it -- not fail because
/// init.toml has never heard of it.
#[test]
fn a_dropin_defined_service_can_be_disabled_and_enabled() {
    let dir = std::env::temp_dir().join("raven-init-dropin-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");

    // The main config defines nothing.
    let main = dir.join("init.toml");
    std::fs::write(&main, "[system]\nhostname = \"t\"\n[[services]]\nname = \"other\"\nexec = \"/bin/true\"\n").unwrap();

    let dropin = dir.join("sshd.toml");
    std::fs::write(
        &dropin,
        "[[services]]\nname = \"sshd\"\ndescription = \"d\"\nexec = \"/bin/sleep\"\nargs = [\"300\"]\nenabled = true\n",
    )
    .unwrap();

    // What load_config would produce: main config plus the folded-in drop-in.
    let mut svc_cfg = sleeper("sshd");
    svc_cfg.exec = "/bin/sleep".to_string();
    let mut cfg = config_with(vec![svc_cfg]);
    cfg.source_path = Some(main.clone());

    let mut services = HashMap::new();

    // Same process-global variable the reload tests use, so the same lock.
    let _env = DROPIN_ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("RAVEN_INIT_DROPIN_DIR", &dir);
    let (reply, _) = control::dispatch("disable sshd", &mut services, &mut cfg);
    std::env::remove_var("RAVEN_INIT_DROPIN_DIR");

    assert!(reply.starts_with("Disabled sshd"), "{reply}");
    // The flag landed in the file that defines the service...
    let rewritten = std::fs::read_to_string(&dropin).unwrap();
    assert!(rewritten.contains("enabled = false"), "{rewritten}");
    // ...and init.toml was not grown a phantom entry.
    let main_after = std::fs::read_to_string(&main).unwrap();
    assert!(!main_after.contains("sshd"), "{main_after}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// pre_exec runs to completion before the daemon starts, and its failure is
/// the service's failure -- sshd with no host keys must fail at start with
/// the keygen's error, not enter a crash loop.
#[test]
fn pre_exec_runs_first_and_its_failure_stops_the_start() {
    let marker = std::env::temp_dir().join("raven-init-pre-exec-marker");
    let _ = std::fs::remove_file(&marker);

    let mut ok = sleeper("with-setup");
    ok.pre_exec = vec!["/bin/touch".to_string(), marker.display().to_string()];
    let svc = Service::start(&ok).expect("starts");
    assert!(marker.exists(), "pre_exec must have run before the daemon");
    let mut svc = svc;
    svc.kill();
    let _ = std::fs::remove_file(&marker);

    let mut broken = sleeper("with-broken-setup");
    broken.pre_exec = vec!["/bin/false".to_string()];
    let err = match Service::start(&broken) {
        Ok(_) => panic!("a failed pre_exec must fail the start"),
        Err(e) => e,
    };
    let msg = format!("{err:#}");
    assert!(msg.contains("pre_exec"), "the error names the phase: {msg}");
}

// ---------------------------------------------------------------------------
// reload
// ---------------------------------------------------------------------------
// The bug these protect against: /etc/raven/init.d was read once, at boot, so a
// daemon installed afterwards -- the normal case, since the base image ships
// none -- was invisible until a reboot. `rvn install openssh` printed "service
// 'sshd' is now available" while `raven-rc start sshd` answered "no such
// service".
//
// RAVEN_INIT_DROPIN_DIR is what makes this testable without touching /etc.

/// RAVEN_INIT_DROPIN_DIR is process-global and the test harness runs threads in
/// parallel, so the reload tests take this in turn. Without it they overwrite
/// each other's environment mid-run and the failures look like reload bugs.
static DROPIN_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Point init's drop-in loader at a private directory for the duration of a
/// test, and put the environment back afterwards.
struct Dropins {
    dir: std::path::PathBuf,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl Dropins {
    /// `key` must be unique per test: these are real directories, and two tests
    /// sharing one path delete each other's files. Naming them by entry count
    /// was the first version of this and produced exactly that collision.
    ///
    /// The empty command line is the point rather than a placeholder. Reload
    /// re-runs the boot-time transforms, and those read /proc/cmdline -- so
    /// without pinning it these tests would synthesize a seat daemon and a
    /// session on a machine booted with `raven.graphics=wayland` and not on
    /// one booted without, and every count and every "nothing changed" below
    /// would depend on how the build host happened to start.
    fn new(key: &str, entries: &[(&str, &str)]) -> Self {
        Self::with_cmdline(key, entries, "")
    }

    /// The same, for the tests that are about what the command line does.
    fn with_cmdline(key: &str, entries: &[(&str, &str)], cmdline: &str) -> Self {
        let guard = DROPIN_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = dropin_dir(key, entries);
        std::env::set_var("RAVEN_INIT_DROPIN_DIR", &dir);
        std::env::set_var("RAVEN_INIT_CMDLINE", cmdline);
        Self { dir, _guard: guard }
    }
}

impl Drop for Dropins {
    fn drop(&mut self) {
        std::env::remove_var("RAVEN_INIT_DROPIN_DIR");
        std::env::remove_var("RAVEN_INIT_CMDLINE");
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A drop-in directory holding one `[[services]]` file per entry.
fn dropin_dir(key: &str, entries: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "raven-reload-{}-{}",
        std::process::id(),
        key
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create drop-in dir");
    for (name, body) in entries {
        std::fs::write(dir.join(format!("{name}.toml")), body).expect("write drop-in");
    }
    dir
}

fn dropin_toml(name: &str) -> String {
    format!(
        "[[services]]\nname = \"{name}\"\nexec = \"/bin/sleep\"\nargs = [\"300\"]\nenabled = false\n"
    )
}

#[test]
fn reload_picks_up_a_service_installed_after_boot() {
    let _dropins = Dropins::new("late", &[("late-arrival", &dropin_toml("late-arrival"))]);

    // Boot-time state: init knows nothing about it.
    let mut services: HashMap<String, Service> = HashMap::new();
    let mut cfg = config_with(Vec::new());

    let (before, _) = control::dispatch("start late-arrival", &mut services, &mut cfg);
    assert!(
        before.contains("no such service"),
        "expected the pre-reload failure, got: {before}"
    );

    let (reply, action) = control::dispatch("reload", &mut services, &mut cfg);
    assert!(matches!(action, Action::None));
    assert!(reply.contains("added"), "reload should report it: {reply}");
    assert!(reply.contains("late-arrival"), "reload should name it: {reply}");

    // And it is now a real service.
    assert!(
        cfg.services.iter().any(|s| s.name == "late-arrival"),
        "definition should be live after reload"
    );

}

#[test]
fn reload_keeps_services_that_no_file_defines() {
    // The bug: `seatd`, `ravend` and `wayland-session` are synthesized from the
    // kernel command line and the installed binaries, so no file names them.
    // Reload re-read the files only, found no incoming definition, and took
    // that for "its file is gone" -- dropping a stopped one outright. The
    // machine then had no way back to a login screen short of a reboot, and
    // `raven-rc start ravend` answered "no such service" while ravend sat
    // installed in /usr/bin.
    //
    // seatd is the one to assert on: it is synthesized whenever the command
    // line asks for Wayland, whether or not the binary is installed, so this
    // does not depend on what the build host has in /usr/bin.
    let _dropins = Dropins::with_cmdline("synthesized", &[], "raven.graphics=wayland");

    // Boot: the transforms put it in the live configuration.
    let mut services: HashMap<String, Service> = HashMap::new();
    let mut cfg = config_with(Vec::new());
    overrides::apply_kernel_cmdline_overrides(&mut cfg).expect("overrides at boot");
    assert!(
        cfg.services.iter().any(|s| s.name == "seatd"),
        "precondition: raven.graphics=wayland should synthesize seatd"
    );

    // The reload that used to delete it. Nothing is running, which is the case
    // that lost the definition rather than merely mislabelling it.
    let (reply, _) = control::dispatch("reload", &mut services, &mut cfg);
    assert!(
        cfg.services.iter().any(|s| s.name == "seatd"),
        "reload dropped a synthesized service: {reply}"
    );

    // The symptom an operator would have hit.
    let (status, _) = control::dispatch("status seatd", &mut services, &mut cfg);
    assert!(
        !status.contains("no such service"),
        "seatd should still be nameable after a reload: {status}"
    );
}

#[test]
fn reload_synthesizes_nothing_without_wayland_on_the_command_line() {
    // The other half: these transforms are conditional, and a reload must not
    // invent a seat daemon on a machine that never asked for a graphical
    // session. Guards against "fix the drop" turning into "start it anyway".
    let _dropins = Dropins::with_cmdline("no-wayland", &[], "root=UUID=whatever rw quiet");

    let mut services: HashMap<String, Service> = HashMap::new();
    let mut cfg = config_with(Vec::new());

    let (reply, _) = control::dispatch("reload", &mut services, &mut cfg);
    assert!(
        !cfg.services.iter().any(|s| s.name == "seatd"),
        "no Wayland asked for, so no seatd should appear: {reply}"
    );
    assert!(
        !cfg.services.iter().any(|s| s.name == "wayland-session"),
        "no Wayland asked for, so no session should appear: {reply}"
    );
}

#[test]
fn reload_does_not_disturb_a_running_service() {
    // The property that makes reload safe to run on a live machine: it reloads
    // definitions, never processes. Same pid before and after.
    let _dropins = Dropins::new("untouched", &[("untouched", &dropin_toml("untouched"))]);

    let mut services = HashMap::new();
    let svc = Service::start(&sleeper("untouched")).expect("start");
    let pid_before = svc.pid().expect("pid").as_raw();
    services.insert("untouched".to_string(), svc);
    let mut cfg = config_with(vec![sleeper("untouched")]);

    let (reply, _) = control::dispatch("reload", &mut services, &mut cfg);

    let svc = services.get("untouched").expect("still tracked");
    assert!(svc.is_running(), "reload must not stop it: {reply}");
    assert_eq!(
        svc.pid().expect("pid").as_raw(),
        pid_before,
        "reload must not restart it: {reply}"
    );

    services.get_mut("untouched").unwrap().stop();
}

#[test]
fn reload_reports_a_changed_definition_as_pending_while_running() {
    // A running process was started from the old definition and still matches
    // it. Reporting the change as applied would be a lie; `restart` is how the
    // operator opts in.
    let _dropins = Dropins::new("mutable", &[("mutable", &dropin_toml("mutable"))]);

    let mut services = HashMap::new();
    let svc = Service::start(&sleeper("mutable")).expect("start");
    services.insert("mutable".to_string(), svc);

    // Live definition differs from what is on disk (args differ from sleeper()).
    let mut cfg = config_with(vec![sleeper("mutable")]);

    let (reply, _) = control::dispatch("reload", &mut services, &mut cfg);
    assert!(
        reply.contains("changed while running"),
        "expected a pending report, got: {reply}"
    );
    assert!(reply.contains("restart"), "should say how to apply: {reply}");

    services.get_mut("mutable").unwrap().stop();
}

#[test]
fn reload_keeps_a_removed_but_still_running_service_addressable() {
    // Dropping the definition of a running process would leave something on the
    // system that `raven-rc stop` could no longer name.
    let _dropins = Dropins::new("orphan", &[]);

    let mut services = HashMap::new();
    let svc = Service::start(&sleeper("orphan")).expect("start");
    services.insert("orphan".to_string(), svc);
    let mut cfg = config_with(vec![sleeper("orphan")]);

    let (reply, _) = control::dispatch("reload", &mut services, &mut cfg);
    assert!(
        reply.contains("removed but still running"),
        "expected the orphan report, got: {reply}"
    );
    assert!(
        cfg.services.iter().any(|s| s.name == "orphan"),
        "definition must survive so `stop orphan` still works"
    );

    let (stopped, _) = control::dispatch("stop orphan", &mut services, &mut cfg);
    assert!(!stopped.contains("no such service"), "{stopped}");

}

#[test]
fn reload_forgets_a_removed_service_that_was_not_running() {
    let _dropins = Dropins::new("gone", &[]);

    let mut services: HashMap<String, Service> = HashMap::new();
    let mut cfg = config_with(vec![sleeper("gone")]);

    let (reply, _) = control::dispatch("reload", &mut services, &mut cfg);
    assert!(reply.contains("removed"), "{reply}");
    assert!(
        !cfg.services.iter().any(|s| s.name == "gone"),
        "a stopped service whose file is gone should be forgotten"
    );

}

/// An account that does not exist must fail the start, not fall back to root.
///
/// This is the whole safety property of `user =`. A service asked to drop
/// privilege and started as uid 0 anyway is worse than one that did not start:
/// nothing is wrong on the surface, and the privilege is only discovered when
/// something uses it. The desktop session is the case that matters -- before
/// `user` existed it ran as root unconditionally, and a silent fallback would
/// quietly restore exactly that.
#[test]
fn a_service_naming_an_unknown_account_refuses_to_start() {
    let mut svc = sleeper("wayland-session");
    svc.user = Some("nosuchuser-9f3a".to_string());
    let mut cfg = config_with(vec![svc]);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("start wayland-session", &mut services, &mut cfg);

    assert!(reply.starts_with("error:"), "{reply}");
    assert!(
        reply.contains("nosuchuser-9f3a"),
        "the reply must name the account: {reply}"
    );
    assert!(
        !services.contains_key("wayland-session"),
        "a service that could not drop privilege must not be left running: {reply}"
    );
}

/// The tty path cannot drop privilege, so it must refuse rather than ignore.
///
/// Silently running as root here would be the same failure as above, reached
/// by a different route: the field is set, nothing complains, and the process
/// is root anyway.
#[test]
fn a_tty_service_naming_an_account_refuses_rather_than_running_as_root() {
    let mut svc = sleeper("getty-tty1");
    svc.tty = Some("/dev/tty1".to_string());
    // A real account, so the failure is about the tty path and not resolution.
    svc.user = Some("root".to_string());
    let mut cfg = config_with(vec![svc]);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("start getty-tty1", &mut services, &mut cfg);

    assert!(reply.starts_with("error:"), "{reply}");
    assert!(
        reply.contains("tty"),
        "the reply must say the tty path is why: {reply}"
    );
}

/// A service with no `user` still starts, and starts as whoever init is.
///
/// The guard against a regression that makes privilege dropping mandatory:
/// seatd, udev and dbus all need root and all leave `user` unset.
#[test]
fn a_service_without_a_user_still_starts() {
    let mut cfg = config_with(vec![sleeper("plain")]);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("start plain", &mut services, &mut cfg);

    assert!(!reply.starts_with("error:"), "{reply}");
    assert!(services.contains_key("plain"), "{reply}");

    // Leave nothing behind for the next test.
    let _ = control::dispatch("stop plain", &mut services, &mut cfg);
}
