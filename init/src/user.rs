//! Resolving an account name to the ids a process needs to run as it.
//!
//! This parses `/etc/passwd` and `/etc/group` itself rather than calling
//! `getpwnam`. That is deliberate, and not only about dependencies: glibc's
//! `getpwnam` goes through NSS, which `dlopen`s `libnss_files.so` (and
//! whatever else `/etc/nsswitch.conf` names) the first time it is called. PID 1
//! resolving a user that way means a dynamic load during boot, before the
//! filesystem holding those modules is necessarily settled, and a
//! `nsswitch.conf` naming a backend that is not there yet -- `systemd`, `sss`,
//! `ldap` -- turns a local lookup into a hang or a failure that no init can
//! usefully report.
//!
//! RavenLinux ships `passwd: files` and has no name service to consult, so
//! reading the file is not a shortcut past NSS; it *is* what NSS would do,
//! with none of the machinery in between.
//!
//! What this is not: a general user database. There is no shadow, no password
//! checking, and no writing. Init needs to answer one question -- "what uid,
//! gid and groups does this name mean" -- and that is all this answers.

use std::fs;

use anyhow::{bail, Result};

/// An account, resolved far enough to become a process's credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: String,
    pub shell: String,
    /// Supplementary groups, from every `/etc/group` line naming this user.
    ///
    /// The primary gid is deliberately included when a group lists the user
    /// explicitly, matching what `initgroups(3)` does: the kernel keeps the
    /// primary gid separately, but a process that is in `video` both by
    /// primary gid and by membership should not lose it if the primary gid
    /// later changes.
    pub groups: Vec<u32>,
}

/// The uid at and above which an account is a person rather than a daemon.
///
/// Matches `UID_MIN` in Arch's `/etc/login.defs`, which is what `useradd`
/// on this system honours.
const FIRST_REGULAR_UID: u32 = 1000;

/// Accounts above this are not people either -- `nobody` sits at 65534.
const LAST_REGULAR_UID: u32 = 60000;

/// Look one account up by name.
pub fn by_name(name: &str) -> Result<Account> {
    let passwd = fs::read_to_string("/etc/passwd")?;
    let Some(account) = parse_passwd(&passwd, name) else {
        bail!("no account named '{name}' in /etc/passwd");
    };
    Ok(with_groups(account))
}

/// The account a graphical session should belong to, when nobody said which.
///
/// The lowest-uid regular account wins. On a machine installed by
/// `raven-install` that is the one account it created, which is the answer
/// anyone would expect; on a machine with several it is the oldest, which is
/// at least stable across boots rather than depending on file order.
///
/// `None` means this system has no regular account at all -- a bare image that
/// has only ever been root. The caller decides what to do about that; there is
/// no sensible fabricated answer here.
pub fn first_regular() -> Option<Account> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;

    let mut best: Option<Account> = None;
    for line in passwd.lines() {
        let Some(account) = parse_line(line) else {
            continue;
        };
        if account.uid < FIRST_REGULAR_UID || account.uid > LAST_REGULAR_UID {
            continue;
        }
        if best.as_ref().is_none_or(|b| account.uid < b.uid) {
            best = Some(account);
        }
    }

    best.map(with_groups)
}

/// Fill in supplementary groups from `/etc/group`.
fn with_groups(mut account: Account) -> Account {
    let Ok(group) = fs::read_to_string("/etc/group") else {
        return account;
    };
    account.groups = parse_groups(&group, &account.name, account.gid);
    account
}

/// One `/etc/passwd` line: `name:passwd:uid:gid:gecos:home:shell`.
///
/// Split into its own function so the field handling is testable without a
/// real `/etc/passwd` -- the parsing, not the file, is where this goes wrong.
fn parse_line(line: &str) -> Option<Account> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let fields: Vec<&str> = line.split(':').collect();
    if fields.len() < 7 {
        return None;
    }

    Some(Account {
        name: fields[0].to_string(),
        uid: fields[2].parse().ok()?,
        gid: fields[3].parse().ok()?,
        home: fields[5].to_string(),
        shell: fields[6].to_string(),
        groups: Vec::new(),
    })
}

