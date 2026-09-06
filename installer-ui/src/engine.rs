//! Running raven-install and watching it.
//!
//! Two streams come back. The progress protocol -- see PROTOCOL at the top of
//! raven-install -- says what phase the install is in and how far through it
//! is; the installer's ordinary output is the detail behind that, and is shown
//! verbatim rather than reconstructed from the records, so a message that
//! grows a second explanatory line is not a message this program truncates.

use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use async_channel::{unbounded, Receiver, Sender};

use crate::answers::Answers;

#[derive(Debug, Clone)]
pub enum Event {
    Phase { id: String, text: String },
    Pct { id: String, pct: u32 },
    Ok(String),
    Warn(String),
    Fail(String),
    Info(String),
    /// The installer has started writing to the disk. Once this is true, a
    /// failure has left the target unbootable and the window must say so.
    Dirty(bool),
    /// A line of the installer's own output.
    Log(String),
    /// The installer's last word. Comes before Ended.
    Done(i32),
    /// The process is gone. Always the final event, whatever happened.
    Ended(i32),
}

/// The phases raven-install's main() runs, in order, with the share of the
/// whole each one is worth.
///
/// The weights are wall-clock, not importance: copy is two thirds of an
/// install and everything else is bookkeeping, and a progress bar that gave
/// each phase a tenth would sit at 60% for four minutes and then finish in
/// three seconds.
pub const PHASES: &[(&str, &str, u32)] = &[
    ("preflight", "Checking this machine", 2),
    ("locate", "Finding the system to install", 3),
    ("disk", "Selecting the disk", 1),
    ("wizard", "Reading the answers", 1),
    ("confirm", "Confirming the plan", 1),
    ("partition", "Partitioning", 4),
    ("format", "Creating filesystems", 6),
    ("copy", "Copying the system", 65),
    ("configure", "Configuring the new system", 8),
    ("boot", "Installing the bootloader", 7),
    ("finish", "Finishing up", 2),
];

/// Overall fraction, 0.0-1.0, from the phase in progress and how far into it
/// the installer says it is.
pub fn overall_fraction(phase_id: &str, phase_pct: u32) -> f64 {
    let total: u32 = PHASES.iter().map(|(_, _, w)| w).sum();
    let mut before = 0u32;
    for (id, _, w) in PHASES {
        if *id == phase_id {
            let within = f64::from(*w) * f64::from(phase_pct.min(100)) / 100.0;
            return (f64::from(before) + within) / f64::from(total);
        }
        before += w;
    }
    0.0
}

/// Where the answers file and the progress file go: a tmpfs the session owns
/// and root can read, so the passwords in the first one never touch a disk.
fn runtime_dir() -> PathBuf {
    if let Ok(d) = std::env::var("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(d);
        if p.is_dir() {
            return p;
        }
    }
    std::env::temp_dir()
}

/// The two files one install run needs. Removing the answers file is what
/// Drop is for: it holds the passwords in the clear, and an installer window
/// that is closed halfway through must not leave them behind.
pub struct Run {
    pub answers_path: PathBuf,
    pub progress_path: PathBuf,
}

impl Run {
    pub fn create(answers: &Answers) -> Result<Self, String> {
        let dir = runtime_dir();
        let pid = std::process::id();
        let answers_path = dir.join(format!("raven-install-answers.{pid}"));
        let progress_path = dir.join(format!("raven-install-progress.{pid}"));

        // 0600 at creation, not after: a file that is briefly world-readable
        // while it is being written is a file that was world-readable. The
        // installer refuses anything looser anyway.
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&answers_path)
            .map_err(|e| format!("cannot write {}: {e}", answers_path.display()))?;
        std::io::Write::write_all(&mut f, answers.to_file().as_bytes())
            .map_err(|e| format!("cannot write {}: {e}", answers_path.display()))?;
        drop(f);
        // Belt and braces for a umask that was already applied to `mode`.
        let _ = fs::set_permissions(&answers_path, fs::Permissions::from_mode(0o600));

