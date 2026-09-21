//! Readiness by notification: waiting for a service's ready path to appear
//! instead of asking for it over and over.
//!
//! A service is ready when the path its definition names exists -- dbus's
//! socket at /run/dbus/system_bus_socket, seatd's at /run/seatd.sock, a pid
//! file, a lock. Init needs to know this twice over. During boot, because a
//! service declaring `after = ["dbus"]` must not be started until dbus can
//! actually answer; and for the rest of the machine's life, because
//! `raven-rc blame` reports how long each service took to become ready and
//! that column is the entire reason the table is printed.
//!
//! # Why this file exists at all
//!
//! Until it did, init asked, and it asked on a timer. `raven-rc blame` on a
//! laptop that had been up for a day reported this:
//!
//! ```text
//! SERVICE              STARTED    READY      TOOK       NOTE
//! powerd                 5.669      6.789      1.120
//! controlsd              5.669      6.789      1.120
//! timed                  5.669      6.789      1.120
//! fprintd                5.669      6.789      1.120
//! ```
//!
//! Four unrelated daemons, none of which has anything to do with the others,
//! each apparently taking exactly 1.120 seconds and each becoming ready at
//! exactly the same microsecond. They were not slow and they were not
//! synchronised. Readiness was not detected when a socket appeared; it was
//! detected when the main loop next looked, and the loop looked at all of them
//! inside one `for` loop, stamping `Instant::now()` on each in turn. The
//! numbers were the time of a tick, reported four times. The loop's idle sleep
//! is two seconds, so the 1.120s was not a measurement of anything -- it was
//! the distance from the start of those services to whenever the loop happened
//! to wake up next.
//!
//! An inotify watch turns that around. The kernel tells init the instant the
//! file appears, the poll the main loop is already sleeping in returns, and
//! the time recorded is the time of the event rather than the time of the next
//! look. The same four services now report the few milliseconds they each
//! really took, and they report four different numbers.
//!
//! # Why the watch goes on the parent directory
//!
//! It cannot go on the file: the file does not exist yet, and that is the
//! whole question being asked. `inotify_add_watch` on a path that is not there
//! fails with ENOENT. So the watch goes on the directory that will contain it
//! and asks for `IN_CREATE` and `IN_MOVED_TO`, and every event on that
//! directory is treated as "look again" rather than as an answer.
//!
//! That ordering matters and is easy to get backwards. A watch must be armed
//! **before** the existence check, never after: a daemon that creates its
//! socket in the window between a check and an `inotify_add_watch` would
//! otherwise never be noticed at all, and the fastest services are exactly the
//! ones that fit in that window. Every entry point in this file arms first and
//! looks second, and says so where it does it.
//!
//! # What this is not
//!
//! It is not a file-watching framework. There are no recursive watches, no
//! watch on a directory that does not exist yet (init simply keeps polling
//! until the directory is there and then arms the watch), and nothing here
//! watches configuration files.
//!
//! It is also not the thing that decides a service is ready. An event from
//! this module is a wakeup and nothing more; the fact is always established by
//! a `stat`, in `Service::note_ready_if_present`, which stays the single place
//! `ready_at` is ever written. That is deliberate, and it is what makes the
//! whole design safe to get wrong: a lost event, a coalesced event, an
//! overflowed queue or a kernel with no inotify at all costs the precision of
//! one reading, never the readiness itself, because the timer path that was
//! here before is still underneath.

use std::collections::HashMap;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify, WatchDescriptor};

/// How long the fallback timer sleeps between looks when there is no watch.
///
/// Reached only when inotify is unavailable or the ready path's directory does
/// not exist yet. It is the interval `wait_for_ready_path` used before this
/// module, kept at the same value so a machine that falls back behaves exactly
/// as it always did rather than differently-but-still-wrong.
const FALLBACK_INTERVAL: Duration = Duration::from_millis(50);

