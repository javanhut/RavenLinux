//! The boot clock, and the milestones init passes on the way up.
//!
//! Everything here is in seconds since the kernel started (CLOCK_MONOTONIC),
//! the clock dmesg prints, so a line in init.log and a line in the kernel log
//! can be read against each other. Service start and ready times are kept on
//! the services themselves (see `Service::started_at` / `ready_at`) and
//! survive a re-exec with them -- and so, now, do the milestones below.
//!
//! They did not used to. `MILESTONES` is process memory and an exec replaces
//! it with an empty one, so after `raven-rc reexec` the boot timeline began at
//! the moment somebody typed the command: `blame` showed a machine that had
//! been up for a day booting in four milliseconds. The hand-off carries them
//! now ([`crate::reexec::Handoff`] holds them and [`restore`] puts them back),
//! which is why the names here are owned `String`s: a milestone outlives the
//! process that recorded it, and what comes back out of a TOML file is not a
//! `&'static str`.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A point boot passed, and the second it passed it.
pub type Milestone = (String, f64);

/// Seconds since the kernel started.
pub fn monotonic_secs() -> f64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes into the timespec we hand it and nothing
    // else; CLOCK_MONOTONIC cannot fail on Linux.
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }
    ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
}

/// An `Instant` on the same scale: `Instant` is CLOCK_MONOTONIC on Linux, so
/// subtracting its age from now is exact rather than approximate.
pub fn instant_secs(t: Instant) -> f64 {
    monotonic_secs() - t.elapsed().as_secs_f64()
}

/// No machine has been up for a century, and CLOCK_MONOTONIC does not run
/// backwards. A reading outside this window came out of a corrupt or a forged
/// hand-off file, not off a clock.
const MAX_PLAUSIBLE_AGE_SECS: f64 = 100.0 * 365.0 * 24.0 * 3600.0;

/// The inverse of [`instant_secs`]: the `Instant` that a CLOCK_MONOTONIC
/// reading names.
///
/// This is how a time written by the previous raven-init becomes a time this
/// one can compare against `Instant::now()`. The reading means the same thing
/// on both sides of an exec, because CLOCK_MONOTONIC belongs to the boot and
/// not to the process, so nothing is lost in the conversion but the
/// nanoseconds an `f64` cannot hold.
///
/// `None` rather than a panic for anything implausible. The release profile
/// sets `panic = "abort"`, this code runs as PID 1, and
/// `Duration::from_secs_f64` panics on a negative or a NaN -- so a hand-off
/// file with a bad number in it would take the machine down rather than cost
/// one column of one table.
pub fn instant_from_secs(mono: f64) -> Option<Instant> {
    let now = Instant::now();
    let age = monotonic_secs() - mono;
    if !age.is_finite() || age > MAX_PLAUSIBLE_AGE_SECS {
        return None;
    }
    // A reading a hair in the future is float arithmetic on two samples of the
    // same clock, not a time traveller; treat it as now.
    now.checked_sub(Duration::from_secs_f64(age.max(0.0)))
}

static MILESTONES: Mutex<Vec<Milestone>> = Mutex::new(Vec::new());

/// Record that boot reached `name` now.
///
/// Takes a `&str` rather than the `&'static str` it used to: the call sites
/// still all pass literals, and the milestones this process was handed by the
/// one before it are owned strings that have to sit in the same list.
pub fn mark(name: &str) {
    if let Ok(mut m) = MILESTONES.lock() {
        m.push((name.to_string(), monotonic_secs()));
    }
}

/// The milestones recorded so far, in the order they were reached.
pub fn milestones() -> Vec<Milestone> {
    MILESTONES.lock().map(|m| m.clone()).unwrap_or_default()
}

/// Put the milestones of the raven-init this one replaced back in front of
/// this one's.
///
/// Called from [`crate::reexec::take`], which is the one moment at which there
/// is a previous supervisor to inherit from. Prepended rather than appended or
/// merged: everything carried across happened before anything this image has
/// recorded, and keeping them in one list in time order is what lets `blame`
/// print a single table.
///
/// The names are deliberately not rewritten on the way in. After a re-exec the
/// table holds two rows called `init started` -- the original boot's and this
/// image's -- and that is the honest report: init did start twice, a day
/// apart, and the second one is where this supervisor's own milestones begin.
pub fn restore(previous: Vec<Milestone>) {
    if previous.is_empty() {
        return;
    }
    if let Ok(mut m) = MILESTONES.lock() {
        m.splice(0..0, previous);
    }
}

/// The first time init reached the milestone `name`, if it ever did.
///
/// "First" matters: after a re-exec `main loop` is in the list twice, and the
/// one `blame` wants when it asks where boot ended is the original boot's, not
/// the moment this image finished adopting what was already running.
pub fn first_milestone(name: &str) -> Option<f64> {
    MILESTONES
        .lock()
        .ok()?
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, at)| *at)
}
