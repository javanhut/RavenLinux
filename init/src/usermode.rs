//! `raven-init --user`: the same supervisor, for one person's session.
//!
//! The graphical session launcher used to hand-roll this: start pipewire,
//! write its pid to a file, wait for its socket, start wireplumber, and so
//! on, with no restarts and no way to ask what was running. raven-init
//! already knows how to do all of that for the system, so the session gets a
//! second instance of it, run as the user by the launcher, supervising the
//! daemons that belong to a session rather than to the machine.
//!
//! What is different from PID 1:
//!
//!   * nothing is mounted, no hostname is set, no signal is a power request;
//!     SIGTERM stops the services and exits;
//!   * services come from `/usr/share/raven/user-services/*.toml`, taken
//!     when their `exec` exists, and from `~/.config/raven/services/*.toml`,
//!     which win by name; there is no init.toml;
//!   * `${XDG_RUNTIME_DIR}` and friends expand in exec, args, environment,
//!     pre_exec, ready_path and runtime_dirs, because a session's paths are
//!     per user and unknown until it starts;
//!   * the control socket, the published status and the logs live under the
//!     user's own directories, so `raven-rc --user` needs no privilege and
//!     the system's raven-rc is untouched.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::{InitConfig, ServiceConfig};

/// Where a session's supervisor keeps its things.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `$XDG_RUNTIME_DIR/raven-init`: the socket and the published status.
    pub runtime: PathBuf,
    pub socket: PathBuf,
    /// `$XDG_STATE_HOME/raven/log` (default `~/.local/state/raven/log`):
    /// init.log and one log per service.
    pub log_dir: PathBuf,
    /// Templates the image ships for session daemons.
    pub templates: PathBuf,
    /// The user's own definitions and overrides.
    pub dropins: PathBuf,
}

/// The shipped templates for session services.
pub const TEMPLATE_DIR: &str = "/usr/share/raven/user-services";

impl Paths {
    /// From the environment. XDG_RUNTIME_DIR must be set: without it there
    /// is nowhere for a socket that belongs to this session alone.
    pub fn from_env() -> Result<Paths> {
        let runtime_root = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .context("XDG_RUNTIME_DIR is not set; a user raven-init needs a session runtime directory")?;
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")?;
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let runtime = runtime_root.join("raven-init");
        // RAVEN_USER_TEMPLATES points at another template directory, for a
        // development tree or a test; the image's is the default.
        let templates = std::env::var_os("RAVEN_USER_TEMPLATES")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(TEMPLATE_DIR));
        Ok(Paths {
            socket: runtime.join("ctl"),
            runtime,
            log_dir: state.join("raven/log"),
            templates,
            dropins: config.join("raven/services"),
        })
    }

    /// Create the runtime and log directories, private to the user.
    pub fn prepare(&self) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        for dir in [&self.runtime, &self.log_dir] {
            std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).ok();
        }
        Ok(())
    }
}

/// Expand `${NAME}` and `$NAME` from the environment. An unset variable
/// expands to nothing, the way a shell would, so a ready_path built from an
/// unset XDG_RUNTIME_DIR does not silently become a relative path: it
/// becomes an absolute one under `/`, which is visibly wrong.
pub fn expand(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            let (name, end) = if bytes[i + 1] == b'{' {
                match text[i + 2..].find('}') {
                    Some(close) => (&text[i + 2..i + 2 + close], i + 3 + close),
                    None => {
                        out.push('$');
                        i += 1;
                        continue;
                    }
                }
            } else {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                    end += 1;
                }
                if end == start {
                    out.push('$');
                    i += 1;
                    continue;
                }
                (&text[start..end], end)
            };
            out.push_str(&std::env::var(name).unwrap_or_default());
            i = end;
        } else {
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn expand_service(svc: &mut ServiceConfig) {
    svc.exec = expand(&svc.exec);
    for a in &mut svc.args {
        *a = expand(a);
    }
    for a in &mut svc.pre_exec {
        *a = expand(a);
    }
    for d in &mut svc.runtime_dirs {
        *d = expand(d);
    }
    if let Some(p) = &svc.ready_path {
        svc.ready_path = Some(expand(p));
    }
    let env: HashMap<String, String> = svc
        .environment
        .iter()
        .map(|(k, v)| (k.clone(), expand(v)))
        .collect();
    svc.environment = env;
}

/// Every service defined by the .toml files in `dir`, in file order. Each
/// file has init.toml's schema; only its services are taken.
fn services_in(dir: &Path) -> Vec<ServiceConfig> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            log::warn!("cannot read {}", path.display());
            continue;
        };
        match toml::from_str::<InitConfig>(&text) {
            Ok(parsed) => out.extend(parsed.services),
            Err(e) => log::warn!("ignoring {}: {}", path.display(), e),
        }
    }
    out
}