/// How many times `drain` will refill its buffer before giving up for this
/// tick.
///
/// `Inotify::read_events` reads into a 4 KiB buffer, so a burst larger than
/// that needs more than one read to clear, and a fd left readable makes the
/// next `poll` return immediately -- a busy loop. The bound exists so that a
/// directory somebody is hammering (untarring into /run, say) cannot hold the
/// supervisor in this function instead of reaping children. Anything left
/// unread is read on the next pass, and since readiness is established by
/// `stat` and not by the events themselves, discarding the backlog would be
/// harmless anyway.
const DRAIN_ROUNDS: usize = 16;

/// Whether the "there is no inotify here" warning has been said already.
///
/// [`Watcher::new`] is called once for the main loop and again for every
/// synchronous wait: once per `after` edge during boot, which on this machine
/// is a dozen of them, and once more on every `raven-rc start`. A kernel that
/// will not hand out an inotify instance -- built without it, or with
/// `fs.inotify.max_user_instances` exhausted -- would otherwise print the same
/// two lines a dozen times across the console the login prompt is trying to
/// use. The first is news; the rest are not. This is the latch cgroup.rs keeps
/// for an absent cgroup2, for exactly the same reason.
static ABSENCE_REPORTED: AtomicBool = AtomicBool::new(false);

/// The directory events that can mean "the ready path is here now".
///
/// `IN_CREATE` is the common case: `bind(2)` on a unix socket, `open(O_CREAT)`
/// on a pid file. `IN_MOVED_TO` covers a file written under a temporary name
/// and renamed into place, which is what anything that cares about a reader
/// seeing a half-written file does -- init writes /run/raven-init/status that
/// way itself. `IN_ATTRIB` is insurance rather than a known case: it is
/// delivered for a link count change as well as a chmod, and a spurious wakeup
/// costs one `stat` on a watch that is only armed for the few seconds a
/// service is starting.
///
/// Not a `const`, because combining two bitflags is a function call and this
/// is cheaper to read than the `from_bits_retain` dance that would make it
/// one.
fn watch_events() -> AddWatchFlags {
    AddWatchFlags::IN_CREATE | AddWatchFlags::IN_MOVED_TO | AddWatchFlags::IN_ATTRIB
}