fn parse_passwd(passwd: &str, name: &str) -> Option<Account> {
    passwd
        .lines()
        .filter_map(parse_line)
        .find(|a| a.name == name)
}

/// Every gid this user belongs to: the primary, plus each group listing them.
///
/// Deduplicated and sorted so the result does not depend on file order --
/// otherwise two systems with the same accounts hand their sessions
/// differently-ordered group lists, and the difference only ever shows up as a
/// permission that works on one machine and not the other.
fn parse_groups(group: &str, user: &str, primary_gid: u32) -> Vec<u32> {
    let mut gids = vec![primary_gid];

    for line in group.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() < 4 {
            continue;
        }
        let Ok(gid) = fields[2].parse::<u32>() else {
            continue;
        };
        if fields[3].split(',').any(|m| m.trim() == user) {
            gids.push(gid);
        }
    }

    gids.sort_unstable();
    gids.dedup();
    gids
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWD: &str = "\
root:x:0:0:root:/root:/bin/bash
bin:x:1:1:bin:/:/usr/bin/nologin
javan:x:1000:1000:Javan:/home/javan:/usr/bin/ravenshell
second:x:1001:1001::/home/second:/bin/sh
nobody:x:65534:65534:Nobody:/:/usr/bin/nologin
";

    const GROUP: &str = "\
root:x:0:
wheel:x:10:javan
video:x:91:javan,second
audio:x:92:second
input:x:97:javan
javan:x:1000:
render:x:998:javan
";

    #[test]
    fn parses_a_regular_account() {
        let a = parse_passwd(PASSWD, "javan").expect("javan is in the fixture");
        assert_eq!(a.uid, 1000);
        assert_eq!(a.gid, 1000);
        assert_eq!(a.home, "/home/javan");
        assert_eq!(a.shell, "/usr/bin/ravenshell");
    }

    #[test]
    fn unknown_name_is_none() {
        assert!(parse_passwd(PASSWD, "nosuchuser").is_none());
    }

    /// A truncated line must be skipped, not panic on an index.
    #[test]
    fn short_lines_are_ignored() {
        assert!(parse_line("broken:x:1000").is_none());
        assert!(parse_line("").is_none());
        assert!(parse_line("# a comment").is_none());
    }

    #[test]
    fn groups_include_primary_and_memberships() {
        let gids = parse_groups(GROUP, "javan", 1000);
        // wheel, video, input, javan(primary), render -- sorted, deduped.
        assert_eq!(gids, vec![10, 91, 97, 998, 1000]);
    }

    /// The primary gid appears once even when a group also lists the user.
    #[test]
    fn primary_gid_is_not_duplicated() {
        let gids = parse_groups("javan:x:1000:javan\n", "javan", 1000);
        assert_eq!(gids, vec![1000]);
    }

    /// Membership is an exact match: `javan` must not match `javanx`.
    #[test]
    fn membership_is_not_a_substring_match() {
        let gids = parse_groups("other:x:50:javanx\n", "javan", 1000);
        assert_eq!(gids, vec![1000]);
    }

    /// The lowest regular uid wins, and system accounts never do.
    #[test]
    fn first_regular_skips_system_accounts() {
        let mut best: Option<Account> = None;
        for line in PASSWD.lines() {
            let Some(a) = parse_line(line) else { continue };
            if a.uid < FIRST_REGULAR_UID || a.uid > LAST_REGULAR_UID {
                continue;
            }
            if best.as_ref().is_none_or(|b| a.uid < b.uid) {
                best = Some(a);
            }
        }
        assert_eq!(best.map(|a| a.name), Some("javan".to_string()));
    }
}
