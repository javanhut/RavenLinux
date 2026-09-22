//! Keeping /var/log/raven from growing until it fills the disk.
//!
//! Every service's stdout and stderr land in /var/log/raven/<name>.log, and
//! init's own messages land in /var/log/raven/init.log beside them. Until this
//! module existed, nothing in the tree ever shortened any of those files.
//! There was no size cap, no rotation and no truncation anywhere: the only
//! thing that ever touched a log after it was opened was `close_log_file` at
//! shutdown, which releases init.log so the root can be remounted read-only
//! and does not remove a byte of it.
//!
//! # Why this file exists at all
//!
//! Because the arithmetic only has one ending. /var/log/raven was 5.1MB after
//! two days on the machine this was written on, which is a rate of something
//! like 900MB a year, from a laptop that mostly sits idle. Nothing about that
//! is a crash: it is dbus reporting its configuration, cawd saying once a
//! minute that there is no wireless port, powerd noticing lid events. The
//! failure mode is not that the logs get big, it is that one day a service
//! with something genuinely wrong with it -- a crash loop writing a backtrace
//! a second, a daemon spinning on an EIO it will not stop retrying -- turns
//! that trickle into a flood, and the first symptom the person at the keyboard
//! sees is not the broken service. It is a full root filesystem: no new
//! session can be created, `rvn` cannot unpack a package, and init itself
//! cannot write the line that would have said which service was responsible.
//! A log that eats the disk destroys exactly the evidence it exists to keep.
//!
//! So there are two limits here and they do different jobs. A per-file size
//! ([`Policy::max_size`], 10MB) bounds how much of one service's history is
//! kept and makes `raven-rc logs` a readable thing rather than a 400MB slurp.
//! A cap across the whole directory ([`Policy::total_max`], 200MB) is the
//! backstop, and it is the one that matters during the flood: it is what holds
//! when forty services have each rotated five times over.
//!
//! # Copy-and-truncate, and why it is not a choice
//!
//! The obvious way to rotate a log is to rename it and open a new one. That is
//! what logrotate does by default and it is wrong here, for a reason specific
//! to how this supervisor hands a log to a service.
//!
//! `Service::open_log` opens /var/log/raven/<name>.log with O_APPEND and dups
//! the resulting fd onto the child's stdout and stderr. From that moment the
//! child is writing to an *inode*, not to a path. Renaming the file does not
//! reach the child at all: it goes on appending to whatever the inode is now
//! called, so a daemon started at boot would spend the rest of the machine's
//! uptime writing into <name>.log.1, then into <name>.log.2 when that was
//! rotated in turn, while <name>.log stayed empty and the tool meant to read
//! it showed nothing. The only way to correct it would be to hand the child a
//! fresh fd, and there is no way to do that: the child has already execed, and
//! for a service adopted across `raven-rc reexec` this init never held the fd
//! in the first place -- it inherited a pid, not a file.
//!
//! Copying the contents out and then truncating the live file to zero keeps
//! the inode. The child's O_APPEND fd stays valid, its next write lands at
//! offset 0 because that is what O_APPEND means after a truncation, and
//! nothing has to be reopened by anybody. It also means a `raven-rc logs -f`
//! holding the same file open survives a rotation without noticing it, which
//! is worth having and is why the follower only has to handle "the file got
//! shorter" rather than "the file I am reading is no longer the file".
//!
//! ## The honest cost
//!
//! Copy-and-truncate has a window and it cannot be closed from here. Between
//! the moment the copy finishes reading and the moment `set_len(0)` returns, a
//! service can append a line; the truncation then discards it. That line is
//! gone -- it is not in the rotated generation, because the copy had already
//! passed that offset, and it is not in the live file, because the file is now
//! empty. There is no lock to take: the writer is another process, it is
//! inside `write(2)` in libc with no idea any of this is happening, and Linux
//! offers nothing that would make "copy these bytes and remove them" one
//! operation on a regular file.
//!
//! The window is a single `ftruncate(2)` wide, so what is at risk is whatever
//! is written in a few microseconds by a service that has just been noticed
//! writing 10MB. It is a real cost and it is paid knowingly: a handful of lost
//! lines once per 10MB, against a daemon whose log silently stops working for
//! the rest of the boot. Retrying -- copy the tail that appeared, check again,
//! truncate -- narrows the window but never closes it, and against a service
//! busy enough for the window to matter it is a loop PID 1 could sit in, which
//! is a worse failure than the one it is avoiding.
//!
//! init.log is the exception and gets the guarantee for free. Init is the only
//! writer of its own log, so the caller in main.rs holds the mutex that guards
//! it across the whole rotation and no line can be written in the window at
//! all. That is also why nothing in this module logs anything: every function
//! here returns its findings in a [`Report`] for the caller to log *after*
//! releasing that mutex, because a `log::warn!` reached from inside the
//! rotation would try to lock a mutex the same thread is already holding, and
//! PID 1 deadlocked in its logger does not come back.
//!
//! # What this is not
//!
//! It is not a journal. There is no index, no binary format, no per-message
//! metadata and no querying: a Raven log is a text file that a service wrote
//! to, and the value of that is that `cat`, `grep` and `less` work on it from
//! a rescue shell with nothing else running. Rotation is the smallest thing
//! that makes plain files survivable, and deliberately nothing more.
//!
//! It is not a general logrotate. There are no per-service policies, no
//! time-based schedules ("daily", "weekly"), no `postrotate` scripts and no
//! mail. A service that wants a different size limit does not get one; the
//! knobs are machine-wide, in `[system]`, and that is enough for a machine
//! whose logs are all written by the same supervisor.
//!
//! It does not compress in-process. The crate has no compression dependency
//! and gaining one to gzip a file once every few days would be a poor trade,
//! so compression is `/bin/gzip`, run as a child. That follows what init
//! already does for /bin/mount, /sbin/swapon and /sbin/hwclock, and it fails
//! softly: if gzip is missing or returns non-zero the generation is simply
//! kept uncompressed, because by then the rotation has already succeeded and
//! the compression is only a saving.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::SystemConfig;

/// The size at which a log is rotated, when `[system] log_max_size` says
/// nothing.
///
/// 10MB is chosen from both ends. It is large enough that the machine
/// described in this module's header -- 5.1MB across every service in two days
/// -- rotates a busy service perhaps once a week and a quiet one never, so the
/// history in any single file covers a long enough stretch to still contain
/// the boot before last when somebody goes looking. It is small enough that
/// the file is readable: `raven-rc logs` tailing 10MB is instant, and a person
/// who decides to open the whole thing in an editor gets it, which is not true
/// at 100MB on a machine with 8GB of RAM and a compositor already in it.
///
/// It is also the size that bounds the copy in [`rotate`], and therefore the
/// time PID 1 spends not supervising anything while it rotates. 10MB is a
/// copy_file_range on the same filesystem, measured in milliseconds.
pub const DEFAULT_MAX_SIZE: u64 = 10 * 1024 * 1024;