/// The directory to watch for `path` to appear in.
///
/// A relative path with no directory component, and the root itself, both land
/// on something that exists rather than on the empty string, which
/// `inotify_add_watch` would reject with ENOENT and leave looking like a
/// missing directory.
fn parent_of(path: &str) -> PathBuf {
    match Path::new(path).parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// A set of inotify watches on the directories that ready paths will appear
/// in, and the fd the main loop sleeps on alongside its other two.
///
/// One of these lives in `main_loop_at` for the life of the loop. Its watches
/// come and go: `arm` adds one for each directory a service is currently
/// waiting on and removes the ones nothing is waiting on any more, so a
/// settled machine holds no watches and is woken by nothing. That matters more
/// than the handful of bytes a watch costs -- init watching /run permanently
/// would be woken by every pid file, lock and socket any program on the
/// machine ever created there, forever, to look at a list of services that are
/// all already ready.
pub struct Watcher {
    inotify: Inotify,
    /// Directory -> the watch on it. Also the answer to "is this armed".
    watched: HashMap<PathBuf, WatchDescriptor>,
    /// Directories arming has already failed for, so the explanation is
    /// written once rather than once per tick. The main loop calls `arm`
    /// several times a second while a service is starting, and a per-tick log
    /// line is the flood the restart backoff's `retry_at` latch exists to
    /// prevent elsewhere in this tree.
    reported: Vec<PathBuf>,
}

impl Watcher {
    /// Open an inotify fd, or `None` if the kernel will not give us one.
    ///
    /// `None` is not an error worth failing a boot over: the caller keeps the
    /// timer it had before this module existed, readiness is still detected,
    /// and the only thing lost is the precision of the number `blame` prints.
    /// The warning says exactly that, because "inotify_init1 failed" on its
    /// own tells the person reading the console nothing about what they have
    /// lost.
    pub fn new() -> Option<Watcher> {
        // O_NONBLOCK because the main loop must never block inside a drain;
        // CLOEXEC because every service init starts is forked from this
        // process and none of them has any business inheriting the fd.
        match Inotify::init(InitFlags::IN_NONBLOCK | InitFlags::IN_CLOEXEC) {
            Ok(inotify) => Some(Watcher {
                inotify,
                watched: HashMap::new(),
                reported: Vec::new(),
            }),
            Err(e) => {
                if ABSENCE_REPORTED.swap(true, Ordering::SeqCst) {
                    log::debug!("readiness: inotify unavailable ({e}); polling");
                } else {
                    log::warn!("Could not open inotify ({e}); readiness will be polled instead");
                    log::warn!(
                        "  service ready times in `raven-rc blame` will be rounded to a tick"
                    );
                }
                None
            }
        }
    }

    /// Arm a watch on the parent directory of each of `ready_paths`, and drop
    /// the watches on directories no longer named.
    ///
    /// Returns whether every path given is now watched. The caller uses that
    /// to decide whether it may sleep: `false` means at least one directory
    /// could not be watched -- it does not exist yet, most often, because the
    /// daemon creates it itself -- and the loop must keep its short timer for
    /// that one. This is the honest answer rather than a convenient one; a
    /// `true` here that was not earned turns a service's readiness into a
    /// two-second wait.
    ///
    /// Safe to call on every pass. Re-arming a directory that is already
    /// watched is skipped, and `inotify_add_watch` would be idempotent anyway.
    pub fn arm<'a, I>(&mut self, ready_paths: I) -> bool
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut wanted: Vec<PathBuf> = Vec::new();
        for path in ready_paths {
            let dir = parent_of(path);
            if !wanted.contains(&dir) {
                wanted.push(dir);
            }
        }

        // Disarm first. A service that has become ready, been stopped or been
        // removed leaves a watch behind that can only cost wakeups.
        let stale: Vec<PathBuf> = self
            .watched
            .keys()
            .filter(|dir| !wanted.contains(dir))
            .cloned()
            .collect();
        for dir in stale {
            if let Some(wd) = self.watched.remove(&dir) {
                // A watch on a directory that has since been deleted is
                // already gone; the kernel said so with IN_IGNORED and this
                // returns EINVAL. Nothing to do about it and nothing to say.
                self.inotify.rm_watch(wd).ok();
            }
            self.reported.retain(|seen| seen != &dir);
        }

        let mut complete = true;
        for dir in wanted {
            if self.watched.contains_key(&dir) {
                continue;
            }
            match self.inotify.add_watch(&dir, watch_events()) {
                Ok(wd) => {
                    self.watched.insert(dir.clone(), wd);
                    self.reported.retain(|seen| seen != &dir);
                }
                Err(e) => {
                    complete = false;
                    if !self.reported.contains(&dir) {
                        // info, not warn: this reaches the log file and not
                        // the console, because it is nearly always the benign
                        // case of a daemon that has not created its own
                        // runtime directory yet, and it resolves itself a
                        // fraction of a second later when the next pass arms
                        // the watch successfully.
                        log::info!("Readiness watch on {} unavailable ({e}); polling", dir.display());
                        self.reported.push(dir);
                    }
                }
            }
        }
        complete
    }

    /// Read and discard everything the kernel has queued.
    ///
    /// The contents are not examined for a filename, and that is on purpose:
    /// correlating an event with a service would mean matching the name in the
    /// event against the basename of a ready path, getting symlinks, renames
    /// and "the directory was replaced" right, and holding a second table that
    /// can disagree with the first. The `stat` that follows this call answers
    /// the same question with none of that, and it is one syscall per service
    /// that is still waiting -- of which, after boot, there are none.
    ///
    /// The one thing worth reading out of an event is `IN_IGNORED`, which says
    /// a watch is gone because the directory was deleted or unmounted. Without
    /// forgetting it here, `arm` would believe that directory was still
    /// covered and would never watch it again if it came back.
    pub fn drain(&mut self) {
        for _ in 0..DRAIN_ROUNDS {
            // EAGAIN on an empty non-blocking fd is the normal way out.
            let Ok(events) = self.inotify.read_events() else {
                return;
            };
            if events.is_empty() {
                return;
            }
            for event in &events {
                if event.mask.contains(AddWatchFlags::IN_IGNORED) {
                    self.watched.retain(|_, wd| *wd != event.wd);
                }
            }
        }
    }

    /// Arm a watch for one ready path, for the callers that wait on a single
    /// dependency rather than supervising a whole machine.
    fn watch_one(&mut self, ready_path: &str) -> bool {
        self.arm(std::iter::once(ready_path))
    }

    /// Sleep until an event arrives or `deadline` passes, then clear the
    /// queue. Returns with nothing to report either way -- the caller decides
    /// what happened by looking at the filesystem.
    fn sleep_until_event(&mut self, deadline: Instant) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        // TryFrom<Duration> only fails past i32::MAX milliseconds, about 24
        // days; MAX is the same order and the caller re-checks its deadline
        // regardless, so there is no way for this to wait past it.
        let timeout = PollTimeout::try_from(remaining).unwrap_or(PollTimeout::MAX);
        let mut fds = [PollFd::new(self.inotify.as_fd(), PollFlags::POLLIN)];
        match poll(&mut fds, timeout) {
            // EINTR is a signal arriving, which for PID 1 is constant; the
            // caller's deadline loop handles the short sleep it causes.
            Ok(_) | Err(nix::errno::Errno::EINTR) => {}
            Err(e) => log::warn!("poll on readiness watch: {e}"),
        }
        self.drain();
    }
}

