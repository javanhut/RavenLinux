//! What went wrong, kept where it survives a reboot.
//!
//! Before this module the evidence that a service had crashed was one INFO
//! line in init.log -- `Service sshd killed by signal SIGSEGV`, worded the same
//! as a service stopped on purpose -- and a restart counter that lived in PID
//! 1's memory and was gone at the next boot. A service that failed to start at
//! boot left an ERROR line and nothing `raven-rc status` could show, and the
//! output the service printed on its way down was in `<name>.log` somewhere
//! above whatever it had printed since, with nothing pointing at it.
//!
//! So every failure now gets a record in `failures.log`, beside the service
//! logs (`/var/log/raven`, or the session's own log directory in user mode):
//!
//! ```text
//! 2026-10-15T11:52:03Z [   12.345] boot 3f2a9c1e sshd: crashed: SIGSEGV (segmentation fault), core dumped, after 0.4s
//!     | sshd: fatal: cannot bind port 22
//!     | ...
//! ```
//!
//! One header line per failure, greppable on its own: the wall clock, the
//! boot clock `init.log` and `blame` use, the first eight characters of the
//! kernel's boot id (so "this boot" is a question the file can answer), the
//! service and what happened. Under it, indented, the last lines the service
//! printed during the run that failed -- only that run's, because the offset
//! the log had reached when it was started is remembered.
//!
//! A crash loop writes one header a minute at the backoff's ceiling, which is
//! what happened and is kept. The output under it is not repeated when it is
//! the same as last time, because forty copies of one backtrace hide the one
//! that differs.
//!
//! What is a failure: a crash (a signal that means the program broke, or the
//! OOM killer), a daemon or one-shot exiting non-zero, a death by any other
//! signal nobody here asked for, a start that never produced a process, and a
//! service the boot could never order. What is not: anything that dies after
//! the supervisor or an operator asked it to stop, a daemon exiting 0, and a
//! service skipped because its binary is not installed (see main.rs for why
//! that one is deliberately quiet).
//!
//! The file is a `.log` in the log directory, so `crate::logrotate` bounds it
//! exactly as it bounds a service's log, with no code here to make that so.

use std::collections::hash_map::DefaultHasher;
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nix::sys::signal::Signal;

/// The file, inside the log directory.
pub const FILE_NAME: &str = "failures.log";

/// How many lines of a service's output go under its record.
///
/// Enough for a panic message and the lines that led to it; a full backtrace
/// is in `raven-rc logs`, which this record exists to send people to.
const OUTPUT_LINES: usize = 10;

/// How far back from the end of the run's output to look for those lines.
const OUTPUT_BYTES: u64 = 4096;

/// A line of output longer than this is cut. A daemon that dumps a binary
/// blob or a minified JSON document must not turn one record into a megabyte.
const OUTPUT_LINE_MAX: usize = 240;

/// How bad an ending was, which decides how loudly it is said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Exited 0, or ended after somebody asked it to. Not a failure.
    Clean,
    /// Exited non-zero, killed by a signal nobody here sent, or never got as
    /// far as a process. WARN on the console.
    Failed,
    /// Died of a signal that means the program itself broke -- SIGSEGV,
    /// SIGABRT and the like -- or was chosen by the OOM killer. ERROR.
    Crashed,
}

/// How a run ended, in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cause {
    pub kind: Kind,
    /// "crashed: SIGSEGV (segmentation fault), core dumped", "exited with
    /// status 1". Lower case, no service name, so it reads after one.
    pub text: String,
}

impl Cause {
    pub fn failed(text: impl Into<String>) -> Self {
        Self {
            kind: Kind::Failed,
            text: text.into(),
        }
    }

    /// Whether this ending belongs in the record at all.
    pub fn is_failure(&self) -> bool {
        self.kind != Kind::Clean
    }

    /// The level the console line about it is logged at.
    pub fn level(&self) -> log::Level {
        match self.kind {
            Kind::Crashed => log::Level::Error,
            Kind::Failed => log::Level::Warn,
            Kind::Clean => log::Level::Info,
        }
    }

    /// What an `exit(status)` means.
    ///
    /// Zero is clean. 126 and 127 are the shell's and `execvp`'s words for a
    /// command that could not be run, which is the most common reason a
    /// service "exits immediately" and worth saying in words. Above 128 is how
    /// a shell reports that the command it ran was killed by signal N, and a
    /// daemon started through a wrapper script crashes as exactly that -- so
    /// the signal is named, as a likelihood rather than a fact, because a
    /// program is free to exit 139 for reasons of its own.
    pub fn from_exit(status: i32) -> Self {
        if status == 0 {
            return Self {
                kind: Kind::Clean,
                text: "exited with status 0".to_string(),
            };
        }
        let hint = match status {
            126 => " (found but not executable)".to_string(),
            127 => " (command not found)".to_string(),
            n if n > 128 => match Signal::try_from(n - 128) {
                Ok(sig) => format!(" (likely {} in a child process)", sig.as_str()),
                Err(_) => String::new(),
            },
            _ => String::new(),
        };
        Self::failed(format!("exited with status {status}{hint}"))
    }

