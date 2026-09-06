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

#[derive(Debug, Clone)]
pub struct Answers {
    pub disk: String,
    pub fs: String,
    pub esp_size: String,
    /// "" means "let the installer pick", "none" means no swap partition.
    pub swap: String,
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
    pub efi_nvram: bool,
}

impl Default for Answers {
    fn default() -> Self {
        // The same defaults the wizard offers in its prompts.
        Self {
            disk: String::new(),
            fs: "ext4".into(),
            esp_size: "512M".into(),
            swap: String::new(),
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
            efi_nvram: false,
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
        // A newline would end the record and turn the rest of the password into
        // a key=value line of its own. Nothing else is off limits.
        for (what, s) in [
            ("password", &self.user_password),
            ("root password", &self.root_password),
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
        put("fs", &self.fs);
        put("esp_size", &self.esp_size);
        put("swap", &self.swap);
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
        put("efi_nvram", if self.efi_nvram { "1" } else { "0" });
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
            "disk", "fs", "esp_size", "swap", "hostname", "username", "fullname",
            "user_password", "user_sudo", "root_password", "timezone", "locale",
            "keymap", "profile", "efi_nvram",
        ] {
            let n = f.lines().filter(|l| l.starts_with(&format!("{k}="))).count();
            assert_eq!(n, 1, "{k} written {n} times");
        }
    }
}
