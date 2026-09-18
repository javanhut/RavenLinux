//! Which stored finger is whose.
//!
//! # Why this file exists at all
//!
//! [`crate::fprint`] was written for a sensor that keeps a name beside every
//! template and hands it back on request, which is what makes a match
//! attributable: the chip says "the finger on the sensor is the one stored as
//! `javanstorm:right-index`", and nothing on the host has to be trusted to
//! remember that.
//!
//! The Elan `0c00` will not do it. It stores a name with a finger and has no
//! command that returns one: `40 ff 12` is refused outright and `43 21` is not
//! answered at all, in every shape either published driver sends them. What it
//! does answer is how many fingers it holds and, on a match, *which slot*
//! matched -- a number from zero to nine and nothing more.
//!
//! So the names live here instead, beside the daemon, and the chip's slot
//! number is what joins the two. This is strictly worse than the sensor
//! holding them, and the ways it is worse are worth naming:
//!
//! * A finger enrolled by something else -- another system on the same
//!   machine, the factory -- is a slot with no name here, and this daemon will
//!   not report a match for it. That is the safe direction: an unattributable
//!   match is refused rather than guessed at.
//! * If this file is lost, the fingers on the chip stay there and become
//!   unusable, and the sensor has to be cleared and enrolled again.
//! * Anything that can write this file can make a finger match a different
//!   account. It is `0600` root in a `0700` directory, like the socket, and
//!   the daemon is the only thing that writes it.
//!
//! # Slots do not move
//!
//! A finger goes into the slot numbered by however many were stored before it,
//! and nothing renumbers the others afterwards, because nothing on this chip
//! removes one finger: the only delete it honours takes the whole store. That
//! is what makes a written-down slot number safe to keep.

use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;

/// Where the roster lives. The directory is the daemon's, `0700` and root's.
const DIR: &str = "/var/lib/raven-fprint";
const PATH: &str = "/var/lib/raven-fprint/fingers";

/// One stored finger: the chip's slot, and who it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub slot: u8,
    /// `<account>:<finger>`, the label the rest of the system uses.
    pub label: String,
}

/// Read the roster, as written.
///
/// A missing file is an empty roster: a machine that has never enrolled a
/// finger is the ordinary case, not an error. A line that does not parse is
/// dropped with a warning rather than failing the read -- one bad line must
/// not cost somebody every finger they have.
pub fn load() -> io::Result<Vec<Entry>> {
    let text = match std::fs::read_to_string(PATH) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse(line) {
            Some(entry) => out.push(entry),
            None => log::warn!("ignoring a roster line that does not parse: {line:?}"),
        }
    }
    Ok(out)
}

/// `<slot> <label>`.
fn parse(line: &str) -> Option<Entry> {
    let (slot, label) = line.split_once(char::is_whitespace)?;
    let slot = slot.parse().ok()?;
    let label = label.trim();
    if label.is_empty() {
        return None;
    }
    Some(Entry {
        slot,
        label: label.to_string(),
    })
}

/// Write the roster, replacing what was there.
///
/// Through a temporary file and a rename, so that a daemon that dies mid-write
/// leaves the old roster rather than half of the new one. Half a roster is
/// fingers that no longer have names.
pub fn save(entries: &[Entry]) -> io::Result<()> {
    std::fs::create_dir_all(DIR)?;
    std::fs::set_permissions(DIR, std::fs::Permissions::from_mode(0o700))?;

    let temporary = format!("{PATH}.new");
    let mut file = std::fs::File::create(&temporary)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    writeln!(file, "# raven-fprintd: which sensor slot holds whose finger.")?;
    writeln!(file, "# <slot> <account>:<finger>")?;
    for entry in entries {
        writeln!(file, "{} {}", entry.slot, entry.label)?;
    }
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, PATH)?;
    Ok(())
}

/// Forget every name. The caller has just cleared the sensor, or is about to.
pub fn clear() -> io::Result<()> {
    match std::fs::remove_file(PATH) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The roster, with anything the sensor cannot be holding dropped.
///
/// `stored` is what the chip says it has. A slot at or past that number is a
/// name for a finger that is gone -- the sensor was cleared by something else,
/// or by a `forget-all` that did not get to write the file -- and keeping it
/// would put a name on the list that no finger can ever match.
///
/// The other direction is left alone: more fingers on the chip than names here
/// means somebody else enrolled one, and those slots simply have no name. A
/// match on one is reported as no match, which is the honest answer.
pub fn reconcile(entries: Vec<Entry>, stored: u8) -> (Vec<Entry>, bool) {
    let before = entries.len();
    let kept: Vec<Entry> = entries.into_iter().filter(|e| e.slot < stored).collect();
    let dropped = before - kept.len();
    if dropped > 0 {
        log::warn!(
            "dropping {dropped} roster entries: the sensor holds {stored} fingers and cannot have those slots"
        );
    }
    (kept, dropped > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_a_slot_and_a_label() {
        assert_eq!(
            parse("3 javanstorm:right-index"),
            Some(Entry {
                slot: 3,
                label: "javanstorm:right-index".to_string()
            })
        );
    }

    /// A label with no slot, or a slot with no label, names nothing.
    #[test]
    fn a_line_that_is_not_both_parses_to_nothing() {
        assert_eq!(parse("javanstorm:right-index"), None);
        assert_eq!(parse("3 "), None);
        assert_eq!(parse("x javanstorm:right-index"), None);
    }

    /// A name for a slot the sensor cannot be holding is a name for a finger
    /// that is gone. Keeping it would show somebody a finger that can never
    /// match.
    #[test]
    fn names_for_slots_the_sensor_does_not_have_are_dropped() {
        let entries = vec![
            Entry { slot: 0, label: "a:one".into() },
            Entry { slot: 1, label: "a:two".into() },
            Entry { slot: 5, label: "a:gone".into() },
        ];
        let (kept, changed) = reconcile(entries, 2);
        assert_eq!(kept.len(), 2);
        assert!(changed);
        assert!(kept.iter().all(|e| e.slot < 2));
    }

    /// A cleared sensor leaves no names behind.
    #[test]
    fn clearing_the_sensor_drops_every_name() {
        let entries = vec![Entry { slot: 0, label: "a:one".into() }];
        let (kept, changed) = reconcile(entries, 0);
        assert!(kept.is_empty());
        assert!(changed);
    }

    /// A finger somebody else enrolled has no name here, and that is not a
    /// reason to throw away the names that are.
    #[test]
    fn unnamed_slots_on_the_sensor_cost_nothing() {
        let entries = vec![Entry { slot: 0, label: "a:one".into() }];
        let (kept, changed) = reconcile(entries.clone(), 3);
        assert_eq!(kept, entries);
        assert!(!changed);
    }
}
