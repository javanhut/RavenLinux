//! The sysctl stage: the kernel parameters the machine boots with.
//!
//! A handful of things about a running kernel are not decided at build time
//! and are not decided by any program: they are numbers in /proc/sys that
//! somebody has to write. Whether an unprivileged process can read the ring
//! buffer, whether /proc/kallsyms hands out kernel addresses, whether a
//! symlink planted in /tmp by one user is followed by a program running as
//! another, how readily the machine pages anonymous memory out. The kernel's
//! own defaults for most of these were chosen for compatibility with
//! everything that ever ran on Linux, which is not the same thing as being
//! right for a laptop built in 2025.
//!
//! The convention for expressing that -- one every distribution shares, and
//! the one the directories on this machine were created for -- is a directory
//! of `*.conf` fragments holding `key = value` lines. This module reads them
//! and writes them into /proc/sys, once, during early boot.
//!
//! # Why this file exists at all
//!
//! The directories were already there. `scripts/lib/skeleton.sh` creates
//! /usr/lib/sysctl.d and /etc/sysctl.d with every other vendor/admin pair
//! (tmpfiles.d, sysusers.d, modules-load.d), the installed image already had
//! four files sitting in the first of them -- Arch's 10-arch.conf and three
//! shipped by systemd -- and nothing on Raven had ever read a single line of
//! any of them. The program that applies those directories on other
//! distributions is systemd-sysctl.service, which is part of systemd, which
//! Raven does not run.
//!
//! So the machine booted with `fs.protected_symlinks = 0` while
//! /usr/lib/sysctl.d/50-default.conf had been saying `1` since the day the
//! image was built, with `kernel.kptr_restrict = 0` handing kernel addresses
//! to anything that could open a file, and with `fs.inotify.max_user_watches`
//! at the kernel's 8192 rather than the 524288 that 10-arch.conf asks for --
//! which is the number a file manager or an editor watching a source tree
//! actually needs. There was no way to express a kernel policy at all: the one
//! sysctl init wrote was a single hardcoded `fs::write` for ping_group_range
//! in `early_boot`, and anything else meant editing PID 1 and rebuilding it.
//!
//! # The format, and which parts of it are honoured
//!
//! `key = value`, one per line. Blank lines and lines beginning with `#` or
//! `;` are comments. Whitespace around the key and the value is stripped;
//! whitespace *inside* a value is not, because several knobs take more than
//! one number (`net.ipv4.ping_group_range = 0 2147483647`).
//!
//! A key is written either in the dotted spelling (`kernel.kptr_restrict`) or
//! in the path spelling (`kernel/kptr_restrict`), and they mean the same
//! /proc/sys file. A key containing a `/` anywhere is taken as a path exactly
//! as written and nothing in it is translated -- that is the documented escape
//! hatch for the one genuinely ambiguous case, an interface whose name
//! contains a dot: `net.ipv4.conf.eth0.1.rp_filter` cannot be read back into
//! a path by any rule, and `net/ipv4/conf/eth0.1/rp_filter` can.
//!
//! A leading `-` on a line means "a failure here is not worth mentioning",
//! and is how a fragment names a knob that only some kernels have.
//!
//! A line naming a key with no `=` at all removes that key from the set about
//! to be applied, so a fragment can cancel an assignment made by an earlier
//! one. That looks like a curiosity until you read the systemd file this
//! machine ships, which does exactly that three times over.
//!
//! # What this is not
//!
//! It is not `sysctl(8)`. There is no command-line interface here, no `-w`,
//! no way to read a value back out; anything interactive is what
//! /proc/sys is for, and `cat` and `echo` do it perfectly well.
//!
//! It does not support glob keys (`net.ipv4.conf.*.rp_filter`). systemd's
//! 50-default.conf uses three of them, and each is skipped with a line in
//! init.log rather than a complaint on the console. What those three would
//! have done is set a per-interface default that is only ever consulted
//! together with the `all` knob of the same name -- the kernel takes the
//! maximum of the two for rp_filter, and Raven's own fragment sets `all`
//! directly -- so what is lost by not expanding them is small and known.
//! Supporting them properly means walking /proc/sys to expand each pattern,
//! and that belongs in a change that is about globs and nothing else.
//!
//! It does not read /etc/sysctl.conf, the single file that predates the
//! directories. Nothing on this machine ships one, sysctl.d(5) describes it as
//! a compatibility path, and a second syntax with a second set of precedence
//! rules is not worth carrying for a file that does not exist.
//!
//! It is also not a thing that runs more than once. See [`apply_boot_sysctls`]
//! for why a re-exec deliberately does not re-apply any of this.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The distribution's own fragments. Owned by packages; a package update
/// replaces what is here, so nothing an operator writes belongs in it.
const VENDOR_DIR: &str = "/usr/lib/sysctl.d";