        // Created by us so the reader below has something to open at once,
        // rather than racing the shell that is about to redirect onto it.
        fs::write(&progress_path, b"")
            .map_err(|e| format!("cannot write {}: {e}", progress_path.display()))?;

        Ok(Self {
            answers_path,
            progress_path,
        })
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.answers_path);
        let _ = fs::remove_file(&self.progress_path);
    }
}

fn parse_record(line: &str) -> Option<Event> {
    let (verb, rest) = match line.split_once(' ') {
        Some((v, r)) => (v, r),
        None => (line, ""),
    };
    Some(match verb {
        "phase" => {
            let (id, text) = rest.split_once(' ').unwrap_or((rest, ""));
            Event::Phase {
                id: id.to_string(),
                text: text.to_string(),
            }
        }
        "pct" => {
            let (id, n) = rest.split_once(' ')?;
            Event::Pct {
                id: id.to_string(),
                pct: n.trim().parse().ok()?,
            }
        }
        "ok" => Event::Ok(rest.to_string()),
        "warn" => Event::Warn(rest.to_string()),
        "fail" => Event::Fail(rest.to_string()),
        "info" => Event::Info(rest.to_string()),
        "dirty" => Event::Dirty(rest.trim() == "1"),
        "done" => Event::Done(rest.trim().parse().unwrap_or(1)),
        _ => return None,
    })
}

/// The argv that runs the installer with fd 3 pointing at the progress file.
///
/// The redirection is done by a shell inside the privilege boundary rather
/// than by dup2 out here, because sudo closes every descriptor above 2 before
/// it execs: an fd 3 opened by this process would not survive the elevation.
fn install_argv(prefix: &[String], installer: &str, run: &Run) -> Vec<String> {
    let mut v = prefix.to_vec();
    v.push("/bin/sh".into());
    v.push("-c".into());
    v.push(
        // stderr joins stdout so the log is in the order it was written, \
        // rather than interleaved by two pipes draining at different rates.
        r#"p="$1"; shift; exec 3>"$p"; exec 2>&1; exec "$@""#.into(),
    );
    v.push("sh".into()); // $0
    v.push(run.progress_path.to_string_lossy().into_owned());
    v.push(installer.into());
    v.push("--answers".into());
    v.push(run.answers_path.to_string_lossy().into_owned());
    v.push("--non-interactive".into());
    v.push("--progress-fd".into());
    v.push("3".into());
    v
}

/// Follow the progress file until `stop` is set, then drain whatever is left.
///
/// A regular file rather than a pipe or a fifo: it never blocks either side,
/// it cannot fill up and stall the installer if this program is slow to read,
/// and a partial one is still a readable record of how far an install got.
fn follow_progress(path: &Path, tx: Sender<Event>, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    use std::sync::atomic::Ordering;
    let Ok(mut f) = fs::File::open(path) else {
        return;
    };
    let mut offset: u64 = 0;
    let mut pending = String::new();

    loop {
        let finished = stop.load(Ordering::SeqCst);
        let mut buf = Vec::new();
        if f.seek(SeekFrom::Start(offset)).is_ok() && f.read_to_end(&mut buf).is_ok() && !buf.is_empty()
        {
            offset += buf.len() as u64;
            pending.push_str(&String::from_utf8_lossy(&buf));
            // Everything up to the last newline. A record that is half-written
            // waits for the rest rather than being parsed into nonsense.
            while let Some(nl) = pending.find('\n') {
                let line: String = pending.drain(..=nl).collect();
                if let Some(ev) = parse_record(line.trim_end()) {
                    if tx.send_blocking(ev).is_err() {
                        return;
                    }
                }
            }
        }
        if finished {
            // `finished` was read *before* the drain above, so this last pass
            // saw everything the installer wrote before it exited.
            return;
        }
        thread::sleep(Duration::from_millis(120));
    }
}

