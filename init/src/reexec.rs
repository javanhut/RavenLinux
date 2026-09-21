//! Replacing PID 1 without a reboot.
//!
//! `raven-rc reexec` asks the running init to `execv` the raven-init binary on
//! disk. The process keeps its pid -- it is still PID 1 -- and so keeps every
//! child it has: the services. What does not survive an exec is memory, so the
//! supervisor writes what it knows about those children to a file, execs, and
//! the new image reads the file back and adopts them.
//!
//! This exists for the development loop. Building a new raven-init on a
//! running machine used to mean a reboot to run it; now it means
//! `imlazy dev && sudo raven-rc reexec`. It is also what a package upgrade of
//! raven-init can use once there is one.
//!
//! # What is and is not carried across
//!
//! Carried: each service's definition, pid, restart count, "stopped by
//! request" flag, when it started and when it became ready -- and the boot
//! milestones init passed on its way up. That is everything the supervisor
//! needs to keep behaving as it was, plus everything `raven-rc blame` needs to
//! keep answering the question it is for. The milestones are the part that is
//! purely a report: without them the new image's timeline began at the moment
//! somebody typed `reexec`, and a machine that had been up for a day appeared
//! to have booted in four milliseconds.
//!
//! Not carried: the control socket (the new image binds it afresh; a
//! `raven-rc` that connects in the gap gets ECONNREFUSED and can try again),
//! and mounts, hostname, and the rest of early boot -- those are properties of
//! the system, not of the process, and are simply still there.
//!
//! # Where the binary comes from
//!
//! `/proc/self/exe` would be the natural choice and is the wrong one: after
//! `install` has replaced the file it points at a deleted inode, and
//! exec-ing it runs the *old* code again. The exec goes through the path init
//! was started by (`argv[0]`, `/sbin/raven-init` from the initramfs hand-off)
//! so it resolves to whatever is on disk now.

use std::ffi::CString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::service::ServiceSnapshot;

/// Where the outgoing supervisor leaves its state for the incoming one.
///
/// `/run` because it is a tmpfs mounted by init itself, so it is always there
/// and never survives a real boot -- a stale hand-off file cannot be mistaken
/// for a fresh one after a power cycle.
pub const STATE_PATH: &str = "/run/raven-init.reexec";

/// Set in the new image's environment so it knows it was re-executed rather
/// than booted, before it has looked at anything else.
const ENV_MARK: &str = "RAVEN_INIT_REEXEC";

/// Overrides the binary to exec. For tests and for "run the one I just built
/// from this tree" without installing it.
const ENV_EXE: &str = "RAVEN_INIT_EXE";

/// Where the hand-off file is written; overridable so tests need no /run.
fn state_path() -> PathBuf {
    std::env::var_os("RAVEN_INIT_REEXEC_STATE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(STATE_PATH))
}

/// One milestone on its way through the hand-off file.
///
/// A named struct rather than the `(String, f64)` the timeline keeps, because
/// this one is written into a file an operator may well read: `[[milestones]]`
/// with `name` and `at` beneath it says what it is, where `[["main loop",
/// 7.001]]` would need explaining.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MilestoneRecord {
    pub name: String,
    /// Seconds since the kernel started -- CLOCK_MONOTONIC, which belongs to
    /// the boot rather than to the process and so means the same number to the
    /// image that reads this file as to the one that wrote it.
    pub at: f64,
}

/// Everything one supervisor hands to the next.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handoff {
    /// Serial number, in case the format ever has to change under a running
    /// system: an old image that cannot read a new file should say so rather
    /// than adopt nothing.
    #[serde(default = "Handoff::current_version")]
    pub version: u32,
    /// The boot milestones so far. Before `services`, because TOML wants its
    /// plain values before its tables and serde writes these fields in the
    /// order they are declared.
    #[serde(default)]
    pub milestones: Vec<MilestoneRecord>,
    #[serde(default)]
    pub services: Vec<ServiceSnapshot>,
}

