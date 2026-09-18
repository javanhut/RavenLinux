//! `raven-fprintd` -- the one process that touches the fingerprint sensor.
//!
//! # Why a daemon, and why this one
//!
//! Because of who is allowed to answer "yes, that was them".
//!
//! The lock screen runs as the person logged in. If it read the sensor itself
//! and then told `ravend` a finger had matched, that claim would carry no
//! secret, and *any* process running as that person could make it. A lock
//! screen exists to stop somebody standing at the keyboard; a process already
//! inside the session is not that somebody, but it becomes them the moment it
//! can unlock the screen. So the sensor belongs to a privileged process, and
//! the lock screen only ever gets to ask.
//!
//! It is not `ravend` itself for the reason [`crate::fprint`]'s USB code is not
//! in PID 1: driving a USB device means claiming an interface and handling
//! whatever a sensor does when it is confused, and that is a surface the
//! process holding `/etc/shadow` should not grow. This daemon has no account
//! database, cannot start a session, and cannot unlock anything. It answers one
//! question -- which stored finger, if any, is on the sensor right now -- and
//! `ravend` decides what that is worth.
//!
//! # The socket
//!
//! `/run/raven-fprint/sensor.sock`, in a `0700` directory owned by root, and
//! every connection's credentials are checked besides. Unlike
//! `/run/raven-lock/verify.sock`, which any session may connect to because all
//! it does there is check a password against the *asker's own* account, this
//! one has no such protection: it reports matches without being told whose
//! they are. Root only.
//!
//! One line in, lines out, like `raven-powerd`'s desktop socket -- a text
//! protocol, because the traffic is a handful of messages per unlock and being
//! able to drive it from `socat` while bringing a sensor up is worth more than
//! the bytes. See [`serve`] for the verbs.
//!
//! # The connection is the operation
//!
//! Waiting for a finger has no deadline, so there is no cancel verb: the daemon
//! polls the sensor and the client's own socket together, and anything at all
//! on the connection -- a byte, a close, a lock screen that died -- ends the
//! wait. A cancel that had to be delivered is a sensor left running for a
//! client that is gone.
//!
//! # One at a time
//!
//! There is one sensor, so there is one connection being served. Another waits
//! in the listen backlog. A daemon that interleaved two enrolments would be one
//! that built a template out of two people's fingers.

mod fprint;

use std::io::{BufRead, BufReader, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::{Duration, Instant};

use fprint::{Retry, Scan, Sensor};

mod roster;

/// Where the daemon listens. Its directory is created `0700` and owned by
/// root; see the module note.
const SOCKET_DIR: &str = "/run/raven-fprint";
const SOCKET_PATH: &str = "/run/raven-fprint/sensor.sock";

/// How long a sensor that would not open is left alone before the next try.
///
/// Opening one that is not answering takes many seconds -- every command in
/// the bring-up waits out its own timeout -- and each request would otherwise
/// try again. A settings page asking for status then queues behind one slow
/// open after another, and every caller behind it is told the reader is busy.
/// In the meantime the failure is answered at once, with the reason it had.
const RETRY_OPEN_AFTER: Duration = Duration::from_secs(10);

/// How a stored finger is named.
///
/// `<account>:<finger>`. It goes to the sensor with the template, for the sake
/// of anything that can read it back, and to [`crate::roster`], which is where
/// this daemon reads it from -- this chip takes a name and has no command that
/// returns one. `ravend` splits it on the colon to decide whose finger
/// matched, so the account comes first and an account may not contain one.
fn label_for(account: &str, finger: &str) -> String {
    format!("{account}:{finger}")
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if let Err(e) = run() {
        log::error!("{e}");
        std::process::exit(1);
    }
}

fn run() -> std::io::Result<()> {
    let listener = listen()?;
    log::info!("listening on {SOCKET_PATH}");

    // Opened lazily rather than here. A daemon that refused to start without a
    // sensor would be one that cannot answer "there is no sensor" -- which is
    // the answer a settings panel most needs, and the one it would instead get
    // as a missing socket and have to guess at. It also gets hotplug for free:
    // a reader that appears later is found by the next request.
    let mut sensor = Slot::default();

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                log::warn!("accept failed: {e}");
                continue;
            }
        };
        match peer_uid(&stream) {
            Ok(0) => {}
            Ok(uid) => {
                log::warn!("refused a connection from uid {uid}");
                let _ = writeln!(stream, "error this socket is root's");
                continue;
            }
            Err(e) => {
                log::warn!("cannot read peer credentials: {e}");
                continue;
            }
        }
        if let Err(e) = serve(stream, &mut sensor) {
            log::debug!("connection ended: {e}");
        }
    }
    Ok(())
}