/// How many rotated generations of a log are kept, when `[system] log_keep`
/// says nothing.
///
/// Five is what makes the per-file limit into a history rather than a guillotine.
/// One generation means a service that rotates twice while you are fetching a
/// coffee has lost the part you wanted; five means roughly 50MB of that
/// service's past, compressed down to a few megabytes, which on real service
/// output reaches back weeks.
///
/// `log_keep = 0` is a legitimate setting and means "cap the file, keep no
/// history": [`rotate`] then truncates without copying anything anywhere.
pub const DEFAULT_KEEP: u32 = 5;

/// The most generations `[system] log_keep` is allowed to ask for.
///
/// This is not a judgement about how much history is useful, it is a guard on
/// a loop. `shift_generations` walks from `keep` down to 1 doing filesystem
/// calls at each step, so `log_keep = 4000000000` -- a plausible typo, or a
/// byte count typed into the one log knob that is not a size -- would put PID
/// 1 into four billion `stat`s with no service being supervised and no way for
/// anybody to say stop. Sixty-four generations is already far past any use
/// anyone has for this, so clamping there costs nothing real and turns an
/// unbootable machine into a warning.
pub const MAX_KEEP: u32 = 64;

/// The cap on everything under the log directory, when `[system]
/// log_total_max` says nothing.
///
/// 200MB is the number that has to hold on the bad day, not the ordinary one.
/// Per-file limits alone bound a machine at (number of services) x
/// (max_size + keep x compressed size), which for the fifteen services this
/// image ships is fine and for a machine somebody has added another twenty to
/// is not. The directory cap does not care how many services there are.
///
/// It is sized against the disk it lives on rather than against the logs: 200MB
/// is a fifth of a percent of the smallest root Raven is installed on, small
/// enough that losing it costs nothing and large enough that a person chasing
/// an intermittent fault still has weeks of every service to read.
pub const DEFAULT_TOTAL_MAX: u64 = 200 * 1024 * 1024;

/// How often the main loop looks at the log directory.
///
/// This is a compromise between noticing a flood and not being a reason for
/// the machine to be awake. A minute is far shorter than the time it takes any
/// realistic service to write past the cap by enough to matter -- a daemon
/// spinning out a line per millisecond needs several minutes to add 10MB --
/// and long enough that the cost, one `readdir` and one `stat` per log, is
/// nothing.
///
/// Critically, this interval never *shortens* a sleep. The main loop's IDLE
/// timeout is 2s, so it already wakes thirty times within any one of these
/// intervals; the sweep just happens on whichever of those passes is the first
/// one due. An idle machine therefore wakes exactly as often as it did before
/// this module existed, which was the requirement -- a log rotator that costs
/// a laptop battery life is a worse bargain than a large log.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(60);

/// The compressor. A full path rather than a PATH lookup, matching how init
/// names /bin/mount and /sbin/hwclock: PID 1's PATH is something
/// `setup_environment` writes, and a log rotation is not a good place to find
/// out somebody changed it.
const GZIP: &str = "/bin/gzip";

/// What the sweep is allowed to do, read once from `[system]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Rotate a log once it reaches this many bytes. `u64::MAX` (written as
    /// `log_max_size = "max"`) disables rotation, leaving only the directory
    /// cap.
    pub max_size: u64,
    /// Rotated generations to keep. 0 means truncate and keep none.
    pub keep: u32,
    /// Prune rotated generations, oldest first, once everything under the log
    /// directory adds up to more than this.
    pub total_max: u64,
    /// Whether to gzip a generation once it has been rotated out.
    pub compress: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            max_size: DEFAULT_MAX_SIZE,
            keep: DEFAULT_KEEP,
            total_max: DEFAULT_TOTAL_MAX,
            compress: true,
        }
    }
}

impl Policy {
    /// Read the policy out of `[system]`, complaining about anything
    /// unreadable rather than refusing to rotate.
    ///
    /// A size that cannot be parsed falls back to the built-in default and
    /// says so. It deliberately does not fail the boot, and it deliberately
    /// does not disable rotation either: a typo in `log_max_size` must not be
    /// the reason a machine fills its disk six months later, and the safe
    /// reading of an unreadable limit is the documented one.
    ///
    /// The complaints come back in the [`Report`] rather than being logged
    /// here, for the reason in the module header -- but also because this is
    /// read once when the main loop starts, so the caller gets to say them
    /// once, at the moment they are actionable, instead of once a minute
    /// forever.
    pub fn from_system(system: &SystemConfig) -> (Policy, Report) {
        let mut report = Report::default();
        let mut policy = Policy::default();

        match crate::cgroup::parse_size(&system.log_max_size) {
            Some(bytes) => policy.max_size = bytes,
            None => report.trouble.push(format!(
                "[system] log_max_size = \"{}\" is not a size; using {}",
                system.log_max_size,
                human(DEFAULT_MAX_SIZE)
            )),
        }
        match crate::cgroup::parse_size(&system.log_total_max) {
            Some(bytes) => policy.total_max = bytes,
            None => report.trouble.push(format!(
                "[system] log_total_max = \"{}\" is not a size; using {}",
                system.log_total_max,
                human(DEFAULT_TOTAL_MAX)
            )),
        }
        policy.keep = system.log_keep.min(MAX_KEEP);
        if system.log_keep > MAX_KEEP {
            report.trouble.push(format!(
                "[system] log_keep = {} is more generations than is useful; using {}",
                system.log_keep, MAX_KEEP
            ));
        }
        policy.compress = system.log_compress;

        // A per-file limit above the whole-directory cap is not an error, but
        // it does mean the first limit can never be the one that fires, and an
        // operator who wrote it probably meant the other order.
        if policy.max_size != u64::MAX && policy.max_size > policy.total_max {
            report.trouble.push(format!(
                "[system] log_max_size ({}) is larger than log_total_max ({}); \
                 logs will be pruned before any of them is ever rotated",
                human(policy.max_size),
                human(policy.total_max)
            ));
        }

        (policy, report)
    }
}

/// What a sweep found, in the caller's words rather than the logger's.
///
/// Two lists because they have two destinations under this project's logging
/// rule: `done` is INFO, which reaches init.log alone, and `trouble` is WARN,
/// which reaches the console -- where, after the gettys are up, it is printed
/// over somebody's login prompt. See main.rs's `maintain_logs` for the latch
/// that keeps a permanently broken log directory from saying so every minute.
#[derive(Debug, Default)]
pub struct Report {
    /// Rotations, compressions and prunes that happened.
    pub done: Vec<String>,
    /// Anything that did not work, phrased for an operator.
    pub trouble: Vec<String>,
}

