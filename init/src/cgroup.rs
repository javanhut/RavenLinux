//! cgroup v2 resource control: the slice every service runs inside, and the
//! per-process limits that are set between fork and exec.
//!
//! init mounts cgroup2 on /sys/fs/cgroup at boot (see
//! `mount_essential_filesystems` in main.rs) and, until this module existed,
//! nothing ever wrote to it. Every service therefore ran in the root cgroup
//! alongside init itself, which cost three things worth having: a daemon that
//! leaked memory took the machine down with it rather than being killed on its
//! own, `raven-rc status` had no way to say what a service was actually
//! consuming, and a service's children were only findable by walking process
//! groups -- which works right up until a daemon calls `setsid` and walks out
//! of the group init put it in.
//!
//! The layout is one level deep and deliberately boring:
//!
//! ```text
//! /sys/fs/cgroup/                  cgroup2 root, mounted by init
//! /sys/fs/cgroup/raven.slice/      every supervised service, and nothing else
//! /sys/fs/cgroup/raven.slice/dbus/ one directory per service, named for it
//! ```
//!
//! The name comes from the `.slice` convention rather than from any intention
//! to be compatible with systemd's hierarchy: it is a directory in a mounted
//! filesystem, and calling it `raven.slice` makes it obvious to anyone reading
//! `systemd-cgls`-shaped output, or `/proc/<pid>/cgroup`, that init put the
//! process there on purpose.
//!
//! # What this is not
//!
//! It is not a delegation framework. There is no per-user slice, no nesting
//! below the service, no freezer, and no cpuset: a service gets one directory
//! and init owns it. Nothing here hands a cgroup's write permission to an
//! unprivileged process, so nothing here needs the `+memory +pids` dance that
//! delegation requires further down a tree.
//!
//! It is also not mandatory. Every operation in this file degrades to a no-op
//! with a warning if cgroup2 is missing, unmounted, read-only or built without
//! the controller in question. An init that refuses to start a login daemon
//! because `memory.max` could not be opened has turned a resource-accounting
//! nicety into an unbootable machine, and the person holding the keyboard
//! cannot fix a cgroup file from a shell they cannot reach.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

use crate::config::{ResourceLimits, ServiceConfig};

/// Where the cgroup2 hierarchy lives, unless `$RAVEN_CGROUP_ROOT` says
/// otherwise.
pub const DEFAULT_ROOT: &str = "/sys/fs/cgroup";

/// The directory under the cgroup2 root that holds one child per service.
pub const SLICE_NAME: &str = "raven.slice";

/// The controllers init asks the kernel to make available inside the slice.
///
/// Only these four, and each for a reason a service definition can actually
/// use: `memory` for `memory_max`, `cpu` for `cpu_weight`, `io` for
/// `io_weight`, and `pids` because `pids.current` is the cheapest honest
/// answer to "how many processes does this service really have", which the
/// process-group walk it replaces got wrong for every daemon that forks.
///
/// `cpuset`, `hugetlb`, `rdma` and `misc` are deliberately left out. Enabling a
/// controller in `cgroup.subtree_control` is not free -- it makes the kernel
/// account for it on every child -- and nothing in a service definition can
/// ask for any of them, so enabling them would buy overhead and no feature.
const CONTROLLERS: [&str; 4] = ["memory", "cpu", "io", "pids"];

/// The narrowest and widest values the kernel accepts for `cpu.weight` and
/// `io.weight`. Both default to 100, so a service asking for 200 is asking for
/// twice the share of one that says nothing.
const WEIGHT_MIN: u32 = 1;
const WEIGHT_MAX: u32 = 10_000;

/// Latches the "there are no cgroups here" warning.
///
/// [`Cgroup::for_service`] runs on every start, and a service in a restart
/// backoff is started once a minute forever. Warning from inside that path
/// without a latch is the same mistake the restart decision made before it
/// grew `retry_at` (see `Service::should_restart`): a line a minute, on the
/// console, about a condition that cannot change without a reboot. The first
/// occurrence is a warning because it is news; every one after it is a debug
/// line because it is not.
static ABSENCE_REPORTED: AtomicBool = AtomicBool::new(false);

