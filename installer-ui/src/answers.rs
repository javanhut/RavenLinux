//! The answers file, and the validation in front of it.
//!
//! raven-install validates everything here a second time and refuses rather
//! than corrects -- see apply_answers in that script. The checks below are the
//! same checks, done early so a person is told about a bad hostname on the
//! page where they typed it rather than by an installer that quit.
//!
//! The duplication is deliberate and bounded: these are four regular
//! expressions and a membership test, they are documented in both places as
//! being the same rule, and the installer's copy is the one that decides. A
//! front-end that skipped them would still be safe; it would just be rude.

use std::fmt::Write as _;

/// A partition manual mode creates: where, how big, and for what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPart {
    pub start: u64,
    pub sectors: u64,
    /// root, home, swap or esp.
    pub role: String,
}

/// An existing partition manual mode puts to work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsePart {
    pub dev: String,
    pub role: String,
    /// keep or format.
    pub action: String,
}

#[derive(Debug, Clone)]
pub struct Answers {
    pub disk: String,
    /// "wipe" or "alongside". The installer defaults to wipe when this is
    /// absent, so an older raven-install reading a newer answers file would
    /// only ever erase -- which is the direction a version mismatch should
    /// fail in, but it is not a mismatch worth having: `mode` is in the
    /// installer's ANSWER_KEYS, and an unknown key is fatal there, so an old
    /// installer refuses this file outright rather than misreading it.
    pub mode: String,
    /// Which partition an alongside install takes its space from. Empty means
    /// "use the unallocated space that is already on the disk".
    pub shrink_part: String,
    /// How much space to take, as a size the installer parses ("120G").
    pub alongside_size: String,
    /// The root filesystem: "ext4", "xfs" or "btrfs", and only ever one the
    /// probe listed -- raven-install refuses anything else, and refuses btrfs
    /// on an image that cannot create its subvolumes.
    ///
    /// "btrfs" carries more than its name. There is no separate answer for the
    /// subvolume layout, because there is no sensible btrfs install here
    /// without one: the reason to choose it is snapshots, snapshots need a
    /// subvolume to be taken of, and the root has to be mounted from @ for a
    /// rollback to be able to swap it out. So choosing btrfs chooses @, @home,
    /// @snapshots, @log and @cache, and the last two are deliberately outside
    /// the snapshots -- see BTRFS_SUBVOLS in raven-install, which is the one
    /// place that layout is written down.
    pub fs: String,
    pub esp_size: String,
    /// "" means "let the installer pick", "none" means no swap partition.
    pub swap: String,
    /// Full-disk encryption: LUKS2 with argon2id over the root partition, and
    /// over the swap partition as well so a hibernation image is not a
    /// plaintext copy of memory. Off by default, and deliberately so: it is
    /// the only answer on any of these pages that cannot be corrected from a
    /// running system afterwards, because a passphrase that was mistyped is a
    /// disk that never opens again.
    pub encrypt: bool,
    /// The passphrase for `encrypt`. Required when it is on -- raven-install
    /// refuses an empty one rather than installing a container nobody can
    /// open -- and meaningless when it is off.
    pub encrypt_password: String,
    pub hostname: String,
    pub username: String,
    pub fullname: String,
    pub user_password: String,
    pub user_sudo: bool,
    pub root_password: String,
    pub timezone: String,
    pub locale: String,
    pub keymap: String,
    pub profile: String,
    /// When the profile's packages go in: "auto" (now if the live session has
    /// a network, else at first boot), "now", or "later". The installer's
    /// --postinstall; "auto" is what it does when the key is absent.
    pub postinstall: String,
    pub efi_nvram: bool,
    /// Ids of the optional applications to install, from the probe's
    /// optional.* records. Empty installs none of them.
    pub optional: Vec<String>,
    /// Manual mode's plan: partitions to delete, to create, and to reuse.
    /// Only written when `mode` is "manual".
    pub manual_delete: Vec<String>,
    pub manual_new: Vec<NewPart>,
    pub manual_use: Vec<UsePart>,
    /// The hardware clock: "local" (as Windows keeps it) or "utc". Empty lets
    /// the installer decide, which it does by looking for Windows.
    pub rtc: String,
    /// What to do about Secure Boot: "skip", "sign" or "enroll".
    ///
    /// Three values rather than a switch, because the middle one is the whole
    /// safety argument. "sign" puts a signature on RavenBoot, the fallback
    /// loader and the kernel; firmware that is not checking ignores it, so it
    /// changes nothing about the next boot and can be undone by deleting a
    /// directory. "enroll" does that and then hands the firmware a new
    /// Platform Key, which is the only step here that changes anything outside
    /// the disk being installed to -- and the only one that can leave a
    /// machine unable to boot, if it is done and the signing is not.
    ///
    /// "enroll" is refused outright by raven-install unless the firmware is in
    /// setup mode, because that is the only state in which the firmware will
    /// accept a new Platform Key at all. "skip" is the default everywhere.
    pub secureboot: String,
}

