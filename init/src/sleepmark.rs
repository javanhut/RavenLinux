//! The sleep marker, and the wait for the desktop to be ready to sleep.
//!
//! Shared by the two processes that can put the machine to sleep: raven-init,
//! which normally does it, and raven-powerd, whose `suspend_directly` does it
//! when init cannot be reached. Both have to publish the same marker and wait
//! for the same answer, or the fallback is a suspend with the desktop on the
//! glass.
//!
//! # The marker
//!
//! `/run/raven-power/state` holds `sleeping <token>` before a sleep and
//! `awake` after it. The token is new for every sleep -- `CLOCK_BOOTTIME` in
//! nanoseconds, which only ever goes up, counts the time spent asleep, and is
//! the same clock in both processes -- so an answer left over from an earlier
//! sleep can never be mistaken for one to this.
//!
//! # The answer
//!
//! A compositor that sees `sleeping <token>` locks its session, turns its
//! screens off, and writes the token to `$XDG_RUNTIME_DIR/huginn/sleep-ready`.
//! Screens off before the sleep is the point of all this: the kernel's display
//! driver restores whatever it was showing the moment the machine resumes,
//! before userspace has thawed, and a compositor that locks only after the
//! resume is a compositor that has already shown the desktop to whoever opened
//! the lid. A CRTC turned off before the sleep comes back off, and stays dark
//! until the compositor lights it with the lock screen.
//!
//! Which compositors to wait for is read from `/run/user/*/huginn/pid`, and a
//! pid is only believed while `/proc/<pid>` is a process called `huginn` owned
//! by the same account as the runtime directory -- so a compositor that
//! crashed, or a pid the kernel has since handed to something else, is not
//! waited on.
//!
//! # The ceiling
//!
//! [`READY_TIMEOUT`], and then the machine sleeps regardless. A laptop shut in
//! a bag must never be held awake by a compositor that has wedged; a session
//! that did not answer in time is logged by uid and slept on anyway, which is
//! exactly what happened to every session before this existed.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

/// Directory holding the marker file. A tmpfs, so it never survives a boot.
pub const RUN_DIR: &str = "/run/raven-power";

/// The marker itself. See the module docs.
pub const STATE_MARKER: &str = "/run/raven-power/state";

/// Where session runtime directories live.
const USER_RUN_ROOT: &str = "/run/user";

/// The compositor's directory within a session's runtime directory.
const COMPOSITOR_DIR: &str = "huginn";

/// The compositor's pid, written when it starts.
const PID_FILE: &str = "pid";

/// The token of the last sleep the compositor is ready for.
const READY_FILE: &str = "sleep-ready";

/// The process name a pid file has to point at to be believed.
const COMPOSITOR_COMM: &str = "huginn";

/// The longest a sleep waits for the desktop. Locking is a few milliseconds
/// and the lock screen's first frame a few dozen; two seconds is a compositor
/// that is not going to answer.
pub const READY_TIMEOUT: Duration = Duration::from_secs(2);

/// How often the answers are looked for while waiting.
const POLL: Duration = Duration::from_millis(5);

/// Tell the desktop the machine is about to sleep, and wait -- up to
/// [`READY_TIMEOUT`] -- until every running compositor says its session is
/// locked and its screens are off.
///
/// Never fails: a sleep that cannot be announced is still a sleep, and a
/// session that does not answer has been logged.
pub fn prepare() {
    let token = token();
    publish(&format!("sleeping {}", token));

    let started = Instant::now();
    let mut waiting = compositors(Path::new(USER_RUN_ROOT));
    if waiting.is_empty() {
        return;
    }
    let total = waiting.len();

    loop {
        waiting.retain(|c| !c.ready(&token));
        if waiting.is_empty() {
            log::info!(
                "Desktop ready to sleep ({} session(s), {} ms)",
                total,
                started.elapsed().as_millis()
            );
            return;
        }
        if started.elapsed() >= READY_TIMEOUT {
            for c in &waiting {
                log::warn!(
                    "Session of uid {} (huginn pid {}) did not lock within {} ms; sleeping anyway",
                    c.uid,
                    c.pid,
                    READY_TIMEOUT.as_millis()
                );
            }
            return;
        }
        thread::sleep(POLL);
    }
}

/// Publish `awake` -- at boot, so the marker exists before anything watches
/// it, and after every sleep.
pub fn awake() {
    publish("awake");
}

/// A compositor that has to answer before the machine sleeps.
#[derive(Debug, PartialEq)]
struct Compositor {
    uid: u32,
    pid: u32,
    ready_file: PathBuf,
}