/// The cgroup2 mount point.
///
/// Overridable through `$RAVEN_CGROUP_ROOT`, which is how a test drives the
/// path init itself takes -- `Service::do_start` asks for a cgroup by name and
/// has no parameter to point somewhere else. Tests that call into this module
/// directly should prefer [`ensure_slice_at`] and [`Cgroup::for_service_in`],
/// which need no environment at all and therefore cannot race a test running
/// beside them. The variable is read on every call rather than cached because
/// a test sets it long after this process started, and because the cost is one
/// `getenv` on a path that already does file I/O.
pub fn root() -> PathBuf {
    std::env::var_os("RAVEN_CGROUP_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ROOT))
}

/// Create `raven.slice` and delegate the controllers into it.
///
/// Idempotent, because it has to be: `early_boot` is skipped entirely on
/// `raven-rc reexec` (see `run_init`), so an init that only set the slice up
/// there would hand every service started after a re-exec an unconfined
/// process. This runs at boot for the log line, and again from
/// [`Cgroup::for_service`] on every start, and the second and thousandth calls
/// must be as harmless as the first.
///
/// Returns false when there is no usable cgroup2 here, which is the caller's
/// cue to carry on without resource control rather than to fail a start.
pub fn ensure_slice() -> bool {
    ensure_slice_at(&root())
}

/// [`ensure_slice`] against a named directory instead of the machine's.
///
/// Exists for the same reason `Service::should_restart_at` takes a clock: a
/// test that has to set an environment variable to reach the code under test
/// is a test that races every other test in the binary, and the one that loses
/// fails somewhere unrelated. The public entry point above supplies the real
/// root and is what init calls.
pub fn ensure_slice_at(root: &Path) -> bool {
    // The presence probe. `cgroup.controllers` exists in every cgroup2
    // directory and nowhere else, so finding it readable proves both that the
    // mount happened and that this kernel has cgroup2 at all -- which a plain
    // `Path::exists` on the mount point does not, since the directory is there
    // whether or not anything is mounted on it.
    if std::fs::read_to_string(root.join("cgroup.controllers")).is_err() {
        report_absence(&format!(
            "{} is not a cgroup2 filesystem; services will run without resource control",
            root.display()
        ));
        return false;
    }

    let slice = root.join(SLICE_NAME);
    if let Err(e) = std::fs::create_dir_all(&slice) {
        report_absence(&format!(
            "cannot create {}: {e}; services will run without resource control",
            slice.display()
        ));
        return false;
    }

    // Order matters and is not obvious: a controller is usable inside
    // `raven.slice/<service>` only if it appears in `raven.slice`'s
    // `cgroup.subtree_control`, and it can only be written there if the root
    // already delegated it downwards. So the root first, then the slice --
    // and the slice's own `cgroup.controllers` does not list anything until
    // the root's write has gone through, which is why this is two passes and
    // not one loop over both directories with a shared list.
    delegate_controllers(root);
    delegate_controllers(&slice);

    true
}

/// Write `+<controller>` into one directory's `cgroup.subtree_control` for
/// every controller this kernel has and this init wants.
///
/// One `write` per controller, on purpose. The kernel parses the whole write
/// as a single transaction: `+memory +cpu +io +pids` on a machine built
/// without the io controller fails with `EINVAL` and enables *none* of them,
/// which is how a missing controller turns into no resource control at all
/// rather than into three quarters of it.
fn delegate_controllers(dir: &Path) {
    let Ok(available) = std::fs::read_to_string(dir.join("cgroup.controllers")) else {
        // Not a cgroup directory (a temporary directory under test, most
        // likely) or unreadable. Either way there is nothing to delegate and
        // the caller has already said its piece.
        return;
    };
    let available: Vec<&str> = available.split_whitespace().collect();

    let subtree = dir.join("cgroup.subtree_control");
    let enabled = std::fs::read_to_string(&subtree).unwrap_or_default();
    let enabled: Vec<&str> = enabled.split_whitespace().collect();

    for controller in CONTROLLERS {
        if !available.contains(&controller) {
            log::debug!(
                "cgroup: kernel has no {controller} controller at {}",
                dir.display()
            );
            continue;
        }
        if enabled.contains(&controller) {
            continue;
        }
        if let Err(e) = std::fs::write(&subtree, format!("+{controller}")) {
            // Not fatal, and not even unusual: enabling a controller in a
            // directory that already holds processes is refused with EBUSY,
            // and on a machine where something else got to /sys/fs/cgroup
            // first that is exactly what will happen.
            log::warn!(
                "cgroup: cannot enable {controller} in {}: {e}",
                subtree.display()
            );
        }
    }
}

/// Say once that there is no resource control here, then stop saying it.
fn report_absence(message: &str) {
    if ABSENCE_REPORTED.swap(true, Ordering::SeqCst) {
        log::debug!("cgroup: {message}");
    } else {
        log::warn!("cgroup: {message}");
    }
}

/// Whether a service's name can be a directory inside the slice.
///
/// A service name reaches the cgroup layer straight out of init.toml, and a
/// name containing a slash would silently create a nested cgroup -- or, with
/// the right dots, one outside the slice entirely. The supervisor has no use
/// for either, so both are refused before a path is ever built from the name.
/// The same three rules are applied by the status publisher to the file names
/// it writes (see `StatusPublisher::publish_inner`), for the same reason.
fn name_is_usable(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.starts_with('.')
}

/// One service's cgroup: the directory, and the operations init performs on it.
///
/// Deliberately holds nothing but a path. The handle is *not* stored on
/// [`crate::service::Service`], because the path is a pure function of the
/// service's name and rebuilding it costs one `join`. That matters on the
/// adopt path: a re-executed init rebuilds its service table from
/// `ServiceSnapshot`s, and a cgroup handle carried in the snapshot would be one
/// more field that has to survive a TOML round-trip in order to describe
/// something the kernel already knows -- cgroup membership is a property of the
/// process, and the exec that replaced the supervisor did not disturb it.
pub struct Cgroup {
    path: PathBuf,
}

impl Cgroup {
    /// The cgroup for `name`, created if it does not exist.
    ///
    /// `None` means "carry on without it": no cgroup2, an unwritable slice, or
    /// a service name that cannot be a directory. Every caller treats that as
    /// a degraded start, never as a failed one.
    pub fn for_service(name: &str) -> Option<Self> {
        Self::for_service_in(&root(), name)
    }

    /// [`Cgroup::for_service`] under a named cgroup2 root, for the tests.
    pub fn for_service_in(root: &Path, name: &str) -> Option<Self> {
        if !name_is_usable(name) {
            log::warn!("cgroup: '{name}' cannot name a cgroup directory; no resource control");
            return None;
        }

        if !ensure_slice_at(root) {
            return None;
        }

        let path = root.join(SLICE_NAME).join(name);
        if let Err(e) = std::fs::create_dir_all(&path) {
            log::warn!("cgroup: cannot create {}: {e}", path.display());
            return None;
        }

        Some(Self { path })
    }

    /// The cgroup `name` already has, or `None` if it has none.
    ///
    /// The difference from [`Cgroup::for_service`] is the whole point: this
    /// one never creates anything. Stopping a service, reporting on one and
    /// tidying up after one all happen at moments when creating a cgroup
    /// would be wrong -- a `raven-rc status` on a service that has never run
    /// would leave a directory behind on the way to printing that it is not
    /// running, and a machine with no cgroup2 would get a warning for every
    /// question asked of it rather than the single latched one
    /// [`ensure_slice`] already printed at boot.
    pub fn existing(name: &str) -> Option<Self> {
        Self::existing_in(&root(), name)
    }

    /// [`Cgroup::existing`] under a named cgroup2 root, for the tests.
    pub fn existing_in(root: &Path, name: &str) -> Option<Self> {
        if !name_is_usable(name) {
            return None;
        }
        let path = root.join(SLICE_NAME).join(name);
        path.is_dir().then_some(Self { path })
    }

    /// Send `signal_to_send` to every process in this cgroup, and say whether
    /// the cgroup was able to do it.
    ///
    /// `false` means the caller should fall back to signalling the process
    /// group: either this cgroup holds nothing (the service is already gone,
    /// or it was started by a supervisor that had no cgroup to put it in) or
    /// its files could not be read at all.
    ///
    /// This is the reason the slice exists. A process group is the wrong
    /// handle for a service tree and always was: the first thing a daemon
    /// written in the 1990s does is `setsid`, which moves it and everything it
    /// forks afterwards out of the group init put it in, and from then on
    /// `kill(-pgid)` reaches nobody. A cgroup is not escapable that way --
    /// membership is inherited by every `fork` and survives `setsid`,
    /// `daemon(3)` and a double fork, and the only way out is a write to
    /// another cgroup's `cgroup.procs`, which a service running as an
    /// unprivileged account cannot perform on a slice init owns.
    pub fn signal_all(&self, signal_to_send: Signal) -> bool {
        // `cgroup.kill` (kernel 5.14 and later) kills the entire subtree in
        // one write, with the kernel holding the cgroup's lock throughout, so
        // a process that forks while it is being killed cannot outrun it. The
        // read-and-signal loop below can be outrun, which is why the kernel
        // grew this file. It is SIGKILL and only SIGKILL, so every other
        // signal takes the loop.
        if signal_to_send == Signal::SIGKILL && self.write_kill_file() {
            return true;
        }
        self.signal_each(signal_to_send)
    }

    /// Ask the kernel to kill everything in this cgroup atomically.
    fn write_kill_file(&self) -> bool {
        let path = self.path.join("cgroup.kill");
        if !path.exists() {
            // A kernel older than 5.14, or the root cgroup, which has no such
            // file. Neither is an error worth a line in anyone's log.
            return false;
        }
        match std::fs::write(&path, "1") {
            Ok(()) => true,
            Err(e) => {
                log::debug!("cgroup: cannot write {}: {e}", path.display());
                false
            }
        }
    }

    /// Signal each pid listed in `cgroup.procs`, one at a time.
    ///
    /// The fallback for every signal that is not SIGKILL, and for kernels
    /// without `cgroup.kill`. There is a race here that cannot be closed from
    /// user space: a pid read out of the file may have exited and had its
    /// number reused by the time the signal is sent. It is narrow -- the read
    /// and the loop are microseconds apart and init is the only process on the
    /// machine reaping, so a number cannot be recycled without init having
    /// waited on it first -- and it is the race every supervisor that predates
    /// `cgroup.kill` lived with. Sending SIGTERM to the wrong process is
    /// survivable in a way that not stopping a service is not.
    fn signal_each(&self, signal_to_send: Signal) -> bool {
        let Ok(text) = std::fs::read_to_string(self.procs()) else {
            return false;
        };

        let mut signalled = 0usize;
        for line in text.lines() {
            let Ok(raw) = line.trim().parse::<i32>() else {
                continue;
            };
            // Never 0 (the caller's own process group), never 1 (init itself).
            // Neither can legitimately appear in a service's cgroup, and both
            // would be catastrophic rather than merely wrong.
            if raw <= 1 {
                continue;
            }
            if signal::kill(Pid::from_raw(raw), signal_to_send).is_ok() {
                signalled += 1;
            }
        }

        signalled > 0
    }

    /// The directory this cgroup is.
    ///
    /// Nothing inside this module needs it -- it is here because everything
    /// that will ever want to act on a service's cgroup from outside (killing
    /// what is in it, reporting on it) starts by naming the directory.
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file a process writes its own pid into to join this cgroup.
    fn procs(&self) -> PathBuf {
        self.path.join("cgroup.procs")
    }

    /// Apply the limits that belong to the cgroup rather than to the process.
    ///
    /// These are written by the parent, before the fork, because they are
    /// properties of the directory: whatever ends up inside it is subject to
    /// them, including children the service forks later and processes adopted
    /// into it by a future restart. Contrast [`ChildResources`], which carries
    /// the per-process settings that only exist once there is a process.
    ///
    /// A value the kernel rejects costs that one setting and nothing else.
    pub fn apply(&self, config: &ServiceConfig) {
        if let Some(text) = &config.memory_max {
            match parse_size(text) {
                Some(u64::MAX) => self.write_attr("memory.max", "max"),
                Some(bytes) => self.write_attr("memory.max", &bytes.to_string()),
                None => log::warn!(
                    "{}: memory_max = \"{text}\" is not a size; expected 512M, 2G or max",
                    self.name()
                ),
            }
        }

        if let Some(weight) = config.cpu_weight {
            let weight = self.clamp_weight("cpu_weight", weight);
            self.write_attr("cpu.weight", &weight.to_string());
        }

        if let Some(weight) = config.io_weight {
            let weight = self.clamp_weight("io_weight", weight);
            // `io.weight` exists only when a weight-capable I/O policy (BFQ,
            // or iocost with a cost model) is active on at least one device.
            // On a machine using mq-deadline the file is simply absent, and
            // write_attr's warning is the whole story -- there is no
            // additional fallback worth writing, because the kernel has no
            // notion of I/O share to express.
            self.write_attr("io.weight", &weight.to_string());
        }
    }

    /// Current charged memory, in bytes.
    ///
    /// What `raven-rc status` reports, and the number a memory limit is
    /// argued about with. It is read on demand, never cached: a counter the
    /// supervisor sampled on a timer would be a number that is wrong by up to
    /// one tick and a write to /run every time it moved.
    pub fn memory_current(&self) -> Option<u64> {
        self.read_u64("memory.current")
    }

    /// How many processes the service has right now, forks included.
    ///
    /// The honest answer the process-group walk could not give: a daemon that
    /// called `setsid` left the group but not the cgroup, so this counts it.
    pub fn pids_current(&self) -> Option<u64> {
        self.read_u64("pids.current")
    }

    /// The CPU time this cgroup has accumulated, from `cpu.stat`.
    ///
    /// Cumulative over the life of the cgroup, which means over the life of
    /// the current run: [`Cgroup::remove`] takes the directory away when the
    /// service exits, so a restarted service starts counting from zero again
    /// rather than reporting a total nobody can attribute to a run.
    pub fn cpu_stat(&self) -> Option<CpuStat> {
        let text = std::fs::read_to_string(self.path.join("cpu.stat")).ok()?;
        let mut stat = CpuStat::default();
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            let (Some(key), Some(value)) = (parts.next(), parts.next()) else {
                continue;
            };
            let Ok(value) = value.parse::<u64>() else {
                continue;
            };
            match key {
                "usage_usec" => stat.usage_usec = value,
                "user_usec" => stat.user_usec = value,
                "system_usec" => stat.system_usec = value,
                _ => {}
            }
        }
        Some(stat)
    }

    /// Remove the cgroup directory, once, without retrying.
    ///
    /// `rmdir` on a cgroup succeeds only once no process is left in it, and
    /// the failure when one is is `EBUSY`. That is not an error here and is
    /// deliberately not waited on: the supervisor calls this from the moment
    /// it reaps a service's leader, and a service whose leader has died while
    /// a grandchild lives on is precisely the case the cgroup exists to
    /// describe. Leaving the directory keeps that grandchild countable, keeps
    /// `raven-rc status` able to say what it is still using, and keeps the
    /// next `stop` able to reach it.
    ///
    /// Retrying would be worse than useless. A loop here runs on PID 1's
    /// thread; a timer here is a per-tick stat on every service that has ever
    /// exited, for a directory costing a few hundred bytes of kernel memory
    /// that the next start reuses verbatim. So: one attempt, no spinning, and
    /// nothing above WARN in the log for the ordinary outcomes -- the machine
    /// is not less healthy for having one directory more than it needs.
    pub fn remove(&self) {
        match std::fs::remove_dir(&self.path) {
            Ok(()) => log::debug!("cgroup: removed {}", self.path.display()),
            // Already gone: two reapers raced, or the service never had one.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            // Still populated. EBUSY is what cgroupfs says; ENOTEMPTY is what
            // an ordinary filesystem says, which is what the tests run on.
            Err(e)
                if matches!(
                    e.raw_os_error(),
                    Some(libc::EBUSY) | Some(libc::ENOTEMPTY)
                ) =>
            {
                log::debug!(
                    "cgroup: {} still has processes in it; leaving it",
                    self.path.display()
                );
            }
            Err(e) => log::debug!("cgroup: cannot remove {}: {e}", self.path.display()),
        }
    }

    /// The service this cgroup belongs to, for messages.
    fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }

    fn clamp_weight(&self, field: &str, weight: u32) -> u32 {
        let clamped = weight.clamp(WEIGHT_MIN, WEIGHT_MAX);
        if clamped != weight {
            log::warn!(
                "{}: {field} = {weight} is outside {WEIGHT_MIN}..{WEIGHT_MAX}; using {clamped}",
                self.name()
            );
        }
        clamped
    }

    fn write_attr(&self, file: &str, value: &str) {
        let path = self.path.join(file);
        if let Err(e) = std::fs::write(&path, value) {
            log::warn!("{}: cannot write {value} to {}: {e}", self.name(), path.display());
        }
    }

    fn read_u64(&self, file: &str) -> Option<u64> {
        std::fs::read_to_string(self.path.join(file))
            .ok()?
            .trim()
            .parse()
            .ok()
    }
}

