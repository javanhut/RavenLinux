//! Service control protocol tests.
//!
//! These drive `control::dispatch` against real child processes rather than a
//! mock, because the behaviour worth protecting is about process lifetime: that
//! a stopped service stays stopped, and that a restarted one comes back.
//!
//! No socket is involved. `dispatch` is deliberately split from the I/O so the
//! interesting half can be tested without PID 1.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Every one of these modules is pulled in whole, for a crate root that is this
// test file, so each carries items only PID 1 itself ever calls. That is not
// dead code, it is code this binary is not the caller of, and the allow says
// which of the two it is rather than leaving the build to shout about it.
#[allow(dead_code)]
#[path = "../src/config.rs"]
mod config;
// service.rs puts every service into a cgroup and applies its rlimits through
// `crate::cgroup`, so that module has to exist under this crate root too.
#[allow(dead_code)]
#[path = "../src/cgroup.rs"]
mod cgroup;
// service.rs resolves a `user =` account through `crate::user`, so that module
// has to exist under this test binary's crate root too.
#[allow(dead_code)]
#[path = "../src/service.rs"]
mod service;
#[allow(dead_code)]
#[path = "../src/user.rs"]
mod user;
// reload re-runs the boot-time transforms, so control.rs names `crate::overrides`
// as well. It in turn reaches for `crate::config` and `crate::user`, both above.
#[allow(dead_code)]
#[path = "../src/overrides.rs"]
mod overrides;
// control.rs refers to its siblings as `crate::config` / `crate::service`; the
// `mod` declarations above are what make those paths resolve, because in a test
// binary the crate root is this file.
// `blame` reads the boot clock and milestones through `crate::timeline`.
#[allow(dead_code)]
#[path = "../src/timeline.rs"]
mod timeline;
// The `reexec` verb checks its target through `crate::reexec` before replying.
#[allow(dead_code)]
#[path = "../src/reexec.rs"]
mod reexec;
// `start` waits for a dependency's ready path through `crate::readiness`, which
// arms an inotify watch instead of sleeping on a timer.
#[allow(dead_code)]
#[path = "../src/readiness.rs"]
mod readiness;
#[allow(dead_code)]
#[path = "../src/control.rs"]
mod control;

/// Serialises the tests that point `$RAVEN_CGROUP_ROOT` at a tree of their own.
///
/// An environment variable is process-global and `cargo test` runs these in
/// threads of one process, so two tests each setting it to their own temporary
/// directory is two tests reading each other's cgroup trees -- and the one
/// that loses fails somewhere unrelated, with a message about a missing file.
/// The module's own unit tests take `ensure_slice_at` and `Cgroup::*_in`
/// instead and need none of this; these two cannot, because what they are
/// testing is the path `Service::start` and `control::dispatch` take by
/// themselves, and that path reads the variable.
static CGROUP_ROOT_ENV: Mutex<()> = Mutex::new(());