impl Default for Answers {
    fn default() -> Self {
        // The same defaults the wizard offers in its prompts.
        Self {
            disk: String::new(),
            mode: "wipe".into(),
            shrink_part: String::new(),
            alongside_size: String::new(),
            fs: "ext4".into(),
            esp_size: "512M".into(),
            swap: String::new(),
            encrypt: false,
            encrypt_password: String::new(),
            hostname: "raven".into(),
            username: "raven".into(),
            fullname: String::new(),
            user_password: String::new(),
            user_sudo: true,
            root_password: String::new(),
            timezone: "UTC".into(),
            locale: "en_US.UTF-8".into(),
            keymap: "us".into(),
            profile: "minimal".into(),
            postinstall: "auto".into(),
            efi_nvram: false,
            optional: Vec::new(),
            manual_delete: Vec::new(),
            manual_new: Vec::new(),
            manual_use: Vec::new(),
            rtc: String::new(),
            secureboot: "skip".into(),
        }
    }
}

/// `^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$` -- valid_hostname in
/// raven-install, without pulling in a regex crate for one pattern.
pub fn valid_hostname(h: &str) -> bool {
    if h.is_empty() || h.len() > 63 {
        return false;
    }
    let ok = |c: char| c.is_ascii_alphanumeric();
    let b = h.as_bytes();
    if !ok(b[0] as char) || !ok(b[b.len() - 1] as char) {
        return false;
    }
    h.chars().all(|c| ok(c) || c == '-')
}