/// The three numbers `cpu.stat` has that a supervisor can use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuStat {
    /// Total CPU time, microseconds.
    pub usage_usec: u64,
    /// Of which spent in user space.
    pub user_usec: u64,
    /// Of which spent in the kernel on this cgroup's behalf.
    pub system_usec: u64,
}

/// Everything that has to be done to the service process itself, packed into a
/// form that is safe to use after `fork(2)`.
///
/// This exists because of a rule the child has no way to bend: between `fork`
/// and `exec` only async-signal-safe work is allowed, which rules out
/// allocating, formatting a string, taking a lock and logging. Every path is
/// therefore turned into a `CString` here, in the parent, and every number into
/// the bytes that will be written; the child does nothing but `open`, `write`,
/// `close` and two raw syscalls.
///
/// The settings are applied in the child rather than by the parent after
/// `spawn` for one reason worth stating: a daemon that forks immediately --
/// and several do -- can produce a grandchild before the parent gets back from
/// `spawn`, and a pid written into `cgroup.procs` a moment too late moves the
/// service in while leaving its first child outside. Doing it before `exec`
/// means there has never been a moment when the process was running the
/// service's code outside its cgroup. The same argument applies to the
/// rlimits, which are inherited across `fork` and preserved across `exec`.
pub struct ChildResources {
    /// Path to this service's `cgroup.procs`, or `None` for a service with no
    /// usable cgroup.
    procs: Option<CString>,
    /// The exact bytes to write to `/proc/self/oom_score_adj`, if the service
    /// asked for a value.
    oom_score_adj: Option<Vec<u8>>,
    /// The nice value to ask `setpriority` for, if it is not the default.
    nice: Option<libc::c_int>,
    /// `setrlimit` arguments, resource by resource.
    rlimits: Vec<(u32, libc::rlimit)>,
}