impl Report {
    fn absorb(&mut self, other: Report) {
        self.done.extend(other.done);
        self.trouble.extend(other.trouble);
    }
}

/// When the next sweep is due.
///
/// A plain deadline rather than a counter of loop passes, because the main
/// loop's pass rate is not a constant: it is 100ms while anything is pending
/// and 2s when nothing is, so "every 600 passes" would mean once a minute on
/// an idle machine and once a minute and a half... on a busy one, which is
/// exactly backwards.
pub struct Schedule {
    next: Instant,
}

impl Schedule {
    /// A schedule whose first sweep is due immediately.
    ///
    /// The first pass of the main loop therefore rotates and prunes before
    /// doing anything else, which is what handles the machine that was powered
    /// off mid-flood: its log directory is already over the cap at boot, and
    /// waiting a minute to notice would be a minute of a machine that may have
    /// no disk space for anything the boot needs to write.
    pub fn due_now() -> Schedule {
        Schedule {
            next: Instant::now(),
        }
    }

    /// True at most once per [`CHECK_INTERVAL`], and true the first time.
    ///
    /// The next deadline is measured from `now` rather than from the previous
    /// deadline: after a suspend, which stops this clock for however long the
    /// lid was shut, measuring from the old deadline would make every missed
    /// interval come due at once on resume. One sweep on resume is the useful
    /// answer; four hundred is not.
    pub fn due(&mut self, now: Instant) -> bool {
        if now < self.next {
            return false;
        }
        self.next = now + CHECK_INTERVAL;
        true
    }
}

/// The current logs in `dir` that have reached the policy's size.
///
/// Only files named `<something>.log` are candidates: a rotated generation is
/// already as small as it is going to get, and something else that happens to
/// be in the directory is not init's to truncate.
///
/// An unreadable directory is not reported as trouble here. It is the ordinary
/// state of things before any service has started and on a machine whose
/// /var/log is read-only, and the sweep runs once a minute forever -- a
/// complaint would be a permanent one.
pub fn oversized(dir: &Path, policy: &Policy) -> Vec<(PathBuf, u64)> {
    if policy.max_size == u64::MAX {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.ends_with(".log") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        if meta.len() >= policy.max_size {
            found.push((entry.path(), meta.len()));
        }
    }
    // Largest first, so that if a sweep is interrupted -- by a shutdown, by
    // the machine losing power -- the work that did happen was the work that
    // mattered most.
    found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    found
}

/// Rotate one log: copy the live file out to a staging generation, truncate it
/// in place, and only then age the kept generations along.
///
/// The order is load-bearing and it is deliberately not the obvious one.
/// Ageing the generations first -- which is what this did until it was found
/// to destroy whole histories -- means that every sweep of a log whose
/// rotation then fails has still pushed one generation off the end and gained
/// nothing in return. The reasons a rotation fails are the reasons that
/// persist: a full disk, a read-only /var, an append-only log. The sweep runs
/// once a minute, so `keep` minutes of a persistent failure leaves no history
/// at all, on exactly the machine whose history is being read. The kept
/// generations are the one thing here that cannot be recreated, so nothing
/// touches them until there is a new generation to put at the front and the
/// live log has actually been shortened.
///
/// Each step therefore fails differently, because they are not equally bad:
///
/// * Copying is the step that must not be skipped. If the copy fails the
///   function returns *without truncating* and without moving a generation.
///   Truncating after a failed copy is the one outcome worse than a log that
///   is too big: it is a log that is gone.
/// * Truncating is where the window described in the module header lives. If
///   it fails, the staged copy is discarded and the rotation is abandoned.
///   Keeping the copy would be worse than losing it: the live log still holds
///   every one of those bytes, it is still oversized, and the next sweep would
///   copy the same unchanged bytes again -- `keep` sweeps of which is every
///   generation replaced by a duplicate of one file.
/// * Ageing the generations is renames of files nobody holds open, and it runs
///   only once the rotation has succeeded. A failure costs one old generation
///   and is reported, but the rotation itself stands.
/// * Compressing is a saving, not part of the rotation. By the time it runs
///   the log has already been shortened, so nothing about a missing or failing
///   /bin/gzip is worth more than a note.
///
/// Nothing here logs. See the module header for why not.
pub fn rotate(path: &Path, policy: &Policy) -> Report {
    let mut report = Report::default();

    let size = match std::fs::metadata(path) {
        Ok(meta) => meta.len(),
        Err(e) => {
            report
                .trouble
                .push(format!("cannot stat {}: {}", path.display(), e));
            return report;
        }
    };
    if size == 0 {
        return report;
    }

    // keep = 0 is "cap the file, keep no history". There is nowhere for the
    // contents to go, so the copy is skipped entirely and with it the window:
    // an operator who asked for no history loses exactly what they said they
    // did not want.
    if policy.keep == 0 {
        match truncate(path) {
            Ok(()) => report.done.push(format!(
                "Truncated {} at {} (log_keep = 0)",
                path.display(),
                human(size)
            )),
            Err(e) => report
                .trouble
                .push(format!("cannot truncate {}: {}", path.display(), e)),
        }
        return report;
    }

    // `<log>.0` is the staging slot. The copy lands there and becomes
    // `<log>.1` only once the live log has been emptied, which is what keeps a
    // rotation that fails half-way from costing a generation. It is numbered
    // rather than given a name of its own so that `prune` recognises it: a
    // machine interrupted mid-rotation is then left with a file the directory
    // cap can still account for and reclaim, rather than an orphan nothing in
    // this module owns.
    let staging = generation(path, 0);

    // A non-empty `.0` on entry is the copy from a sweep that emptied the live
    // log and then could not file the copy away. Those bytes exist nowhere
    // else, so filing them is the first thing this sweep does -- and if it
    // still cannot, the sweep stops, because the copy below would overwrite
    // them. A zero-length `.0` is the debris of a copy that failed at its
    // first write and holds nothing, so the copy is free to land on top of it;
    // the `is_file` test is what stops a directory somebody left at that name
    // from being mistaken for a rotation in progress.
    if std::fs::metadata(&staging)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
    {
        report.absorb(file_away(path, policy));
        if staging.exists() {
            report.trouble.push(format!(
                "{} still holds an unfiled rotation of {}; not rotating again until it can be moved",
                staging.display(),
                path.display()
            ));
            return report;
        }
    }

    // Ask whether the log can be shortened *before* spending the copy on it.
    // This is not belt and braces for the check after the copy, it is the
    // thing that stops a persistent failure being paid for every minute: a log
    // carrying the append-only attribute, or one on a filesystem that has gone
    // read-only, refuses this open for the same reason `truncate` refuses its
    // own, and a 10MB copy made once a sweep and thrown away again is a
    // gigabyte an hour of pointless writes. The check after the copy stays,
    // because it covers the case this cannot: a filesystem that goes read-only
    // in between.
    if let Err(e) = std::fs::OpenOptions::new().write(true).open(path) {
        report.trouble.push(format!(
            "could not truncate {}: {}; not rotating it, because a copy that \
             cannot be followed by a truncation would only duplicate the log",
            path.display(),
            e
        ));
        return report;
    }

    if let Err(e) = std::fs::copy(path, &staging) {
        // std::fs::copy creates the destination before it can fail on the
        // write, so what a failure leaves behind is a partial generation.
        // Removing it is what keeps a copy that fails every minute -- which is
        // what a full disk looks like -- from being a rotation every minute.
        let _ = std::fs::remove_file(&staging);
        report.trouble.push(format!(
            "cannot copy {} to {}: {}; leaving it alone rather than losing it",
            path.display(),
            staging.display(),
            e
        ));
        return report;
    }

    if let Err(e) = truncate(path) {
        // The live log still holds every byte, so the staged copy is redundant
        // and keeping it would be actively harmful: see this function's doc
        // comment. Discard it and report a rotation that did not happen, which
        // is what this is -- the log is over its limit and still growing.
        let _ = std::fs::remove_file(&staging);
        report.trouble.push(format!(
            "could not truncate {} after copying it: {}; \
             nothing was rotated and its generations are untouched",
            path.display(),
            e
        ));
        return report;
    }

    report
        .done
        .push(format!("Rotated {} at {}", path.display(), human(size)));

    report.absorb(file_away(path, policy));

    report
}