/// Fragments generated at runtime, by an installer or a first-boot script.
/// Nothing on Raven writes this yet and the directory usually does not exist,
/// which costs one failed `read_dir`; it is read because the convention says
/// it is read, and because a generated fragment that silently did nothing
/// would be a genuinely puzzling bug to find later.
const RUNTIME_DIR: &str = "/run/sysctl.d";

/// The administrator's fragments. Read last, so a file here wins over a
/// vendor file of the same name outright, and over any other vendor file by
/// being sorted later -- which is why the convention is to number local files
/// 90-something.
const ADMIN_DIR: &str = "/etc/sysctl.d";

/// Where the knobs themselves live. A parameter, not a hardcoded path, so the
/// tests can point the whole stage at a directory tree they built.
const PROC_SYS: &str = "/proc/sys";

/// What one pass over the directories did.
///
/// Returned rather than only logged so that a caller -- today the boot stage,
/// tomorrow perhaps a `raven-rc` verb that re-applies the files -- can say
/// something about the result without parsing its own log.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// Fragments read, after shadowing by name.
    pub files: usize,
    /// Keys written to /proc/sys successfully.
    pub applied: usize,
    /// Keys whose /proc/sys path does not exist on this kernel.
    pub missing: usize,
    /// Keys whose path exists but whose write failed.
    pub failed: usize,
}

/// Apply every fragment under the three sysctl.d directories to the running
/// kernel.
///
/// Called once, from `early_boot`, after the filesystems are mounted -- it
/// writes to /proc/sys, so it cannot run before /proc is there -- and before
/// any service starts, because a service is entitled to assume it is running
/// on the kernel the machine's policy describes.
///
/// # Why a re-exec does not do this again
///
/// `early_boot` is skipped when init re-executes itself (`raven-rc reexec`),
/// and that is the right answer here rather than an oversight to work around.
/// A sysctl is kernel state, not process state: every value written at boot is
/// still in place, because replacing PID 1's image does not disturb the kernel
/// it is running on. Re-applying them would therefore change nothing on a
/// machine nobody had touched -- and on a machine somebody *had* touched, it
/// would quietly undo them. `sysctl -w vm.swappiness=100` typed while chasing a
/// problem would survive until the next `raven-rc reexec`, at which point an
/// unrelated upgrade of the init binary would put it back to 10 with no
/// message anywhere. Init re-reading the files after somebody edited them is a
/// reasonable thing to want, but it should be a thing that is asked for by
/// name, not a side effect of replacing a binary.
pub fn apply_boot_sysctls() -> Report {
    let dirs = [
        Path::new(VENDOR_DIR),
        Path::new(RUNTIME_DIR),
        Path::new(ADMIN_DIR),
    ];
    apply_all(&dirs, Path::new(PROC_SYS))
}

