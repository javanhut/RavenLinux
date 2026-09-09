//! The boot clock, and the milestones init passes on the way up.
//!
//! Everything here is in seconds since the kernel started (CLOCK_MONOTONIC),
//! the clock dmesg prints, so a line in init.log and a line in the kernel log
//! can be read against each other. Service start and ready times are kept on
//! the services themselves (see `Service::started_at` / `ready_at`) and
//! survive a re-exec with them; the milestones below do not, which is why
//! `raven-rc blame` says which raven-init's start it is counting from.

use std::sync::Mutex;
use std::time::Instant;

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

static MILESTONES: Mutex<Vec<(&'static str, f64)>> = Mutex::new(Vec::new());

/// Record that boot reached `name` now.
pub fn mark(name: &'static str) {
    if let Ok(mut m) = MILESTONES.lock() {
        m.push((name, monotonic_secs()));
    }
}

/// The milestones recorded so far, in the order they were reached.
pub fn milestones() -> Vec<(&'static str, f64)> {
    MILESTONES.lock().map(|m| m.clone()).unwrap_or_default()
}