    /// What a death by `signal` means.
    ///
    /// `oom_killed` is whether the service's cgroup counted an OOM kill during
    /// this run, read before the cgroup was removed. SIGKILL on its own says
    /// only that somebody was certain; together with that counter it says who.
    pub fn from_signal(signal: Signal, core_dumped: bool, oom_killed: bool) -> Self {
        let core = if core_dumped { ", core dumped" } else { "" };
        if signal == Signal::SIGKILL && oom_killed {
            return Self {
                kind: Kind::Crashed,
                text: "killed by the OOM killer (SIGKILL)".to_string(),
            };
        }
        match crash_meaning(signal) {
            Some(meaning) => Self {
                kind: Kind::Crashed,
                text: format!("crashed: {} ({meaning}){core}", signal.as_str()),
            },
            None => Self::failed(format!("killed by {}{core}", signal.as_str())),
        }
    }
}

/// The signals that mean a program broke rather than that somebody stopped it,
/// and what each one is called when it happens to you.
fn crash_meaning(signal: Signal) -> Option<&'static str> {
    Some(match signal {
        Signal::SIGSEGV => "segmentation fault",
        Signal::SIGBUS => "bus error",
        Signal::SIGILL => "illegal instruction",
        Signal::SIGFPE => "arithmetic exception",
        Signal::SIGABRT => "aborted",
        Signal::SIGSYS => "bad system call",
        Signal::SIGTRAP => "trace/breakpoint trap",
        _ => return None,
    })
}

/// One failure, waiting to be written down.
#[derive(Debug, Clone)]
pub struct Failure {
    pub cause: Cause,
    /// When it happened, for `raven-rc status`.
    pub at: Instant,
    /// The same moment on the wall clock, for the record.
    pub wall: SystemTime,
    /// How long the run lasted, when there was a run.
    pub ran_for: Option<Duration>,
}

impl Failure {
    pub fn now(cause: Cause, ran_for: Option<Duration>) -> Self {
        Self {
            cause,
            at: Instant::now(),
            wall: SystemTime::now(),
            ran_for,
        }
    }

    /// What `status` and the record's header say: the cause, and for a run
    /// that existed, how long it lasted -- the difference between "crashed at
    /// startup" and "crashed after a week" is most of a diagnosis.
    pub fn summary(&self) -> String {
        match self.ran_for {
            Some(d) => format!("{}, after {}", self.cause.text, duration(d)),
            None => self.cause.text.clone(),
        }
    }
}

/// What to put under a record, decided by the caller who knows what was
/// written last time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    /// These lines.
    Lines(Vec<String>),
    /// The same as this service's previous record; said in one line.
    Unchanged,
    /// Nothing: the run printed nothing, or there was no run.
    None,
}

/// Where the record goes for a given log directory.
pub fn path_in(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// Append one record. A single `write_all` on an `O_APPEND` file, so a
/// rotation copying the file out never sees half of one.
pub fn record(dir: &Path, service: &str, failure: &Failure, output: &Output) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = format_record(
        service,
        failure,
        output,
        crate::timeline::instant_secs(failure.at),
        boot_id(),
    );
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path_in(dir))?
        .write_all(text.as_bytes())
}

/// The record's text, separated from the I/O so its shape can be tested.
pub fn format_record(
    service: &str,
    failure: &Failure,
    output: &Output,
    mono_secs: f64,
    boot: &str,
) -> String {
    let unix = failure
        .wall
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut text = format!(
        "{} [{:9.3}] boot {} {}: {}\n",
        rfc3339(unix),
        mono_secs,
        boot,
        service,
        failure.summary()
    );
    match output {
        Output::Lines(lines) => {
            for line in lines {
                text.push_str("    | ");
                text.push_str(line);
                text.push('\n');
            }
        }
        Output::Unchanged => text.push_str("    (output as in the previous record for this service)\n"),
        Output::None => {}
    }
    text
}

/// A service's log, inside the log directory. Must match `Service::open_log`.
pub fn log_path(dir: &Path, service: &str) -> PathBuf {
    dir.join(format!("{service}.log"))
}

