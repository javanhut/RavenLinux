//! Suspend to RAM.
//!
//! Sleeping is a privileged, one-writer operation: the kernel takes it from a
//! single `write(2)` to `/sys/power/state`, and that write does not return
//! until the machine is awake again. So it belongs to PID 1 for the same
//! reason `poweroff` does -- there is exactly one of it, it has to happen with
//! nothing else half-way through the same transition, and every caller that
//! wants it (`raven-rc suspend`, `raven-powerd` acting on the lid) can reach
//! init over the control socket it already has.
//!
//! # What this is not
//!
//! There is no inhibitor framework here. logind has one because it has to
//! arbitrate between a dozen desktop components that each believe they own the
//! lid; Raven has one daemon watching the buttons and one compositor drawing
//! the screen, and a knob in `/etc/raven/power.toml` settles the argument
//! before it starts. If a component ever does need to hold sleep off, the hook
//! directory below is the place to grow that, not a bus API.
//!
//! # The marker file
//!
//! `/run/raven-power/state` holds one word, `sleeping` or `awake`, rewritten
//! either side of the sleep. It exists because a Wayland compositor that held
//! DRM master across a suspend has to re-take the device and repaint: without
//! logind there is no `PrepareForSleep` to tell it so, and a file it can watch
//! with the inotify it already runs is the cheapest signal that does not
//! involve giving an unprivileged session a socket into PID 1. It is
//! world-readable on purpose -- the session runs as a normal user.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

/// The kernel's sleep entry point. A write here blocks until we are back.
const STATE_PATH: &str = "/sys/power/state";

/// Which sleep states we will use, best first.
///
/// `mem` is suspend-to-RAM, which is what a laptop lid means. `freeze` is the
/// software-only fallback (no firmware involvement, everything just stops); it
/// saves far less power but it works on hardware whose S3 is broken, and a
/// machine that resumes is better than one that never slept. `disk` is
/// deliberately absent: hibernation needs a resume= swap device configured at
/// boot, and picking one here on a machine that has not asked for it would be
/// how you lose a session.
const PREFERRED_STATES: &[&str] = &["mem", "freeze"];

/// Scripts run either side of the sleep, with `pre` or `post` as argv[1].
const HOOK_DIR: &str = "/etc/raven/sleep.d";

/// Directory holding the marker file. A tmpfs, so it never survives a boot.
const RUN_DIR: &str = "/run/raven-power";

/// The marker itself. See the module docs.
pub const STATE_MARKER: &str = "/run/raven-power/state";

/// Publish `awake` at boot, so the marker exists before anything watches it.
///
/// Without this the file appears for the first time halfway through the first
/// suspend, and a compositor that started before then found no directory to
/// watch and gave up. Cheap, and it makes the marker's absence mean one thing
/// only: this machine is not running raven-init.
pub fn publish_at_boot() {
    publish("awake");
}

/// Sleep, and return once the machine is awake again.
///
/// Everything before the write is ordered on the assumption that the write may
/// never return -- a suspend that wedges in firmware is a power-cycle, and
/// anything not on disk by then is gone.
pub fn suspend() -> Result<()> {
    let state = choose_state()?;

    log::info!("Suspending (state: {})", state);

    run_hooks("pre");
    publish("sleeping");

    // Dirty pages first. The kernel syncs on its own before a suspend, but it
    // does that after the hooks and after the freeze has already started; this
    // one happens while userspace is still running and can still be killed by
    // a person with a power button.
    sync();

    let result = write_state(state);

    // Before the hooks, deliberately. The compositor is watching this file and
    // the screen is black until it repaints, so the marker is what should
    // reach it first -- not whatever a `post` hook decides to spend a second
    // doing.
    publish("awake");
    run_hooks("post");

    match result {
        Ok(()) => {
            log::info!("Resumed from {}", state);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// The best sleep state this kernel actually offers.
fn choose_state() -> Result<&'static str> {
    let available = fs::read_to_string(STATE_PATH).with_context(|| {
        format!(
            "Cannot read {} -- this kernel has no suspend support",
            STATE_PATH
        )
    })?;

    // The file is a space-separated list of the states that will be accepted,
    // e.g. "freeze mem disk". Anything not listed is rejected with EINVAL.
    let offered: Vec<&str> = available.split_whitespace().collect();

    for state in PREFERRED_STATES {
        if offered.contains(state) {
            return Ok(state);
        }
    }

    bail!(
        "No usable sleep state: {} offers '{}', none of {:?}",
        STATE_PATH,
        available.trim(),
        PREFERRED_STATES
    )
}

/// The write that actually sleeps the machine.
fn write_state(state: &str) -> Result<()> {
    // Opened without truncation: sysfs ignores O_TRUNC, and asking for it on a
    // file whose write has this much meaning is noise.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(STATE_PATH)
        .with_context(|| format!("Cannot open {} for writing", STATE_PATH))?;

    // ---- the machine stops inside this call ----
    file.write_all(state.as_bytes()).with_context(|| {
        format!(
            "The kernel refused to enter '{}' -- a driver or a frozen task \
             blocked it; see dmesg",
            state
        )
    })?;

    Ok(())
}

/// Write one word to the marker file, for whoever is watching it.
///
/// Failure here is logged and swallowed. A compositor that misses a repaint is
/// a bad frame; refusing to suspend the machine over it would be worse.
fn publish(phase: &str) {
    if let Err(e) = publish_inner(phase) {
        log::warn!("Could not update {}: {:#}", STATE_MARKER, e);
    }
}

fn publish_inner(phase: &str) -> Result<()> {
    if !Path::new(RUN_DIR).is_dir() {
        fs::create_dir_all(RUN_DIR).with_context(|| format!("Cannot create {}", RUN_DIR))?;
        fs::set_permissions(RUN_DIR, fs::Permissions::from_mode(0o755)).ok();
    }

    // Written whole and replaced by rename, so a watcher that wakes on the
    // event never reads a half-written or empty file.
    let tmp = format!("{}.new", STATE_MARKER);
    fs::write(&tmp, format!("{}\n", phase)).with_context(|| format!("Cannot write {}", tmp))?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644)).ok();
    fs::rename(&tmp, STATE_MARKER).with_context(|| format!("Cannot install {}", STATE_MARKER))?;

    Ok(())
}

/// Run `/etc/raven/sleep.d/*` in name order, with the phase as argv[1].
///
/// Same shape as the shutdown hooks: executable files only, output discarded,
/// failures ignored. The one addition is the argument, because a sleep hook
/// almost always has two halves -- stop the thing, then start it again -- and
/// making that one script keeps the pair from drifting apart.
fn run_hooks(phase: &str) {
    if !Path::new(HOOK_DIR).is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(HOOK_DIR) else {
        return;
    };

    let mut scripts: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    scripts.sort_by_key(|e| e.file_name());

    for entry in scripts {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Ok(metadata) = path.metadata() else {
            continue;
        };
        if metadata.permissions().mode() & 0o111 == 0 {
            continue;
        }

        log::info!("Running sleep hook ({}): {:?}", phase, path);
        let _ = Command::new(&path)
            .arg(phase)
            .env("RAVEN_SLEEP_PHASE", phase)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Flush dirty pages. `sync(2)` cannot fail and returns nothing.
fn sync() {
    unsafe {
        libc::sync();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_states_are_ordered_deep_first() {
        // If this ever flips, every laptop starts "sleeping" at full power.
        assert_eq!(PREFERRED_STATES[0], "mem");
    }

    #[test]
    fn the_marker_lives_under_the_run_dir() {
        assert!(STATE_MARKER.starts_with(RUN_DIR));
    }
}