impl ChildResources {
    /// Work out, in the parent, everything the child will need.
    ///
    /// `cgroup` is `None` on a machine without cgroup2, and the result is then
    /// simply a set of resources that does not include the join -- the nice
    /// value and the rlimits still apply, because they are plain POSIX and
    /// have nothing to do with cgroups.
    pub fn prepare(config: &ServiceConfig, cgroup: Option<&Cgroup>) -> Self {
        // CString::new fails only on an interior NUL, which a path read from a
        // filesystem cannot contain; `.ok()` rather than a panic all the same,
        // because this is PID 1 and the release profile aborts on unwind.
        let procs = cgroup.and_then(|cg| CString::new(cg.procs().as_os_str().as_bytes()).ok());

        let oom_score_adj = if config.oom_score_adj == 0 {
            // Zero is what a child of init inherits anyway, so writing it
            // would be an open, a write and a close to change nothing.
            None
        } else {
            Some(format!("{}\n", config.oom_score_adj).into_bytes())
        };

        let nice = if config.nice == 0 {
            None
        } else {
            Some(config.nice as libc::c_int)
        };

        Self {
            procs,
            oom_score_adj,
            nice,
            rlimits: rlimits_for(&config.name, &config.limits),
        }
    }

    /// True when there is nothing to do, so the caller can skip registering a
    /// `pre_exec` closure at all.
    pub fn is_empty(&self) -> bool {
        self.procs.is_none()
            && self.oom_score_adj.is_none()
            && self.nice.is_none()
            && self.rlimits.is_empty()
    }