impl AsFd for Watcher {
    /// The fd the main loop adds to its poll set, beside the SIGCHLD self-pipe
    /// and the control socket.
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inotify.as_fd()
    }
}

/// Wait up to `timeout` for `path` to exist. Returns whether it does.
///
/// This is the synchronous half of readiness, used where init has nothing else
/// to do until a dependency answers: resolving `after` during boot
/// (`start_services`) and on a manual `raven-rc start` (`control::start_service`).
/// Both of those used to be sleep-and-look loops -- 50ms and 20ms respectively
/// -- which meant a dependency that was ready in one millisecond still cost
/// the whole interval, once per service in the chain, on every boot.
///
/// The timeout is unchanged in meaning and unchanged in effect: a path that
/// never appears still costs exactly `timeout` and still returns false, so the
/// "did not become ready" errors both callers print are reached on the same
/// schedule they always were. Only the success case got faster.
pub fn wait_for_path(path: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;

    let Some(mut watcher) = Watcher::new() else {
        return poll_for_path(path, deadline);
    };

    // Arm, then look -- in that order. See the module doc: a check before the
    // watch loses every service fast enough to answer in between, which is
    // most of them.
    if !watcher.watch_one(path) {
        // The directory is not there yet, so there is nothing to watch and no
        // event to wait for. Fall back to the timer for this one.
        return poll_for_path(path, deadline);
    }

    if Path::new(path).exists() {
        return true;
    }

    loop {
        if Instant::now() >= deadline {
            break;
        }
        watcher.sleep_until_event(deadline);
        if Path::new(path).exists() {
            return true;
        }
    }

    // One last look, because the deadline may have expired between the final
    // event and the check above.
    Path::new(path).exists()
}

