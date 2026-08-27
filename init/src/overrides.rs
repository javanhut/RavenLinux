//! The transforms applied to a loaded configuration before init uses it.
//!
//! `config::load()` answers "what is written down". This module answers "what
//! this machine should therefore run", which is not the same question: the
//! kernel command line decides whether there is a graphical session at all,
//! and which services exist to support one is decided by which binaries are
//! installed rather than by anything in a file.
//!
//! # Why this is its own module
//!
//! It used to live in main.rs, and that made it unreachable from
//! `control::reload_config` -- control.rs can only name things under `crate`,
//! and in the test binaries the crate root is the test file, not main.rs. So
//! reload re-read the files and stopped there, which quietly deleted every
//! service synthesized here: `ravend` and `wayland-session` are in no file, so
//! a reload found no definition for them and dropped them. A stopped one then
//! could not be started again short of a reboot. Reload now runs the same
//! transforms the boot path does, for the same reason it already reuses
//! `config::load()` -- two answers to "what is configured" is one too many.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::Result;
use nix::unistd;

use crate::config::{InitConfig, ServiceConfig};
use crate::user;

/// The kernel command line, or the test harness's stand-in for it.
///
/// `RAVEN_INIT_CMDLINE` exists for the same reason `RAVEN_INIT_DROPIN_DIR`
/// does: none of this was testable while the only source was /proc/cmdline,
/// because the answer then depends on how the machine running `cargo test`
/// happens to have booted -- these transforms do nothing at all without
/// `raven.graphics=wayland`, so the suite passed or was skipped by accident.
/// PID 1's environment comes from the kernel, so this is not a channel anyone
/// who could not already edit init.toml can reach.
fn read_cmdline() -> String {
    match std::env::var("RAVEN_INIT_CMDLINE") {
        Ok(value) => value,
        Err(_) => fs::read_to_string("/proc/cmdline").unwrap_or_default(),
    }
}

/// Find a system program, trying the paths things actually get installed to.
///
/// Hardcoding one path is how seatd ended up unstartable: the synthesized
/// service named /bin/seatd while the image installs /sbin/seatd, so seatd
/// never ran, and the compositor failed with "Failed to open session: No such
/// file or directory" -- an error about a missing socket, which reads like a
/// seat/permission problem rather than a daemon that was never started. The
/// same mistake had already been made with unix_chkpwd and with login.
fn find_program(name: &str) -> Option<String> {
    find_program_in(&SYSTEM_BIN_DIRS, name)
}

/// Where system programs get installed, in search order.
const SYSTEM_BIN_DIRS: [&str; 5] = ["/sbin", "/usr/sbin", "/bin", "/usr/bin", "/usr/local/bin"];

/// The searchable half of [`find_program`], split out so the ordering can be
/// tested without depending on what the build host happens to have installed.
fn find_program_in(dirs: &[&str], name: &str) -> Option<String> {
    dirs.iter()
        .map(|dir| format!("{}/{}", dir, name))
        .find(|path| Path::new(path).is_file())
}

pub(crate) fn fixup_getty_login_programs(config: &mut InitConfig) {
    if Path::new("/bin/raven-shell").exists() {
        return;
    }

    for svc in &mut config.services {
        if !svc.exec.ends_with("agetty") {
            continue;
        }
        let mut idx = 0;
        while idx + 1 < svc.args.len() {
            if svc.args[idx] == "--login-program" && svc.args[idx + 1] == "/bin/raven-shell" {
                svc.args[idx + 1] = "/bin/sh".to_string();
            }
            idx += 1;
        }
    }
}