/// How long a service's log is now: where the next run's output will start.
pub fn log_len(dir: &Path, service: &str) -> u64 {
    std::fs::metadata(log_path(dir, service))
        .map(|m| m.len())
        .unwrap_or(0)
}

/// A start that never produced a `Service` -- at boot, or for a service not
/// yet in the running set -- has nobody to queue its failure, so it is
/// written here, with whatever its `pre_exec` hook printed since `offset`.
pub fn record_start_failure(
    dir: &Path,
    service: &str,
    error: &anyhow::Error,
    offset: u64,
) -> io::Result<()> {
    let lines = output_since(&log_path(dir, service), Some(offset));
    let output = if lines.is_empty() {
        Output::None
    } else {
        Output::Lines(lines)
    };
    let failure = Failure::now(Cause::failed(format!("failed to start: {:#}", error)), None);
    record(dir, service, &failure, &output)
}

/// The last lines a service printed since its log was `offset` bytes long.
///
/// `offset` is where the log ended when the run started. A log that is now
/// shorter than that was rotated during the run, and its whole current
/// content is this run's. `None` -- a service adopted across a re-exec, whose
/// start this supervisor never saw -- reads the tail as it is.
pub fn output_since(log: &Path, offset: Option<u64>) -> Vec<String> {
    read_output(log, offset).unwrap_or_default()
}

fn read_output(log: &Path, offset: Option<u64>) -> io::Result<Vec<String>> {
    let mut file = File::open(log)?;
    let end = file.seek(SeekFrom::End(0))?;
    let from = match offset {
        Some(o) if o <= end => o,
        _ => 0,
    };
    let from = from.max(end.saturating_sub(OUTPUT_BYTES));
    file.seek(SeekFrom::Start(from))?;
    let mut bytes = Vec::with_capacity((end - from) as usize);
    file.take(end - from).read_to_end(&mut bytes)?;
    Ok(last_lines(&String::from_utf8_lossy(&bytes), from > 0 && offset != Some(from)))
}

/// The last [`OUTPUT_LINES`] non-blank lines of `text`, cleaned for a record.
///
/// `cut_front` says the text starts mid-file, so its first line is probably
/// the back half of one and is dropped rather than printed as if it were whole.
fn last_lines(text: &str, cut_front: bool) -> Vec<String> {
    let mut lines: Vec<&str> = text.lines().collect();
    if cut_front && !lines.is_empty() {
        lines.remove(0);
    }
    let lines: Vec<&str> = lines.into_iter().filter(|l| !l.trim().is_empty()).collect();
    let skip = lines.len().saturating_sub(OUTPUT_LINES);
    lines[skip..].iter().map(|l| clean(l)).collect()
}

/// One line made safe to put in a text file somebody will `cat` on a console:
/// control characters (a terminal escape in a crashing TUI's output, say)
/// become `?`, and an overlong line is cut.
fn clean(line: &str) -> String {
    let mut out: String = line
        .chars()
        .map(|c| if c.is_control() && c != '\t' { '?' } else { c })
        .take(OUTPUT_LINE_MAX)
        .collect();
    if line.chars().count() > OUTPUT_LINE_MAX {
        out.push_str(" [...]");
    }
    out
}

/// A fingerprint of some output, for "is this the same as last time".
pub fn digest(lines: &[String]) -> u64 {
    let mut h = DefaultHasher::new();
    lines.hash(&mut h);
    h.finish()
}

/// The first eight characters of this boot's id, or `????????`.
fn boot_id() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .ok()
            .map(|s| s.trim().chars().take(8).collect())
            .filter(|s: &String| s.len() == 8)
            .unwrap_or_else(|| "????????".to_string())
    })
}

/// "0.4s", "12s", "3m12s", "2h05m", "4d03h".
fn duration(d: Duration) -> String {
    let secs = d.as_secs();
    match secs {
        0..=9 => format!("{:.1}s", d.as_secs_f64()),
        10..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m{:02}s", secs / 60, secs % 60),
        3600..=86_399 => format!("{}h{:02}m", secs / 3600, (secs / 60) % 60),
        _ => format!("{}d{:02}h", secs / 86_400, (secs / 3600) % 24),
    }
}