impl Handoff {
    /// Still 1 with the milestones and the per-service timings added, and
    /// deliberately: every field is `#[serde(default)]` and unknown keys are
    /// ignored, so a file written by either image parses in the other. Bumping
    /// it would make the *old* image refuse a file it can in fact read, and
    /// the refusal path adopts nothing -- which means starting every daemon a
    /// second time on a machine where they are all still running. A version
    /// that costs that much is for a change that truly cannot be read across,
    /// not for a change that merely carries more.
    const VERSION: u32 = 1;

    fn current_version() -> u32 {
        Self::VERSION
    }

    /// The hand-off for a supervisor about to be replaced.
    ///
    /// The milestones are collected here rather than passed in because they
    /// are not the caller's to remember: there is exactly one boot timeline in
    /// this process, `timeline` owns it, and a hand-off built without it is a
    /// hand-off that silently loses the boot.
    pub fn new(services: Vec<ServiceSnapshot>) -> Self {
        Self {
            version: Self::VERSION,
            milestones: crate::timeline::milestones()
                .into_iter()
                .map(|(name, at)| MilestoneRecord { name, at })
                .collect(),
            services,
        }
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string(self).context("Failed to serialise the hand-off")
    }

    pub fn from_toml(text: &str) -> Result<Self> {
        let handoff: Self = toml::from_str(text).context("Failed to parse the hand-off")?;
        if handoff.version > Self::VERSION {
            bail!(
                "hand-off is version {}, this raven-init reads up to {}",
                handoff.version,
                Self::VERSION
            );
        }
        Ok(handoff)
    }
}

/// The binary a re-exec would run, checked to exist and be executable.
///
/// Called at request time so the error reaches the operator; the exec itself
/// happens later, on the main loop, where there is nobody left to tell.
pub fn target() -> Result<PathBuf> {
    let path = match std::env::var_os(ENV_EXE) {
        Some(p) => PathBuf::from(p),
        None => match std::env::args_os().next().map(PathBuf::from) {
            Some(argv0) if argv0.is_absolute() => argv0,
            _ => PathBuf::from("/sbin/raven-init"),
        },
    };

    let meta = fs::metadata(&path).with_context(|| format!("{} is not there", path.display()))?;
    if !meta.is_file() {
        bail!("{} is not a regular file", path.display());
    }
    if meta.permissions().mode() & 0o111 == 0 {
        bail!("{} is not executable", path.display());
    }
    Ok(path)
}

/// Write the hand-off, then replace this process with `target()`.
///
/// Only returns on failure, in which case nothing has changed: the file is
/// removed again and the caller carries on supervising with the image it
/// has. Every service is untouched either way -- this function never signals
/// a child.
pub fn handoff(handoff: &Handoff) -> Result<std::convert::Infallible> {
    let exe = target()?;
    let path = state_path();

    write_state(&path, handoff)?;

    let exe_c = CString::new(exe.as_os_str().as_encoded_bytes())
        .context("exec path contains a NUL byte")?;
    // argv[0] stays the path, so the next re-exec after this one resolves the
    // same way this one did.
    let argv = [exe_c.clone()];

    std::env::set_var(ENV_MARK, "1");
    std::env::set_var("RAVEN_INIT_REEXEC_STATE", &path);

    log::info!(
        "Re-executing {} with {} services handed over",
        exe.display(),
        handoff.services.len()
    );

    // execv keeps the environment, which is how ENV_MARK gets across. Every
    // fd Rust opened is CLOEXEC, so the log file and the control socket close
    // here and the new image opens its own.
    let err = nix::unistd::execv(&exe_c, &argv).unwrap_err();

    std::env::remove_var(ENV_MARK);
    fs::remove_file(&path).ok();
    Err(anyhow::anyhow!("execv {} failed: {}", exe.display(), err))
}

fn write_state(path: &Path, handoff: &Handoff) -> Result<()> {
    let text = handoff.to_toml()?;
    fs::write(path, text).with_context(|| format!("Failed to write {}", path.display()))?;
    // Service definitions can carry environment variables, and those can be
    // secrets. Root only, like the control socket.
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).ok();
    Ok(())
}