/// `^[a-z_][a-z0-9_-]{0,31}$` -- valid_username in raven-install.
pub fn valid_username(u: &str) -> bool {
    if u.is_empty() || u.len() > 32 {
        return false;
    }
    let first = u.as_bytes()[0] as char;
    if !(first.is_ascii_lowercase() || first == '_') {
        return false;
    }
    u.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

impl Answers {
    /// Why this set of answers cannot be installed, in the order a person
    /// would fix them. Empty means it can.
    pub fn problems(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.disk.is_empty() {
            v.push("No target disk chosen.".into());
        }
        if !valid_hostname(&self.hostname) {
            v.push(
                "Hostnames are letters, digits and hyphens, and cannot start or \
                 end with a hyphen."
                    .into(),
            );
        }
        if self.username == "root" {
            v.push("Pick a name other than root; the root account is configured separately.".into());
        } else if !valid_username(&self.username) {
            v.push(
                "Usernames are lowercase letters, digits, underscore and hyphen, \
                 starting with a letter or underscore."
                    .into(),
            );
        }
        if self.fullname.contains(':') {
            // It becomes a colon-separated field of /etc/passwd.
            v.push("The full name cannot contain a colon.".into());
        }
        if self.encrypt && self.encrypt_password.is_empty() {
            // apply_answers refuses this too. Caught here so it is said on the
            // page with the box on it, rather than by an installer that quit
            // after the summary had been confirmed.
            v.push("An encrypted disk needs a passphrase; there is no empty one.".into());
        }
        // A newline would end the record and turn the rest of the password into
        // a key=value line of its own. Nothing else is off limits.
        for (what, s) in [
            ("password", &self.user_password),
            ("root password", &self.root_password),
            ("encryption passphrase", &self.encrypt_password),
            ("full name", &self.fullname),
            ("hostname", &self.hostname),
        ] {
            if s.contains('\n') || s.contains('\r') {
                v.push(format!("The {what} cannot contain a line break."));
            }
        }
        if self.timezone.is_empty() {
            v.push("No timezone chosen.".into());
        }
        if self.profile.is_empty() {
            v.push("No package profile chosen.".into());
        }
        if self.mode == "alongside" && !self.shrink_part.is_empty() {
            // Only checked for shape. Whether the partition can actually give
            // up this much is a question about a filesystem, and the answer
            // lives in the probe and is checked again by plan_shrink -- which
            // is the copy that decides. What is caught here is an empty or
            // unparseable box, which the front-end can say something useful
            // about at the moment it is typed into.
            if self.alongside_size.is_empty() {
                v.push("Say how much space to take from the partition being shrunk.".into());
            } else if crate::probe::size_to_mb(&self.alongside_size).is_none() {
                v.push(
                    "The space for RavenLinux should be a size like 120G or 40960M."
                        .into(),
                );
            }
        }
        v
    }

    /// True when neither account can be logged into. Not an error -- the
    /// wizard only warns -- but the summary page says so in red.
    pub fn no_password_anywhere(&self) -> bool {
        self.user_password.is_empty() && self.root_password.is_empty()
    }

    /// The file raven-install reads. One key=value per line; the value is the
    /// rest of the line, so nothing here is quoted or escaped.
    pub fn to_file(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# Written by raven-installer-ui. Holds passwords.");
        let mut put = |k: &str, v: &str| {
            let _ = writeln!(s, "{k}={v}");
        };
        put("disk", &self.disk);
        put("mode", &self.mode);
        // Both are only meaningful for an alongside install, and writing them
        // for a wipe would be writing a plan that contradicts the mode. The
        // installer would ignore them; a person reading the file would not.
        if self.mode == "alongside" {
            put("shrink_part", &self.shrink_part);
            put("alongside_size", &self.alongside_size);
        }
        if self.mode == "manual" {
            put("manual_delete", &self.manual_delete.join(","));
            let new: Vec<String> = self
                .manual_new
                .iter()
                .map(|n| format!("{}:{}:{}", n.start, n.sectors, n.role))
                .collect();
            put("manual_new", &new.join(";"));
            let used: Vec<String> = self
                .manual_use
                .iter()
                .map(|u| format!("{}:{}:{}", u.dev, u.role, u.action))
                .collect();
            put("manual_use", &used.join(";"));
        }
        put("fs", &self.fs);
        put("esp_size", &self.esp_size);
        put("swap", &self.swap);
        put("encrypt", if self.encrypt { "1" } else { "0" });
        // Written even when it is empty, and unconditionally rather than only
        // when `encrypt` is on. The parity test wants every key present, and
        // an installer reading encrypt=0 ignores this one -- it warns if it is
        // non-empty, which is the right noise to make about a passphrase that
        // was typed and then not used.
        put("encrypt_password", &self.encrypt_password);
        put("hostname", &self.hostname);
        put("username", &self.username);
        put("fullname", &self.fullname);
        put("user_password", &self.user_password);
        put("user_sudo", if self.user_sudo { "1" } else { "0" });
        put("root_password", &self.root_password);
        put("timezone", &self.timezone);
        put("locale", &self.locale);
        put("keymap", &self.keymap);
        put("profile", &self.profile);
        put("postinstall", &self.postinstall);
        put("efi_nvram", if self.efi_nvram { "1" } else { "0" });
        // Always written: an empty value is the answer "none of them", which
        // is what the switches default to.
        put("optional", &self.optional.join(","));
        put("rtc", &self.rtc);
        // Always written, and "skip" is a real answer rather than an absence:
        // an installer told nothing about Secure Boot does nothing about it,
        // which is the same outcome, but a person reading this file afterwards
        // should be able to see that the question was asked and answered.
        put("secureboot", &self.secureboot);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Answers {
        Answers {
            disk: "/dev/nvme0n1".into(),
            fullname: "Raven User".into(),
            ..Default::default()
        }
    }

    #[test]
    fn hostnames() {
        assert!(valid_hostname("raven"));
        assert!(valid_hostname("raven-01"));
        assert!(!valid_hostname("-raven"));
        assert!(!valid_hostname("raven-"));
        assert!(!valid_hostname("my laptop"));
        assert!(!valid_hostname(""));
    }

    #[test]
    fn usernames() {
        assert!(valid_username("raven"));
        assert!(valid_username("_svc"));
        assert!(!valid_username("Raven"));
        assert!(!valid_username("1raven"));
        assert!(!valid_username(""));
    }

    #[test]
    fn alongside_needs_a_size_when_it_shrinks() {
        let mut a = good();
        a.mode = "alongside".into();
        a.shrink_part = "/dev/nvme0n1p3".into();
        assert!(!a.problems().is_empty(), "no size given");

        a.alongside_size = "not a size".into();
        assert!(!a.problems().is_empty(), "unparseable size");

        a.alongside_size = "120G".into();
        assert!(a.problems().is_empty(), "120G is a size: {:?}", a.problems());
    }

    #[test]
    fn alongside_without_a_shrink_needs_no_size() {
        // Taking space that is already unallocated needs no number from
        // anybody: it is however much is there.
        let mut a = good();
        a.mode = "alongside".into();
        assert!(a.problems().is_empty(), "{:?}", a.problems());
    }

    #[test]
    fn wipe_does_not_write_a_shrink_plan() {
        // The two fields survive in the struct when someone switches back to
        // erasing -- the UI clears them, but the file is the thing the
        // installer reads, and it must not describe a shrink of a partition
        // that is about to be erased.
        let mut a = good();
        a.shrink_part = "/dev/nvme0n1p3".into();
        a.alongside_size = "120G".into();
        let f = a.to_file();
        assert!(f.contains("mode=wipe"));
        assert!(!f.contains("shrink_part="), "{f}");
        assert!(!f.contains("alongside_size="), "{f}");

        a.mode = "alongside".into();
        let f = a.to_file();
        assert!(f.contains("shrink_part=/dev/nvme0n1p3"), "{f}");
        assert!(f.contains("alongside_size=120G"), "{f}");
    }

    #[test]
    fn a_good_set_has_no_problems() {
        assert!(good().problems().is_empty());
    }

    #[test]
    fn root_is_refused_as_the_primary_account() {
        let a = Answers {
            username: "root".into(),
            ..good()
        };
        assert!(a.problems().iter().any(|p| p.contains("other than root")));
    }

    #[test]
    fn a_newline_in_a_password_is_refused() {
        // It would end the record and make the rest of the password a key.
        let a = Answers {
            user_password: "abc\ndisk=/dev/sda".into(),
            ..good()
        };
        assert!(a.problems().iter().any(|p| p.contains("line break")));
    }

    #[test]
    fn a_password_may_contain_anything_else() {
        let a = Answers {
            user_password: "  #=\"'$(reboot) spaces  ".into(),
            ..good()
        };
        assert!(a.problems().is_empty());
        let f = a.to_file();
        assert!(f.contains("user_password=  #=\"'$(reboot) spaces  \n"));
    }

    #[test]
    fn every_key_is_written_exactly_once() {
        let f = good().to_file();
        for k in [
            "disk", "mode", "fs", "esp_size", "swap", "encrypt", "encrypt_password",
            "hostname", "username", "fullname",
            "user_password", "user_sudo", "root_password", "timezone", "locale",
            "keymap", "profile", "postinstall", "efi_nvram",
        ] {
            let n = f.lines().filter(|l| l.starts_with(&format!("{k}="))).count();
            assert_eq!(n, 1, "{k} written {n} times");
        }
    }

    #[test]
    fn encryption_needs_a_passphrase() {
        let mut a = good();
        a.encrypt = true;
        assert!(
            a.problems().iter().any(|p| p.contains("passphrase")),
            "{:?}",
            a.problems()
        );
        a.encrypt_password = "correct horse".into();
        assert!(a.problems().is_empty(), "{:?}", a.problems());
    }

    #[test]
    fn an_unencrypted_install_needs_no_passphrase() {
        // The switch is off by default, and leaving the box empty must not be
        // a reason the Install button stays greyed out.
        let a = good();
        assert!(!a.encrypt);
        assert!(a.problems().is_empty(), "{:?}", a.problems());
        assert!(a.to_file().lines().any(|l| l == "encrypt=0"));
    }

    #[test]
    fn a_newline_in_the_passphrase_is_refused() {
        // Same reason as the account passwords: it would end the record and
        // make the rest of the passphrase a key of its own. Here it would
        // also produce a container whose passphrase is the first line and a
        // person who believes it is the whole thing.
        let a = Answers {
            encrypt: true,
            encrypt_password: "abc\nswap=none".into(),
            ..good()
        };
        assert!(a.problems().iter().any(|p| p.contains("line break")));
    }

    #[test]
    fn optional_is_always_written() {
        let mut a = good();
        assert!(a.to_file().lines().any(|l| l == "optional="));
        a.optional = vec!["tutorial".into(), "oracle".into()];
        assert!(a.to_file().lines().any(|l| l == "optional=tutorial,oracle"));
    }
}