/// The stage itself, with every path it touches handed to it.
///
/// `dirs` are read in the order given and later ones shadow earlier ones by
/// identical file name; the surviving fragments are then applied in
/// lexicographic order of that name, regardless of which directory each came
/// from. That is two different orderings doing two different jobs and it is
/// worth being precise about which is which, because getting them the wrong
/// way round is invisible until the day it matters:
///
/// * The directory order is about *replacement*. /etc/sysctl.d/50-default.conf
///   replaces /usr/lib/sysctl.d/50-default.conf entirely -- every line of it,
///   including the lines the admin copy does not mention. Symlinking a name in
///   /etc to /dev/null therefore switches a vendor fragment off, which is the
///   documented way to do that and which falls out of this for free: the
///   symlink is read as an empty file.
///
/// * The name order is about *precedence between different fragments*.
///   10-arch.conf is applied before 50-raven.conf, which is applied before an
///   admin's 90-local.conf, and where two of them name the same key the last
///   one to name it is the one the kernel ends up with.
///
/// Nothing here is fatal and nothing here returns an error. A fragment that
/// cannot be read, a line that is not a setting, a knob this kernel does not
/// have and a write the kernel refuses are each a log line and nothing more,
/// which is the same rule the rest of init follows: this is policy, and a
/// machine with the wrong value for `vm.swappiness` is still a machine that
/// must finish booting.
pub fn apply_all(dirs: &[&Path], proc_sys: &Path) -> Report {
    let files = files_to_read(dirs);
    let mut settings = Settings::default();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            // Unreadable rather than absent: the directory listed it a moment
            // ago. Worth a word, because a fragment that is not applied looks
            // exactly like a fragment that does nothing.
            log::warn!(
                "sysctl: cannot read {}; its settings are not applied",
                file.display()
            );
            continue;
        };
        parse_into(file, &text, &mut settings);
    }

    let mut report = Report {
        files: files.len(),
        ..Report::default()
    };
    for setting in &settings.order {
        apply_one(proc_sys, setting, &mut report);
    }

    // The report goes to init.log at Info and not to the console. Every line
    // of it is the machine working as configured, and the console during boot
    // belongs to the person watching for the thing that did not work -- the
    // same rule the logger itself states (see DualLogger in main.rs). The
    // individual failures above are warnings and do reach the console, because
    // a policy that did not take effect is exactly what somebody standing
    // there needs to know.
    //
    // `system.log_level` in init.toml is not consulted, and cannot be: this
    // stage runs in early boot, several phases before config::load() has read
    // the file. The level that applies is the one the logger was installed
    // with, which is Info.
    log::info!(
        "Applied {} kernel parameter(s) from {} sysctl.d file(s)",
        report.applied,
        report.files
    );
    if report.missing > 0 || report.failed > 0 {
        log::info!(
            "  {} key(s) absent on this kernel, {} refused by it",
            report.missing,
            report.failed
        );
    }
    report
}

/// The fragments to read, in the order to read them.
fn files_to_read(dirs: &[&Path]) -> Vec<PathBuf> {
    let mut chosen: HashMap<String, PathBuf> = HashMap::new();
    for dir in dirs {
        // A directory that does not exist is the normal case for /run/sysctl.d
        // and a perfectly ordinary one for /etc/sysctl.d on a fresh install.
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() != Some(std::ffi::OsStr::new("conf")) {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
                continue;
            };
            if let Some(shadowed) = chosen.insert(name, path.clone()) {
                log::info!("sysctl: {} replaces {}", path.display(), shadowed.display());
            }
        }
    }

    let mut files: Vec<(String, PathBuf)> = chosen.into_iter().collect();
    // Byte order on the file name, which is what every other implementation of
    // this convention sorts by. Not the full path: 10-arch.conf from
    // /usr/lib must come before 90-local.conf from /etc, and comparing paths
    // would put every /etc file after every /usr/lib one.
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files.into_iter().map(|(_, path)| path).collect()
}

/// One `key = value` a fragment asked for.
struct Setting {
    /// The key as the file spelled it, for messages. Somebody reading a
    /// warning should see the text they can search their fragment for.
    key: String,
    /// The same key as a path relative to /proc/sys, validated.
    slug: String,
    /// `None` for a line that named the key and gave no value: a request to
    /// drop an assignment an earlier fragment made.
    value: Option<String>,
    /// The line began with `-`.
    ignore_failure: bool,
    source: PathBuf,
    line: usize,
}

