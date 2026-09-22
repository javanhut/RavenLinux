//! Suspend to RAM, and hibernation to disk.
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
//! world-readable on purpose -- the session runs as a normal user. It is
//! written for a hibernation exactly as it is for a suspend: from the
//! compositor's side the two are the same event, a machine that stopped and
//! came back with the DRM device needing to be re-taken.
//!
//! # Hibernation, and why it is not just another state in the list
//!
//! Suspend picks the best state the kernel offers and gets on with it, because
//! every state it will pick costs nothing but power if it goes wrong: the
//! machine is still in RAM and a power cycle loses only what an unexpected
//! reboot would have lost anyway. `disk` is not like that. The image is
//! written to a swap device and the *only* thing that brings it back is the
//! `resume=` the next boot is told about, by the kernel or by the initramfs.
//! Hibernating a machine that has no resume device configured does not fail --
//! it succeeds, powers the machine down, and silently throws the session away
//! on the next boot, which is the worst failure in this file.
//!
//! So hibernation is a separate entry point with a precondition rather than an
//! extra word in [`PREFERRED_STATES`], and the precondition is checked here,
//! in the one process that is going to do the write, rather than being left to
//! whoever asked. That is also why `noresume` on the kernel command line is a
//! refusal: raven-snapshot puts it on every snapshot boot entry, and a machine
//! booted that way has explicitly been told not to come back from an image.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

/// The kernel's sleep entry point. A write here blocks until we are back.
const STATE_PATH: &str = "/sys/power/state";

/// Which sleep states [`suspend`] will use, best first.
///
/// `mem` is suspend-to-RAM, which is what a laptop lid means. `freeze` is the
/// software-only fallback (no firmware involvement, everything just stops); it
/// saves far less power but it works on hardware whose S3 is broken, and a
/// machine that resumes is better than one that never slept.
///
/// `disk` is deliberately absent, and the reason has changed since this list
/// was written. It used to be "hibernation needs a resume= swap device
/// configured at boot", which is no longer true of a machine this project
/// installs -- the installer writes `resume=` into boot.cfg and the initramfs
/// acts on it. The reason it stays out of *this* list is narrower and
/// permanent: a fallback must be safe to take without being asked for, and
/// `disk` never is. It throws the session away on any machine whose resume
/// device is missing, and a lid close that silently hibernated because S3 was
/// broken would be exactly that. Hibernation is [`hibernate`], which asks
/// first.
const PREFERRED_STATES: &[&str] = &["mem", "freeze"];

/// The state that hibernates. Never reached through [`PREFERRED_STATES`]; see
/// its comment.
const HIBERNATE_STATE: &str = "disk";

/// Where the kernel reports the device it will resume from, as `major:minor`.
///
/// `0:0` means none is set. The kernel fills this in from `resume=` on the
/// command line, and the initramfs writes it directly when it resolves a
/// resume device of its own, so between this and the command line the two ways
/// a Raven machine can be told where its image goes are both covered.
const RESUME_PATH: &str = "/sys/power/resume";

/// The kernel command line, read for `resume=` and for `noresume`.
const CMDLINE_PATH: &str = "/proc/cmdline";

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

    enter(state)
}

/// Hibernate, and return once the machine has been booted back into this
/// image -- or, if the write fails, once the kernel has refused.
///
/// The precondition is the whole difference between this and [`suspend`], and
/// it is checked here rather than anywhere above because there are three ways
/// in -- the control socket, the /run command file, and `raven-powerd` acting
/// on a lid or a button, which arrives over that same socket -- and a check
/// repeated at each of them is a check that will eventually disagree with
/// itself. This is the last code that runs before the write, so it is the only
/// place the answer cannot go stale. See the module header for why hibernating
/// without a resume device is worse than refusing to hibernate at all.
pub fn hibernate() -> Result<()> {
    let resume = resume_device(Path::new(RESUME_PATH), Path::new(CMDLINE_PATH))?;

    if !offers(HIBERNATE_STATE)? {
        bail!(
            "This kernel does not offer hibernation: {} does not list '{}'",
            STATE_PATH,
            HIBERNATE_STATE
        );
    }

    log::info!("Hibernating (resume device: {})", resume);

    enter(HIBERNATE_STATE)
}