    /// Apply everything, in the child, between `fork` and `exec`.
    ///
    /// Every failure is swallowed. There is nowhere to report one to -- the
    /// child cannot log and must not allocate -- and none of these settings is
    /// worth failing a start over: a service that runs without its nice value
    /// is a service that runs. The cases that *can* be diagnosed were
    /// diagnosed in the parent, where [`Cgroup::for_service`] already warned
    /// if the cgroup could not be created and [`rlimits_for`] already warned
    /// about a limit it could not parse. What is left here is the small set of
    /// failures that only the kernel can produce, at a moment when saying so
    /// is impossible.
    ///
    /// # Safety
    ///
    /// Must be called only in a child process between `fork` and `exec`, with
    /// the ordinary async-signal-safety rules in force. Everything it calls is
    /// a raw syscall, and every buffer it reads was allocated before the fork.
    pub unsafe fn apply_in_child(&self) {
        // The cgroup join comes first, so that a memory limit is already in
        // force before the service's image is loaded, and so it happens while
        // the process is still root -- `cgroup.procs` in a slice init owns is
        // not writable by the account a service may be about to drop to.
        if let Some(procs) = &self.procs {
            let fd = libc::open(procs.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
            if fd >= 0 {
                // "0" means "the process doing the writing". The kernel reads
                // it as `current`, which spares the child having to format its
                // own pid into a buffer it is not allowed to allocate.
                libc::write(fd, b"0\n".as_ptr().cast(), 2);
                libc::close(fd);
            }
        }

        // Lowering oom_score_adj below zero needs privilege, so this too has
        // to happen before any setuid.
        if let Some(bytes) = &self.oom_score_adj {
            const OOM_PATH: &[u8] = b"/proc/self/oom_score_adj\0";
            let fd = libc::open(
                OOM_PATH.as_ptr().cast(),
                libc::O_WRONLY | libc::O_TRUNC | libc::O_CLOEXEC,
            );
            if fd >= 0 {
                libc::write(fd, bytes.as_ptr().cast(), bytes.len());
                libc::close(fd);
            }
        }

        // A negative nice value needs CAP_SYS_NICE; a positive one is always
        // allowed. Same reasoning, same placement.
        if let Some(nice) = self.nice {
            libc::setpriority(libc::PRIO_PROCESS as _, 0, nice);
        }

        // Raising a hard limit needs CAP_SYS_RESOURCE, which this process has
        // and the service's account very likely does not.
        for (resource, limit) in &self.rlimits {
            libc::setrlimit(*resource as _, limit);
        }
    }
}

/// Turn a service's `[services.limits]` table into `setrlimit` arguments.
///
/// Soft and hard are set to the same value, which is the useful reading of
/// `nofile = 8192` in a configuration file: a service that can raise its own
/// soft limit back up has not been limited, and one whose soft limit is below
/// its hard limit just makes every library that calls `getrlimit` guess. A
/// daemon that genuinely wants the split can call `setrlimit` itself, which is
/// the only place the distinction is ever used deliberately.
///
/// The `as u32` on each `RLIMIT_*` constant is a portability cast, not a
/// redundant one, and clippy is allowed to be wrong about it here: the libc
/// crate types those constants as `__rlimit_resource_t`, which is `c_uint` on
/// glibc and `c_int` on musl. On a glibc build the cast is a no-op and clippy
/// says so; drop it for that reason and the musl build -- which is how several
/// Raven components are shipped -- stops compiling.
#[allow(clippy::unnecessary_cast)]
fn rlimits_for(service: &str, limits: &ResourceLimits) -> Vec<(u32, libc::rlimit)> {
    let mut out = Vec::new();

    let mut push = |resource: u32, value: u64| {
        out.push((
            resource,
            libc::rlimit {
                rlim_cur: value as libc::rlim_t,
                rlim_max: value as libc::rlim_t,
            },
        ));
    };

    // Counts are plain numbers in TOML; sizes are strings, because "64M" is
    // not a TOML integer and writing 67108864 in a configuration file is how
    // an off-by-one-zero gets shipped.
    if let Some(n) = limits.nofile {
        push(libc::RLIMIT_NOFILE as u32, n);
    }
    if let Some(n) = limits.nproc {
        push(libc::RLIMIT_NPROC as u32, n);
    }
    for (field, text, resource) in [
        ("memlock", &limits.memlock, libc::RLIMIT_MEMLOCK as u32),
        ("core", &limits.core, libc::RLIMIT_CORE as u32),
    ] {
        let Some(text) = text else { continue };
        match parse_size(text) {
            Some(bytes) => push(resource, bytes),
            None => log::warn!(
                "{service}: limits.{field} = \"{text}\" is not a size; expected 64M, 0 or unlimited"
            ),
        }
    }

    out
}

/// Parse "512", "64K", "2G" or "max" into a byte count.
///
/// Written here rather than pulled in as a crate because the only thing a
/// dependency would add is a table of suffixes and a parser for the half-dozen
/// spellings nobody writes, at the cost of a new entry in Cargo.lock for a
/// program that has to be buildable from a machine that has just been
/// bootstrapped.
///
/// Binary multiples throughout: `1K` is 1024 bytes, not 1000. That is what the
/// kernel means everywhere it accepts a suffix, and a memory limit that turns
/// out to be 2.4% smaller than the number in the file is the kind of surprise
/// that gets diagnosed as a leak in the daemon.
///
/// `max`, `unlimited` and `infinity` all return `u64::MAX`, which is what
/// `RLIM_INFINITY` is and what [`Cgroup::apply`] turns back into the literal
/// string `max` for `memory.max`. `None` is returned for anything that is not
/// a size at all, and every caller reports that and moves on rather than
/// guessing.
pub fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let lowered = text.to_ascii_lowercase();
    if matches!(lowered.as_str(), "max" | "unlimited" | "infinity") {
        return Some(u64::MAX);
    }