/// Every setting the fragments asked for, in the order they were first named.
///
/// A `Vec` for the order and a map for the lookup rather than an ordered map,
/// because the order that matters is not the order keys are finally written in
/// -- each key is written exactly once, so that order is arbitrary -- but the
/// order a later assignment to the same key has to find the earlier one in.
#[derive(Default)]
struct Settings {
    order: Vec<Setting>,
    index: HashMap<String, usize>,
}

impl Settings {
    fn assign(&mut self, setting: Setting) {
        let Some(&at) = self.index.get(&setting.slug) else {
            self.index.insert(setting.slug.clone(), self.order.len());
            self.order.push(setting);
            return;
        };

        let previous = &self.order[at];
        // Only worth a line when the outcome actually changed. Two fragments
        // agreeing on a value is the usual case -- a local file copied from
        // the vendor one with a single line edited -- and it is not news.
        match (&previous.value, &setting.value) {
            (Some(old), Some(new)) if old != new => log::info!(
                "sysctl: {} line {} sets {} = {}, overriding {} from {} line {}",
                setting.source.display(),
                setting.line,
                setting.key,
                new,
                old,
                previous.source.display(),
                previous.line
            ),
            (Some(old), None) => log::info!(
                "sysctl: {} line {} drops {} = {}, set by {} line {}",
                setting.source.display(),
                setting.line,
                previous.key,
                old,
                previous.source.display(),
                previous.line
            ),
            _ => {}
        }
        // The slot keeps its position. A key first named by 10-arch.conf and
        // changed by 90-local.conf is still written where 10-arch.conf put it
        // in the queue, which matters for nothing today and keeps the order
        // stable and explicable if it ever does.
        self.order[at] = setting;
    }
}

/// Read one fragment into the set.
fn parse_into(source: &Path, text: &str, settings: &mut Settings) {
    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();
        // `;` as well as `#`: both spellings are in the wild, and sysctl.d(5)
        // accepts either.
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        let (ignore_failure, body) = match line.strip_prefix('-') {
            Some(rest) => (true, rest.trim_start()),
            None => (false, line),
        };

        let (written_key, value) = match body.split_once('=') {
            Some((key, value)) => (key.trim(), Some(value.trim().to_string())),
            // No `=` at all. Not a mistake: it is how a fragment cancels an
            // assignment an earlier one made. systemd's 50-default.conf does
            // it three times, once per glob it wants to apply to every
            // interface except `all`.
            None => (body.trim(), None),
        };

        // A key with whitespace in it is not a key. This is the only thing
        // separating the cancel-an-assignment form above from a line of prose
        // somebody forgot to comment out, and without the check that line
        // would silently delete a setting.
        if written_key.is_empty() || written_key.split_whitespace().count() != 1 {
            log::warn!(
                "sysctl: {} line {} is not a `key = value` setting; ignored",
                source.display(),
                line_number
            );
            continue;
        }

        if written_key.contains(['*', '?', '[']) {
            // Deliberate and documented at the top of this file. Info, not
            // warn: the fragments that use globs are systemd's, they ship on
            // every machine, and a warning on the console at every boot for a
            // file the operator did not write and cannot fix is noise.
            log::info!(
                "sysctl: {} line {} uses a glob key ({}), which this stage does not expand; ignored",
                source.display(),
                line_number,
                written_key
            );
            continue;
        }

        let Some(slug) = knob_slug(written_key) else {
            log::warn!(
                "sysctl: {} line {} names {}, which is not a key under /proc/sys; ignored",
                source.display(),
                line_number,
                written_key
            );
            continue;
        };

        settings.assign(Setting {
            key: written_key.to_string(),
            slug,
            value,
            ignore_failure,
            source: source.to_path_buf(),
            line: line_number,
        });
    }
}