/// Everything either side of the write, shared by both entry points.
///
/// Suspend and hibernation differ in what they check beforehand and in
/// nothing afterwards: the same hooks run, the same marker is published for
/// the compositor, and the same sync happens while userspace can still be
/// interrupted by a person with a power button. Keeping it in one function is
/// what stops the two from drifting into doing different amounts of that.
fn enter(state: &str) -> Result<()> {
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

/// The device this machine would resume from, or an error saying why there
/// isn't one.
///
/// Three sources, and they answer slightly different questions. `noresume` on
/// the command line is checked first because it is not a missing resume
/// device, it is an instruction to ignore one: raven-snapshot puts it on every
/// snapshot entry so that booting a snapshot cannot be hijacked by an image
/// written from the live system, and `noresume` is also what somebody types
/// when a resume has already gone wrong once. Hibernating either of those
/// machines would write an image nothing will ever read.
///
/// Then /sys/power/resume, which is what the kernel will actually use and is
/// therefore the authoritative answer; `0:0` is how it spells "nothing".
/// Finally `resume=` on the command line, because an initramfs that resolves
/// the device itself may not have written the sysfs file at the point this is
/// asked, and a `resume=UUID=...` the operator can see in their boot entry is
/// a better answer than a refusal.
///
/// Takes its paths so the tests can put a /sys and a /proc of their own in
/// front of it; the production callers pass the two constants.
fn resume_device(resume_path: &Path, cmdline_path: &Path) -> Result<String> {
    let cmdline = fs::read_to_string(cmdline_path).unwrap_or_default();

    if cmdline.split_whitespace().any(|word| word == "noresume") {
        bail!(
            "This machine was booted with 'noresume', so an image written now \
             would never be read back; refusing to hibernate"
        );
    }

    if let Ok(text) = fs::read_to_string(resume_path) {
        let device = text.trim();
        if !device.is_empty() && device != "0:0" {
            return Ok(device.to_string());
        }
    }

    if let Some(arg) = cmdline
        .split_whitespace()
        .find(|word| word.starts_with("resume="))
    {
        let value = &arg["resume=".len()..];
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }

    bail!(
        "No resume device is configured: {} names none and there is no \
         resume= on the kernel command line. Hibernating now would power the \
         machine down and lose the session on the next boot",
        resume_path.display()
    )
}

/// Whether `/sys/power/state` will accept this word.
fn offers(state: &str) -> Result<bool> {
    let available = fs::read_to_string(STATE_PATH).with_context(|| {
        format!(
            "Cannot read {} -- this kernel has no suspend support",
            STATE_PATH
        )
    })?;

    Ok(available.split_whitespace().any(|offered| offered == state))
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
    use std::path::PathBuf;

    #[test]
    fn preferred_states_are_ordered_deep_first() {
        // If this ever flips, every laptop starts "sleeping" at full power.
        assert_eq!(PREFERRED_STATES[0], "mem");
    }

    #[test]
    fn the_marker_lives_under_the_run_dir() {
        assert!(STATE_MARKER.starts_with(RUN_DIR));
    }

    /// `disk` must never be something a suspend can fall into. A lid close
    /// that quietly hibernated because this machine's S3 is broken would throw
    /// the session away on every machine without a resume device.
    #[test]
    fn suspend_can_never_fall_through_to_hibernation() {
        assert!(
            !PREFERRED_STATES.contains(&HIBERNATE_STATE),
            "hibernation is not a fallback; it is {}",
            stringify!(hibernate)
        );
    }

    /// One directory for this process's fixtures, so `cargo test` running
    /// these in threads cannot have two of them in the same place.
    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!("raven-power-{}", std::process::id()))
    }

    /// A little /sys/power/resume and /proc/cmdline of our own.
    fn fixture(tag: &str, resume: &str, cmdline: &str) -> (PathBuf, PathBuf) {
        let root = test_root().join(tag);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");
        let resume_path = root.join("resume");
        let cmdline_path = root.join("cmdline");
        fs::write(&resume_path, resume).expect("write resume");
        fs::write(&cmdline_path, cmdline).expect("write cmdline");
        (resume_path, cmdline_path)
    }

    /// The guard that stands between `hibernate` and a lost session.
    ///
    /// Every branch of it is a machine that really exists: one the installer
    /// gave swap and a `resume=`, one installed without swap, one booted into
    /// a snapshot (which raven-snapshot marks `noresume`), and one whose
    /// initramfs resolved the device itself before the kernel was told.
    #[test]
    fn hibernation_is_refused_unless_something_will_read_the_image_back() {
        // The ordinary installed laptop: the kernel has a resume device.
        let (resume, cmdline) = fixture(
            "configured",
            "259:2\n",
            "root=UUID=1234 resume=UUID=abcd rw\n",
        );
        assert_eq!(
            resume_device(&resume, &cmdline).expect("should hibernate"),
            "259:2"
        );

        // Installed without swap. /sys/power/resume spells "nothing" 0:0.
        let (resume, cmdline) = fixture("no-swap", "0:0\n", "root=UUID=1234 rw\n");
        let refusal = resume_device(&resume, &cmdline).expect_err("must refuse");
        assert!(
            format!("{:#}", refusal).contains("No resume device"),
            "{:#}",
            refusal
        );

        // A snapshot boot entry. The resume device is perfectly real and the
        // machine has been told not to use it, which is a refusal and not a
        // fallback to the sysfs value.
        let (resume, cmdline) = fixture(
            "noresume",
            "259:2\n",
            "root=UUID=1234 resume=UUID=abcd noresume rw\n",
        );
        let refusal = resume_device(&resume, &cmdline).expect_err("must refuse");
        assert!(
            format!("{:#}", refusal).contains("noresume"),
            "{:#}",
            refusal
        );

        // The initramfs resolved the device but the kernel has not recorded
        // it. The command line is still a true answer.
        let (resume, cmdline) = fixture(
            "cmdline-only",
            "0:0\n",
            "root=UUID=1234 resume=/dev/mapper/swap rw\n",
        );
        assert_eq!(
            resume_device(&resume, &cmdline).expect("should hibernate"),
            "/dev/mapper/swap"
        );

        // `resume=` with nothing after it is not a resume device.
        let (resume, cmdline) = fixture("empty-resume", "0:0\n", "root=UUID=1234 resume= rw\n");
        assert!(resume_device(&resume, &cmdline).is_err());

        // And a machine with no /sys/power/resume at all -- a kernel built
        // without hibernation -- is refused rather than panicking on the read.
        let (resume, cmdline) = fixture("absent", "0:0\n", "root=UUID=1234 rw\n");
        let _ = fs::remove_file(&resume);
        assert!(resume_device(&resume, &cmdline).is_err());

        let _ = fs::remove_dir_all(test_root());
    }
}