pub(crate) fn apply_kernel_cmdline_overrides(config: &mut InitConfig) -> Result<()> {
    let cmdline = read_cmdline();
    let graphics = cmdline
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("raven.graphics="));
    let wayland_choice = cmdline
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("raven.wayland="));

    if graphics != Some("wayland") {
        return Ok(());
    }

    log::info!("Kernel cmdline requested Wayland graphics");

    // Disable tty1 getty by default to avoid fighting for the tty.
    for svc in &mut config.services {
        if svc.name == "getty-tty1" {
            svc.enabled = false;
        }
    }

    // Avoid starting both a compositor and the session wrapper at once.
    for svc in &mut config.services {
        if svc.name == "raven-compositor" || svc.name == "wayland-session" {
            svc.enabled = false;
        }
    }

    // Who the graphical session belongs to.
    //
    // `raven.user=<name>` on the kernel cmdline decides it; otherwise the
    // lowest-uid regular account, which on a machine installed by
    // raven-install is the one account it created.
    //
    // The fallback is root, and it is a real fallback rather than the design:
    // an image that has never had a user added has nobody else to be. It is
    // logged at warn because a desktop running as uid 0 is worth noticing --
    // that used to be the only behaviour, which made every compositor process
    // and everything launched from the dock run as root, and made the
    // video/render/input groups the installer sets up irrelevant because root
    // ignores them.
    let session_user = cmdline
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("raven.user="))
        .and_then(|name| match user::by_name(name) {
            Ok(account) => Some(account),
            Err(e) => {
                log::warn!("raven.user={name} is unusable ({e}); falling back");
                None
            }
        })
        .or_else(user::first_regular);

    match &session_user {
        Some(account) => log::info!(
            "Wayland session will run as {} (uid {})",
            account.name,
            account.uid
        ),
        None => log::warn!(
            "No regular account found; the Wayland session will run as root. \
             Create a user and it will be picked up on the next boot, or set \
             raven.user=<name> on the kernel command line."
        ),
    }

    // The session's XDG_RUNTIME_DIR. Wayland puts its socket here, so it has
    // to exist and be owned by whoever the compositor runs as before the
    // compositor starts -- there is no logind here to make it on login.
    let (session_uid, session_gid) = session_user
        .as_ref()
        .map(|a| (a.uid, a.gid))
        .unwrap_or((0, 0));
    let runtime_dir = format!("/run/user/{session_uid}");

    fs::create_dir_all(&runtime_dir).ok();
    let _ = fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700));
    // 0700 is only a lock if the owner is the session, not root. chown after
    // the mkdir rather than before: /run is a fresh tmpfs each boot, so this
    // directory is always ours to claim and never someone else's to steal.
    if session_uid != 0 {
        if let Err(e) = unistd::chown(
            runtime_dir.as_str(),
            Some(unistd::Uid::from_raw(session_uid)),
            Some(unistd::Gid::from_raw(session_gid)),
        ) {
            log::warn!("Cannot chown {runtime_dir} to {session_uid}: {e}");
        }
    }

    // Without seatd there is no seat, and without a seat the compositor cannot
    // take DRM master -- so if it is missing, say so here rather than leaving
    // the compositor to fail in a restart loop with a misleading message.
    let seatd_path = find_program("seatd");
    if seatd_path.is_none() {
        log::warn!("seatd not found; a Wayland session will not be able to acquire a seat");
    }

    ensure_service(
        &mut config.services,
        ServiceConfig {
            name: "seatd".to_string(),
            description: "Seat management daemon".to_string(),
            exec: seatd_path.unwrap_or_else(|| "/sbin/seatd".to_string()),
            args: vec!["-g".to_string(), "video".to_string()],
            restart: true,
            enabled: true,
            critical: false,
            environment: HashMap::new(),
            pre_exec: Vec::new(),
            tty: None,
            user: None,
            runtime_dirs: Vec::new(),
            after: vec!["udev".to_string()],
            ready_path: Some("/run/seatd.sock".to_string()),
            ready_timeout: 5,
            stop_exec: None,
            stop_args: Vec::new(),
            stop_timeout: 5,
        },
    );

    let mut compositor_env = HashMap::new();
    compositor_env.insert("XDG_RUNTIME_DIR".to_string(), runtime_dir.clone());
    compositor_env.insert("LIBSEAT_BACKEND".to_string(), "seatd".to_string());

    // The rest of what a login would have set. Nothing here has a sensible
    // default once the session is not root: a compositor started with HOME
    // still pointing at /root writes its state, its font cache and every
    // application's dotfiles into a directory the session user cannot read,
    // and the failure surfaces as unrelated breakage in whatever runs first.
    if let Some(account) = &session_user {
        compositor_env.insert("HOME".to_string(), account.home.clone());
        compositor_env.insert("USER".to_string(), account.name.clone());
        compositor_env.insert("LOGNAME".to_string(), account.name.clone());
        compositor_env.insert("SHELL".to_string(), account.shell.clone());
    }

    // The account the session service runs as, or None to stay root.
    let session_account = session_user.as_ref().map(|a| a.name.clone());

    // Settle udev before the session starts, as root.
    //
    // The GPU drivers are modules and nothing waits for the coldplug, so a
    // compositor that starts too early takes the only DRM device that exists
    // yet -- simpledrm, the EFI framebuffer -- and dies when the real driver
    // loads and the kernel revokes it. The session script used to do this
    // itself, which worked only while the session was root; it now runs as a
    // regular user and cannot drive udev at all.
    //
    // pre_exec is the right place because init runs it before dropping
    // privilege, so it still happens as root no matter who the session
    // belongs to. raven-udev is idempotent -- whoever runs second settles and
    // moves on.
    let udev_settle = find_program("raven-udev")
        .map(|p| vec![p])
        .unwrap_or_default();

    // A login screen, if this image has one.
    //
    // ravend draws the greeter, checks the password and starts the session
    // itself -- the same job the service below does, minus the "auto". Only
    // one of the two can hold the GPU and the seat, so finding ravend replaces
    // that service rather than adding to it.
    //
    // Deciding it here, on whether the binary is installed, is what
    // RavenLogin's README asks for and is the difference between a login
    // screen you get by installing a package and one you get by editing
    // init.toml and disabling a getty by hand. Removing ravend falls back to
    // the autologin session with no edit anywhere and nothing on the kernel
    // cmdline either way.
    if let Some(ravend_exec) = find_program("ravend") {
        log::info!("Found ravend; the graphical session starts behind a login screen");

        let mut env = HashMap::new();
        env.insert("LIBSEAT_BACKEND".to_string(), "seatd".to_string());

        ensure_service(
            &mut config.services,
            ServiceConfig {
                name: "ravend".to_string(),
                description: "Raven login daemon".to_string(),
                exec: ravend_exec,
                args: Vec::new(),
                // Restarted, and with no fallback to the passwordless session
                // anywhere below: a login daemon that fails leaves a machine
                // that keeps trying to ask for a password, not one that gives
                // up and lets whoever is standing there in. A prompt that can
                // be skipped by breaking it is not a prompt.
                restart: true,
                enabled: true,
                // Not critical, which here means init does not panic the boot
                // over it. The console gettys are still running and are the
                // way in to fix a machine whose greeter will not start.
                critical: false,
                environment: env,
                // The same coldplug wait the session needs, for the same
                // reason: the greeter's compositor is a compositor, and one
                // that starts before the real DRM driver takes simpledrm and
                // dies when the kernel revokes it.
                pre_exec: udev_settle,
                tty: None,
                // Root, and no session account: it reads /etc/shadow, and it
                // drops privilege itself once it knows whose session it is
                // starting. Handing it an account here would defeat the point.
                user: None,
                runtime_dirs: Vec::new(),
                after: vec!["udev".to_string(), "seatd".to_string()],
                ready_path: None,
                ready_timeout: 5,
                stop_exec: None,
                stop_args: Vec::new(),
                // Longer than the session's: SIGTERM has to reach the greeter,
                // its compositor, and whatever session is running behind them.
                stop_timeout: 10,
            },
        );
        return Ok(());
    }

    let session_path = find_program("raven-wayland-session");
    if let Some(ref session_exec) = session_path {
        let mut env = compositor_env;
        env.insert(
            "RAVEN_WAYLAND_COMPOSITOR".to_string(),
            wayland_choice.unwrap_or("raven-compositor").to_string(),
        );

        ensure_service(
            &mut config.services,
            ServiceConfig {
                name: "wayland-session".to_string(),
                description: "Raven Wayland session".to_string(),
                exec: session_exec.clone(),
                args: Vec::new(),
                restart: true,
                enabled: true,
                critical: false,
                environment: env,
                pre_exec: udev_settle.clone(),
                tty: None,
                user: session_account.clone(),
                runtime_dirs: Vec::new(),
                after: vec!["udev".to_string(), "seatd".to_string()],
                ready_path: None,
                ready_timeout: 5,
                stop_exec: None,
                stop_args: Vec::new(),
                stop_timeout: 5,
            },
        );
    } else {
        // Fallback to raven-compositor directly
        ensure_service(
            &mut config.services,
            ServiceConfig {
                name: "raven-compositor".to_string(),
                description: "Raven Wayland compositor".to_string(),
                exec: "/bin/raven-compositor".to_string(),
                args: vec!["--backend".to_string(), "udev".to_string()],
                restart: true,
                enabled: true,
                critical: false,
                environment: compositor_env,
                pre_exec: udev_settle,
                tty: None,
                user: session_account,
                runtime_dirs: Vec::new(),
                after: vec!["udev".to_string(), "seatd".to_string()],
                ready_path: None,
                ready_timeout: 5,
                stop_exec: None,
                stop_args: Vec::new(),
                stop_timeout: 5,
            },
        );
    }

    Ok(())
}