/// Turn a key as written into a path relative to /proc/sys, or refuse it.
///
/// The refusal half is the point. This is a string out of a file being turned
/// into a path that something running as PID 1 is about to write to, and
/// `Path::join` on a component like `..` or on anything absolute walks
/// straight out of /proc/sys -- `../../etc/shadow` would be joined without
/// complaint, and an absolute key would replace the base entirely. So the
/// result is assembled component by component and anything that is not a plain
/// name is rejected outright.
fn knob_slug(key: &str) -> Option<String> {
    let written = key.trim_start_matches('/');
    if written.is_empty() {
        return None;
    }

    // Dots become slashes only when the key has no slashes of its own. A key
    // written with slashes is taken exactly as it stands, which is what makes
    // `net/ipv4/conf/eth0.1/rp_filter` expressible at all.
    let slashed = if written.contains('/') {
        written.to_string()
    } else {
        written.replace('.', "/")
    };

    let mut slug = String::with_capacity(slashed.len());
    for component in slashed.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return None;
        }
        if !slug.is_empty() {
            slug.push('/');
        }
        slug.push_str(component);
    }
    Some(slug)
}

/// Write one setting, and account for what happened.
fn apply_one(proc_sys: &Path, setting: &Setting, report: &mut Report) {
    // A cancelled assignment. It is in the list so that a later fragment can
    // find it and assign to it again; it is not a thing to write.
    let Some(value) = setting.value.as_deref() else {
        return;
    };

    let path = proc_sys.join(&setting.slug);
    if !path.exists() {
        report.missing += 1;
        // Kernels differ, and this is the single most likely thing to go
        // "wrong" on a machine whose kernel config moved: CONFIG_SECURITY_YAMA
        // off means there is no kernel/yama directory at all and
        // kernel.yama.ptrace_scope names nothing. A missing knob must never
        // stop the stage, let alone the boot -- the rest of the file is still
        // worth applying, and the fragment is still right about the kernels
        // that do have it.
        if setting.ignore_failure {
            log::info!(
                "sysctl: no {} on this kernel ({} line {}, marked optional)",
                setting.key,
                setting.source.display(),
                setting.line
            );
        } else {
            log::warn!(
                "sysctl: no {} on this kernel; {} line {} had no effect",
                setting.key,
                setting.source.display(),
                setting.line
            );
        }
        return;
    }

    // Read before write, for the log alone. It costs one open of a file that
    // is already in memory and it turns "applied kernel.kptr_restrict = 2"
    // into a line that says whether anything actually changed, which is the
    // first question anybody reading this log has.
    let previous = std::fs::read_to_string(&path)
        .ok()
        .map(|text| text.trim().to_string());

    match std::fs::write(&path, value) {
        Ok(()) => {
            report.applied += 1;
            match previous {
                Some(old) if old != value => {
                    log::info!("sysctl: {} = {} (was {})", setting.key, value, old)
                }
                _ => log::info!("sysctl: {} = {}", setting.key, value),
            }
        }
        Err(e) => {
            report.failed += 1;
            // The knob exists and the kernel said no. EINVAL for a value it
            // will not take, EPERM for one it will not take from here (a
            // container, or a knob namespaced to the host), EACCES for one
            // that is read-only.
            if setting.ignore_failure {
                log::info!(
                    "sysctl: kernel refused {} = {} ({}); {} line {} marked it optional",
                    setting.key,
                    value,
                    e,
                    setting.source.display(),
                    setting.line
                );
            } else {
                log::warn!(
                    "sysctl: cannot set {} = {} ({}): from {} line {}",
                    setting.key,
                    value,
                    e,
                    setting.source.display(),
                    setting.line
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A vendor directory, a runtime directory, an admin directory and a
    /// stand-in for /proc/sys, under one temporary root.
    ///
    /// The knobs have to be created before the stage runs, because a knob that
    /// does not exist is precisely the case the stage treats as "not on this
    /// kernel" -- so a test tree with no files in it would report everything
    /// missing and apply nothing.
    struct Tree {
        root: PathBuf,
        vendor: PathBuf,
        runtime: PathBuf,
        admin: PathBuf,
        proc_sys: PathBuf,
    }

    impl Tree {
        fn new(tag: &str) -> Tree {
            let root =
                std::env::temp_dir().join(format!("raven-sysctl-{}-{}", tag, std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let tree = Tree {
                vendor: root.join("usr/lib/sysctl.d"),
                runtime: root.join("run/sysctl.d"),
                admin: root.join("etc/sysctl.d"),
                proc_sys: root.join("proc/sys"),
                root,
            };
            for dir in [&tree.vendor, &tree.admin, &tree.proc_sys] {
                std::fs::create_dir_all(dir).expect("mkdir");
            }
            tree
        }

        /// A knob this pretend kernel has, holding its default value.
        fn knob(&self, slug: &str, value: &str) {
            let path = self.proc_sys.join(slug);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            std::fs::write(path, value).expect("write");
        }

        fn fragment(&self, dir: &Path, name: &str, text: &str) {
            std::fs::create_dir_all(dir).expect("mkdir");
            std::fs::write(dir.join(name), text).expect("write");
        }

        fn read(&self, slug: &str) -> String {
            std::fs::read_to_string(self.proc_sys.join(slug)).expect("read")
        }

        fn apply(&self) -> Report {
            apply_all(
                &[
                    self.vendor.as_path(),
                    self.runtime.as_path(),
                    self.admin.as_path(),
                ],
                &self.proc_sys,
            )
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// The two orderings from `apply_all`'s doc comment, both at once: an
    /// admin file replacing a vendor file of the same name, and a
    /// later-numbered file winning a key from an earlier-numbered one in a
    /// different directory.
    #[test]
    fn the_last_fragment_to_name_a_key_is_the_one_the_kernel_gets() {
        let tree = Tree::new("order");
        tree.knob("vm/swappiness", "60");
        tree.knob("fs/inotify/max_user_watches", "8192");
        tree.knob("kernel/sysrq", "16");

        tree.fragment(
            &tree.vendor,
            "10-arch.conf",
            "fs.inotify.max_user_watches = 524288\nvm.swappiness = 60\n",
        );
        tree.fragment(&tree.vendor, "50-raven.conf", "vm.swappiness = 10\n");
        // Same name as a vendor file: replaces it outright, so the vendor
        // copy's kernel.sysrq line is never seen.
        tree.fragment(&tree.vendor, "50-default.conf", "kernel.sysrq = 1\n");
        tree.fragment(
            &tree.admin,
            "50-default.conf",
            "# nothing here on purpose\n",
        );
        // Sorts after everything, which is the whole convention behind 90-.
        tree.fragment(&tree.admin, "90-local.conf", "vm.swappiness = 100\n");

        let report = tree.apply();

        assert_eq!(tree.read("vm/swappiness"), "100", "the admin file wins");
        assert_eq!(tree.read("fs/inotify/max_user_watches"), "524288");
        assert_eq!(
            tree.read("kernel/sysrq"),
            "16",
            "an admin file of the same name replaces the vendor file entirely"
        );
        // Four surviving fragments: 10-arch, 50-raven, 50-default (the admin
        // copy) and 90-local.
        assert_eq!(report.files, 4);
        // swappiness and max_user_watches. sysrq was never asked for.
        assert_eq!(report.applied, 2);
        assert_eq!(report.missing, 0);
        assert_eq!(report.failed, 0);
    }

    /// Comments, both spellings; the dotted and the path spelling of a key;
    /// a value with a space in it; and the `-` prefix.
    #[test]
    fn the_documented_syntax_is_the_one_that_is_read() {
        let tree = Tree::new("syntax");
        tree.knob("net/ipv4/ping_group_range", "1\t0");
        tree.knob("kernel/kptr_restrict", "0");
        tree.knob("net/ipv4/conf/eth0.1/rp_filter", "0");

        tree.fragment(
            &tree.vendor,
            "50-raven.conf",
            concat!(
                "# A hash comment\n",
                "; A semicolon comment\n",
                "\n",
                "   kernel.kptr_restrict   =   2   \n",
                "-net.ipv4.ping_group_range = 0 2147483647\n",
                // The reason the path spelling exists: no rule could turn
                // `net.ipv4.conf.eth0.1.rp_filter` back into this path.
                "net/ipv4/conf/eth0.1/rp_filter = 1\n",
            ),
        );

        let report = tree.apply();

        assert_eq!(tree.read("kernel/kptr_restrict"), "2");
        assert_eq!(tree.read("net/ipv4/ping_group_range"), "0 2147483647");
        assert_eq!(tree.read("net/ipv4/conf/eth0.1/rp_filter"), "1");
        assert_eq!(report.applied, 3);
    }

    /// The Yama case, which is not hypothetical: the shipped fragment names
    /// kernel.yama.ptrace_scope and the shipped kernel is still built without
    /// CONFIG_SECURITY_YAMA, so this is what every boot does today.
    #[test]
    fn a_knob_this_kernel_does_not_have_does_not_stop_the_stage() {
        let tree = Tree::new("missing");
        tree.knob("kernel/kptr_restrict", "0");

        tree.fragment(
            &tree.vendor,
            "50-raven.conf",
            concat!(
                "kernel.yama.ptrace_scope = 1\n",
                "kernel.kptr_restrict = 2\n",
                "-vm.no_such_knob_at_all = 7\n",
            ),
        );

        let report = tree.apply();

        assert_eq!(
            tree.read("kernel/kptr_restrict"),
            "2",
            "a missing key must not cost the keys after it"
        );
        assert_eq!(report.missing, 2);
        assert_eq!(report.applied, 1);
        assert_eq!(report.failed, 0);
        assert!(
            !tree.proc_sys.join("kernel/yama").exists(),
            "nothing is created"
        );
    }

    /// A key is a path this runs as root, so it does not get to leave
    /// /proc/sys. `..` and an absolute spelling are the two ways out.
    #[test]
    fn a_key_cannot_name_a_file_outside_proc_sys() {
        let tree = Tree::new("escape");
        std::fs::write(tree.root.join("hostage"), "untouched").expect("write");
        tree.knob("vm/swappiness", "60");

        tree.fragment(
            &tree.vendor,
            "50-raven.conf",
            concat!(
                "../../hostage = owned\n",
                "../hostage = owned\n",
                "vm/../../hostage = owned\n",
                "vm.swappiness = 10\n",
            ),
        );

        let report = tree.apply();

        assert_eq!(
            std::fs::read_to_string(tree.root.join("hostage")).expect("read"),
            "untouched"
        );
        assert_eq!(tree.read("vm/swappiness"), "10");
        assert_eq!(report.applied, 1);

        // And the same judgement without the filesystem: an absolute key is
        // the case `Path::join` would silently accept by discarding the base.
        assert_eq!(
            knob_slug("kernel.kptr_restrict").as_deref(),
            Some("kernel/kptr_restrict")
        );
        assert_eq!(knob_slug("/etc/shadow").as_deref(), Some("etc/shadow"));
        assert_eq!(knob_slug("../../etc/shadow"), None);
        assert_eq!(knob_slug("kernel/../../etc/shadow"), None);
        assert_eq!(knob_slug(""), None);
        assert_eq!(knob_slug("/"), None);
    }

    /// Verbatim from /usr/lib/sysctl.d/50-default.conf, which ships on this
    /// machine and which the stage now reads on every boot. Two constructs in
    /// it are not Raven's: a glob key, and a key named with no value at all to
    /// cancel what the glob would otherwise have done. Neither may produce a
    /// mess, and the ordinary lines around them must still be applied.
    #[test]
    fn the_shipped_systemd_fragment_is_read_without_making_a_mess() {
        let tree = Tree::new("systemd");
        tree.knob("net/ipv4/conf/default/rp_filter", "0");
        tree.knob("net/ipv4/conf/all/rp_filter", "0");
        tree.knob("fs/protected_symlinks", "0");

        tree.fragment(
            &tree.vendor,
            "50-default.conf",
            concat!(
                "# Source route verification\n",
                "net.ipv4.conf.default.rp_filter = 2\n",
                "net.ipv4.conf.*.rp_filter = 2\n",
                "-net.ipv4.conf.all.rp_filter\n",
                "\n",
                "# Enable hard and soft link protection\n",
                "fs.protected_symlinks = 1\n",
            ),
        );

        let report = tree.apply();

        assert_eq!(tree.read("net/ipv4/conf/default/rp_filter"), "2");
        assert_eq!(tree.read("fs/protected_symlinks"), "1");
        assert_eq!(
            tree.read("net/ipv4/conf/all/rp_filter"),
            "0",
            "the valueless line cancels the assignment, it does not make one"
        );
        assert_eq!(report.applied, 2);
        assert_eq!(report.failed, 0);
        assert_eq!(report.missing, 0);
    }

    /// The other half of the valueless-key rule: a fragment that cancels an
    /// earlier assignment does not stop a *later* fragment from making its
    /// own, or the admin's file could not overrule a vendor cancellation.
    #[test]
    fn a_cancelled_key_can_be_set_again_by_a_later_fragment() {
        let tree = Tree::new("recancel");
        tree.knob("net/ipv4/conf/all/rp_filter", "0");

        tree.fragment(
            &tree.vendor,
            "10-first.conf",
            "net.ipv4.conf.all.rp_filter = 2\n",
        );
        tree.fragment(
            &tree.vendor,
            "50-second.conf",
            "-net.ipv4.conf.all.rp_filter\n",
        );
        tree.fragment(
            &tree.admin,
            "90-local.conf",
            "net.ipv4.conf.all.rp_filter = 1\n",
        );

        let report = tree.apply();

        assert_eq!(tree.read("net/ipv4/conf/all/rp_filter"), "1");
        assert_eq!(report.applied, 1);
    }

    /// A line of prose that lost its `#` is a mistake, and the cancel form is
    /// the reason it is a dangerous one: without a check, "swappiness should
    /// be lower" would parse as a request to drop a key. It is refused, and
    /// nothing around it is disturbed.
    #[test]
    fn a_line_that_is_not_a_setting_is_refused_rather_than_guessed_at() {
        let tree = Tree::new("prose");
        tree.knob("vm/swappiness", "60");

        tree.fragment(
            &tree.vendor,
            "50-raven.conf",
            concat!(
                "vm.swappiness = 10\n",
                "vm.swappiness should really be lower\n",
                "= 5\n",
            ),
        );

        let report = tree.apply();

        assert_eq!(tree.read("vm/swappiness"), "10");
        assert_eq!(report.applied, 1);
    }

    /// Only `*.conf`. Editors and package managers leave .conf.bak, .conf.new
    /// and .conf.pacnew beside the file they are about to replace, and every
    /// one of them is a copy of a policy that is no longer wanted.
    #[test]
    fn only_conf_files_are_read() {
        let tree = Tree::new("extension");
        tree.knob("vm/swappiness", "60");

        tree.fragment(&tree.vendor, "50-raven.conf", "vm.swappiness = 10\n");
        tree.fragment(&tree.vendor, "50-raven.conf.bak", "vm.swappiness = 90\n");
        tree.fragment(&tree.admin, "99-local.conf.pacnew", "vm.swappiness = 80\n");
        tree.fragment(&tree.admin, "README", "vm.swappiness = 70\n");

        let report = tree.apply();

        assert_eq!(tree.read("vm/swappiness"), "10");
        assert_eq!(report.files, 1);
    }

    /// A directory that is not there is the normal state of /run/sysctl.d, and
    /// an empty /etc/sysctl.d is the normal state of a fresh install. Neither
    /// is a failure, and a machine with no fragments at all boots with the
    /// kernel's own defaults and one log line.
    #[test]
    fn missing_and_empty_directories_are_not_a_failure() {
        let tree = Tree::new("empty");
        assert!(!tree.runtime.exists());

        let report = tree.apply();

        assert_eq!(report, Report::default());
    }
}
