//! Whether the hardware clock keeps UTC or local time.
//!
//! UTC unless /etc/adjtime says otherwise, the util-linux convention: its
//! third line is `UTC` or `LOCAL`. LOCAL exists for one reason -- Windows keeps
//! the RTC in local time, and on a machine that also boots Windows the two
//! systems otherwise disagree by the timezone offset every time either one
//! writes the clock. raven-install writes LOCAL when it finds Windows.
//!
//! Shared by raven-init (reads the RTC at boot) and raven-timed (writes it
//! after a step) through `#[path]`, so the two cannot disagree either.

use std::fs;

/// The hwclock flag for this machine: `--localtime` or `--utc`.
pub fn hwclock_flag() -> &'static str {
    match fs::read_to_string("/etc/adjtime") {
        Ok(s) => flag_from_adjtime(&s),
        Err(_) => "--utc",
    }
}

fn flag_from_adjtime(s: &str) -> &'static str {
    match s.lines().nth(2).map(str::trim) {
        Some("LOCAL") => "--localtime",
        _ => "--utc",
    }
}

#[cfg(test)]
mod tests {
    use super::flag_from_adjtime;

    #[test]
    fn reads_the_third_line() {
        assert_eq!(flag_from_adjtime("0.0 0 0.0\n0\nLOCAL\n"), "--localtime");
        assert_eq!(flag_from_adjtime("0.0 0 0.0\n0\nUTC\n"), "--utc");
        assert_eq!(flag_from_adjtime(""), "--utc");
        assert_eq!(flag_from_adjtime("garbage"), "--utc");
    }
}