/// The session's service set: shipped templates whose program is installed,
/// overridden by name from the user's own directory. A user file may also
/// set `enabled = false` on a shipped one, which is how a session opts out
/// of, say, pipewire, without deleting anything the image owns.
pub fn load_config(paths: &Paths) -> InitConfig {
    let mut services: Vec<ServiceConfig> = Vec::new();
    for mut svc in services_in(&paths.templates) {
        expand_service(&mut svc);
        if !Path::new(&svc.exec).exists() {
            log::info!("template {} skipped: {} is not installed", svc.name, svc.exec);
            continue;
        }
        services.push(svc);
    }
    for mut svc in services_in(&paths.dropins) {
        expand_service(&mut svc);
        match services.iter().position(|s| s.name == svc.name) {
            Some(i) => {
                log::info!("{} overridden by {}", svc.name, paths.dropins.display());
                services[i] = svc;
            }
            None => services.push(svc),
        }
    }
    InitConfig {
        services,
        ..InitConfig::default_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("raven-usermode-{}-{}", tag, std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn variables_expand_like_a_shell_would() {
        // SAFETY-free: tests set process env only for themselves.
        std::env::set_var("RAVEN_TEST_RT", "/run/user/1000");
        assert_eq!(expand("${RAVEN_TEST_RT}/pipewire-0"), "/run/user/1000/pipewire-0");
        assert_eq!(expand("$RAVEN_TEST_RT/x"), "/run/user/1000/x");
        assert_eq!(expand("plain"), "plain");
        assert_eq!(expand("a $ b"), "a $ b");
        assert_eq!(expand("${RAVEN_TEST_UNSET_ZZ}/sock"), "/sock");
        assert_eq!(expand("${unterminated"), "${unterminated");
    }

    #[test]
    fn templates_need_their_program_and_user_files_win_by_name() {
        let root = temp("load");
        let templates = root.join("templates");
        let dropins = root.join("dropins");
        std::fs::create_dir_all(&templates).unwrap();
        std::fs::create_dir_all(&dropins).unwrap();
        std::env::set_var("RAVEN_TEST_RT2", root.to_str().unwrap());
        std::fs::write(
            templates.join("sleeper.toml"),
            "[[services]]\nname = \"sleeper\"\nexec = \"/bin/sleep\"\nargs = [\"300\"]\nready_path = \"${RAVEN_TEST_RT2}/ready\"\nrestart = true\nenabled = true\n",
        )
        .unwrap();
        std::fs::write(
            templates.join("absent.toml"),
            "[[services]]\nname = \"absent\"\nexec = \"/nonexistent/daemon\"\nenabled = true\n",
        )
        .unwrap();
        std::fs::write(
            dropins.join("mine.toml"),
            "[[services]]\nname = \"sleeper\"\nexec = \"/bin/sleep\"\nargs = [\"1\"]\nenabled = false\n\n[[services]]\nname = \"extra\"\nexec = \"/bin/true\"\nenabled = true\n",
        )
        .unwrap();
        let paths = Paths {
            runtime: root.join("rt"),
            socket: root.join("rt/ctl"),
            log_dir: root.join("log"),
            templates,
            dropins,
        };
        let cfg = load_config(&paths);
        let names: Vec<&str> = cfg.services.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["sleeper", "extra"], "absent program skipped; user file appended");
        let sleeper = &cfg.services[0];
        assert!(!sleeper.enabled, "the user's file overrides the template by name");
        assert_eq!(sleeper.args, vec!["1"]);
        assert_eq!(cfg.system.hostname, "", "no hostname in a session config");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_template_ready_path_is_expanded() {
        let root = temp("expand");
        let templates = root.join("templates");
        std::fs::create_dir_all(&templates).unwrap();
        std::env::set_var("RAVEN_TEST_RT3", "/run/user/42");
        std::fs::write(
            templates.join("pw.toml"),
            "[[services]]\nname = \"pw\"\nexec = \"/bin/sleep\"\nready_path = \"${RAVEN_TEST_RT3}/pipewire-0\"\nenvironment = { SOCK = \"$RAVEN_TEST_RT3/s\" }\nenabled = true\n",
        )
        .unwrap();
        let paths = Paths {
            runtime: root.join("rt"),
            socket: root.join("rt/ctl"),
            log_dir: root.join("log"),
            templates,
            dropins: root.join("none"),
        };
        let cfg = load_config(&paths);
        assert_eq!(cfg.services[0].ready_path.as_deref(), Some("/run/user/42/pipewire-0"));
        assert_eq!(cfg.services[0].environment["SOCK"], "/run/user/42/s");
        std::fs::remove_dir_all(&root).ok();
    }
}
