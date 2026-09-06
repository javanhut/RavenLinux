//! Getting to root.
//!
//! There is no polkit agent on this image -- packages/gui/raven-store's
//! manifest says so outright -- so sudo is the privilege boundary and this is
//! all of it. The window itself never runs as root: it collects answers as
//! whoever is logged in, and only the one call that installs is elevated.

use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priv {
    /// The session is already root -- the live image's usually is.
    Root,
    /// sudo works without a password, so nothing has to be asked for.
    Passwordless,
    /// sudo works but wants a password first.
    NeedsPassword,
    /// There is no way up from here.
    Unavailable,
}

/// Which of the four situations this session is in, for the command that will
/// actually be run.
///
/// `installer` matters: the live image's sudoers drop-in grants
/// `/usr/bin/raven-install` and nothing else, so a blanket `sudo -n true`
/// answers "a password is needed" on the very image that was configured not to
/// need one, and the window would prompt for a placeholder password nobody
/// booting an ISO has been told. The question has to be asked about the
/// command that is going to be run.
pub fn detect(installer: &str) -> Priv {
    // SAFETY: geteuid cannot fail and touches no memory we own.
    if unsafe { libc_geteuid() } == 0 {
        return Priv::Root;
    }
    // The development escape hatch, and the companion to RAVEN_INSTALL: run
    // the installer with no elevation at all, so the wizard can be worked on
    // without a root session and without rebuilding an ISO to see a button.
    //
    // It grants nothing. --probe answers happily as an ordinary user (it
    // reports the missing root as a fact), and an install started this way
    // stops in preflight on raven-install's own "must run as root" -- before
    // the disk is touched, because that check is the first thing it does.
    if std::env::var("RAVEN_INSTALLER_UI_NO_PRIVESC").as_deref() == Ok("1") {
        return Priv::Root;
    }
    // -l asks whether the command may be run; it does not run it. With -n it
    // cannot prompt, so success means "allowed, and without a password" --
    // which is exactly the distinction that decides whether to show the
    // password page.
    let Ok(out) = Command::new("sudo")
        .args(["-n", "-l", "--", installer])
        .output()
    else {
        return Priv::Unavailable;
    };
    if out.status.success() {
        return Priv::Passwordless;
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    // "user is not allowed to execute" / "may not run sudo": there is no
    // password that would help, so saying so beats a prompt that can only be
    // answered wrongly.
    if stderr.contains("not allowed to execute") || stderr.contains("may not run") {
        return Priv::Unavailable;
    }
    Priv::NeedsPassword
}

// The one libc call this program makes. Declared here rather than pulling the
// libc crate in for it: the sysroot's Rust builds this crate against glibc, so
// the symbol is already linked.
extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}

/// Fill sudo's timestamp with the given password, so the install itself can
/// run under `sudo -n` and never has to be handed the password again. The
/// password is not kept anywhere after this returns.
pub fn authenticate(password: &str) -> Result<(), String> {
    let mut child = Command::new("sudo")
        .args(["-S", "-p", "", "-v"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run sudo: {e}"))?;

    {
        let mut stdin = child.stdin.take().ok_or("sudo took no stdin")?;
        // The newline is the whole of the protocol. Written and then dropped,
        // which closes the pipe -- without that, sudo waits for more.
        stdin
            .write_all(format!("{password}\n").as_bytes())
            .map_err(|e| format!("could not talk to sudo: {e}"))?;
    }

    let out = child
        .wait_with_output()
        .map_err(|e| format!("sudo did not finish: {e}"))?;

    if out.status.success() {
        return Ok(());
    }

    // An account with no usable password -- a locked one, or one whose hash
    // authenticates nothing -- fails here for every input, so a bare
    // "Incorrect password" would be a prompt that cannot be satisfied and
    // does not say so.
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("no password") || stderr.contains("account is locked") {
        Err("This account has no password set, so sudo cannot authenticate it. \
             Log in as root, or run raven-install in a terminal."
            .to_string())
    } else {
        Err("Incorrect password.".to_string())
    }
}

/// Prefix `argv` with whatever it takes to run it as root.
///
/// `-n` even after authenticate(): the timestamp is already filled, and a sudo
/// that decided to prompt anyway would block on a terminal that is not there,
/// which from the window looks exactly like an install that hung.
pub fn wrap(level: Priv, argv: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    if level != Priv::Root {
        v.push("sudo".into());
        v.push("-n".into());
    }
    v.extend(argv.iter().map(|s| s.to_string()));
    v
}