/// `2026-09-01T12:34:56Z` from Unix seconds.
///
/// The same Hinnant civil-from-days as `rfc3339` in timed.rs, which is a
/// different binary; a dozen lines are not worth a shared module between
/// PID 1 and a daemon that has nothing else in common with it.
fn rfc3339(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;

    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "raven-failures-{}-{}-{}",
            tag,
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_segfault_is_a_crash_and_says_so() {
        let cause = Cause::from_signal(Signal::SIGSEGV, true, false);
        assert_eq!(cause.kind, Kind::Crashed);
        assert_eq!(cause.text, "crashed: SIGSEGV (segmentation fault), core dumped");
        assert_eq!(cause.level(), log::Level::Error);
    }

    #[test]
    fn sigkill_is_the_oom_killer_only_when_the_cgroup_counted_one() {
        let oom = Cause::from_signal(Signal::SIGKILL, false, true);
        assert_eq!(oom.kind, Kind::Crashed);
        assert!(oom.text.contains("OOM killer"));

        let plain = Cause::from_signal(Signal::SIGKILL, false, false);
        assert_eq!(plain.kind, Kind::Failed);
        assert_eq!(plain.text, "killed by SIGKILL");
    }

    #[test]
    fn a_stray_sigterm_is_a_failure_not_a_crash() {
        let cause = Cause::from_signal(Signal::SIGTERM, false, false);
        assert_eq!(cause.kind, Kind::Failed);
        assert_eq!(cause.level(), log::Level::Warn);
    }

    #[test]
    fn exit_statuses_are_read_the_way_a_shell_writes_them() {
        assert_eq!(Cause::from_exit(0).kind, Kind::Clean);
        assert!(!Cause::from_exit(0).is_failure());
        assert_eq!(Cause::from_exit(1).text, "exited with status 1");
        assert!(Cause::from_exit(127).text.contains("command not found"));
        assert!(Cause::from_exit(139).text.contains("likely SIGSEGV"));
    }

    #[test]
    fn a_record_is_one_header_line_and_indented_output() {
        let failure = Failure {
            cause: Cause::from_signal(Signal::SIGABRT, false, false),
            at: Instant::now(),
            wall: UNIX_EPOCH + Duration::from_secs(1_760_529_123),
            ran_for: Some(Duration::from_millis(400)),
        };
        let text = format_record(
            "sshd",
            &failure,
            &Output::Lines(vec!["fatal: no host keys".to_string()]),
            12.345,
            "3f2a9c1e",
        );
        assert_eq!(
            text,
            "2025-10-15T11:52:03Z [   12.345] boot 3f2a9c1e sshd: \
             crashed: SIGABRT (aborted), after 0.4s\n    | fatal: no host keys\n"
        );
    }

    #[test]
    fn output_is_only_what_this_run_printed() {
        let dir = scratch("since");
        let log = dir.join("svc.log");
        std::fs::write(&log, "old run line\n").unwrap();
        let offset = std::fs::metadata(&log).unwrap().len();
        let mut f = OpenOptions::new().append(true).open(&log).unwrap();
        f.write_all(b"starting\n\npanic: oh no\n").unwrap();

        assert_eq!(output_since(&log, Some(offset)), vec!["starting", "panic: oh no"]);
        // Adopted: no start offset, so the tail as it stands.
        assert_eq!(output_since(&log, None).len(), 3);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_log_rotated_during_the_run_is_read_from_the_start() {
        let dir = scratch("rotated");
        let log = dir.join("svc.log");
        std::fs::write(&log, "after rotation\n").unwrap();
        assert_eq!(output_since(&log, Some(1_000_000)), vec!["after rotation"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn output_is_bounded_and_made_safe() {
        let dir = scratch("bounded");
        let log = dir.join("svc.log");
        let mut text = String::new();
        for i in 0..50 {
            text.push_str(&format!("line {i}\n"));
        }
        text.push_str("\x1b[31mred\x1b[0m\n");
        text.push_str(&"x".repeat(1000));
        text.push('\n');
        std::fs::write(&log, text).unwrap();

        let lines = output_since(&log, Some(0));
        assert_eq!(lines.len(), OUTPUT_LINES);
        assert_eq!(lines[OUTPUT_LINES - 2], "?[31mred?[0m");
        assert!(lines[OUTPUT_LINES - 1].ends_with(" [...]"));
        assert!(lines[OUTPUT_LINES - 1].len() < 300);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn records_append() {
        let dir = scratch("append");
        let failure = Failure::now(Cause::failed("exited with status 1"), None);
        record(&dir, "a", &failure, &Output::None).unwrap();
        record(&dir, "b", &failure, &Output::Unchanged).unwrap();
        let text = std::fs::read_to_string(path_in(&dir)).unwrap();
        assert_eq!(text.lines().count(), 3);
        assert!(text.lines().next().unwrap().contains(" a: exited with status 1"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn durations_read_at_a_glance() {
        assert_eq!(duration(Duration::from_millis(400)), "0.4s");
        assert_eq!(duration(Duration::from_secs(192)), "3m12s");
        assert_eq!(duration(Duration::from_secs(7500)), "2h05m");
        assert_eq!(duration(Duration::from_secs(3 * 86_400 + 7200)), "3d02h");
    }
}