/// Age the kept generations along and move the staged `<log>.0` into
/// `<log>.1`.
///
/// Every filesystem call in here destroys or displaces a generation, which is
/// why it is a function of its own and why [`rotate`] calls it in exactly two
/// places: after a rotation that has already succeeded -- the copy is on disk
/// and the live log has been emptied -- and at the start of a sweep that finds
/// a staged copy a previous sweep could not file. Those are the same work, and
/// neither of them may happen on behalf of a rotation that did not take place.
fn file_away(path: &Path, policy: &Policy) -> Report {
    let mut report = shift_generations(path, policy.keep);

    let staging = generation(path, 0);
    let first = generation(path, 1);
    // Anything still at .1 after the shift is a generation whose rename
    // failed, which `shift_generations` has already reported. It is a
    // duplicate of what should now be at .2, it is in the way, and the staged
    // copy is newer than it, so the staged copy is the one to keep.
    let _ = std::fs::remove_file(&first);
    let _ = std::fs::remove_file(compressed(&first));

    if let Err(e) = std::fs::rename(&staging, &first) {
        // The rotation itself stands: the bytes are on disk under the staging
        // name and the live log has been emptied. Naming the file is the whole
        // point of the message -- `raven-rc logs` walks the generations from
        // .1 and will not show a .0, so until the next sweep files it away the
        // only way to read those lines is to know where they are.
        report.trouble.push(format!(
            "rotated {} but could not move {} to {}: {}; \
             the rotated contents are in {} until a later sweep can file them",
            path.display(),
            staging.display(),
            first.display(),
            e,
            staging.display()
        ));
        return report;
    }

    if policy.compress {
        report.absorb(compress(&first));
    }

    report
}

/// Bring everything under `dir` back below the directory cap by removing
/// rotated generations, oldest first.
///
/// A *current* log is never removed, however old and however large. Two
/// reasons, and the second is the one that decides it. A current log is the
/// only file in the directory a running process holds an fd on, so removing it
/// would leave that service writing into an unlinked inode -- the disk space
/// would not even be returned until the service exited, which makes it the one
/// deletion here that can fail to achieve the thing it was for. And a current
/// log is by definition the most recent evidence there is about a service;
/// pruning it to make room for an older generation of some other service's log
/// is the wrong trade in every case anyone would care about.
///
/// The consequence, and it is deliberate: on a machine whose current logs
/// alone exceed the cap, this removes every rotated generation it has and then
/// reports that it is still over. Rotation is what bounds the current logs,
/// and if rotation is disabled or failing then the cap has nothing left to
/// work with -- which is a thing to be told, not to be silently fixed by
/// deleting a running service's output.
pub fn prune(dir: &Path, policy: &Policy) -> Report {
    let mut report = Report::default();
    if policy.total_max == u64::MAX {
        return report;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return report;
    };

    let mut total: u64 = 0;
    // (modified time, path, size) for the generations that may be removed.
    let mut candidates: Vec<(std::time::SystemTime, PathBuf, u64)> = Vec::new();

    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        // Everything in the directory counts towards the cap, including files
        // this module did not create. The cap is about the disk, not about
        // tidiness, and a 300MB file somebody left in /var/log/raven fills it
        // exactly as effectively as a log would.
        total = total.saturating_add(meta.len());

        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_rotated(name) {
            continue;
        }
        // A file with no usable mtime sorts as oldest, which makes it the
        // first to go. That is the right way round: the reason to read the
        // mtime at all is to decide what to lose first, and a file the
        // filesystem cannot describe is not the one to protect.
        let when = meta
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        candidates.push((when, entry.path(), meta.len()));
    }

    if total <= policy.total_max {
        return report;
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut freed: u64 = 0;
    let over = total - policy.total_max;
    for (_, path, size) in candidates {
        if freed >= over {
            break;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {
                freed = freed.saturating_add(size);
                report
                    .done
                    .push(format!("Pruned {} ({})", path.display(), human(size)));
            }
            Err(e) => report
                .trouble
                .push(format!("cannot remove {}: {}", path.display(), e)),
        }
    }

    if freed < over {
        report.trouble.push(format!(
            "{} holds {} against a cap of {} and there are no rotated logs left to remove; \
             the current logs alone are over the limit",
            dir.display(),
            human(total.saturating_sub(freed)),
            human(policy.total_max)
        ));
    }

    report
}

/// Move `<log>.N` (and `<log>.N.gz`) up one, dropping whatever falls past
/// `keep`.
///
/// Counted down from the top so no rename ever lands on a generation that has
/// not moved yet. Renaming is correct *here*, unlike for the current log,
/// because a rotated generation is a file nobody holds open: no service was
/// ever handed an fd on it and `raven-rc logs` reads the current file.
fn shift_generations(path: &Path, keep: u32) -> Report {
    let mut report = Report::default();

    // The generation that falls off the end. Both spellings, because a policy
    // that had compression switched off will have left plain files behind.
    let last = generation(path, keep);
    for victim in [last.clone(), compressed(&last)] {
        if !victim.exists() {
            continue;
        }
        if let Err(e) = std::fs::remove_file(&victim) {
            report
                .trouble
                .push(format!("cannot remove {}: {}", victim.display(), e));
        }
    }

    for n in (1..keep).rev() {
        let from = generation(path, n);
        let to = generation(path, n + 1);
        for (from, to) in [
            (from.clone(), to.clone()),
            (compressed(&from), compressed(&to)),
        ] {
            if !from.exists() {
                continue;
            }
            if let Err(e) = std::fs::rename(&from, &to) {
                report.trouble.push(format!(
                    "cannot rename {} to {}: {}",
                    from.display(),
                    to.display(),
                    e
                ));
            }
        }
    }

    report
}