fn ensure_service(services: &mut Vec<ServiceConfig>, desired: ServiceConfig) {
    let Some(existing) = services.iter_mut().find(|s| s.name == desired.name) else {
        services.push(desired);
        return;
    };

    existing.description = desired.description;
    existing.exec = desired.exec;
    existing.args = desired.args;
    existing.restart = desired.restart;
    existing.enabled = desired.enabled;
    existing.critical = desired.critical;
    existing.environment = desired.environment;
}

#[cfg(test)]
mod path_resolution_tests {
    use super::*;

    /// A directory layout mirroring the image: seatd and login in /sbin only.
    fn fixture(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("raven-find-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in ["sbin", "bin"] {
            fs::create_dir_all(root.join(d)).expect("mkdir");
        }
        fs::write(root.join("sbin/seatd"), "#!/bin/sh\n").expect("write");
        fs::write(root.join("bin/sh"), "#!/bin/sh\n").expect("write");
        root
    }

    #[test]
    fn finds_a_program_that_lives_only_in_sbin() {
        // The bug this exists for: seatd, login and unix_chkpwd all install to
        // /sbin, and each was looked up at a hardcoded /bin path. The service
        // then failed with an error about a missing socket or a broken auth
        // stack rather than a program that was never run.
        let root = fixture("sbin");
        let dirs: Vec<String> = ["sbin", "bin"]
            .iter()
            .map(|d| root.join(d).display().to_string())
            .collect();
        let refs: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();

        let found = find_program_in(&refs, "seatd").expect("seatd must be found in sbin");
        assert!(found.ends_with("/sbin/seatd"), "{found}");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_programs_report_absent_rather_than_guessing_a_path() {
        let root = fixture("missing");
        let dirs: Vec<String> = ["sbin", "bin"]
            .iter()
            .map(|d| root.join(d).display().to_string())
            .collect();
        let refs: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();

        assert_eq!(find_program_in(&refs, "definitely-not-installed"), None);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_order_prefers_sbin_over_bin() {
        let root = fixture("order");
        // Same name in both; /sbin is where system daemons belong.
        fs::write(root.join("sbin/dup"), "s").expect("write");
        fs::write(root.join("bin/dup"), "b").expect("write");

        let dirs: Vec<String> = ["sbin", "bin"]
            .iter()
            .map(|d| root.join(d).display().to_string())
            .collect();
        let refs: Vec<&str> = dirs.iter().map(|s| s.as_str()).collect();

        let found = find_program_in(&refs, "dup").expect("found");
        assert!(found.contains("/sbin/"), "{found}");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_real_search_path_covers_where_the_image_installs_things() {
        // Guards the constant itself: seatd, login and unix_chkpwd all land in
        // /sbin in this image, so dropping it from the list would silently
        // reintroduce every one of those bugs.
        assert!(SYSTEM_BIN_DIRS.contains(&"/sbin"));
        assert!(SYSTEM_BIN_DIRS.contains(&"/usr/bin"));
        assert!(SYSTEM_BIN_DIRS.contains(&"/bin"));
    }
}