/// If this process is the second half of a re-exec, take the hand-off.
///
/// `None` on a normal boot. The file is removed on the way out, so a crash
/// after this point cannot leave a hand-off lying around for a later image
/// to adopt pids that by then belong to something else.
///
/// Taking the hand-off also puts the previous supervisor's boot milestones
/// back into [`crate::timeline`], which is a side effect and is here on
/// purpose: this is the one moment in the process's life at which there is a
/// previous supervisor to inherit a boot from, and the alternative -- asking
/// every caller to remember to restore them -- is how the timeline came to be
/// lost in the first place. The services are restored by the caller, from the
/// value returned; the timeline has one global home and is restored into it.
pub fn take() -> Option<Handoff> {
    if std::env::var_os(ENV_MARK).is_none() {
        return None;
    }
    std::env::remove_var(ENV_MARK);

    let path = state_path();
    let text = fs::read_to_string(&path);
    fs::remove_file(&path).ok();
    std::env::remove_var("RAVEN_INIT_REEXEC_STATE");

    match text.map_err(anyhow::Error::from).and_then(|t| Handoff::from_toml(&t)) {
        Ok(handoff) => {
            crate::timeline::restore(
                handoff
                    .milestones
                    .iter()
                    .map(|m| (m.name.clone(), m.at))
                    .collect(),
            );
            Some(handoff)
        }
        Err(e) => {
            // The services are still running whether or not we can read
            // about them. Adopt nothing, and let the start path find each
            // one already holding its socket -- noisy, but it recovers.
            log::error!("Re-executed, but the hand-off could not be read: {:#}", e);
            log::error!("  Services will be started as at boot; expect conflicts");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServiceConfig;

    fn snap(name: &str, pid: Option<i32>) -> ServiceSnapshot {
        ServiceSnapshot {
            config: ServiceConfig {
                name: name.to_string(),
                exec: "/bin/true".to_string(),
                restart: true,
                ..ServiceConfig::default()
            },
            pid,
            started_mono: Some(3.5),
            first_started_mono: Some(3.5),
            uptime_secs: 42,
            restart_count: 3,
            manually_stopped: pid.is_none(),
            ..ServiceSnapshot::default()
        }
    }

    #[test]
    fn handoff_round_trips_through_toml() {
        let h = Handoff::new(vec![snap("a", Some(1234)), snap("b", None)]);
        let text = h.to_toml().unwrap();
        let back = Handoff::from_toml(&text).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn a_newer_handoff_is_refused_not_misread() {
        let text = "version = 99\n";
        let err = Handoff::from_toml(text).unwrap_err();
        assert!(err.to_string().contains("version 99"), "{err}");
    }

    /// The boot timeline is the half of the hand-off that no service needs
    /// and `blame` cannot do without. It goes through the file with the
    /// services, and it comes back out in the order it was recorded.
    #[test]
    fn the_boot_timeline_crosses_with_the_services() {
        crate::timeline::mark("handoff test milestone");
        let h = Handoff::new(vec![snap("a", Some(1234))]);
        let carried = h
            .milestones
            .iter()
            .any(|m| m.name == "handoff test milestone");
        assert!(carried, "milestones must be collected by Handoff::new");

        let back = Handoff::from_toml(&h.to_toml().unwrap()).unwrap();
        assert_eq!(h.milestones, back.milestones);
        let names: Vec<&str> = back.milestones.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"handoff test milestone"), "{names:?}");
    }

    /// A hand-off written before the timings existed still adopts. The fields
    /// it does not have default, and the ones it does are read.
    #[test]
    fn an_older_handoff_still_parses() {
        let text = "version = 1\n\n[[services]]\npid = 7\nuptime_secs = 42\n\n                    [services.config]\nname = \"old\"\nexec = \"/bin/true\"\n";
        let h = Handoff::from_toml(text).expect("an older hand-off must still be readable");
        let svc = h.services.first().expect("one service");
        assert_eq!(svc.uptime_secs, 42);
        assert_eq!(svc.started_mono, None);
        assert!(h.milestones.is_empty());
    }

    #[test]
    fn an_empty_file_is_a_valid_empty_handoff() {
        let h = Handoff::from_toml("").unwrap();
        assert_eq!(h.version, Handoff::VERSION);
        assert!(h.services.is_empty());
    }

    #[test]
    fn take_is_a_noop_without_the_mark() {
        std::env::remove_var(ENV_MARK);
        assert!(take().is_none());
    }
}