    // Split at the first character that is not part of the number. The suffix
    // is matched case-insensitively and both "M" and "MiB" are accepted,
    // because both appear in every other tool's documentation and refusing one
    // of them teaches nothing.
    let digits_end = lowered
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(lowered.len());
    let (digits, suffix) = lowered.split_at(digits_end);

    let number: u64 = digits.parse().ok()?;
    let multiplier: u64 = match suffix.trim() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1024,
        "m" | "mb" | "mib" => 1024 * 1024,
        "g" | "gb" | "gib" => 1024 * 1024 * 1024,
        "t" | "tb" | "tib" => 1024 * 1024 * 1024 * 1024,
        _ => return None,
    };

    // A size that overflows is a typo, not a request for everything: report it
    // as unparseable so the operator sees the warning naming their value.
    number.checked_mul(multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_binary_multiples_and_suffixes_are_optional() {
        assert_eq!(parse_size("0"), Some(0));
        assert_eq!(parse_size("512"), Some(512));
        assert_eq!(parse_size("512B"), Some(512));
        assert_eq!(parse_size("64K"), Some(64 * 1024));
        assert_eq!(parse_size("2M"), Some(2 * 1024 * 1024));
        assert_eq!(parse_size("2G"), Some(2 * 1024 * 1024 * 1024));
        assert_eq!(parse_size(" 1 g "), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("1MiB"), Some(1024 * 1024));
        assert_eq!(parse_size("1mb"), Some(1024 * 1024));
    }

    #[test]
    fn the_words_for_no_limit_all_mean_rlim_infinity() {
        assert_eq!(parse_size("max"), Some(u64::MAX));
        assert_eq!(parse_size("unlimited"), Some(u64::MAX));
        assert_eq!(parse_size("Infinity"), Some(u64::MAX));
    }

    /// A value nobody can act on must not silently become one somebody can:
    /// "2 gigabytes" parsed as 2 bytes would OOM-kill a daemon at startup and
    /// look like the daemon's fault.
    #[test]
    fn a_size_that_is_not_a_size_is_refused_rather_than_guessed_at() {
        assert_eq!(parse_size(""), None);
        assert_eq!(parse_size("   "), None);
        assert_eq!(parse_size("G"), None);
        assert_eq!(parse_size("2 gigabytes"), None);
        assert_eq!(parse_size("-1"), None);
        assert_eq!(parse_size("1.5G"), None);
        // An unknown suffix, and a value that overflows u64: both are
        // reported rather than truncated into something plausible.
        assert_eq!(parse_size("16P"), None);
        assert_eq!(parse_size("16777216T"), None);
        assert_eq!(parse_size("99999999999999999999"), None);
    }

    /// The whole module has to survive a machine with no cgroup2 without ever
    /// returning an error a caller might treat as a failed start.
    #[test]
    fn a_missing_cgroup2_degrades_to_no_resource_control() {
        let dir = std::env::temp_dir().join(format!("raven-cgroup-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");

        // No cgroup.controllers in there, so this is not a cgroup2 tree, and
        // nothing below may create one anyway: an init that mkdir'd a
        // raven.slice into whatever happened to be at /sys/fs/cgroup would be
        // writing to a directory on the root filesystem.
        assert!(!ensure_slice_at(&dir), "a plain directory is not cgroup2");
        assert!(Cgroup::for_service_in(&dir, "anything").is_none());
        assert!(!dir.join(SLICE_NAME).exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The service name goes straight into a path, so the one thing that must
    /// never happen is a definition escaping the slice.
    #[test]
    fn a_name_that_is_not_a_directory_name_gets_no_cgroup() {
        let root = PathBuf::from("/nonexistent-cgroup-root");
        assert!(Cgroup::for_service_in(&root, "").is_none());
        assert!(Cgroup::for_service_in(&root, "../elsewhere").is_none());
        assert!(Cgroup::for_service_in(&root, "a/b").is_none());
        assert!(Cgroup::for_service_in(&root, ".hidden").is_none());
    }

    /// The directory and the limits that live in it, on a tree that looks
    /// enough like cgroup2 to be treated as one. What the kernel then does
    /// with `memory.max` is the kernel's business; that init writes the right
    /// number to the right file is this module's.
    #[test]
    fn a_service_cgroup_carries_the_limits_from_its_definition() {
        let root = std::env::temp_dir().join(format!("raven-cgroup-apply-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir");
        std::fs::write(root.join("cgroup.controllers"), "cpu io memory pids\n").expect("probe");

        let cg = Cgroup::for_service_in(&root, "limited").expect("a cgroup");
        cg.apply(&ServiceConfig {
            name: "limited".to_string(),
            memory_max: Some("256M".to_string()),
            cpu_weight: Some(200),
            // Above the kernel's ceiling: clamped, warned about, and still
            // written, because a weight nobody can act on is worse than the
            // largest one that works.
            io_weight: Some(99_999),
            ..ServiceConfig::default()
        });

        let read = |file: &str| {
            std::fs::read_to_string(cg.path().join(file))
                .unwrap_or_default()
                .trim()
                .to_string()
        };
        assert_eq!(read("memory.max"), (256 * 1024 * 1024).to_string());
        assert_eq!(read("cpu.weight"), "200");
        assert_eq!(read("io.weight"), WEIGHT_MAX.to_string());

        // "max" is a word the kernel understands and a number it does not.
        cg.apply(&ServiceConfig {
            name: "limited".to_string(),
            memory_max: Some("max".to_string()),
            ..ServiceConfig::default()
        });
        assert_eq!(read("memory.max"), "max");

        std::fs::remove_dir_all(&root).ok();
    }

    /// Looking at a service's cgroup must never be what creates it: a
    /// `raven-rc status` on a service that has never run would otherwise
    /// leave a directory behind on its way to printing that it is not
    /// running, and the reaper would recreate the very directory it is
    /// trying to remove.
    #[test]
    fn a_cgroup_that_is_not_there_is_not_created_by_looking_for_it() {
        let root = std::env::temp_dir().join(format!("raven-cgroup-look-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(SLICE_NAME)).expect("temp dir");

        assert!(Cgroup::existing_in(&root, "never-ran").is_none());
        assert!(!root.join(SLICE_NAME).join("never-ran").exists());
        // The same three rules the creating path applies, applied here too.
        assert!(Cgroup::existing_in(&root, "../elsewhere").is_none());
        assert!(Cgroup::existing_in(&root, "").is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    /// The counters `raven-rc status` prints, read back from files shaped the
    /// way the kernel writes them. `cpu.stat` in particular carries lines this
    /// supervisor has no use for -- `nr_periods`, `throttled_usec` and more on
    /// a kernel with the bandwidth controller enabled -- and a parser that
    /// assumed three lines in a fixed order would report a throttling count as
    /// CPU time.
    #[test]
    fn the_counters_are_read_out_of_the_files_the_kernel_writes() {
        let root = std::env::temp_dir().join(format!("raven-cgroup-read-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join(SLICE_NAME).join("measured");
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join("memory.current"), "149118976\n").expect("memory.current");
        std::fs::write(dir.join("pids.current"), "4\n").expect("pids.current");
        std::fs::write(
            dir.join("cpu.stat"),
            "usage_usec 3104711\nuser_usec 2900000\nsystem_usec 204711\nnr_periods 0\n             nr_throttled 0\nthrottled_usec 0\n",
        )
        .expect("cpu.stat");

        let cg = Cgroup::existing_in(&root, "measured").expect("the directory is there");
        assert_eq!(cg.memory_current(), Some(149_118_976));
        assert_eq!(cg.pids_current(), Some(4));
        assert_eq!(
            cg.cpu_stat(),
            Some(CpuStat {
                usage_usec: 3_104_711,
                user_usec: 2_900_000,
                system_usec: 204_711,
            })
        );

        // A service that has no cgroup has no numbers, rather than zeroes: a
        // status line reading "0 bytes" for a daemon that is plainly resident
        // is a worse answer than no line at all.
        let bare = root.join(SLICE_NAME).join("bare");
        std::fs::create_dir_all(&bare).expect("temp dir");
        let cg = Cgroup::existing_in(&root, "bare").expect("the directory is there");
        assert_eq!(cg.memory_current(), None);
        assert_eq!(cg.cpu_stat(), None);

        std::fs::remove_dir_all(&root).ok();
    }

    /// The reason the slice exists: a signal aimed at the cgroup reaches a
    /// process whatever it has done to its process group. This test proves
    /// only the mechanism -- that every pid listed in `cgroup.procs` is
    /// signalled -- because the kernel's own membership rules are not this
    /// module's to verify.
    #[test]
    fn every_process_listed_in_a_cgroup_is_signalled() {
        let root = std::env::temp_dir().join(format!("raven-cgroup-signal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join(SLICE_NAME).join("noisy");
        std::fs::create_dir_all(&dir).expect("temp dir");

        let mut child = std::process::Command::new("/bin/sleep")
            .arg("300")
            .spawn()
            .expect("a process to signal");
        std::fs::write(dir.join("cgroup.procs"), format!("{}\n", child.id())).expect("procs");

        let cg = Cgroup::existing_in(&root, "noisy").expect("the directory is there");
        assert!(
            cg.signal_all(Signal::SIGTERM),
            "a cgroup holding a live pid reports that it did the signalling"
        );
        let status = child.wait().expect("the child is reaped");
        assert!(!status.success(), "SIGTERM reached it: {status:?}");

        // Now that the pid is reaped it names nothing, and the cgroup says so
        // rather than claiming to have stopped a service it did not touch --
        // which is what makes the caller fall back to the process group.
        assert!(
            !cg.signal_all(Signal::SIGTERM),
            "a cgroup with nothing live in it must not claim the stop"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// `cgroup.kill` is preferred for SIGKILL because it cannot be outrun by a
    /// process forking while it is being killed. The pid list here holds only
    /// the two numbers that must never be signalled, so a run that went
    /// through the loop instead would either do nothing (and fail the first
    /// assertion) or signal init and this test's own process group.
    #[test]
    fn the_kernels_atomic_kill_is_preferred_where_it_exists() {
        let root = std::env::temp_dir().join(format!("raven-cgroup-kill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join(SLICE_NAME).join("stubborn");
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join("cgroup.kill"), "0\n").expect("kill file");
        std::fs::write(dir.join("cgroup.procs"), "0\n1\n").expect("procs");

        let cg = Cgroup::existing_in(&root, "stubborn").expect("the directory is there");
        assert!(cg.signal_all(Signal::SIGKILL), "the kill file answers for it");
        assert_eq!(
            std::fs::read_to_string(dir.join("cgroup.kill"))
                .unwrap_or_default()
                .trim(),
            "1"
        );

        // Every other signal takes the pid list, which here holds only pid 0
        // (this process's own group) and pid 1 (init). Both are skipped, so
        // the cgroup reports that it stopped nothing.
        assert!(
            !cg.signal_all(Signal::SIGTERM),
            "pid 0 and pid 1 are never signalled out of a cgroup"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// Cleanup has to be exactly one attempt: the directory goes when it is
    /// empty and stays when it is not, and either way the supervisor moves on
    /// rather than waiting for a process it has already decided to outlive.
    #[test]
    fn a_cgroup_is_removed_when_empty_and_kept_while_it_is_not() {
        let root = std::env::temp_dir().join(format!("raven-cgroup-rmdir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let empty = root.join(SLICE_NAME).join("finished");
        let busy = root.join(SLICE_NAME).join("lingering");
        std::fs::create_dir_all(&empty).expect("temp dir");
        std::fs::create_dir_all(&busy).expect("temp dir");
        // Against cgroupfs this is a cgroup that still holds a process, and
        // `rmdir` answers EBUSY. Against the ordinary filesystem a test runs
        // on the nearest equivalent is a directory with a file in it, which
        // answers ENOTEMPTY; both are handled and neither is an error.
        std::fs::write(busy.join("cgroup.procs"), "4242\n").expect("procs");

        Cgroup::existing_in(&root, "finished")
            .expect("the directory is there")
            .remove();
        assert!(!empty.exists(), "an empty cgroup is reaped");

        Cgroup::existing_in(&root, "lingering")
            .expect("the directory is there")
            .remove();
        assert!(busy.exists(), "a cgroup that is still in use is left alone");
        // And a second attempt is just as harmless as the first, because the
        // supervisor makes one every time a service of that name exits.
        Cgroup::existing_in(&root, "lingering")
            .expect("the directory is there")
            .remove();
        assert!(busy.exists());

        std::fs::remove_dir_all(&root).ok();
    }
}