/// Bind the socket, having made its directory and removed anything stale.
fn listen() -> std::io::Result<UnixListener> {
    std::fs::create_dir_all(SOCKET_DIR)?;
    std::fs::set_permissions(SOCKET_DIR, std::fs::Permissions::from_mode(0o700))?;
    // A socket left by a daemon that was killed rather than stopped. Removing
    // it is safe because the directory is root's: nothing else could have put
    // a file here to be clobbered.
    if Path::new(SOCKET_PATH).exists() {
        std::fs::remove_file(SOCKET_PATH)?;
    }
    let listener = UnixListener::bind(SOCKET_PATH)?;
    std::fs::set_permissions(SOCKET_PATH, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Who is on the other end.
fn peer_uid(stream: &UnixStream) -> std::io::Result<u32> {
    let mut creds: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: `stream` is a connected unix socket, and `creds`/`len` are a
    // valid ucred and its size, which is what SO_PEERCRED writes.
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::addr_of_mut!(creds).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(creds.uid)
}

/// Serve one connection until it closes.
///
/// Verbs, one per line:
///
/// | Line | Answers |
/// |---|---|
/// | `status` | `ok present <stages> <enrolled> <firmware>`, or `ok absent` |
/// | `list` | `finger <account>:<name>` per stored finger, then `ok` |
/// | `verify` | `retry <advice>` per unusable reading, then `match <record>`, |
/// |   | `nomatch`, or `error <why>` |
/// | `enrol <account> <finger>` | `frame <done> <of>` or `retry <advice> <done> <of>` |
/// |   | per reading, then `ok`, or `error <why>` |
/// | `forget <account> <finger>` | `ok`, or `error <why>`; only the last finger |
/// |   | can be removed, because the sensor has no per-finger delete |
/// | `forget-all` | `ok` |
///
/// Every error is a line beginning `error` and never a closed connection: a
/// caller that cannot tell a broken sensor from a crashed daemon has to treat
/// both as the worst case.
fn serve(stream: UnixStream, sensor: &mut Slot) -> std::io::Result<()> {
    // Three handles on the one connection, because they are held at once: the
    // reader owns its buffer for the whole conversation, replies are written
    // while that reader still has it, and the wait for a finger polls a third
    // alongside the sensor. Dups of the same socket, so a peer that goes away
    // hangs up all three.
    let reader = BufReader::new(stream.try_clone()?);
    let cancel = stream.try_clone()?;
    let mut out = stream;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let verb = parts.next().unwrap_or("");
        let result = match verb {
            "status" => status(&mut out, sensor),
            "list" => open_sensor(sensor).and_then(|s| list(&mut out, s)),
            "verify" => open_sensor(sensor).and_then(|s| verify(&mut out, s, cancel.as_fd())),
            "enrol" | "enroll" => {
                let account = parts.next().unwrap_or("");
                let finger = parts.next().unwrap_or("");
                if account.is_empty() || finger.is_empty() {
                    writeln!(out, "error enrol wants an account and a finger")?;
                    continue;
                }
                open_sensor(sensor)
                    .and_then(|s| enrol(&mut out, s, account, finger, cancel.as_fd()))
            }
            "forget" => {
                let account = parts.next().unwrap_or("");
                let finger = parts.next().unwrap_or("");
                if account.is_empty() || finger.is_empty() {
                    writeln!(out, "error forget wants an account and a finger")?;
                    continue;
                }
                open_sensor(sensor).and_then(|s| forget(&mut out, s, account, finger))
            }
            "forget-all" => open_sensor(sensor)
                .and_then(Sensor::forget_all)
                .and_then(|()| roster::clear())
                .and_then(|()| writeln!(out, "ok")),
            other => {
                writeln!(out, "error unknown verb {other}")?;
                continue;
            }
        };
        if let Err(e) = result {
            writeln!(out, "error {e}")?;
            // A sensor that failed may be unplugged, wedged, or merely
            // confused. Dropping it means the next request opens it again,
            // which recovers every one of those without a restart.
            sensor.sensor = None;
        }
    }
    Ok(())
}

/// The sensor, opened on demand, and how the last attempt to open it went.
#[derive(Default)]
struct Slot {
    sensor: Option<Sensor>,
    /// When opening last failed, and why. Cleared by the next open that works.
    failed: Option<(Instant, String)>,
}

impl Slot {
    /// The open sensor, opening one if there is not already one; `None` for a
    /// machine with no reader.
    ///
    /// Lazy for the reason [`run`] gives: a machine with no reader has to be
    /// able to say so, and a reader plugged in later has to be found without a
    /// restart. A reader that failed to open is not tried again for
    /// [`RETRY_OPEN_AFTER`].
    fn open(&mut self) -> std::io::Result<Option<&mut Sensor>> {
        if self.sensor.is_none() {
            if let Some((at, why)) = &self.failed {
                if at.elapsed() < RETRY_OPEN_AFTER {
                    return Err(std::io::Error::other(why.clone()));
                }
            }
            let started = Instant::now();
            match Sensor::open() {
                Ok(sensor) => {
                    if let Some(open) = &sensor {
                        log::info!(
                            "opened the sensor in {} ms: firmware {:04x}, {} stages, {} stored",
                            started.elapsed().as_millis(),
                            open.firmware(),
                            open.stages(),
                            open.enrolled()
                        );
                    }
                    self.sensor = sensor;
                    self.failed = None;
                }
                Err(e) => {
                    log::warn!(
                        "cannot open the sensor (gave up after {} ms): {e}",
                        started.elapsed().as_millis()
                    );
                    self.failed = Some((Instant::now(), e.to_string()));
                    return Err(e);
                }
            }
        }
        Ok(self.sensor.as_mut())
    }
}

/// The open sensor, or an error if there is none.
fn open_sensor(slot: &mut Slot) -> std::io::Result<&mut Sensor> {
    slot.open()?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no fingerprint reader"))
}

/// `ok present ...` or `ok absent`, or `error <why>` for a reader that is there
/// but would not open -- which is not the same answer as having none.
fn status(stream: &mut UnixStream, slot: &mut Slot) -> std::io::Result<()> {
    match slot.open() {
        Ok(Some(open)) => writeln!(
            stream,
            "ok present {} {} {:04x}",
            open.stages(),
            open.enrolled(),
            open.firmware()
        ),
        Ok(None) => writeln!(stream, "ok absent"),
        Err(e) => writeln!(stream, "error {e}"),
    }
}

/// The roster, with anything the sensor is not holding dropped first.
///
/// Every verb that names a finger goes through this: the file and the chip can
/// disagree -- a sensor cleared by something else, a roster written for fingers
/// that are gone -- and the chip is the one telling the truth about what it
/// holds.
fn fingers(sensor: &mut Sensor) -> std::io::Result<Vec<roster::Entry>> {
    let stored = sensor.count()?;
    let (entries, changed) = roster::reconcile(roster::load()?, stored);
    if changed {
        roster::save(&entries)?;
    }
    Ok(entries)
}

fn list(out: &mut UnixStream, sensor: &mut Sensor) -> std::io::Result<()> {
    for entry in fingers(sensor)? {
        writeln!(out, "finger {}", entry.label)?;
    }
    writeln!(out, "ok")
}

/// Remove one finger, when that can be done at all.
///
/// This sensor has no per-finger delete -- see [`fprint::Sensor::forget_all`]
/// -- so the only removal it can carry out is of the last finger there is, by
/// clearing the store. Anything else is refused rather than half done, because
/// the alternative is a removal that silently takes somebody's other fingers
/// with it.
fn forget(
    out: &mut UnixStream,
    sensor: &mut Sensor,
    account: &str,
    finger: &str,
) -> std::io::Result<()> {
    let label = label_for(account, finger);
    let known = fingers(sensor)?;
    if !known.iter().any(|entry| entry.label == label) {
        // Removing a finger that is not there is the state the caller asked
        // for, so it is an `ok` and not an error: a settings page retrying a
        // removal must not be told it failed the second time.
        log::info!("no stored {label} to forget");
        return writeln!(out, "ok");
    }
    if known.len() > 1 {
        return writeln!(
            out,
            "error this sensor cannot remove one finger; it can only clear all {} of them",
            known.len()
        );
    }
    sensor.forget_all()?;
    roster::clear()?;
    log::info!("forgot {label}, the only finger stored");
    writeln!(out, "ok")
}

/// Wait for a finger and report what it was.
///
/// Unusable readings are reported and not counted: this loops on them, and the
/// caller decides when to stop asking. How many clean non-matches are allowed
/// before the password is the only way in is a policy question, and it is
/// `ravend`'s -- see `raven-fprint`'s `Gate` in RavenGUI, which both sides use.
fn verify(
    out: &mut UnixStream,
    sensor: &mut Sensor,
    cancel: BorrowedFd<'_>,
) -> std::io::Result<()> {
    loop {
        let Some(scan) = sensor.verify(cancel)? else {
            // Cancelled: the connection went away, so there is nobody to tell.
            return Ok(());
        };
        match scan {
            Scan::Match(slot) => {
                // Which finger matched is looked up rather than reported as a
                // bare slot number, so the caller can tell whose it was -- and
                // so that a session cannot be unlocked by a finger enrolled to
                // a different account. The sensor answers with a slot and
                // nothing else, so the name comes from the roster; see
                // [`crate::roster`] for why it is kept there and not on the
                // chip.
                let label = fingers(sensor)?
                    .into_iter()
                    .find(|entry| entry.slot == slot)
                    .map(|entry| entry.label);
                return match label {
                    Some(label) if !label.is_empty() => writeln!(out, "match {label}"),
                    // The sensor matched something it will not name: an index
                    // past the end of its own list, or a record with no name
                    // in it. Reported as no match, because a match nobody can
                    // attribute is one that could unlock the wrong account --
                    // and "some finger, not saying whose" is exactly the
                    // answer an unlock must not accept.
                    _ => {
                        log::warn!("the sensor matched slot {slot} and would not name it");
                        writeln!(out, "nomatch")
                    }
                };
            }
            Scan::NoMatch => return writeln!(out, "nomatch"),
            Scan::Retry(why) => writeln!(out, "retry {}", advice(why))?,
            // The sensor does not report enrolment frames to a verify.
            Scan::Frame => return writeln!(out, "error the sensor answered a verify with a frame"),
        }
    }
}

/// Feed the sensor frames until it has a template, then store it.
///
/// Gives up after a run of unusable readings rather than asking forever: a dry
/// finger or a dirty sensor will go on producing nothing for as long as anybody
/// keeps pressing, and a dialog with no floor under it is one that asks for a
/// fingertip until somebody closes it. A good reading restores the budget, so a
/// finger that keeps working enrols however many poor readings it took.
fn enrol(
    out: &mut UnixStream,
    sensor: &mut Sensor,
    account: &str,
    finger: &str,
    cancel: BorrowedFd<'_>,
) -> std::io::Result<()> {
    /// Unusable readings in a row that end an enrolment.
    const PATIENCE: u8 = 10;

    let stages = sensor.stages();
    let label = label_for(account, finger);
    let known = fingers(sensor)?;

    // Re-enrolling used to replace the stored finger. It cannot here: this
    // sensor has no command that removes one, so enrolling the same finger
    // again would leave two templates under one name, the second unreachable
    // and neither removable except by clearing the lot. Refused with the way
    // out said plainly.
    if known.iter().any(|entry| entry.label == label) {
        return writeln!(
            out,
            "error that finger is already enrolled; this sensor can only remove every finger at once"
        );
    }
    if known.len() >= usize::from(fprint::MAX_ENROLLED) {
        return writeln!(out, "error the sensor is full");
    }

    let mut done: u8 = 0;
    let mut wasted: u8 = 0;
    while done < stages {
        let Some(scan) = sensor.enrol_frame(done, cancel)? else {
            // Cancelled. The half-built template goes, or it costs a slot.
            let _ = sensor.enrol_abandon();
            return Ok(());
        };
        match scan {
            Scan::Frame => {
                wasted = 0;
                done += 1;
                writeln!(out, "frame {done} {stages}")?;
            }
            Scan::Retry(why) => {
                wasted += 1;
                if wasted >= PATIENCE {
                    let _ = sensor.enrol_abandon();
                    return writeln!(out, "error the sensor could not read that finger");
                }
                writeln!(out, "retry {} {done} {stages}", advice(why))?;
            }
            Scan::Match(_) | Scan::NoMatch => {
                // Neither means anything while enrolling -- there is nothing
                // yet to match against -- so the safe reading is that the
                // frame was no good.
                wasted += 1;
                writeln!(out, "retry try again {done} {stages}")?;
            }
        }
    }
    let slot = sensor.enrol_commit(label.as_bytes())?;
    // Written down before the caller is told it worked: a finger the sensor
    // holds and the roster does not is one nothing can attribute, and this is
    // the only moment its slot is known.
    let mut known = known;
    known.push(roster::Entry {
        slot,
        label: label.clone(),
    });
    roster::save(&known)?;
    log::info!("enrolled {account}:{finger} in slot {slot}");
    writeln!(out, "ok")
}

/// One word the caller can put on screen. Imperative, and never blaming the
/// person for a sensor that could not read.
fn advice(why: Retry) -> &'static str {
    match why {
        Retry::Centre => "centre",
        Retry::Area => "cover",
        Retry::Dirty => "wipe",
        Retry::Unknown(_) => "again",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A label has to say whose finger it is, or a template outlives the
    /// account it belonged to with no way to find it.
    #[test]
    fn a_label_names_the_account_and_the_finger() {
        assert_eq!(label_for("javanstorm", "right-index"), "javanstorm:right-index");
    }

    /// The label written to the roster is the one `ravend` splits on a colon
    /// to decide whose finger matched, so the account must come first and the
    /// separator must be there.
    #[test]
    fn a_label_splits_back_into_the_account_and_the_finger() {
        let label = label_for("javanstorm", "right-index");
        let (account, finger) = label.split_once(':').expect("a label has a colon");
        assert_eq!(account, "javanstorm");
        assert_eq!(finger, "right-index");
    }

    /// Every retry has a word for the screen, including one the sensor never
    /// documented.
    #[test]
    fn every_retry_has_advice() {
        for why in [
            Retry::Centre,
            Retry::Area,
            Retry::Dirty,
            Retry::Unknown(0x77),
        ] {
            assert!(!advice(why).is_empty());
        }
    }
}