/// The same treatment for `$RAVEN_INIT_EXE`, and for the same reason.
///
/// Without it these two tests failed together about one run in six: one sets
/// the variable to a path that does not exist and removes it two lines later,
/// and the other asks what `reexec` would target in between, gets the missing
/// path and reports a refusal where it expected a plan. The failure names the
/// wrong test and looks like a broken `reexec`.
static INIT_EXE_ENV: Mutex<()> = Mutex::new(());

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
        ..ServiceConfig::default()
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
fn stop_reaches_children_in_the_service_process_group() {
    // ravend is shaped like this shell: the supervised PID owns long-lived
    // children (the compositor, greeter and session). Killing only the leader
    // leaves those children holding DRM master across shutdown.
    let child_file =
        std::env::temp_dir().join(format!("raven-init-service-child-{}", std::process::id()));
    std::fs::remove_file(&child_file).ok();

    let mut cfg_svc = sleeper("process-tree");
    cfg_svc.exec = "/bin/sh".to_string();
    cfg_svc.args = vec![
        "-c".to_string(),
        format!(
            "trap 'exit 0' TERM; /bin/sleep 300 & child=$!; printf '%s\\n' \"$child\" > {}; wait \"$child\"",
            child_file.display()
        ),
    ];

    let mut services = HashMap::new();
    let svc = Service::start(&cfg_svc).expect("starts");
    let leader = svc.pid().expect("leader pid").as_raw();
    services.insert("process-tree".to_string(), svc);

    let deadline = Instant::now() + Duration::from_secs(5);
    let child = loop {
        if let Ok(text) = std::fs::read_to_string(&child_file) {
            if let Ok(pid) = text.trim().parse::<i32>() {
                break pid;
            }
        }
        assert!(Instant::now() < deadline, "child pid was never published");
        std::thread::sleep(Duration::from_millis(20));
    };

    assert_eq!(
        nix::unistd::getpgid(Some(nix::unistd::Pid::from_raw(leader)))
            .expect("leader process group")
            .as_raw(),
        leader,
        "a supervised service must lead its own process group"
    );
    assert_eq!(
        nix::unistd::getpgid(Some(nix::unistd::Pid::from_raw(child)))
            .expect("child process group")
            .as_raw(),
        leader,
        "service children must inherit the supervised group"
    );

    let mut cfg = config_with(vec![cfg_svc]);
    let (reply, _) = control::dispatch("stop process-tree", &mut services, &mut cfg);
    assert!(reply.contains("Stopping"), "{reply}");
    assert!(
        wait_gone(child, Duration::from_secs(5)),
        "stopping the service must also stop its child"
    );

    std::fs::remove_file(child_file).ok();
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

    let svc = services.get("restartable").unwrap();
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

    let svc = services.get("on-demand").expect("now tracked");
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
    assert!(services.get("cycle").unwrap().is_running());

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

/// Hibernation has its own action, its own single spelling, and its own reply.
///
/// Before this existed the only writer of /sys/power/state in the whole tree
/// wrote "mem" or "freeze", so nothing Raven shipped could reach S4 -- the
/// installer's `resume=`, the encrypted swap that keeps hibernation working
/// and the `noresume` escape hatch all had no trigger. The verb is what gives
/// them one.
///
/// No `sleep`-style alias, deliberately. It is the one verb here that can lose
/// work when it is asked for by mistake, so every spelling that reaches it is
/// one somebody typed on purpose.
#[test]
fn hibernate_is_an_action_of_its_own() {
    let mut services = HashMap::new();
    let mut cfg = config_with(vec![]);

    let (reply, action) = control::dispatch("hibernate", &mut services, &mut cfg);
    assert_eq!(action, control::Action::Hibernate);
    assert_eq!(reply, "Hibernating\n");

    // Not reachable by any other word, and not the same thing as a suspend:
    // init checks for a resume device before it writes "disk", and a machine
    // that cannot resume must not arrive here by way of "suspend".
    assert_ne!(
        control::dispatch("suspend", &mut services, &mut cfg).1,
        control::Action::Hibernate
    );
    // Trailing words are ignored for every verb this parser knows, so
    // "hibernate now" is not a near miss -- these are.
    for near_miss in ["hibernation", "suspend-to-disk", "disk", "hiberate"] {
        let (reply, action) = control::dispatch(near_miss, &mut services, &mut cfg);
        assert_eq!(action, control::Action::None, "{near_miss}");
        assert!(reply.starts_with("error:"), "{near_miss}: {reply}");
    }

    // And it is listed where somebody who mistyped it will see it.
    let (usage, _) = control::dispatch("frobnicate", &mut services, &mut cfg);
    assert!(usage.contains("hibernate"), "{usage}");
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

#[test]
fn a_socket_another_supervisor_answers_on_is_not_taken() {
    // Regression: listen_at removed whatever was at the path and bound its
    // own, so a second `raven-init --user` — launched again when the
    // compositor restarted — took the socket from the first and started a
    // second copy of every session service. Two wireplumbers then fought
    // over the default sink until nothing had one.
    let path = temp_socket("held");
    let first = control::listen_at(&path).expect("binds");
    assert!(control::is_live(std::path::Path::new(&path)));

    let second = control::listen_at(&path);
    assert!(second.is_err(), "a second listener took a live socket");
    assert!(
        control::is_live(std::path::Path::new(&path)),
        "the first supervisor must still be reachable"
    );

    drop(first);
    std::fs::remove_file(&path).ok();
}

#[test]
fn a_stale_socket_file_is_replaced() {
    // What the removal was always for: a socket left by a supervisor that is
    // gone answers nothing, and must not stop the next one binding.
    let path = temp_socket("stale");
    drop(control::listen_at(&path).expect("binds"));
    assert!(std::path::Path::new(&path).exists(), "the file outlives its listener");
    assert!(!control::is_live(std::path::Path::new(&path)));

    let listener = control::listen_at(&path).expect("rebinds over a stale file");
    assert!(control::is_live(std::path::Path::new(&path)));

    drop(listener);
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
    // Restarted, after the first backoff delay: the tick that sees the death
    // schedules it, and the tick at which it is due says yes.
    let now = Instant::now();
    assert!(
        !svc.should_restart_at(now),
        "the first tick after a crash schedules the restart rather than doing it"
    );
    let due = svc
        .retry_at()
        .expect("a crashed service with restart = true is scheduled");
    assert_eq!(due - now, service::restart_delay(1));
    assert!(
        svc.should_restart_at(due),
        "a crashed service with restart = true must be restarted once its delay is up"
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
fn restart_delays_double_to_a_ceiling() {
    use service::{restart_delay, RESTART_BACKOFF_MAX};
    let secs: Vec<u64> = (1..=9).map(|n| restart_delay(n).as_secs()).collect();
    assert_eq!(secs, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
    // However long a service has been looping, the wait stays at the ceiling
    // and the arithmetic stays in range.
    assert_eq!(restart_delay(u32::MAX), RESTART_BACKOFF_MAX);
}

#[test]
fn a_crash_looping_service_is_backed_off_not_given_up_on() {
    // The previous policy gave up after five deaths in a minute. For a login
    // daemon that meant a machine with no login screen and nobody able to
    // reach a shell to start it again. Now each death waits twice as long as
    // the last, up to a minute, forever -- and the decision is still made and
    // logged once per death, not on every 100ms supervisor tick.
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

    // Drive the supervisor with a clock of our own, so the schedule is
    // checked exactly rather than waited out.
    let mut now = Instant::now();
    for attempt in 1..=10u32 {
        svc.wait_for_exit(Duration::from_secs(2));
        let expected = service::restart_delay(attempt);

        // Not yet: the decision is made on the first tick, and it is a wait.
        assert!(
            !svc.should_restart_at(now),
            "attempt {attempt}: must not restart on the tick the death was seen"
        );
        let due = svc.retry_at().expect("a restart is scheduled");
        assert_eq!(due - now, expected, "attempt {attempt}");

        // Every tick until then answers no, from the recorded decision.
        let just_before = now + expected - Duration::from_millis(1);
        for _ in 0..50 {
            assert!(!svc.should_restart_at(just_before));
        }
        assert_eq!(svc.retry_at(), Some(due), "the decision must not move");

        now += expected;
        assert!(svc.should_restart_at(now), "attempt {attempt}: due now");
        assert_ne!(
            svc.state(),
            service::ServiceState::Failed,
            "backing off is not giving up"
        );
        let _ = svc.restart();
        assert_eq!(svc.restart_count(), attempt);
    }

    // Let the last restart die before the operator steps in.
    svc.wait_for_exit(Duration::from_secs(2));

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
    assert_eq!(
        svc.restart_count(),
        0,
        "an operator start resets the backoff"
    );
    let now = Instant::now();
    assert!(!svc.should_restart_at(now));
    assert_eq!(
        svc.retry_at().map(|at| at - now),
        Some(service::restart_delay(1)),
        "after an operator start the schedule begins again from the shortest wait"
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
        runtime_dirs: vec![rundir.display().to_string()],
        ..ServiceConfig::default()
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

/// Resource control end to end, which is the only way to test it: everything
/// interesting happens in a child process between `fork` and `exec`, where
/// nothing can be asserted and nothing can be logged, so the proof has to be
/// what the exec'd program sees.
///
/// The cgroup root is a temporary directory rather than /sys/fs/cgroup --
/// `ensure_slice` accepts any directory holding a `cgroup.controllers` file,
/// and the kernel's own semantics are not this test's business. What is this
/// test's business is that init writes the right number into the right file,
/// that the child joins its cgroup before it execs, and that an rlimit and a
/// nice value survive into the program: all four have silent failure modes
/// where the service starts, looks perfectly healthy, and has none of the
/// limits its definition asked for.
#[test]
fn a_service_is_started_inside_its_cgroup_and_under_its_limits() {
    let _env = CGROUP_ROOT_ENV
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = std::env::temp_dir().join(format!("raven-svc-cgroup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cgroup_root = root.join("cgroup");
    let out = root.join("seen-by-the-service");

    std::fs::create_dir_all(&cgroup_root).expect("temp cgroup root");
    std::fs::write(
        cgroup_root.join("cgroup.controllers"),
        "cpu io memory pids\n",
    )
    .expect("the probe cgroup2 is recognised by");

    // The kernel creates a cgroup's attribute files when the directory is
    // made; a temporary directory does not. `cgroup.procs` has to exist
    // beforehand because the child opens it without O_CREAT -- which is
    // exactly right against the real filesystem, where creating a file in a
    // cgroup directory is not a thing that can happen.
    //
    // $RAVEN_CGROUP_ROOT is what points the module at this tree rather than at
    // the machine's, and it is set before `ensure_slice` so that the entry
    // point init itself calls is the one under test here.
    std::env::set_var("RAVEN_CGROUP_ROOT", &cgroup_root);
    assert!(cgroup::ensure_slice(), "a tree with cgroup.controllers is usable");
    let svc_cgroup = cgroup_root.join(cgroup::SLICE_NAME).join("limited");
    std::fs::create_dir_all(&svc_cgroup).expect("service cgroup");
    std::fs::write(svc_cgroup.join("cgroup.procs"), "").expect("procs file");

    // Field 19 of /proc/self/stat is the nice value. The shell reports the
    // soft RLIMIT_NOFILE it inherited; 512 is below every default this can
    // run under, so the call is a lowering and needs no privilege.
    let cfg_svc = ServiceConfig {
        name: "limited".to_string(),
        exec: "/bin/sh".to_string(),
        args: vec![
            "-c".to_string(),
            format!(
                "ulimit -n > {out}; cut -d' ' -f19 /proc/self/stat >> {out}",
                out = out.display()
            ),
        ],
        nice: 5,
        memory_max: Some("256M".to_string()),
        cpu_weight: Some(200),
        limits: config::ResourceLimits {
            nofile: Some(512),
            ..Default::default()
        },
        ..ServiceConfig::default()
    };

    let mut svc = Service::start(&cfg_svc).expect("starts");
    svc.wait_for_exit(Duration::from_secs(5));
    std::env::remove_var("RAVEN_CGROUP_ROOT");

    let attr = |file: &str| {
        std::fs::read_to_string(svc_cgroup.join(file))
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    assert_eq!(attr("memory.max"), (256 * 1024 * 1024).to_string());
    assert_eq!(attr("cpu.weight"), "200");
    // "0" is how a process names itself to cgroup.procs; that the write
    // happened at all is what says the service did not exec outside its
    // cgroup, which is the failure nobody can see from the outside.
    assert_eq!(
        attr("cgroup.procs"),
        "0",
        "the service must join its cgroup before it execs"
    );

    let seen = std::fs::read_to_string(&out).expect("the service wrote what it saw");
    let mut lines = seen.lines();
    assert_eq!(lines.next(), Some("512"), "RLIMIT_NOFILE: {seen}");
    assert_eq!(lines.next(), Some("5"), "nice value: {seen}");

    std::fs::remove_dir_all(&root).ok();
}

/// Stopping a service must reach the processes that left its process group,
/// and the only way to prove which of the two paths did the stopping is to
/// make them disagree: the cgroup here lists a process that is *not* in the
/// service's process group, and nothing else is listed at all. A stop that
/// went out to the process group would kill the leader and leave that process
/// running, which is precisely the bluetoothd failure the slice was built for.
#[test]
fn stopping_a_service_signals_its_cgroup_and_falls_back_to_the_group() {
    let _env = CGROUP_ROOT_ENV
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let root = std::env::temp_dir().join(format!("raven-svc-stop-cg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cgroup_root = root.join("cgroup");
    std::fs::create_dir_all(&cgroup_root).expect("temp cgroup root");
    std::fs::write(cgroup_root.join("cgroup.controllers"), "cpu io memory pids\n")
        .expect("the probe cgroup2 is recognised by");
    let svc_cgroup = cgroup_root.join(cgroup::SLICE_NAME).join("escapee");
    std::fs::create_dir_all(&svc_cgroup).expect("service cgroup");
    std::fs::write(svc_cgroup.join("cgroup.procs"), "").expect("procs file");

    std::env::set_var("RAVEN_CGROUP_ROOT", &cgroup_root);

    // The stand-in for a daemon that called `setsid` and walked out of the
    // group init put it in: started separately, so it shares neither the
    // service's process group nor its session, and known to the supervisor
    // only through the cgroup.
    let mut escaped = std::process::Command::new("/bin/sleep")
        .arg("300")
        .spawn()
        .expect("a process outside the service's group");
    let escaped_pid = escaped.id() as i32;

    let cfg_svc = sleeper("escapee");
    let mut cfg = config_with(vec![cfg_svc.clone()]);
    let mut services = HashMap::new();
    let svc = Service::start(&cfg_svc).expect("starts");
    let leader = svc.pid().expect("leader pid").as_raw();
    services.insert("escapee".to_string(), svc);

    // Written after the start, because joining the cgroup is what the child
    // does to this file and the service's own "0" would otherwise be here.
    std::fs::write(svc_cgroup.join("cgroup.procs"), format!("{escaped_pid}\n"))
        .expect("procs file");

    let (reply, _) = control::dispatch("stop escapee", &mut services, &mut cfg);
    assert!(reply.contains("Stopping"), "{reply}");
    assert!(
        wait_gone(escaped_pid, Duration::from_secs(5)),
        "the stop must reach a process that is only in the cgroup"
    );
    escaped.wait().ok();
    assert!(
        nix::sys::signal::kill(nix::unistd::Pid::from_raw(leader), None).is_ok(),
        "this stop went to the cgroup, so it cannot also have gone to the process group"
    );

    // With the cgroup emptied -- a machine with no cgroup2, or a service
    // adopted from a raven-init that predates the slice -- the same stop has
    // to go back to the process group, which is what the fallback is for.
    std::fs::write(svc_cgroup.join("cgroup.procs"), "").expect("procs file");
    services.get_mut("escapee").expect("still there").stop();
    assert!(
        wait_gone(leader, Duration::from_secs(5)),
        "with nothing in the cgroup, the stop falls back to the process group"
    );

    std::env::remove_var("RAVEN_CGROUP_ROOT");
    std::fs::remove_dir_all(&root).ok();
}

/// `raven-rc status` has to answer "what is this service using", and the
/// published copy under /run has to not answer it: the publisher writes a file
/// whenever the text it renders differs from last time, so a memory counter in
/// that text is a write to /run every time a daemon touches a page. Both
/// halves are asserted here because the churn only shows up in production, as
/// a machine that writes to /run several times a second while doing nothing.
#[test]
fn status_reports_what_a_service_is_using_but_the_published_copy_does_not() {
    let _env = CGROUP_ROOT_ENV
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let root = std::env::temp_dir().join(format!("raven-svc-status-cg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cgroup_root = root.join("cgroup");
    let published = root.join("run");
    std::fs::create_dir_all(&cgroup_root).expect("temp cgroup root");
    std::fs::write(cgroup_root.join("cgroup.controllers"), "cpu io memory pids\n")
        .expect("the probe cgroup2 is recognised by");
    let svc_cgroup = cgroup_root.join(cgroup::SLICE_NAME).join("measured");
    std::fs::create_dir_all(&svc_cgroup).expect("service cgroup");
    std::fs::write(svc_cgroup.join("cgroup.procs"), "").expect("procs file");
    std::fs::write(svc_cgroup.join("memory.current"), "149118976\n").expect("memory.current");
    std::fs::write(svc_cgroup.join("pids.current"), "4\n").expect("pids.current");
    std::fs::write(
        svc_cgroup.join("cpu.stat"),
        "usage_usec 3104000\nuser_usec 2900000\nsystem_usec 204000\n",
    )
    .expect("cpu.stat");

    std::env::set_var("RAVEN_CGROUP_ROOT", &cgroup_root);

    let cfg_svc = sleeper("measured");
    let mut cfg = config_with(vec![cfg_svc.clone()]);
    let mut services = HashMap::new();
    services.insert(
        "measured".to_string(),
        Service::start(&cfg_svc).expect("starts"),
    );

    let (reply, _) = control::dispatch("status measured", &mut services, &mut cfg);
    assert!(reply.contains("memory       142.2M"), "{reply}");
    assert!(
        reply.contains("cpu          3.104s (2.900s user, 0.204s system)"),
        "{reply}"
    );
    assert!(reply.contains("processes    4"), "{reply}");
    // Tracked since the supervisor was written and shown nowhere until now.
    assert!(reply.contains("restarts     0"), "{reply}");

    let mut publisher = control::StatusPublisher::at(&published);
    publisher.publish(&services, &cfg);
    let one = std::fs::read_to_string(published.join("services").join("measured"))
        .expect("service published");
    assert!(one.contains("state        running"), "{one}");
    assert!(one.contains("restarts     0"), "{one}");
    assert!(
        !one.contains("memory") && !one.contains("cpu"),
        "the published copy must carry nothing that moves by itself: {one}"
    );

    services.get_mut("measured").expect("still there").kill();
    std::env::remove_var("RAVEN_CGROUP_ROOT");
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
    assert!(
        wait_gone(pid, Duration::from_secs(5)),
        "process should exit"
    );

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
    assert!(
        reply.contains("0644"),
        "the mode belongs in the message: {reply}"
    );

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
    std::fs::write(
        &main,
        "[system]\nhostname = \"t\"\n[[services]]\nname = \"other\"\nexec = \"/bin/true\"\n",
    )
    .unwrap();

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
    let dir = std::env::temp_dir().join(format!("raven-reload-{}-{}", std::process::id(), key));
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
    assert!(
        reply.contains("late-arrival"),
        "reload should name it: {reply}"
    );

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
    assert!(
        reply.contains("restart"),
        "should say how to apply: {reply}"
    );

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

// ---------------------------------------------------------------------------
// reexec
// ---------------------------------------------------------------------------

/// The verb answers with an action, like the other process-level verbs, and
/// names the binary it is about to run so an operator can see it resolved to
/// the right file. In a test the binary is this one: argv[0] is absolute.
#[test]
fn reexec_is_an_action_that_names_its_target() {
    let mut services = HashMap::new();
    let mut cfg = config_with(Vec::new());
    let _env = INIT_EXE_ENV
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (reply, action) = control::dispatch("reexec", &mut services, &mut cfg);
    assert_eq!(action, Action::Reexec, "{reply}");
    assert!(reply.starts_with("Re-executing /"), "{reply}");
}

/// A target that does not exist is an error to the client, not a promise the
/// main loop then fails to keep in the log.
#[test]
fn reexec_refuses_a_missing_binary() {
    let mut services = HashMap::new();
    let mut cfg = config_with(Vec::new());
    let _env = INIT_EXE_ENV
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::env::set_var("RAVEN_INIT_EXE", "/nonexistent/raven-init");
    let (reply, action) = control::dispatch("reexec", &mut services, &mut cfg);
    std::env::remove_var("RAVEN_INIT_EXE");
    assert_eq!(action, Action::None);
    assert!(reply.starts_with("error: cannot re-exec"), "{reply}");
}

/// The property a re-exec exists for: the process started by one supervisor is
/// the process the next one supervises. Snapshot, serialise, parse, adopt --
/// the same pid comes back running, and stopping it through the adopted
/// handle actually stops it.
#[test]
fn an_adopted_service_is_the_same_process() {
    let cfg_svc = sleeper("survivor");
    let original = Service::start(&cfg_svc).expect("start sleeper");
    let pid = original.pid().expect("pid").as_raw();

    let text = reexec::Handoff::new(vec![original.snapshot()])
        .to_toml()
        .unwrap();
    // The old supervisor is gone from here on; only the text crosses over.
    drop(original);

    let handoff = reexec::Handoff::from_toml(&text).unwrap();
    let snapshot = handoff.services.into_iter().next().unwrap();
    assert_eq!(snapshot.pid, Some(pid));

    let mut adopted = Service::adopt(snapshot, cfg_svc);
    assert!(adopted.is_running());
    assert_eq!(adopted.pid().map(|p| p.as_raw()), Some(pid));

    adopted.stop_by_request();
    assert!(wait_gone(pid, Duration::from_secs(5)), "SIGTERM through the adopted handle");
}

/// A pid that died in the hand-off window is reported as exited, which is the
/// state the supervisor restarts from -- not as running, which it would never
/// leave.
#[test]
fn a_dead_pid_is_adopted_as_exited() {
    let cfg_svc = sleeper("casualty");
    let mut svc = Service::start(&cfg_svc).expect("start sleeper");
    let pid = svc.pid().expect("pid").as_raw();
    let snapshot = svc.snapshot();
    svc.kill();
    assert!(wait_gone(pid, Duration::from_secs(5)));
    // Reap it, so the pid is truly gone rather than a zombie kill(0) can see.
    let _ = nix::sys::wait::waitpid(nix::unistd::Pid::from_raw(pid), None);

    let adopted = Service::adopt(snapshot, cfg_svc);
    assert!(!adopted.is_running());
    assert_eq!(adopted.state(), service::ServiceState::Exited);
}

/// "Stopped by request" survives the swap; otherwise a re-exec would be a
/// way to bring back every service an operator deliberately took down.
#[test]
fn a_manually_stopped_service_stays_stopped_across_adoption() {
    let cfg_svc = sleeper("parked");
    let mut svc = Service::start(&cfg_svc).expect("start sleeper");
    let pid = svc.pid().expect("pid").as_raw();
    svc.stop_by_request();
    assert!(wait_gone(pid, Duration::from_secs(5)));
    svc.poll_exit();

    let adopted = Service::adopt(svc.snapshot(), cfg_svc);
    assert!(!adopted.is_running());
    assert!(adopted.is_manually_stopped());
    assert_eq!(adopted.state(), service::ServiceState::Stopped);
}

#[test]
fn published_status_tracks_service_state_without_the_socket() {
    use std::os::unix::fs::PermissionsExt;

    let dir = format!(
        "{}/raven-init-test-status-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::remove_dir_all(&dir).ok();

    let cfg_svc = sleeper("pub-svc");
    let mut cfg = config_with(vec![cfg_svc.clone()]);
    let mut services = HashMap::new();
    services.insert(
        "pub-svc".to_string(),
        Service::start(&cfg_svc).expect("starts"),
    );

    let mut publisher = control::StatusPublisher::at(&dir);
    publisher.publish(&services, &cfg);

    let list = std::fs::read_to_string(format!("{dir}/status")).expect("list published");
    assert!(list.contains("pub-svc"), "{list}");
    assert!(list.contains("running"), "{list}");
    let one = std::fs::read_to_string(format!("{dir}/services/pub-svc")).expect("service published");
    assert!(one.contains("state        running"), "{one}");

    // World-readable, which is the point.
    let mode = std::fs::metadata(format!("{dir}/status")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o644, "status file mode");
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "status dir mode");

    // A change over the socket path shows up on the next publish, once the
    // main loop has reaped the exit (poll_exit stands in for the reaper here).
    let pid = services["pub-svc"].pid().expect("has a pid").as_raw();
    let (reply, _) = control::dispatch("stop pub-svc", &mut services, &mut cfg);
    assert!(!reply.starts_with("error:"), "{reply}");
    assert!(wait_gone(pid, Duration::from_secs(5)), "process should exit");
    services.get_mut("pub-svc").unwrap().poll_exit();
    publisher.publish(&services, &cfg);
    let one = std::fs::read_to_string(format!("{dir}/services/pub-svc")).expect("service published");
    assert!(one.contains("stopped (by request)"), "{one}");
    let list = std::fs::read_to_string(format!("{dir}/status")).expect("list published");
    assert!(list.contains("stopped (by request)"), "{list}");

    // A service that leaves the configuration loses its file.
    services.remove("pub-svc");
    cfg.services.clear();
    publisher.publish(&services, &cfg);
    assert!(
        !std::path::Path::new(&format!("{dir}/services/pub-svc")).exists(),
        "file for a removed service should be gone"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// A session's raven-init refuses the verbs that act on the machine, and
/// says which raven-rc to use instead. Everything about its own services
/// works as before.
#[test]
fn user_mode_refuses_machine_verbs_and_keeps_service_verbs() {
    let cfg_svc = sleeper("user-mode-svc");
    let mut cfg = config_with(vec![cfg_svc.clone()]);
    let mut services = HashMap::new();
    services.insert(
        "user-mode-svc".to_string(),
        Service::start(&cfg_svc).expect("starts"),
    );
    control::set_user_mode(true);
    for verb in ["poweroff", "halt", "reboot", "suspend", "sleep", "reexec"] {
        let (reply, action) = control::dispatch(verb, &mut services, &mut cfg);
        assert_eq!(action, Action::None, "{verb} must not act");
        assert!(reply.starts_with("error:") && reply.contains("--user"), "{verb}: {reply}");
    }
    let (reply, _) = control::dispatch("list", &mut services, &mut cfg);
    assert!(reply.contains("user-mode-svc"), "{reply}");
    let (reply, _) = control::dispatch("stop user-mode-svc", &mut services, &mut cfg);
    assert!(!reply.starts_with("error:"), "{reply}");
    control::set_user_mode(false);
    for svc in services.values_mut() {
        svc.kill();
    }
}

#[test]
fn blame_reports_start_and_ready_times_slowest_first() {
    let dir = format!(
        "{}/raven-init-test-blame-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    // One service with a ready path, one without.
    let ready_file = format!("{dir}/ready.sock");
    let mut with_path = sleeper("blame-ready");
    with_path.ready_path = Some(ready_file.clone());
    let plain = sleeper("blame-plain");
    let mut cfg = config_with(vec![with_path.clone(), plain.clone()]);

    let mut services = HashMap::new();
    services.insert(
        "blame-ready".to_string(),
        Service::start(&with_path).expect("starts"),
    );
    services.insert(
        "blame-plain".to_string(),
        Service::start(&plain).expect("starts"),
    );

    // Nothing is ready yet: the path does not exist. A milestone recorded
    // before the request shows up above the table.
    timeline::mark("test milestone");
    control::observe_readiness(&mut services);
    let (reply, _) = control::dispatch("blame", &mut services, &mut cfg);
    assert!(reply.contains("SERVICE"), "{reply}");
    assert!(reply.contains("test milestone"), "{reply}");
    assert!(reply.contains("not ready yet"), "{reply}");
    assert!(reply.contains("no ready path"), "{reply}");

    // The daemon "creates its socket"; the next tick notices.
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(&ready_file, b"").unwrap();
    control::observe_readiness(&mut services);
    let (reply, _) = control::dispatch("blame", &mut services, &mut cfg);
    let ready_line = reply
        .lines()
        .find(|l| l.starts_with("blame-ready"))
        .expect("a row for the ready service");
    let cols: Vec<&str> = ready_line.split_whitespace().collect();
    // name, started, ready, took
    assert!(cols.len() >= 4, "{ready_line}");
    let started: f64 = cols[1].parse().expect("started is a number");
    let ready: f64 = cols[2].parse().expect("ready is a number");
    let took: f64 = cols[3].parse().expect("took is a number");
    assert!(ready >= started, "{ready_line}");
    assert!(
        (0.03..5.0).contains(&took),
        "took should be about the sleep: {ready_line}"
    );
    // Slowest-to-ready first: the row with a took precedes the one without.
    let pos_ready = reply.find("blame-ready").unwrap();
    let pos_plain = reply.find("blame-plain").unwrap();
    assert!(pos_ready < pos_plain, "{reply}");
    assert!(reply.contains("span"), "{reply}");

    // Published for readers without root, alongside status.
    let mut publisher = control::StatusPublisher::at(format!("{dir}/pub"));
    publisher.publish(&services, &cfg);
    let published =
        std::fs::read_to_string(format!("{dir}/pub/blame")).expect("blame published");
    assert!(published.contains("blame-ready"), "{published}");

    for svc in services.values_mut() {
        svc.kill();
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// `raven-rc blame` reported powerd, controlsd, timed and fprintd all ready at
/// exactly 6.789s, each "taking" exactly 1.120s. None of them was slow and none
/// of them had anything to do with the others: readiness was noticed when the
/// main loop next looked, all four were looked at inside one `for` loop, and
/// the loop's idle sleep is two seconds. Every digit after the decimal point
/// was a property of the tick.
///
/// This drives the three pieces main_loop_at now wires together -- arm the
/// watches, sleep in poll, look again -- and asserts the sleep ends when the
/// file appears rather than when the timer runs out.
#[test]
fn a_ready_file_appearing_wakes_the_supervisor_rather_than_a_tick() {
    use std::os::fd::AsFd;

    let dir = format!(
        "{}/raven-init-test-watch-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let ready_file = format!("{dir}/ready.sock");

    let mut cfg_svc = sleeper("watched-ready");
    cfg_svc.ready_path = Some(ready_file.clone());
    let mut services = HashMap::new();
    services.insert(
        "watched-ready".to_string(),
        Service::start(&cfg_svc).expect("starts"),
    );

    let mut watcher = readiness::Watcher::new().expect("inotify");

    // Exactly what the main loop does at the top of each pass, in the order it
    // does it: arm every directory still being waited on, then look. Nothing
    // is there yet, so nothing is ready.
    assert!(
        watcher.arm(
            services
                .values()
                .filter(|svc| svc.is_running() && svc.ready_at().is_none())
                .filter_map(|svc| svc.ready_path()),
        ),
        "the directory exists, so it must be watchable"
    );
    control::observe_readiness(&mut services);
    assert!(services["watched-ready"].ready_at().is_none());

    // With the watch armed, waiting for a ready path is no longer time-driven
    // work, so the loop is allowed to sleep for its full idle interval instead
    // of waking ten times a second to stat a file.
    assert!(
        !control::wants_quick_tick(&services, true),
        "a watched ready path must not hold the loop at the busy tick"
    );
    assert!(
        control::wants_quick_tick(&services, false),
        "an unwatchable ready path must still keep the old timer"
    );

    // The daemon creates its socket while the supervisor is asleep.
    let writer = ready_file.clone();
    let hand = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(60));
        std::fs::write(&writer, b"").expect("ready file");
    });

    let began = Instant::now();
    {
        use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
        // Two seconds is the loop's IDLE interval. Before the watch existed
        // this is how long the four services above waited to be noticed.
        let mut fds = [PollFd::new(watcher.as_fd(), PollFlags::POLLIN)];
        poll(&mut fds, PollTimeout::from(2000u16)).expect("poll");
    }
    let woke = began.elapsed();
    watcher.drain();
    control::observe_readiness(&mut services);
    hand.join().expect("writer");

    assert!(
        woke >= Duration::from_millis(50),
        "woke before the file was written: {woke:?}"
    );
    assert!(
        woke < Duration::from_millis(1500),
        "the poll waited out its timeout instead of being woken: {woke:?}"
    );

    let ready_at = services["watched-ready"]
        .ready_at()
        .expect("the event must have produced a ready time");
    assert!(
        ready_at.elapsed() < Duration::from_millis(500),
        "the ready time must be stamped when the event arrived, not later"
    );
    let started = services["watched-ready"].started_at().expect("started");
    let took = ready_at.duration_since(started);
    assert!(
        took >= Duration::from_millis(50) && took < Duration::from_millis(1500),
        "took should be about the write, not about a tick: {took:?}"
    );

    // Once the service is ready nothing is waiting on that directory, so the
    // watch goes away and a settled machine is woken by nothing.
    assert!(watcher.arm(
        services
            .values()
            .filter(|svc| svc.is_running() && svc.ready_at().is_none())
            .filter_map(|svc| svc.ready_path()),
    ));

    for svc in services.values_mut() {
        svc.kill();
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn ready_time_survives_a_snapshot() {
    let dir = format!(
        "{}/raven-init-test-snap-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let ready_file = format!("{dir}/ready.sock");
    let mut cfg_svc = sleeper("snap-ready");
    cfg_svc.ready_path = Some(ready_file.clone());
    let mut svc = Service::start(&cfg_svc).expect("starts");
    std::fs::write(&ready_file, b"").unwrap();
    assert!(svc.note_ready_if_present());
    assert!(
        !svc.note_ready_if_present(),
        "second look must not reset the first time"
    );
    let first = svc.ready_at();
    std::thread::sleep(Duration::from_millis(5));
    svc.mark_ready();
    assert_eq!(svc.ready_at(), first, "mark_ready must keep the first time");
    let snap = svc.snapshot();
    assert!(snap.ready_secs_ago.is_some());
    let adopted = Service::adopt(snap, cfg_svc);
    assert!(adopted.ready_at().is_some(), "ready time lost across adopt");
    svc.kill();
    std::fs::remove_dir_all(&dir).ok();
}

/// The bug `blame` was reported for, in miniature.
///
/// On the live machine the table was headed "seconds since the kernel
/// started", its milestones ended at seven, and obexd's row said 97618 --
/// because obexd had restarted after a resume, and the column was reading the
/// current run. cawd said 2751 for the same reason, and the footer folded them
/// both into `span 97613.069s`, which is the machine's uptime with a boot
/// time's name on it.
///
/// A restart must move nothing in this table: the boot happened when it
/// happened.
#[test]
fn blame_reports_the_first_start_not_the_latest_restart() {
    let dir = format!(
        "{}/raven-init-test-blame-first-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let ready_file = format!("{dir}/ready.sock");

    let mut cfg_svc = sleeper("blame-restarted");
    cfg_svc.ready_path = Some(ready_file.clone());
    let mut cfg = config_with(vec![cfg_svc.clone()]);
    let mut services = HashMap::new();
    services.insert(
        "blame-restarted".to_string(),
        Service::start(&cfg_svc).expect("starts"),
    );

    // It comes up and becomes ready, as it would at boot.
    std::fs::write(&ready_file, b"").unwrap();
    control::observe_readiness(&mut services);
    let first_started = services["blame-restarted"]
        .first_started_at()
        .expect("a first start");
    let first_ready = services["blame-restarted"]
        .first_ready_at()
        .expect("a first ready");

    // Much later -- here, 80ms later -- it dies and the supervisor brings it
    // back. On the machine this was a suspend and a day.
    std::thread::sleep(Duration::from_millis(80));
    let svc = services.get_mut("blame-restarted").unwrap();
    let pid = svc.pid().expect("pid").as_raw();
    svc.kill();
    assert!(wait_gone(pid, Duration::from_secs(5)));
    svc.restart().expect("comes back");

    // The current run moved; the boot did not.
    assert!(
        svc.started_at().expect("a current run") > first_started,
        "the restart must move the current run's start"
    );
    assert_eq!(svc.first_started_at(), Some(first_started));
    assert_eq!(svc.first_ready_at(), Some(first_ready));
    assert!(
        svc.ready_at().is_none(),
        "the new run has not reached its ready path yet"
    );

    let (reply, _) = control::dispatch("blame", &mut services, &mut cfg);
    let row = reply
        .lines()
        .find(|l| l.starts_with("blame-restarted"))
        .expect("a row for the restarted service")
        .to_string();
    let cols: Vec<&str> = row.split_whitespace().collect();
    let started: f64 = cols[1].parse().expect("started is a number");
    let ready: f64 = cols[2].parse().expect("ready is a number");
    assert!(
        (started - timeline::instant_secs(first_started)).abs() < 0.002,
        "blame must report the first start: {row}"
    );
    assert!(
        (ready - timeline::instant_secs(first_ready)).abs() < 0.002,
        "blame must report the first ready: {row}"
    );
    assert!(
        !row.contains("not ready yet"),
        "a service that was ready at boot is not 'not ready yet': {row}"
    );

    // And the footer measures the boot, not the afternoon.
    let footer = reply
        .lines()
        .find(|l| l.starts_with("services:"))
        .expect("a summary");
    let span: f64 = footer
        .rsplit_once("span ")
        .and_then(|(_, s)| s.trim_end_matches("s").parse().ok())
        .expect("a span");
    assert!(
        span < 1.0,
        "the span is the boot's, and this one took milliseconds: {footer}"
    );

    for svc in services.values_mut() {
        svc.kill();
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// "This service has nothing to wait for" and "this service has something to
/// wait for and it has never happened" are different machines to be standing
/// in front of, and used to print the same bare dash.
#[test]
fn a_service_that_was_never_ready_reads_differently_from_one_with_no_ready_path() {
    let dir = format!(
        "{}/raven-init-test-blame-never-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    let mut waiting = sleeper("never-ready");
    waiting.ready_path = Some(format!("{dir}/never.sock"));
    let plain = sleeper("no-ready-path");
    let mut cfg = config_with(vec![waiting.clone(), plain.clone()]);

    let mut services = HashMap::new();
    services.insert(
        "never-ready".to_string(),
        Service::start(&waiting).expect("starts"),
    );
    services.insert(
        "no-ready-path".to_string(),
        Service::start(&plain).expect("starts"),
    );
    control::observe_readiness(&mut services);

    let row_of = |reply: &str, name: &str| -> String {
        reply
            .lines()
            .find(|l| l.starts_with(name))
            .unwrap_or_else(|| panic!("a row for {name}"))
            .to_string()
    };

    let (reply, _) = control::dispatch("blame", &mut services, &mut cfg);
    let waiting_row = row_of(&reply, "never-ready");
    let plain_row = row_of(&reply, "no-ready-path");
    assert_eq!(
        waiting_row.split_whitespace().nth(2),
        Some("waiting"),
        "{waiting_row}"
    );
    assert!(waiting_row.contains("not ready yet"), "{waiting_row}");
    assert_eq!(
        plain_row.split_whitespace().nth(2),
        Some("-"),
        "{plain_row}"
    );
    assert!(plain_row.contains("no ready path"), "{plain_row}");

    // Nothing became ready, so there is no span to report -- and the start
    // times must not be folded in as though there were one.
    let footer = reply
        .lines()
        .find(|l| l.starts_with("services:"))
        .expect("a summary");
    assert!(footer.contains("nothing ready yet"), "{footer}");
    assert!(!footer.contains("span"), "{footer}");

    // It dies without ever having been ready, which is a third thing again.
    services.get_mut("never-ready").unwrap().kill();
    let (reply, _) = control::dispatch("blame", &mut services, &mut cfg);
    let waiting_row = row_of(&reply, "never-ready");
    assert_eq!(
        waiting_row.split_whitespace().nth(2),
        Some("never"),
        "{waiting_row}"
    );
    assert!(waiting_row.contains("exited before ready"), "{waiting_row}");

    for svc in services.values_mut() {
        svc.kill();
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A daemon somebody starts by hand at lunchtime is not part of how long the
/// machine took to boot. It belongs in the table -- it is running, and `blame`
/// lists what is running -- but folding it into the summary is how `span` came
/// to be reported in five figures.
///
/// The boundary is passed in rather than marked, because the milestone list is
/// process-wide and `cargo test` runs these in threads of one process.
#[test]
fn a_service_started_after_boot_is_marked_and_left_out_of_the_span() {
    let dir = format!(
        "{}/raven-init-test-blame-late-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    let mut early = sleeper("booted-with-the-machine");
    early.ready_path = Some(format!("{dir}/early.sock"));
    let mut late = sleeper("started-at-lunchtime");
    late.ready_path = Some(format!("{dir}/late.sock"));
    let cfg = config_with(vec![early.clone(), late.clone()]);

    let mut services = HashMap::new();
    services.insert(
        "booted-with-the-machine".to_string(),
        Service::start(&early).expect("starts"),
    );
    std::fs::write(format!("{dir}/early.sock"), b"").unwrap();
    control::observe_readiness(&mut services);

    // Boot ends here -- a clear 20ms after the early service became ready,
    // because the footer prints to three decimal places and a boundary drawn
    // within half a millisecond of a time it is compared against is a test
    // that fails on rounding a run in ten.
    std::thread::sleep(Duration::from_millis(20));
    let boot_done = timeline::monotonic_secs();
    std::thread::sleep(Duration::from_millis(40));

    services.insert(
        "started-at-lunchtime".to_string(),
        Service::start(&late).expect("starts"),
    );
    std::fs::write(format!("{dir}/late.sock"), b"").unwrap();
    control::observe_readiness(&mut services);

    let text = control::blame_services(&services, &cfg, Some(boot_done));
    let late_row = text
        .lines()
        .find(|l| l.starts_with("started-at-lunchtime"))
        .expect("the late service is still listed")
        .to_string();
    assert!(late_row.contains("started after boot"), "{late_row}");
    let early_row = text
        .lines()
        .find(|l| l.starts_with("booted-with-the-machine"))
        .expect("a row")
        .to_string();
    assert!(!early_row.contains("started after boot"), "{early_row}");

    let footer = text
        .lines()
        .find(|l| l.starts_with("services:"))
        .expect("a summary");
    let last_ready: f64 = footer
        .rsplit_once("last ready ")
        .and_then(|(_, s)| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
        .expect("a last ready");
    assert!(
        last_ready < boot_done,
        "the span must end where boot ended: {footer}"
    );

    // With no boundary -- an init that never reached its main loop, or a test
    // driving this by hand -- everything counts, because there is nothing to
    // say otherwise.
    let text = control::blame_services(&services, &cfg, None);
    assert!(
        !text.contains("started after boot"),
        "nothing can be after a boundary that does not exist: {text}"
    );

    for svc in services.values_mut() {
        svc.kill();
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// `blame` prints three decimal places, and across a re-exec every one of them
/// used to be invented: the hand-off carried whole seconds, written with
/// `as_secs()`, which floors. A service that started at 3.998 came back as one
/// that started at 3.000, and `took` was the difference of two such numbers.
#[test]
fn a_reexec_keeps_the_milliseconds_blame_prints() {
    let dir = format!(
        "{}/raven-init-test-precision-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let ready_file = format!("{dir}/ready.sock");

    let mut cfg_svc = sleeper("precise");
    cfg_svc.ready_path = Some(ready_file.clone());
    let mut svc = Service::start(&cfg_svc).expect("starts");
    std::fs::write(&ready_file, b"").unwrap();
    assert!(svc.note_ready_if_present());

    let started = timeline::instant_secs(svc.first_started_at().expect("a first start"));
    let ready = timeline::instant_secs(svc.first_ready_at().expect("a first ready"));

    // The hand-off is written some time after the service started, which is
    // where the old format lost a second of the fraction.
    std::thread::sleep(Duration::from_millis(120));
    let text = reexec::Handoff::new(vec![svc.snapshot()])
        .to_toml()
        .expect("serialises");
    let snapshot = reexec::Handoff::from_toml(&text)
        .expect("parses")
        .services
        .into_iter()
        .next()
        .expect("one service");
    assert!(
        snapshot.started_mono.is_some(),
        "an absolute start is carried"
    );
    assert!(snapshot.first_ready_mono.is_some(), "so is the first ready");

    let adopted = Service::adopt(snapshot, cfg_svc);
    let adopted_started =
        timeline::instant_secs(adopted.first_started_at().expect("first start survives"));
    let adopted_ready =
        timeline::instant_secs(adopted.first_ready_at().expect("first ready survives"));
    assert!(
        (adopted_started - started).abs() < 0.002,
        "start moved by {}s across the hand-off",
        adopted_started - started
    );
    assert!(
        (adopted_ready - ready).abs() < 0.002,
        "ready moved by {}s across the hand-off",
        adopted_ready - ready
    );

    svc.kill();
    std::fs::remove_dir_all(&dir).ok();
}

/// The hand-off written by the raven-init already installed on a machine has
/// no absolute readings in it, and a re-exec onto this image has to adopt it
/// anyway. The whole seconds are believed when there is nothing better -- and
/// not believed when they say nothing, because `uptime_secs = 0` means both
/// "started this instant" and "never started at all".
#[test]
fn an_older_handoff_still_gives_a_service_a_start_time() {
    let cfg_svc = sleeper("from-an-older-init");

    let carried = service::ServiceSnapshot {
        config: cfg_svc.clone(),
        pid: None,
        uptime_secs: 5,
        ..Default::default()
    };
    let adopted = Service::adopt(carried, cfg_svc.clone());
    let age = adopted
        .first_started_at()
        .expect("an older hand-off still dates the run it describes")
        .elapsed()
        .as_secs_f64();
    assert!((age - 5.0).abs() < 0.5, "adopted as {age}s old");

    let silent = service::ServiceSnapshot {
        config: cfg_svc.clone(),
        pid: None,
        ..Default::default()
    };
    assert!(
        Service::adopt(silent, cfg_svc).first_started_at().is_none(),
        "a service that has never run must not be given a start time of now"
    );
}

/// Restarts are a fact about a service's life rather than about the boot, so
/// `status` is where they are counted -- and a count with no date on it cannot
/// tell a service that flapped last Tuesday from one flapping now.
#[test]
fn status_says_when_a_service_last_came_back() {
    let dir = format!(
        "{}/raven-init-test-last-restart-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    std::fs::remove_dir_all(&dir).ok();

    let cfg_svc = sleeper("comeback");
    let mut cfg = config_with(vec![cfg_svc.clone()]);
    let mut svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("pid").as_raw();
    svc.kill();
    assert!(wait_gone(pid, Duration::from_secs(5)));
    svc.restart().expect("comes back");

    let mut services = HashMap::new();
    services.insert("comeback".to_string(), svc);

    let (reply, _) = control::dispatch("status comeback", &mut services, &mut cfg);
    assert!(reply.contains("restarts     1"), "{reply}");
    assert!(reply.contains("last restart"), "{reply}");
    assert!(reply.contains("into this boot"), "{reply}");
    // Asked over the socket, the answer carries the live reading too.
    assert!(reply.contains(" ago)"), "{reply}");
    // And the current run is dated, which is the number `blame` does not show.
    assert!(reply.contains("started      "), "{reply}");

    // The published copy is written whenever its text changes, so it carries
    // the boot-clock time and not the "four minutes ago" that would differ on
    // every tick for as long as the machine is up.
    let mut publisher = control::StatusPublisher::at(format!("{dir}/pub"));
    publisher.publish(&services, &cfg);
    let published =
        std::fs::read_to_string(format!("{dir}/pub/services/comeback")).expect("status published");
    assert!(published.contains("last restart"), "{published}");
    assert!(!published.contains(" ago)"), "{published}");

    for svc in services.values_mut() {
        svc.kill();
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A re-exec replaces the supervisor and nothing else, so the boot it is
/// supervising is still the same boot. The milestones come back in front of
/// this image's own, and a name that now appears twice resolves to the first
/// of the two -- which is the one that says where boot ended.
#[test]
fn the_boot_timeline_comes_back_in_front_of_this_images_own() {
    timeline::mark("carried-milestone-test");
    let mine = timeline::first_milestone("carried-milestone-test").expect("just marked");

    timeline::restore(vec![("carried-milestone-test".to_string(), 0.5)]);

    let all = timeline::milestones();
    let first = all
        .iter()
        .position(|(n, _)| n == "carried-milestone-test")
        .expect("in the list");
    assert_eq!(all[first].1, 0.5, "the carried one comes first");
    assert!(
        all.iter()
            .any(|(n, at)| n == "carried-milestone-test" && *at == mine),
        "this image's own milestone is still there"
    );
    assert_eq!(
        timeline::first_milestone("carried-milestone-test"),
        Some(0.5),
        "the original boot's is the one a boundary is taken from"
    );
}

/// A service whose job is to finish, for the one-shot tests.
///
/// `/bin/sleep` rather than a shell one-liner where a duration matters: the
/// point of these tests is what the supervisor does while a one-shot is still
/// running, and a shell that has to be started first is a second process's
/// worth of noise in the middle of the measurement.
fn oneshot(name: &str, exec: &str, args: &[&str]) -> ServiceConfig {
    ServiceConfig {
        name: name.to_string(),
        description: format!("test one-shot {}", name),
        exec: exec.to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
        service_type: config::ServiceType::Oneshot,
        ..ServiceConfig::default()
    }
}

/// Every boot, `raven-rc list` described udev's coldplug, console-font and the
/// DHCP pass as "exited" -- the same word it uses for a daemon that died --
/// and `raven-rc blame` put "no ready path" beside them, which is true of
/// every one-shot there has ever been and says nothing about any of them.
#[test]
fn a_finished_oneshot_is_completed_rather_than_exited() {
    let cfg_svc = oneshot("coldplug", "/bin/true", &[]);
    let mut cfg = config_with(vec![cfg_svc.clone()]);

    let mut svc = Service::start(&cfg_svc).expect("starts");
    assert_eq!(
        svc.wait_until_finished(Duration::from_secs(5)),
        Some(service::OneshotOutcome::Completed),
        "/bin/true exits zero"
    );

    let mut services = HashMap::new();
    services.insert("coldplug".to_string(), svc);

    let (list, _) = control::dispatch("list", &mut services, &mut cfg);
    let row = list
        .lines()
        .find(|l| l.starts_with("coldplug"))
        .expect("listed");
    assert!(row.contains("completed"), "{row}");
    assert!(!row.contains("exited"), "{row}");

    let (status, _) = control::dispatch("status coldplug", &mut services, &mut cfg);
    assert!(status.contains("state        completed"), "{status}");
    assert!(status.contains("completed in "), "{status}");

    let (blame, _) = control::dispatch("blame", &mut services, &mut cfg);
    let row = blame
        .lines()
        .find(|l| l.starts_with("coldplug"))
        .expect("in the table");
    assert!(row.contains("completed in "), "{row}");
    assert!(!row.contains("no ready path"), "{row}");
    // The finish is the one-shot's readiness, so the READY column is a time
    // and TOOK is how long the work took -- not two dashes.
    assert!(!row.contains(" -  "), "{row}");
}

/// The one exit on this machine that is unambiguously a failure. It used to be
/// spelled exactly like the successful one.
#[test]
fn a_oneshot_that_exits_non_zero_is_a_failure_not_an_exit() {
    let cfg_svc = oneshot("badplug", "/bin/false", &[]);
    let mut cfg = config_with(vec![cfg_svc.clone()]);

    let mut svc = Service::start(&cfg_svc).expect("starts");
    let outcome = svc.wait_until_finished(Duration::from_secs(5));
    assert!(
        matches!(outcome, Some(service::OneshotOutcome::Failed(code)) if code != 0),
        "{outcome:?}"
    );

    let mut services = HashMap::new();
    services.insert("badplug".to_string(), svc);

    let (list, _) = control::dispatch("list", &mut services, &mut cfg);
    let row = list
        .lines()
        .find(|l| l.starts_with("badplug"))
        .expect("listed");
    assert!(row.contains("failed (status 1)"), "{row}");

    let (blame, _) = control::dispatch("blame", &mut services, &mut cfg);
    let row = blame
        .lines()
        .find(|l| l.starts_with("badplug"))
        .expect("in the table");
    assert!(row.contains("failed (status 1)"), "{row}");
}

/// The substantive half of the one-shot type: `after = ["<a one-shot>"]` means
/// "once that has finished", not "once that has been forked".
///
/// Before this, the two spawns were tens of microseconds apart and the
/// ordering everything after udev's coldplug was written to express was
/// satisfied by luck.
#[test]
fn a_service_after_a_oneshot_waits_for_it_to_finish() {
    let mut work = oneshot("slow-work", "/bin/sleep", &["0.3"]);
    work.enabled = false;
    let mut dependent = sleeper("after-the-work");
    dependent.enabled = false;
    dependent.after = vec!["slow-work".to_string()];

    let mut cfg = config_with(vec![work, dependent]);
    let mut services = HashMap::new();

    let started_asking = Instant::now();
    let (reply, _) = control::dispatch("start after-the-work", &mut services, &mut cfg);
    assert!(reply.contains("Started after-the-work"), "{reply}");
    assert!(
        started_asking.elapsed() >= Duration::from_millis(250),
        "the start returned before the one-shot could have finished"
    );

    let finished = services["slow-work"]
        .ready_at()
        .expect("a completed one-shot records when it finished");
    let dependent_started = services["after-the-work"]
        .started_at()
        .expect("the dependant started");
    assert!(
        dependent_started >= finished,
        "the dependant started before its one-shot finished"
    );

    services.get_mut("after-the-work").unwrap().kill();
}

/// A one-shot that has finished is the state its dependants were waiting for,
/// so a second dependant must not set it going again. Without this, every
/// `raven-rc start` of anything ordered after udev would re-run the coldplug.
#[test]
fn a_finished_oneshot_is_not_run_again_for_the_next_dependant() {
    let mut work = oneshot("done-once", "/bin/true", &[]);
    work.enabled = false;
    let mut first = sleeper("first-dependant");
    first.enabled = false;
    first.after = vec!["done-once".to_string()];
    let mut second = sleeper("second-dependant");
    second.enabled = false;
    second.after = vec!["done-once".to_string()];

    let mut cfg = config_with(vec![work, first, second]);
    let mut services = HashMap::new();

    control::dispatch("start first-dependant", &mut services, &mut cfg);
    let finished = services["done-once"].ready_at().expect("it completed");

    control::dispatch("start second-dependant", &mut services, &mut cfg);
    assert_eq!(
        services["done-once"].ready_at(),
        Some(finished),
        "the one-shot was run a second time"
    );
    assert_eq!(
        services["done-once"].restart_count(),
        0,
        "the one-shot was restarted"
    );

    services.get_mut("first-dependant").unwrap().kill();
    services.get_mut("second-dependant").unwrap().kill();
}

/// `raven-rc start` on a one-shot that works reported an error and named a log
/// file containing a successful run -- and, because `start_service` abandons a
/// dependency whose reply begins "error:", that error also stopped everything
/// ordered after it from starting at all.
#[test]
fn starting_a_oneshot_by_hand_reports_completion_not_an_immediate_death() {
    let mut work = oneshot("by-hand", "/bin/true", &[]);
    work.enabled = false;
    let mut cfg = config_with(vec![work]);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("start by-hand", &mut services, &mut cfg);
    assert!(reply.starts_with("Completed by-hand"), "{reply}");
    assert!(!reply.contains("error:"), "{reply}");
}

/// A one-shot that fails does not stop what is ordered after it: `after` is an
/// ordering, and a coldplug that could not probe one device is not a machine
/// with no devices. The failure is reported rather than acted on.
#[test]
fn a_failed_oneshot_is_reported_but_does_not_block_its_dependants() {
    let mut work = oneshot("fails-once", "/bin/false", &[]);
    work.enabled = false;
    let mut dependent = sleeper("still-starts");
    dependent.enabled = false;
    dependent.after = vec!["fails-once".to_string()];

    let mut cfg = config_with(vec![work, dependent]);
    let mut services = HashMap::new();

    let (reply, _) = control::dispatch("start still-starts", &mut services, &mut cfg);
    assert!(
        reply.contains("warning: fails-once failed with status 1"),
        "{reply}"
    );
    assert!(reply.contains("Started still-starts"), "{reply}");
    assert!(services["still-starts"].is_running());

    services.get_mut("still-starts").unwrap().kill();
}

/// A one-shot still working when `ready_timeout` runs out has not failed. The
/// supervisor has merely stopped waiting, because the alternative is a boot a
/// slow coldplug can hang forever.
#[test]
fn a_oneshot_that_outlasts_its_timeout_is_not_called_a_failure() {
    let cfg_svc = oneshot("still-going", "/bin/sleep", &["30"]);
    let mut svc = Service::start(&cfg_svc).expect("starts");

    let outcome = svc.wait_until_finished(Duration::from_millis(60));
    assert_eq!(
        outcome,
        Some(service::OneshotOutcome::Running),
        "{outcome:?}"
    );

    svc.kill();
}

/// A service that is not a one-shot has no finishing to wait for, and asking
/// must not turn into a `ready_timeout`-long sleep on PID 1's thread.
#[test]
fn waiting_on_a_service_that_is_not_a_oneshot_returns_at_once() {
    let cfg_svc = sleeper("ordinary");
    let mut svc = Service::start(&cfg_svc).expect("starts");

    let asked = Instant::now();
    assert_eq!(svc.wait_until_finished(Duration::from_secs(30)), None);
    assert!(asked.elapsed() < Duration::from_secs(1));

    svc.kill();
}

/// A re-exec carries a service's times and not its exit status, so a one-shot
/// that had already finished came back from the hand-off with nothing to say
/// for itself. Its recorded finish is the evidence, and it is carried.
#[test]
fn a_completed_oneshot_is_still_completed_after_a_reexec() {
    let cfg_svc = oneshot("carried-over", "/bin/true", &[]);
    let mut svc = Service::start(&cfg_svc).expect("starts");
    assert_eq!(
        svc.wait_until_finished(Duration::from_secs(5)),
        Some(service::OneshotOutcome::Completed)
    );

    let adopted = Service::adopt(svc.snapshot(), cfg_svc.clone());
    assert_eq!(
        adopted.oneshot_outcome(),
        Some(service::OneshotOutcome::Completed),
        "the hand-off lost that the one-shot had finished"
    );

    let mut cfg = config_with(vec![cfg_svc]);
    let mut services = HashMap::new();
    services.insert("carried-over".to_string(), adopted);
    let (list, _) = control::dispatch("list", &mut services, &mut cfg);
    let row = list
        .lines()
        .find(|l| l.starts_with("carried-over"))
        .expect("listed");
    assert!(row.contains("completed"), "{row}");
}

/// A service's cgroup directory has to go away when the service does, and has
/// to stay when something the service forked did not.
///
/// The removal is wired into `mark_exited` and `mark_signaled` rather than
/// into the reaper, because those two are the only places that know a service
/// has died no matter which path it died down -- the main loop's `waitpid`,
/// `wait_for_exit` after a stop, or an adopted child going away across a
/// re-exec. `Cgroup::remove`'s own unit test covers what `rmdir` answers; what
/// this covers is that anything calls it at all, which is the failure that
/// looks like nothing: every service that ever ran leaves a directory behind,
/// `raven-rc status` reports stale numbers for a process that exited hours
/// ago, and nobody notices until the slice has a thousand children in it.
///
/// The "still in use" half is what EBUSY means on cgroupfs. A temporary
/// directory cannot produce EBUSY, so it produces the nearest thing an
/// ordinary filesystem has -- a directory that is not empty, and ENOTEMPTY --
/// which is the other error `remove` treats as "leave it alone". The empty
/// half needs the attribute files gone, because on real cgroupfs those are
/// synthetic and `rmdir` ignores them, while here they are real files that
/// would keep the directory alive and make this test pass for the wrong
/// reason.
#[test]
fn a_service_gives_its_cgroup_back_when_it_exits() {
    let _env = CGROUP_ROOT_ENV
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let root = std::env::temp_dir().join(format!("raven-svc-cgroup-rm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cgroup_root = root.join("cgroup");
    std::fs::create_dir_all(&cgroup_root).expect("temp cgroup root");
    std::fs::write(cgroup_root.join("cgroup.controllers"), "cpu io memory pids\n")
        .expect("the probe cgroup2 is recognised by");

    std::env::set_var("RAVEN_CGROUP_ROOT", &cgroup_root);
    assert!(cgroup::ensure_slice(), "a tree with cgroup.controllers is usable");

    let slice = cgroup_root.join(cgroup::SLICE_NAME);
    let ephemeral = slice.join("ephemeral");
    let lingering = slice.join("lingering");
    for dir in [&ephemeral, &lingering] {
        std::fs::create_dir_all(dir).expect("service cgroup");
        // The child opens this without O_CREAT, exactly as it would against
        // the real filesystem, so it has to exist before the service starts.
        std::fs::write(dir.join("cgroup.procs"), "").expect("procs file");
    }

    // One that leaves nothing behind. `/bin/true` has exited by the time
    // `wait_for_exit` returns, and the attribute files are cleared out first
    // so that what is left is a directory holding nothing -- which is what a
    // cgroup whose last process is gone looks like to `rmdir`.
    let finished = oneshot("ephemeral", "/bin/true", &[]);
    let mut svc = Service::start(&finished).expect("starts");
    for entry in std::fs::read_dir(&ephemeral).expect("readable") {
        std::fs::remove_file(entry.expect("entry").path()).expect("clearable");
    }
    svc.wait_for_exit(Duration::from_secs(5));
    assert!(
        !ephemeral.exists(),
        "a service that exited kept its cgroup directory"
    );

    // And one that does. The procs file standing in for a surviving process is
    // what makes the directory non-empty, and a service whose grandchild
    // outlived it is the case the directory is worth keeping for: it is still
    // what `raven-rc status` counts and what the next `stop` signals.
    let survivor = oneshot("lingering", "/bin/true", &[]);
    let mut svc = Service::start(&survivor).expect("starts");
    svc.wait_for_exit(Duration::from_secs(5));
    assert!(
        lingering.exists(),
        "a cgroup that still has something in it must not be taken away"
    );

    std::env::remove_var("RAVEN_CGROUP_ROOT");
    std::fs::remove_dir_all(&root).ok();
}

/// Serialises the tests that set `$RAVEN_PRE_EXEC_TIMEOUT_MS`, for the reason
/// `CGROUP_ROOT_ENV` exists: the variable is process-global and these run in
/// threads of one process.
static PRE_EXEC_ENV: Mutex<()> = Mutex::new(());

/// A hook that never finishes must fail the start, not the boot.
///
/// `pre_exec` was waited on with `Command::status()` -- no timeout at all --
/// from `start_services`, which runs on PID 1's only thread before the main
/// loop exists. The documented example is sshd's `ssh-keygen -A`, which blocks
/// in `getrandom(2)` until the kernel CRNG is initialised; take the
/// documentation's advice on a machine with no entropy source and the boot
/// stops mid-list with no error, no console prompt, no control socket and
/// nothing reaping. `/bin/sleep 300` stands in for the hook that is not coming
/// back, and what this asserts is that `start` returns at all.
#[test]
fn a_pre_exec_hook_that_hangs_fails_the_start_instead_of_hanging_init() {
    let _env = PRE_EXEC_ENV
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    std::env::set_var("RAVEN_PRE_EXEC_TIMEOUT_MS", "300");

    let mut cfg_svc = sleeper("wedged-hook");
    cfg_svc.pre_exec = vec!["/bin/sleep".to_string(), "300".to_string()];

    let began = Instant::now();
    let started = Service::start(&cfg_svc);
    let took = began.elapsed();

    std::env::remove_var("RAVEN_PRE_EXEC_TIMEOUT_MS");

    let err = match started {
        Ok(mut svc) => {
            svc.kill();
            panic!("a hook that never finishes must not produce a started service");
        }
        Err(e) => format!("{e:#}"),
    };
    assert!(
        err.contains("pre_exec") && err.contains("/bin/sleep"),
        "the failure must name the hook that caused it: {err}"
    );
    assert!(
        took < Duration::from_secs(10),
        "the wait must be bounded by the timeout, not by the hook: took {took:?}"
    );
}

/// `wait_for_exit` must come back even when SIGKILL does not work.
///
/// Its deadline branch used to send SIGKILL and then call `waitpid(pid, None)`
/// -- no WNOHANG, no deadline -- in a function documented as bounded because
/// it runs on PID 1's thread. SIGKILL is not a guarantee: a task in
/// uninterruptible sleep on a yanked USB volume does not act on it until its
/// I/O errors out, and PID 1 blocked in that call reaps nothing, answers no
/// further `raven-rc` request and does not honour poweroff until something
/// unrelated happens to die.
///
/// D state cannot be conjured in a test, so the signal is swallowed instead:
/// with a `cgroup.kill` file under a temporary cgroup root, `Cgroup::signal_all`
/// reports the kill as delivered and the real process carries on -- which is
/// exactly the state the supervisor is in when it believes it has killed
/// something that cannot die. Before the fix this test does not fail; it
/// hangs, which is the point.
#[test]
fn wait_for_exit_gives_up_when_sigkill_does_not_land() {
    let _env = CGROUP_ROOT_ENV
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let root = std::env::temp_dir().join(format!("raven-svc-unkillable-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cgroup_root = root.join("cgroup");
    std::fs::create_dir_all(&cgroup_root).expect("temp cgroup root");
    std::fs::write(cgroup_root.join("cgroup.controllers"), "cpu io memory pids\n")
        .expect("the probe cgroup2 is recognised by");

    std::env::set_var("RAVEN_CGROUP_ROOT", &cgroup_root);
    assert!(
        cgroup::ensure_slice(),
        "a tree with cgroup.controllers is usable"
    );

    let dir = cgroup_root.join(cgroup::SLICE_NAME).join("unkillable");
    std::fs::create_dir_all(&dir).expect("service cgroup");
    // Opened by the child without O_CREAT, so it has to exist beforehand.
    std::fs::write(dir.join("cgroup.procs"), "").expect("procs file");
    // The file that makes the kill a no-op: a real one takes the whole
    // subtree with it, and this one takes the write and does nothing.
    std::fs::write(dir.join("cgroup.kill"), "").expect("kill file");

    let cfg_svc = sleeper("unkillable");
    let mut svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("has a pid").as_raw();

    let began = Instant::now();
    svc.wait_for_exit(Duration::from_millis(100));
    let took = began.elapsed();

    std::env::remove_var("RAVEN_CGROUP_ROOT");

    assert!(
        took < Duration::from_secs(5),
        "wait_for_exit must not block on a process that will not die: took {took:?}"
    );
    assert!(
        std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok(),
        "the test only proves anything while the SIGKILL really was swallowed"
    );
    assert!(
        !svc.is_running(),
        "the service must be off the supervisor's books once it has given up on it"
    );

    // Not `svc.kill()`: the supervisor has forgotten the pid, and the point of
    // the fake cgroup is that signalling through it would do nothing anyway.
    let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), nix::sys::signal::SIGKILL);
    std::fs::remove_dir_all(&root).ok();
}

/// A service that ignores SIGTERM must still end up stopped.
///
/// `raven-rc stop` sends SIGTERM and deliberately does not wait, so nothing
/// escalated a manual stop: the daemon stayed alive and flagged
/// `manually_stopped` at once, a state the operator could not leave, because a
/// second `stop` only sent another SIGTERM and `start` answered "already
/// running". The escalation now happens on the main loop's next pass through
/// `control::poll`, which is what `escalate_pending_stops` is called directly
/// here to stand in for.
#[test]
fn a_stop_that_sigterm_ignores_escalates_to_sigkill() {
    let mut cfg_svc = sleeper("stubborn");
    cfg_svc.exec = "/bin/sh".to_string();
    cfg_svc.args = vec![
        "-c".to_string(),
        "trap '' TERM; while true; do sleep 0.2; done".to_string(),
    ];
    // The shortest the config can express, so the test waits it out rather
    // than reaching into the clock.
    cfg_svc.stop_timeout = 1;

    let mut cfg = config_with(vec![cfg_svc.clone()]);
    let mut services = HashMap::new();
    let svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("has a pid").as_raw();
    services.insert("stubborn".to_string(), svc);

    // The trap has to be installed before the SIGTERM arrives, or this test
    // would pass by killing a shell that was not ignoring anything yet.
    std::thread::sleep(Duration::from_millis(300));

    let (reply, _) = control::dispatch("stop stubborn", &mut services, &mut cfg);
    assert!(reply.contains("Stopping"), "{reply}");

    // Nothing has been waited for: the reply is immediate and the process is
    // still there, which is the design this fix had to keep.
    assert!(
        !wait_gone(pid, Duration::from_millis(200)),
        "a service ignoring SIGTERM is the case under test"
    );

    // A pass before the deadline must not escalate -- stop_timeout is the
    // daemon's chance to leave on its own terms.
    control::escalate_pending_stops(&mut services);
    assert!(
        !wait_gone(pid, Duration::from_millis(200)),
        "the escalation must not jump the stop_timeout"
    );

    std::thread::sleep(Duration::from_secs(1));
    control::escalate_pending_stops(&mut services);
    assert!(
        wait_gone(pid, Duration::from_secs(5)),
        "a stop that SIGTERM did not achieve must become a SIGKILL"
    );

    assert!(
        services["stubborn"].is_manually_stopped(),
        "escalating a stop must not undo the operator's intent"
    );
}

/// The escalation must never reach the process a restart just started.
///
/// `stop` followed by `start` inside the stop timeout is how an operator
/// restarts a service by hand, and the deadline recorded for the old process
/// would otherwise have the main loop SIGKILL the new one seconds after it
/// came up -- a service that stays down for reasons nothing in the logs
/// explains.
#[test]
fn a_restart_inside_the_stop_timeout_is_not_killed_by_the_old_deadline() {
    let mut cfg_svc = sleeper("quick-turnaround");
    cfg_svc.stop_timeout = 1;

    let mut cfg = config_with(vec![cfg_svc.clone()]);
    let mut services = HashMap::new();
    services.insert(
        "quick-turnaround".to_string(),
        Service::start(&cfg_svc).expect("starts"),
    );

    let (reply, _) = control::dispatch("stop quick-turnaround", &mut services, &mut cfg);
    assert!(reply.contains("Stopping"), "{reply}");
    let old_pid = services["quick-turnaround"]
        .pid()
        .map(|p| p.as_raw())
        .expect("still recorded while it dies");
    assert!(wait_gone(old_pid, Duration::from_secs(5)), "sleep takes SIGTERM");

    let (reply, _) = control::dispatch("start quick-turnaround", &mut services, &mut cfg);
    assert!(reply.contains("Started"), "{reply}");
    let new_pid = services["quick-turnaround"]
        .pid()
        .map(|p| p.as_raw())
        .expect("running again");
    assert_ne!(old_pid, new_pid);

    // Past the deadline the stop recorded, with a sweep on either side of it.
    control::escalate_pending_stops(&mut services);
    std::thread::sleep(Duration::from_millis(1200));
    control::escalate_pending_stops(&mut services);

    assert!(
        !wait_gone(new_pid, Duration::from_millis(200)),
        "the new process must survive the old process's kill deadline"
    );
    assert!(
        services["quick-turnaround"].is_running(),
        "and the supervisor must still consider the service up"
    );

    services.get_mut("quick-turnaround").unwrap().kill();
}

/// The other half of `a_manually_stopped_service_stays_stopped_across_adoption`:
/// the service the operator stopped has not finished dying yet.
///
/// `stop` is asynchronous by design -- it sends SIGTERM and returns, so PID 1
/// is not held waiting on a daemon's shutdown -- which means "stopped by the
/// operator" and "still has a live process" is an ordinary state and not a
/// contradiction. `Service::adopt` modelled it correctly in the branch where
/// the pid is gone and hardcoded `manually_stopped: false` in the branch where
/// it is not, so a `raven-rc reexec` inside the stop-to-exit window handed the
/// service back auto-restartable: the SIGTERM lands a moment later,
/// `check_services` sees Exited with `restart = true` and no operator flag,
/// and the service the operator took down is back up with nothing saying why.
#[test]
fn a_stop_still_in_flight_survives_adoption() {
    let marker = std::env::temp_dir().join(format!(
        "raven-stubborn-trapped-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&marker);

    // A shell that ignores SIGTERM, so the window the bug lives in stays open
    // for as long as the test needs rather than for as long as /bin/sleep
    // takes to die. The inner `sleep 1` is a child and does take the signal;
    // the loop is what keeps the leader alive afterwards. It touches the
    // marker once the trap is really in force, because a SIGTERM that arrives
    // before the shell has run `trap` kills it outright -- which would leave
    // this test exercising the dead branch of `adopt`, the one that was
    // already right.
    let mut cfg_svc = sleeper("stubborn");
    cfg_svc.exec = "/bin/sh".to_string();
    cfg_svc.args = vec![
        "-c".to_string(),
        format!(
            "trap '' TERM; : > {}; while true; do sleep 1; done",
            marker.display()
        ),
    ];

    let mut svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("has a pid").as_raw();

    let armed_at = Instant::now();
    let mut armed = false;
    while armed_at.elapsed() < Duration::from_secs(5) {
        if marker.exists() {
            armed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    svc.stop_by_request();
    let flagged = svc.is_manually_stopped();
    // The premise: still there, because it was asked to go and has not.
    let still_alive = !wait_gone(pid, Duration::from_millis(300));

    let adopted = Service::adopt(svc.snapshot(), cfg_svc);
    let adopted_running = adopted.is_running();
    let adopted_stopped = adopted.is_manually_stopped();

    // Cleanup comes before the assertions, and that is not tidiness. This
    // shell ignores every signal the supervisor sends it, and a child left
    // behind by a panicking test still holds the stdout and stderr it
    // inherited -- which is the test harness's own pipe, so `cargo test` waits
    // on it forever, long after the test binary itself has gone.
    let _ = nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(pid),
        nix::sys::signal::Signal::SIGKILL,
    );
    let _ = nix::sys::wait::waitpid(nix::unistd::Pid::from_raw(pid), None);
    let _ = std::fs::remove_file(&marker);

    assert!(armed, "the shell never got as far as ignoring SIGTERM");
    assert!(flagged, "the stop is recorded on the live service");
    assert!(
        still_alive,
        "the test only proves anything while the process is still alive"
    );
    assert!(
        adopted_running,
        "this is the live branch of adopt, which is the one under test"
    );
    assert!(
        adopted_stopped,
        "a re-exec must not undo an operator's stop just because the process \
         has not finished dying yet"
    );
}

/// A pending restart must be a deadline the loop sleeps until, not a reason to
/// spin at the busy tick for the whole backoff.
///
/// `wants_quick_tick` answered yes for any dead service with a `retry_at`,
/// however far off it was, so one crash-looping service whose backoff had
/// saturated at RESTART_BACKOFF_MAX held the main loop at 100ms for sixty
/// seconds at a time -- six hundred wakes to compare two Instants -- and did
/// it for the life of the boot. That is exactly the idle sleep readiness.rs
/// was built to make possible, given away by the one service least likely to
/// be fixed before the next reboot.
#[test]
fn a_distant_restart_does_not_hold_the_loop_at_the_busy_tick() {
    let cfg_svc = sleeper("looper");
    let mut svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("has a pid").as_raw();
    svc.kill();
    let _ = nix::sys::wait::waitpid(nix::unistd::Pid::from_raw(pid), None);
    svc.mark_exited(1);

    // Latch the restart decision the way the supervisor does, then ask from a
    // clock that is still well short of it.
    let now = Instant::now();
    assert!(
        !svc.should_restart_at(now),
        "the first tick after a death decides the delay rather than restarting"
    );
    let due = svc.retry_at().expect("a restart is pending");
    assert!(due > now, "and it is in the future");

    let mut services = HashMap::new();
    services.insert("looper".to_string(), svc);

    assert!(
        !control::wants_quick_tick(&services, true),
        "a restart that is a second away is a deadline, not ten looks a second"
    );

    let until = control::next_retry_in(&services, now).expect("the deadline is offered instead");
    assert_eq!(
        until,
        due.saturating_duration_since(now),
        "and it is offered as the time the loop may sleep for"
    );

    // A deadline already in the past asks for no sleep at all, which the main
    // loop floors at its busy tick rather than polling on; the value here must
    // be zero rather than a wrapped-around eternity.
    assert_eq!(
        control::next_retry_in(&services, due + Duration::from_secs(1)),
        Some(Duration::ZERO)
    );

    // An operator stop leaves the deadline behind -- `should_restart` refuses a
    // manually stopped service without clearing it -- and that leftover must
    // not keep the loop awake for the rest of the boot.
    services.get_mut("looper").unwrap().stop_by_request();
    assert!(
        control::next_retry_in(&services, now).is_none(),
        "a stopped service has no restart coming, whatever its old deadline says"
    );
}

/// A console session must not inherit PID 1's ignored SIGPIPE.
///
/// Rust's runtime sets SIGPIPE to SIG_IGN before `main`, so PID 1 runs with it
/// ignored; SIG_IGN survives `execve`, `login(1)` does not reset it, and POSIX
/// requires a shell to leave an inherited-ignored signal ignored. The ordinary
/// `Command` path is saved from this by std, which resets SIGPIPE and empties
/// the signal mask before it execs, but the tty path forks and execs by hand
/// and did neither -- so agetty, login, the login shell and every command the
/// person at the keyboard ran had SIGPIPE ignored. What it looks like at the
/// prompt is `cat /dev/zero | head -c1` printing a write error instead of
/// `cat` dying quietly, and a producer that does not check write(2) never
/// finishing at all when its reader goes away.
///
/// The service here is a shell on a pty rather than a getty on /dev/tty1,
/// because a pty is the one controlling terminal a test may have: the child
/// takes the same three steps on it -- open, TIOCSCTTY, tcsetpgrp -- that it
/// takes on a real console. It also carries an `environment` entry, which is
/// the other half of the same fix: the variables used to be set with `setenv`
/// in the child, which allocates, and are now in the block `execvpe` is given.
#[test]
fn a_tty_service_execs_with_a_standard_signal_state() {
    let _env = CGROUP_ROOT_ENV
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Pointed at a directory that is not a cgroup2 tree, so this neither
    // touches the machine's real hierarchy nor races the two tests that build
    // one of their own: `Cgroup::for_service` answers None and the service
    // starts unconfined, which is all this test is about.
    let root = std::env::temp_dir().join(format!("raven-tty-signals-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("temp root");
    std::env::set_var("RAVEN_CGROUP_ROOT", root.join("not-a-cgroup2-tree"));

    // A pty pair. The master stays open for the length of the test: closing it
    // would hang up the session on the other side.
    let master = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    assert!(
        master >= 0,
        "no pty available: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(unsafe { libc::grantpt(master) }, 0, "grantpt");
    assert_eq!(unsafe { libc::unlockpt(master) }, 0, "unlockpt");
    let slave = unsafe { libc::ptsname(master) };
    assert!(!slave.is_null(), "ptsname");
    let slave = unsafe { std::ffi::CStr::from_ptr(slave) }
        .to_str()
        .expect("a pts name is ASCII")
        .to_string();

    let out = root.join("seen-by-the-session");
    let mut svc = ServiceConfig {
        name: "getty-pty".to_string(),
        description: "a shell on a pty".to_string(),
        exec: "/bin/sh".to_string(),
        args: vec![
            "-c".to_string(),
            format!(
                "grep -E '^Sig(Ign|Blk):' /proc/self/status > {out}; \
                 printf '%s\n' \"env=$RAVEN_TTY_TEST\" \"path=${{PATH:+set}}\" >> {out}",
                out = out.display()
            ),
        ],
        restart: false,
        tty: Some(slave),
        ..ServiceConfig::default()
    };
    svc.environment
        .insert("RAVEN_TTY_TEST".to_string(), "arrived".to_string());

    let started = Service::start(&svc).expect("the session starts");
    let pid = started.pid().expect("the fork returned a pid").as_raw();

    // Wait for the shell to have written, then kill and reap it BEFORE any
    // assertion. A test that fails with a child still holding cargo's stdout
    // pipe open hangs the harness rather than reporting the failure.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !out.exists() {
        std::thread::sleep(Duration::from_millis(20));
    }
    unsafe {
        libc::kill(pid, libc::SIGKILL);
        let mut status = 0;
        libc::waitpid(pid, &mut status, 0);
        libc::close(master);
    }

    let seen = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        !seen.is_empty(),
        "the shell on the pty wrote nothing; it did not get as far as exec"
    );

    let mask = |field: &str| -> u64 {
        let line = seen
            .lines()
            .find(|l| l.starts_with(field))
            .unwrap_or_else(|| panic!("no {field} in {seen}"));
        let hex = line.split_whitespace().nth(1).expect("a mask");
        u64::from_str_radix(hex, 16).expect("a hex mask")
    };

    // Bit 12 of the mask is signal 13, SIGPIPE.
    assert_eq!(
        mask("SigIgn:") & (1 << 12),
        0,
        "the session must not have inherited init's ignored SIGPIPE: {seen}"
    );
    assert_eq!(
        mask("SigBlk:"),
        0,
        "and it must start with nothing blocked: {seen}"
    );

    // The environment reached the process through `execvpe`, the service's own
    // entry and init's alike -- dropping either would be a different bug with
    // the same cause.
    assert!(seen.contains("env=arrived"), "{seen}");
    assert!(seen.contains("path=set"), "{seen}");
}

/// A service leads its own session, not init's.
///
/// init is pid 1 and never called `setsid`, so its session is 0 -- a session
/// whose leader is a pid that does not exist. A service left in it hands that
/// session to every process it starts, which on a graphical boot is the whole
/// desktop, and anything that asks who a caller's session leader is finds
/// nobody. rvnd asks exactly that before prompting for an install, and
/// refused every install started from the store because of it.
///
/// The process group is checked in the same breath: `setsid` replaced an
/// explicit `process_group(0)`, and `stop` signals a service by negative pid,
/// which only reaches its children while pgid == pid.
#[test]
fn a_service_leads_its_own_session_and_process_group() {
    let cfg_svc = sleeper("own-session");
    let mut svc = Service::start(&cfg_svc).expect("starts");
    let pid = svc.pid().expect("has a pid").as_raw();

    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).expect("stat");
    // comm can contain spaces and parens; the fields after the closing paren
    // are state, ppid, pgrp, session.
    let fields: Vec<&str> = stat
        .rsplit_once(')')
        .expect("stat has a comm")
        .1
        .split_whitespace()
        .collect();
    let pgrp: i32 = fields[2].parse().expect("pgrp");
    let session: i32 = fields[3].parse().expect("session");

    assert_eq!(session, pid, "the service must lead its own session");
    assert_eq!(pgrp, pid, "and its own process group");
    assert!(
        std::path::Path::new(&format!("/proc/{session}")).exists(),
        "the session leader must be a process that exists"
    );

    svc.kill();
}