impl Compositor {
    fn ready(&self, token: &str) -> bool {
        fs::read_to_string(&self.ready_file).is_ok_and(|text| text.trim() == token)
    }
}

/// Every compositor running under `root` (normally `/run/user`).
fn compositors(root: &Path) -> Vec<Compositor> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let dir = entry.path().join(COMPOSITOR_DIR);
            let uid = fs::metadata(entry.path()).ok()?.uid();
            let pid: u32 = fs::read_to_string(dir.join(PID_FILE))
                .ok()?
                .trim()
                .parse()
                .ok()?;
            if !is_compositor(Path::new("/proc"), pid, uid) {
                return None;
            }
            Some(Compositor {
                uid,
                pid,
                ready_file: dir.join(READY_FILE),
            })
        })
        .collect()
}

/// Whether `pid` is a live compositor owned by `uid`.
fn is_compositor(proc_root: &Path, pid: u32, uid: u32) -> bool {
    let proc_dir = proc_root.join(pid.to_string());
    let Ok(meta) = fs::metadata(&proc_dir) else {
        return false;
    };
    meta.uid() == uid
        && fs::read_to_string(proc_dir.join("comm")).is_ok_and(|c| c.trim() == COMPOSITOR_COMM)
}

/// A token no earlier sleep this boot can have had.
fn token() -> String {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable timespec for the duration of the call.
    unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    format!("{}{:09}", ts.tv_sec, ts.tv_nsec)
}

/// Write one line to the marker file, for whoever is watching it.
///
/// Failure here is logged and swallowed. A compositor that misses a repaint is
/// a bad frame; refusing to suspend the machine over it would be worse.
fn publish(phase: &str) {
    if let Err(e) = publish_inner(phase) {
        log::warn!("Could not update {}: {}", STATE_MARKER, e);
    }
}

fn publish_inner(phase: &str) -> std::io::Result<()> {
    if !Path::new(RUN_DIR).is_dir() {
        fs::create_dir_all(RUN_DIR)?;
        fs::set_permissions(RUN_DIR, fs::Permissions::from_mode(0o755)).ok();
    }

    // Written whole and replaced by rename, so a watcher that wakes on the
    // event never reads a half-written or empty file.
    let tmp = format!("{}.new", STATE_MARKER);
    fs::write(&tmp, format!("{}\n", phase))?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644)).ok();
    fs::rename(&tmp, STATE_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "raven-sleepmark-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_marker_lives_under_the_run_dir() {
        assert!(STATE_MARKER.starts_with(RUN_DIR));
    }

    #[test]
    fn tokens_only_go_up() {
        let a: u128 = token().parse().unwrap();
        thread::sleep(Duration::from_millis(1));
        let b: u128 = token().parse().unwrap();
        assert!(b > a);
    }

    #[test]
    fn an_answer_to_an_earlier_sleep_is_not_an_answer() {
        let dir = scratch("stale");
        let c = Compositor {
            uid: 0,
            pid: 0,
            ready_file: dir.join(READY_FILE),
        };
        assert!(!c.ready("200"), "no file is not ready");
        fs::write(&c.ready_file, "100\n").unwrap();
        assert!(!c.ready("200"));
        fs::write(&c.ready_file, "200\n").unwrap();
        assert!(c.ready("200"));
    }

    #[test]
    fn a_pid_is_believed_only_for_a_live_compositor_of_that_account() {
        let proc_root = scratch("proc");
        let me = fs::metadata(&proc_root).unwrap().uid();

        let live = proc_root.join("42");
        fs::create_dir_all(&live).unwrap();
        fs::write(live.join("comm"), "huginn\n").unwrap();
        assert!(is_compositor(&proc_root, 42, me));

        // Someone else's pid, or a pid since reused by another program.
        assert!(!is_compositor(&proc_root, 42, me + 1));
        fs::write(live.join("comm"), "bash\n").unwrap();
        assert!(!is_compositor(&proc_root, 42, me));

        // A compositor that exited.
        assert!(!is_compositor(&proc_root, 43, me));
    }

    #[test]
    fn sessions_without_a_compositor_are_not_waited_on() {
        let root = scratch("users");
        // A runtime dir with no huginn in it, and one whose pid file points
        // at nothing: neither is waited on.
        fs::create_dir_all(root.join("1000")).unwrap();
        fs::create_dir_all(root.join("1001").join(COMPOSITOR_DIR)).unwrap();
        fs::write(
            root.join("1001").join(COMPOSITOR_DIR).join(PID_FILE),
            "4000000000\n",
        )
        .unwrap();
        assert!(compositors(&root).is_empty());
    }
}