/// Start the install. Events arrive on the returned channel; `Ended` is last.
pub fn start(prefix: &[String], installer: &str, run: &Run) -> Result<Receiver<Event>, String> {
    let (tx, rx) = unbounded();
    let argv = install_argv(prefix, installer, run);
    let (head, tail) = argv.split_first().ok_or("empty install command")?;

    let mut child = Command::new(head)
        .args(tail)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run {head}: {e}"))?;

    let stdout = child.stdout.take().ok_or("no stdout from the installer")?;

    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    {
        let tx = tx.clone();
        let path = run.progress_path.clone();
        let stop = stop.clone();
        thread::spawn(move || follow_progress(&path, tx, stop));
    }

    {
        let tx = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send_blocking(Event::Log(strip_ansi(&line))).is_err() {
                    return;
                }
            }
        });
    }

    thread::spawn(move || {
        let code = child
            .wait()
            .ok()
            .and_then(|s| s.code())
            .unwrap_or(1);
        // Only now: the follower's last pass has to happen after the installer
        // has written its final record, not while it still might.
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        thread::sleep(Duration::from_millis(250));
        let _ = tx.send_blocking(Event::Ended(code));
    });

    Ok(rx)
}

/// The installer colours its output when it thinks it is on a terminal. It is
/// not, here, so this normally does nothing -- but anything it shells out to
/// may disagree, and escape sequences in a GtkTextView are shown rather than
/// interpreted.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if c == '\r' {
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(line: &str) -> Event {
        parse_record(line).expect("record should parse")
    }

    #[test]
    fn records() {
        assert!(matches!(ev("phase copy Copying the system to /dev/sda3"),
            Event::Phase { id, text } if id == "copy" && text == "Copying the system to /dev/sda3"));
        assert!(matches!(ev("pct copy 42"), Event::Pct { id, pct } if id == "copy" && pct == 42));
        assert!(matches!(ev("dirty 1"), Event::Dirty(true)));
        assert!(matches!(ev("done 0"), Event::Done(0)));
        assert!(matches!(ev("warn Secure Boot is ENABLED"), Event::Warn(t) if t == "Secure Boot is ENABLED"));
    }

    #[test]
    fn an_unknown_verb_is_ignored_not_guessed_at() {
        assert!(parse_record("wibble something").is_none());
    }

    #[test]
    fn a_message_with_no_text_still_parses() {
        assert!(matches!(ev("ok"), Event::Ok(t) if t.is_empty()));
    }

    #[test]
    fn progress_is_weighted_towards_the_copy() {
        assert_eq!(overall_fraction("preflight", 0), 0.0);
        let start_of_copy = overall_fraction("copy", 0);
        let end_of_copy = overall_fraction("copy", 100);
        // The copy is two thirds of the bar, which is what makes the bar honest.
        assert!(end_of_copy - start_of_copy > 0.6);
        assert!(overall_fraction("finish", 100) > 0.99);
        // An id from a newer installer moves nothing rather than jumping back.
        assert_eq!(overall_fraction("unknown-phase", 50), 0.0);
    }

    #[test]
    fn phases_are_monotonic() {
        let mut last = -1.0f64;
        for (id, _, _) in PHASES {
            let f = overall_fraction(id, 0);
            assert!(f >= last, "{id} went backwards");
            last = f;
        }
    }

    #[test]
    fn ansi_escapes_are_removed() {
        assert_eq!(strip_ansi("\u{1b}[1;32mOK\u{1b}[0m done"), "OK done");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    #[test]
    fn the_installer_is_invoked_with_fd_3_opened_inside_the_shell() {
        let run = Run {
            answers_path: PathBuf::from("/run/a"),
            progress_path: PathBuf::from("/run/p"),
        };
        let argv = install_argv(&["sudo".into(), "-n".into()], "/usr/bin/raven-install", &run);
        assert_eq!(&argv[0..2], &["sudo", "-n"]);
        assert_eq!(argv[2], "/bin/sh");
        assert_eq!(argv[3], "-c");
        // $0 for the script, then the progress path as $1, then the installer
        // and its own arguments as everything the script shifts down to.
        assert_eq!(argv[5], "sh");
        assert_eq!(argv[6], "/run/p");
        assert_eq!(argv[7], "/usr/bin/raven-install");
        assert!(argv.contains(&"--non-interactive".to_string()));
        assert_eq!(argv.last().unwrap(), "3");
    }
}