/// The timer that `wait_for_path` falls back to when there is no watch to be
/// had: no inotify, or a parent directory the daemon has not created yet.
///
/// Kept rather than deleted because the machines this has to boot include ones
/// where /run is not yet what it will be, and a supervisor that only works
/// when the kernel cooperates is not a supervisor.
fn poll_for_path(path: &str, deadline: Instant) -> bool {
    while Instant::now() < deadline {
        if Path::new(path).exists() {
            return true;
        }
        std::thread::sleep(FALLBACK_INTERVAL);
    }
    Path::new(path).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "raven-init-readiness-{}-{}-{:?}",
            tag,
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch directory");
        dir
    }

    #[test]
    fn a_path_that_is_already_there_is_seen_without_waiting() {
        let dir = scratch("present");
        let path = dir.join("ready.sock");
        std::fs::write(&path, b"").expect("ready file");

        let began = Instant::now();
        assert!(wait_for_path(
            path.to_str().expect("utf-8"),
            Duration::from_secs(5)
        ));
        assert!(
            began.elapsed() < Duration::from_millis(200),
            "an existing path must not cost a sleep: {:?}",
            began.elapsed()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The race this module exists to lose gracefully: the file appears after
    /// the caller decided to wait and before anything could look again. The
    /// watch has to be armed before the first check for this to be caught at
    /// all, and caught in milliseconds rather than at the next 50ms step.
    #[test]
    fn a_path_created_while_waiting_is_seen_at_once() {
        let dir = scratch("appears");
        let path = dir.join("ready.sock");
        let writer = path.clone();

        let hand = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            std::fs::write(&writer, b"").expect("ready file");
        });

        let began = Instant::now();
        assert!(wait_for_path(
            path.to_str().expect("utf-8"),
            Duration::from_secs(5)
        ));
        let took = began.elapsed();
        hand.join().expect("writer");

        assert!(took >= Duration::from_millis(50), "returned too early: {took:?}");
        assert!(
            took < Duration::from_millis(500),
            "an event should arrive in milliseconds, not at a tick: {took:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A ready path that never appears must still cost exactly its timeout and
    /// still answer false, because that is what the callers' "did not become
    /// ready" errors are built on.
    #[test]
    fn a_path_that_never_appears_times_out_and_says_so() {
        let dir = scratch("never");
        let path = dir.join("nothing-creates-this");

        let began = Instant::now();
        assert!(!wait_for_path(
            path.to_str().expect("utf-8"),
            Duration::from_millis(300)
        ));
        let took = began.elapsed();
        assert!(took >= Duration::from_millis(300), "returned early: {took:?}");
        assert!(took < Duration::from_secs(3), "overshot the timeout: {took:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The main loop's half: watches follow what is actually being waited for,
    /// so a machine with nothing starting holds none at all.
    #[test]
    fn watches_are_dropped_when_nothing_is_waiting_on_them() {
        let dir = scratch("arm");
        let path = dir.join("ready.sock");
        let path = path.to_str().expect("utf-8").to_string();
        let mut watcher = Watcher::new().expect("inotify");

        assert!(watcher.arm(std::iter::once(path.as_str())));
        assert_eq!(watcher.watched.len(), 1);

        // Arming again with the same path must not open a second watch.
        assert!(watcher.arm(std::iter::once(path.as_str())));
        assert_eq!(watcher.watched.len(), 1);

        assert!(watcher.arm(std::iter::empty()));
        assert!(
            watcher.watched.is_empty(),
            "a settled machine must hold no watches"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A directory that does not exist yet is the ordinary case for a daemon
    /// that creates its own runtime directory, and arming must report it
    /// rather than claiming a watch it does not have -- the caller uses that
    /// answer to decide whether it is allowed to sleep.
    #[test]
    fn a_missing_directory_is_reported_as_unwatched() {
        let dir = scratch("missing");
        let path = dir.join("not-here-yet").join("ready.sock");
        let path = path.to_str().expect("utf-8").to_string();
        let mut watcher = Watcher::new().expect("inotify");

        assert!(!watcher.arm(std::iter::once(path.as_str())));
        assert!(watcher.watched.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