/// gzip a rotated generation, in a child process, never blocking on anything
/// but the compressor itself.
///
/// This is synchronous, and that is a choice rather than an oversight. The
/// alternative -- spawn gzip and let the main loop's `waitpid(-1)` reap it --
/// would leave a compressor running over `<log>.1` while the *next* rotation
/// is free to start and overwrite that very file, and the lock that would fix
/// it is a lock PID 1 would have to hold across main-loop passes. Rotation
/// happens once per [`Policy::max_size`] of output, which on a real machine is
/// days apart, and gzip over 10MB is measured in tenths of a second: paying it
/// inline, rarely, is a far smaller thing than making the rotation state
/// machine concurrent.
///
/// stdin, stdout and stderr all go to /dev/null. Inheriting them would put
/// gzip's diagnostics on the console over whatever is on it, which is the
/// exact problem `Service::open_log` exists to solve.
fn compress(path: &Path) -> Report {
    let mut report = Report::default();
    if !path.exists() {
        return report;
    }
    let target = compressed(path);
    // gzip refuses to overwrite and prompts when its output exists, and a
    // prompt read from /dev/null is a non-zero exit with no explanation. Clear
    // the way first rather than passing -f, which would also make gzip follow
    // a symlink somebody planted in the log directory.
    let _ = std::fs::remove_file(&target);

    let outcome = Command::new(GZIP)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match outcome {
        Ok(status) if status.success() => {
            report.done.push(format!("Compressed {}", target.display()));
        }
        Ok(status) => {
            report.trouble.push(format!(
                "{} {} exited with {}; keeping the rotated log uncompressed",
                GZIP,
                path.display(),
                status
            ));
        }
        Err(e) => {
            report.trouble.push(format!(
                "cannot run {}: {}; keeping rotated logs uncompressed",
                GZIP, e
            ));
        }
    }
    report
}

/// Empty a file without replacing it, so every fd already open on it keeps
/// working.
///
/// `write(true)` and then `set_len(0)`, rather than `truncate(true)` on the
/// open: they reach the same syscall, but spelling it this way makes it
/// impossible to read the line as "open a new file here", which is the thing
/// this whole module is at pains not to do.
fn truncate(path: &Path) -> io::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.set_len(0)
}

/// `<dir>/<name>.log` -> `<dir>/<name>.log.<n>`.
fn generation(path: &Path, n: u32) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{}.{}", name, n))
}

/// The same path with `.gz` on the end.
fn compressed(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{}.gz", name))
}

/// Whether a file name is one of this module's rotated generations.
///
/// `<anything>.log.<digits>`, optionally with `.gz`. Deliberately strict: the
/// answer decides what [`prune`] is willing to delete, and a rule that
/// accepted, say, anything containing ".log." would take a file called
/// `dbus.log.keepme` with it.
fn is_rotated(name: &str) -> bool {
    let stem = name.strip_suffix(".gz").unwrap_or(name);
    let Some((head, digits)) = stem.rsplit_once('.') else {
        return false;
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    head.ends_with(".log")
}

/// A byte count as an operator would write it.
///
/// Binary multiples, and the same shape as `control::format_bytes` -- which is
/// a separate copy on purpose, because that one is part of the `raven-rc
/// status` text and is not something a log line should be able to change by
/// being edited. If one of the two is reformatted, the other is free to stay
/// as it is.
pub fn human(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    if bytes == u64::MAX {
        return "unlimited".to_string();
    }
    let value = bytes as f64;
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if value < KIB * KIB {
        format!("{:.1}K", value / KIB)
    } else if value < KIB * KIB * KIB {
        format!("{:.1}M", value / (KIB * KIB))
    } else {
        format!("{:.2}G", value / (KIB * KIB * KIB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A log directory of our own, so no test needs /var/log and no two tests
    /// can see each other's files. Same shape as sysctl.rs's `Tree` and
    /// main.rs's `fixture`, and for the same reason: an environment variable
    /// would be process-global and `cargo test` runs these in threads.
    fn dir(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "raven-logrotate-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("mkdir");
        root
    }

    fn write(path: &Path, bytes: usize) {
        std::fs::write(path, "x".repeat(bytes)).expect("write");
    }

    /// The property the whole design exists for: after a rotation the file a
    /// service is writing to is the same file it was writing to before, so an
    /// O_APPEND fd opened before the rotation still reaches the live log.
    ///
    /// This is the test that fails if anybody ever "simplifies" the copy into
    /// a rename.
    #[test]
    fn a_rotation_keeps_the_inode_a_service_is_writing_to() {
        use std::io::Write as _;
        use std::os::unix::fs::MetadataExt as _;

        let root = dir("inode");
        let log = root.join("chatty.log");
        write(&log, 4096);

        // Exactly what open_log hands a child: an O_APPEND handle on the path.
        let mut held = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .expect("open");
        let before = std::fs::metadata(&log).expect("stat").ino();

        let policy = Policy {
            max_size: 1024,
            keep: 3,
            total_max: u64::MAX,
            compress: false,
        };
        let report = rotate(&log, &policy);
        assert!(report.trouble.is_empty(), "{:?}", report.trouble);

        let after = std::fs::metadata(&log).expect("stat").ino();
        assert_eq!(before, after, "rotation replaced the inode");
        assert_eq!(std::fs::metadata(&log).expect("stat").len(), 0);

        // And the fd handed out before the rotation still lands in the live
        // file, at offset 0, because it is O_APPEND on a truncated inode.
        held.write_all(b"after\n").expect("write");
        assert_eq!(
            std::fs::read_to_string(&log).expect("read"),
            "after\n",
            "the pre-rotation fd stopped reaching the live log"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The same property, but against a real forked-and-exec'd process
    /// holding the fd -- which is the only arrangement that matters, because
    /// it is the one every service is in.
    ///
    /// The child is handed the log exactly the way `Service::do_start` hands
    /// it over: an O_APPEND handle from the same `OpenOptions` call, turned
    /// into the child's stdout by `Stdio::from`. It writes a line, waits for a
    /// sentinel file rather than for a stretch of time (a rotation racing a
    /// `sleep` is a flaky test, and this one has to be believed), and writes
    /// another line after the rotation has happened underneath it.
    ///
    /// If rotation is ever changed to a rename, "after" ends up in
    /// `service.log.1` and this test says so.
    #[test]
    fn a_running_child_keeps_writing_to_the_live_log_across_a_rotation() {
        let root = dir("child");
        let log = root.join("service.log");
        let sentinel = root.join("rotated");

        let handle = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .expect("open");
        let errors = handle.try_clone().expect("dup");

        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "echo before; while [ ! -f {} ]; do sleep 0.02; done; echo after",
                sentinel.display()
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::from(handle))
            .stderr(Stdio::from(errors))
            .spawn()
            .expect("spawn");

        // Wait for the first line rather than assuming it is there: the shell
        // has to be exec'd before it can write anything.
        let mut waited = Duration::ZERO;
        while std::fs::metadata(&log).map(|m| m.len()).unwrap_or(0) == 0 {
            assert!(waited < Duration::from_secs(5), "the child never wrote");
            std::thread::sleep(Duration::from_millis(10));
            waited += Duration::from_millis(10);
        }

        let policy = Policy {
            max_size: 1,
            keep: 2,
            total_max: u64::MAX,
            compress: false,
        };
        let report = rotate(&log, &policy);
        assert!(report.trouble.is_empty(), "{:?}", report.trouble);

        std::fs::write(&sentinel, "go").expect("write");
        child.wait().expect("wait");

        assert_eq!(
            std::fs::read_to_string(&log).expect("read"),
            "after\n",
            "the running child stopped writing to the live log"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("service.log.1")).expect("read"),
            "before\n"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Compression is a saving applied after the rotation, so the generation
    /// ends up as `.1.gz` and the live log is untouched either way.
    ///
    /// Skipped, rather than failed, where there is no /bin/gzip: a machine
    /// without it is a supported configuration -- that is the whole reason
    /// `compress` reports rather than bails -- and a test that fails on one is
    /// testing the build host.
    #[test]
    fn a_rotated_generation_is_gzipped_when_gzip_is_there() {
        if !Path::new(GZIP).exists() {
            return;
        }
        let root = dir("gzip");
        let log = root.join("verbose.log");
        // Compressible, and large enough that gzip is not tempted to store it.
        std::fs::write(&log, "a line of service output\n".repeat(400)).expect("write");

        let policy = Policy {
            max_size: 1024,
            keep: 3,
            total_max: u64::MAX,
            compress: true,
        };
        let report = rotate(&log, &policy);
        assert!(report.trouble.is_empty(), "{:?}", report.trouble);

        assert!(root.join("verbose.log.1.gz").exists());
        assert!(
            !root.join("verbose.log.1").exists(),
            "gzip removes its input on success"
        );
        assert_eq!(std::fs::metadata(&log).expect("stat").len(), 0);

        // And the compressed generation still shifts along on the next
        // rotation, which is what makes `keep` mean anything once compression
        // is on.
        std::fs::write(&log, "more\n".repeat(400)).expect("write");
        rotate(&log, &policy);
        assert!(root.join("verbose.log.2.gz").exists());
        assert!(root.join("verbose.log.1.gz").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The contents have to survive the rotation, not merely leave the live
    /// file.
    #[test]
    fn what_was_in_the_log_is_in_the_first_generation() {
        let root = dir("contents");
        let log = root.join("dbus.log");
        std::fs::write(&log, "the line that mattered\n").expect("write");

        let policy = Policy {
            max_size: 1,
            keep: 5,
            total_max: u64::MAX,
            compress: false,
        };
        rotate(&log, &policy);

        assert_eq!(
            std::fs::read_to_string(root.join("dbus.log.1")).expect("read"),
            "the line that mattered\n"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Generations shift along and the one past `keep` falls off. Rotating
    /// `keep + 1` times must leave exactly `keep` of them, or the per-file
    /// limit is not a limit at all.
    #[test]
    fn generations_shift_along_and_the_oldest_falls_off_the_end() {
        let root = dir("shift");
        let log = root.join("cawd.log");
        let policy = Policy {
            max_size: 1,
            keep: 3,
            total_max: u64::MAX,
            compress: false,
        };

        for n in 0..5 {
            std::fs::write(&log, format!("run {}\n", n)).expect("write");
            rotate(&log, &policy);
        }

        // The most recent rotation is .1 and the oldest kept is .3; .4 was
        // never allowed to exist.
        assert_eq!(
            std::fs::read_to_string(root.join("cawd.log.1")).expect("read"),
            "run 4\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("cawd.log.3")).expect("read"),
            "run 2\n"
        );
        assert!(!root.join("cawd.log.4").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `log_keep = 0` means no history, and must not leave a `.1` behind.
    #[test]
    fn keeping_no_generations_truncates_without_copying() {
        let root = dir("keep0");
        let log = root.join("noisy.log");
        write(&log, 2048);

        let policy = Policy {
            max_size: 1024,
            keep: 0,
            total_max: u64::MAX,
            compress: false,
        };
        rotate(&log, &policy);

        assert_eq!(std::fs::metadata(&log).expect("stat").len(), 0);
        assert!(!root.join("noisy.log.1").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Only current logs at or over the limit are candidates, and a rotated
    /// generation is never one however large it is.
    #[test]
    fn only_oversized_current_logs_are_picked_up() {
        let root = dir("oversized");
        write(&root.join("big.log"), 4096);
        write(&root.join("small.log"), 10);
        write(&root.join("big.log.1"), 9999);
        write(&root.join("notes.txt"), 9999);

        let policy = Policy {
            max_size: 1024,
            ..Policy::default()
        };
        let found = oversized(&root, &policy);
        assert_eq!(found.len(), 1, "{:?}", found);
        assert_eq!(found[0].0, root.join("big.log"));

        // "max" switches rotation off entirely; the directory cap is then the
        // only limit, which is a supported configuration.
        let off = Policy {
            max_size: u64::MAX,
            ..policy
        };
        assert!(oversized(&root, &off).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The cap removes rotated generations oldest-first and stops as soon as
    /// it is under, rather than emptying the directory.
    #[test]
    fn the_directory_cap_prunes_the_oldest_generations_first() {
        let root = dir("cap");
        // 4KB each; the cap allows three of them.
        for (name, age) in [
            ("a.log.3", 300),
            ("a.log.2", 200),
            ("a.log.1", 100),
            ("a.log", 0),
        ] {
            let path = root.join(name);
            write(&path, 4096);
            // Distinct mtimes, oldest generation furthest in the past.
            let when =
                std::time::SystemTime::now() - Duration::from_secs(age);
            let times = std::fs::FileTimes::new().set_modified(when);
            let f = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .expect("open");
            f.set_times(times).expect("utimes");
        }

        let policy = Policy {
            max_size: u64::MAX,
            keep: 5,
            total_max: 12 * 1024,
            compress: false,
        };
        let report = prune(&root, &policy);
        assert!(report.trouble.is_empty(), "{:?}", report.trouble);

        assert!(!root.join("a.log.3").exists(), "the oldest should have gone");
        assert!(root.join("a.log.2").exists(), "only one needed to go");
        assert!(root.join("a.log.1").exists());
        assert!(
            root.join("a.log").exists(),
            "a current log must never be pruned"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// When only current logs are left, the cap says so instead of deleting
    /// the output of a running service.
    #[test]
    fn a_cap_that_cannot_be_met_complains_rather_than_eating_current_logs() {
        let root = dir("stuck");
        write(&root.join("a.log"), 8192);
        write(&root.join("b.log"), 8192);

        let policy = Policy {
            max_size: u64::MAX,
            keep: 5,
            total_max: 4096,
            compress: false,
        };
        let report = prune(&root, &policy);

        assert!(root.join("a.log").exists());
        assert!(root.join("b.log").exists());
        assert_eq!(report.trouble.len(), 1, "{:?}", report.trouble);
        assert!(report.trouble[0].contains("no rotated logs left"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// What `prune` is allowed to delete. This is the rule that keeps a file
    /// somebody parked in the log directory from being mistaken for a
    /// generation.
    #[test]
    fn a_rotated_generation_is_recognised_and_nothing_else_is() {
        assert!(is_rotated("dbus.log.1"));
        assert!(is_rotated("dbus.log.12"));
        assert!(is_rotated("dbus.log.1.gz"));
        assert!(is_rotated("init.log.5.gz"));

        assert!(!is_rotated("dbus.log"));
        assert!(!is_rotated("dbus.log.gz"));
        assert!(!is_rotated("dbus.log.keepme"));
        assert!(!is_rotated("dbus.log.1.keepme"));
        assert!(!is_rotated("notes.txt"));
        assert!(!is_rotated("1"));
    }

    /// A rotation whose copy cannot be made must leave the log alone. Losing
    /// the log is the one outcome worse than the log being too large, and it
    /// is what a rotation written as "truncate, then copy" would do every time
    /// the filesystem filled up.
    ///
    /// The failure is arranged by taking write permission off the directory,
    /// which is what a read-only /var or a full disk looks like from inside
    /// `std::fs::copy`: the existing log is still writable through its own
    /// inode, so a buggy rotation really would be able to empty it.
    #[test]
    fn a_failed_copy_does_not_truncate_the_log() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = dir("failed-copy");
        let log = root.join("service.log");
        std::fs::write(&log, "irreplaceable\n").expect("write");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555))
            .expect("chmod");

        let policy = Policy {
            max_size: 1,
            keep: 3,
            total_max: u64::MAX,
            compress: false,
        };
        let report = rotate(&log, &policy);

        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755))
            .expect("chmod back");

        assert_eq!(
            std::fs::read_to_string(&log).expect("read"),
            "irreplaceable\n",
            "the log was truncated after a failed copy"
        );
        assert_eq!(report.trouble.len(), 1, "{:?}", report.trouble);
        assert!(
            report.trouble[0].contains("cannot copy"),
            "{:?}",
            report.trouble
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A rotation that cannot make its copy must not cost a generation.
    ///
    /// This is the half of the copy-failure contract the older test does not
    /// reach. That one makes the copy fail by taking write permission off the
    /// *directory*, which also makes every rename in `shift_generations` a
    /// no-op, so the generations survived there by accident rather than by
    /// design. Here the directory stays writable -- which is what a full disk,
    /// the realistic cause, actually looks like -- so a rotation that ages the
    /// generations before it knows the copy worked really does lose one per
    /// sweep, and `keep` sweeps is the whole history gone.
    ///
    /// The failure is arranged by making the live log write-only, because the
    /// copy is the only step that reads it -- and leaving it writable is what
    /// makes this a failed *copy* rather than a log that was refused before
    /// the copy for not being truncatable. Under a uid that ignores file
    /// permissions there is no way to arrange either, so the test says so and
    /// steps aside rather than passing without having tested anything.
    #[test]
    fn a_failing_copy_never_ages_the_generations() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = dir("failing-copy-generations");
        let log = root.join("service.log");
        std::fs::write(&log, "current\n").expect("write");
        std::fs::write(root.join("service.log.1"), "newest history\n").expect("write");
        std::fs::write(root.join("service.log.2"), "oldest history\n").expect("write");

        std::fs::set_permissions(&log, std::fs::Permissions::from_mode(0o222)).expect("chmod");
        if std::fs::File::open(&log).is_ok() {
            // root, or CAP_DAC_OVERRIDE. Nothing can be proved here.
            let _ = std::fs::remove_dir_all(&root);
            return;
        }

        let policy = Policy {
            max_size: 1,
            keep: 3,
            total_max: u64::MAX,
            compress: false,
        };

        // Four sweeps against keep = 3: enough that a rotation which shifts
        // first has pushed both real generations off the end.
        for sweep in 1..=4 {
            let report = rotate(&log, &policy);
            assert!(
                report.trouble.iter().any(|t| t.contains("cannot copy")),
                "sweep {} did not report the failed copy: {:?}",
                sweep,
                report.trouble
            );
            assert!(report.done.is_empty(), "sweep {}: {:?}", sweep, report.done);
        }

        std::fs::set_permissions(&log, std::fs::Permissions::from_mode(0o644)).expect("chmod back");

        assert_eq!(
            std::fs::read_to_string(root.join("service.log.1")).expect("read .1"),
            "newest history\n",
            "a failed copy aged the generations"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("service.log.2")).expect("read .2"),
            "oldest history\n",
            "a failed copy aged the generations"
        );
        assert!(
            !root.join("service.log.3").exists(),
            "a failed copy shifted a generation into .3"
        );
        assert_eq!(
            std::fs::read_to_string(&log).expect("read log"),
            "current\n",
            "the live log was touched by a rotation that never copied it"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A rotation whose truncate fails must leave no copy behind.
    ///
    /// Otherwise the log is still oversized, the next sweep copies the same
    /// unchanged bytes to .1 again, and within `keep` sweeps every generation
    /// is a duplicate of one file and the real history has been unlinked. The
    /// live log is intact throughout, so discarding the copy loses nothing.
    ///
    /// The failure is arranged with a read-only log, which is what the two
    /// real causes -- an append-only attribute, a filesystem that went
    /// read-only after the copy was buffered -- look like to `truncate`: the
    /// copy can still read it and `ftruncate` is refused.
    #[test]
    fn a_failing_truncate_leaves_no_copy_to_duplicate() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = dir("failing-truncate");
        let log = root.join("service.log");
        std::fs::write(&log, "live and unrotatable\n").expect("write");
        std::fs::write(root.join("service.log.1"), "newest history\n").expect("write");
        std::fs::write(root.join("service.log.2"), "oldest history\n").expect("write");

        std::fs::set_permissions(&log, std::fs::Permissions::from_mode(0o444)).expect("chmod");
        if std::fs::OpenOptions::new().write(true).open(&log).is_ok() {
            // root, or CAP_DAC_OVERRIDE. Nothing can be proved here.
            let _ = std::fs::remove_dir_all(&root);
            return;
        }

        let policy = Policy {
            max_size: 1,
            keep: 3,
            total_max: u64::MAX,
            compress: false,
        };

        for sweep in 1..=4 {
            let report = rotate(&log, &policy);
            assert!(
                report.trouble.iter().any(|t| t.contains("could not truncate")),
                "sweep {} did not report the failed truncate: {:?}",
                sweep,
                report.trouble
            );
            assert!(
                report.done.is_empty(),
                "sweep {} claimed a rotation it did not perform: {:?}",
                sweep,
                report.done
            );
            assert!(
                !root.join("service.log.0").exists(),
                "sweep {} left a staged copy behind",
                sweep
            );
        }

        std::fs::set_permissions(&log, std::fs::Permissions::from_mode(0o644)).expect("chmod back");

        assert_eq!(
            std::fs::read_to_string(root.join("service.log.1")).expect("read .1"),
            "newest history\n",
            "the generations were filled with copies of the un-truncated log"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("service.log.2")).expect("read .2"),
            "oldest history\n",
            "the generations were filled with copies of the un-truncated log"
        );
        assert!(
            !root.join("service.log.3").exists(),
            "a rotation that did not happen aged the generations"
        );
        assert_eq!(
            std::fs::read_to_string(&log).expect("read log"),
            "live and unrotatable\n"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A staged copy left by an interrupted sweep is filed away by the next
    /// one rather than being overwritten.
    ///
    /// `.0` exists only between the truncate and the rename, so the machine
    /// has to stop in that window for this to happen -- but when it does, the
    /// staged file holds the only copy of that stretch of the log, and a sweep
    /// that copied straight over it would be the data loss this whole ordering
    /// exists to prevent.
    #[test]
    fn a_staged_copy_from_an_interrupted_sweep_is_recovered() {
        let root = dir("interrupted-sweep");
        let log = root.join("service.log");
        std::fs::write(&log, "written since the interruption\n").expect("write");
        std::fs::write(root.join("service.log.0"), "the interrupted rotation\n").expect("write");
        std::fs::write(root.join("service.log.1"), "older history\n").expect("write");

        let policy = Policy {
            max_size: 1,
            keep: 3,
            total_max: u64::MAX,
            compress: false,
        };
        let report = rotate(&log, &policy);
        assert!(report.trouble.is_empty(), "{:?}", report.trouble);

        // The staged copy is the newest generation, the one that was at .1 is
        // now at .2, and this sweep's own rotation is at .1.
        assert_eq!(
            std::fs::read_to_string(root.join("service.log.1")).expect("read .1"),
            "written since the interruption\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("service.log.2")).expect("read .2"),
            "the interrupted rotation\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("service.log.3")).expect("read .3"),
            "older history\n"
        );
        assert!(!root.join("service.log.0").exists());
        assert_eq!(std::fs::metadata(&log).expect("stat").len(), 0);

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The sweep is due once per interval and not once per main-loop pass.
    #[test]
    fn the_sweep_is_due_once_an_interval_however_often_it_is_asked() {
        let mut schedule = Schedule::due_now();
        let start = Instant::now();

        assert!(schedule.due(start), "the first sweep is due immediately");
        // The main loop asks thirty times a minute at IDLE and six hundred at
        // BUSY; none of those may be a sweep.
        for step in 1..30 {
            assert!(!schedule.due(start + Duration::from_secs(step)));
        }
        assert!(schedule.due(start + CHECK_INTERVAL));

        // And a resume from suspend, which moves the clock forward by hours,
        // is one sweep rather than one per interval that was missed.
        let mut schedule = Schedule::due_now();
        let start = Instant::now();
        assert!(schedule.due(start));
        assert!(schedule.due(start + Duration::from_secs(4 * 3600)));
        assert!(!schedule.due(start + Duration::from_secs(4 * 3600 + 1)));
    }

    /// The knobs come out of `[system]`, and an unreadable one falls back to
    /// the documented default rather than switching rotation off.
    #[test]
    fn an_unreadable_size_falls_back_to_the_default_and_says_so() {
        let mut system = SystemConfig::default();
        let (policy, report) = Policy::from_system(&system);
        assert_eq!(policy.max_size, DEFAULT_MAX_SIZE);
        assert_eq!(policy.total_max, DEFAULT_TOTAL_MAX);
        assert_eq!(policy.keep, DEFAULT_KEEP);
        assert!(report.trouble.is_empty(), "{:?}", report.trouble);

        system.log_max_size = "ten megabytes".to_string();
        let (policy, report) = Policy::from_system(&system);
        assert_eq!(policy.max_size, DEFAULT_MAX_SIZE);
        assert_eq!(report.trouble.len(), 1, "{:?}", report.trouble);
        assert!(report.trouble[0].contains("log_max_size"));

        // A generation count that would put PID 1 into a four-billion-step
        // loop is clamped and named, not obeyed.
        system.log_max_size = "10M".to_string();
        system.log_keep = 4_000_000_000;
        let (policy, report) = Policy::from_system(&system);
        assert_eq!(policy.keep, MAX_KEEP);
        assert_eq!(report.trouble.len(), 1, "{:?}", report.trouble);
        assert!(report.trouble[0].contains("log_keep"));
        system.log_keep = DEFAULT_KEEP;

        // The same size vocabulary as memory_max, including "max".
        system.log_max_size = "64M".to_string();
        let (policy, _) = Policy::from_system(&system);
        assert_eq!(policy.max_size, 64 * 1024 * 1024);
        system.log_max_size = "max".to_string();
        let (policy, _) = Policy::from_system(&system);
        assert_eq!(policy.max_size, u64::MAX);
    }

    /// A per-file limit above the directory cap is a configuration worth
    /// naming: the first limit can never fire.
    #[test]
    fn a_per_file_limit_above_the_directory_cap_is_reported() {
        let system = SystemConfig {
            log_max_size: "500M".to_string(),
            log_total_max: "200M".to_string(),
            ..SystemConfig::default()
        };
        let (_, report) = Policy::from_system(&system);
        assert_eq!(report.trouble.len(), 1, "{:?}", report.trouble);
        assert!(report.trouble[0].contains("log_total_max"));
    }
}
