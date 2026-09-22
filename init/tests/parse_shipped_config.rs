// Temporary verification: the configs this repo ships must deserialize with
// the ServiceConfig schema PID 1 actually uses.
use std::path::Path;

// Pulled in whole for a crate root that is this test file, so it carries items
// only PID 1 calls. Not dead code -- code this binary is not the caller of.
#[allow(dead_code)]
#[path = "../src/config.rs"]
mod config;

/// Every service template under `configs/raven/services` must parse with the
/// same schema as `init.toml`, because that is what copying one into
/// `/etc/raven/init.d` asks init to do with it. A template that does not parse
/// is a service that silently does not exist, and the instructions for using it
/// are in a comment at the top of the file that no test reads.
#[test]
fn shipped_service_templates_parse() {
    for sub in ["services", "user-services"] {
        templates_parse(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../configs/raven").join(sub));
    }
}

fn templates_parse(dir: &Path) {
    let mut seen = 0;

    for entry in std::fs::read_dir(dir).expect("the template directory exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        seen += 1;

        let text = std::fs::read_to_string(&path).expect("readable");
        let cfg: config::InitConfig = toml::from_str(&text)
            .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
        assert!(
            !cfg.services.is_empty(),
            "{} defines no service at all",
            path.display()
        );
    }

    assert!(seen > 0, "no templates found in {}", dir.display());
}

#[test]
fn shipped_configs_parse() {
    for rel in ["../etc/raven/init.toml", "../init/config/init.toml"] {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        let text = std::fs::read_to_string(&p).expect("readable");
        let cfg: config::InitConfig = toml::from_str(&text).expect("parses");

        let cawd = cfg
            .services
            .iter()
            .find(|s| s.name == "cawd")
            .expect("cawd present");
        assert_eq!(cawd.stop_exec.as_deref(), Some("/usr/bin/caw"));
        assert_eq!(cawd.stop_args, vec!["shutdown".to_string()]);
        assert_eq!(cawd.stop_timeout, 5);
        assert!(cawd.enabled);

        // The lid daemon. Shipped enabled: a laptop whose lid does nothing is
        // the thing this service exists to stop being true.
        let powerd = cfg
            .services
            .iter()
            .find(|s| s.name == "powerd")
            .expect("powerd present");
        assert_eq!(powerd.exec, "/usr/bin/raven-powerd");
        assert!(powerd.enabled);
        assert!(powerd.restart, "a dead powerd is a dead lid");

        // A service with no stop fields must still parse, and default sanely.
        let getty = cfg
            .services
            .iter()
            .find(|s| s.name == "getty-tty1")
            .expect("getty present");
        assert_eq!(getty.stop_exec, None);
        assert!(getty.stop_args.is_empty());
        assert_eq!(getty.stop_timeout, 5, "serde default must apply, not 0");

        // cawd is the only wireless daemon in the image; iwd is not shipped,
        // because two nl80211 daemons on one wiphy fight over the interface.
        assert!(cfg.services.iter().all(|s| s.name != "iwd"), "iwd is gone");

        let ports = cfg
            .services
            .iter()
            .find(|s| s.name == "ports")
            .expect("ports service is shipped");
        assert_eq!(ports.exec, "/usr/bin/raven-ports");
        assert_eq!(ports.args, vec!["watch".to_string(), "--react".to_string()]);
        assert!(ports.restart, "a dead watcher is a dock NIC with no address");
        assert!(ports.after.contains(&"udev".to_string()));
        let cawd = cfg
            .services
            .iter()
            .find(|s| s.name == "cawd")
            .expect("cawd present");
        assert!(cawd.enabled, "cawd must be enabled at boot");

        // The log rotation policy is shipped explicitly rather than left to
        // serde, because a knob an administrator cannot see in the file is a
        // knob that does not exist to them. Asserting against
        // `SystemConfig::default()` rather than against literals is the point
        // of the test: it does not care what the numbers are, only that the
        // file and the code still agree about them. A default changed in
        // config.rs without the shipped file following would otherwise be
        // discovered on a machine, as a rotation that happens at a size
        // nobody wrote down.
        let defaults = config::SystemConfig::default();
        assert_eq!(cfg.system.log_max_size, defaults.log_max_size);
        assert_eq!(cfg.system.log_keep, defaults.log_keep);
        assert_eq!(cfg.system.log_total_max, defaults.log_total_max);
        assert_eq!(cfg.system.log_compress, defaults.log_compress);

        // The resource policy both configs carry. dbus is the one service
        // defined in both files that has any, so it is the one that can be
        // checked in this loop; see the test below for the rest.
        let dbus = cfg
            .services
            .iter()
            .find(|s| s.name == "dbus")
            .expect("dbus present");
        assert_eq!(
            dbus.oom_score_adj, -500,
            "every portal and half the desktop hangs on this socket rather \
             than dying with it; it must not be the kernel's first choice"
        );
        assert_eq!(
            dbus.limits.nofile,
            Some(8192),
            "a bus holding a descriptor per client meets the kernel's soft \
             1024 as EMFILE from accept(), which reads as a hang"
        );

        println!("{} ok: {} services", rel, cfg.services.len());
    }
}

/// The resource and credential policy that only the shipped config carries.
///
/// Separate from `shipped_configs_parse` because `init/config/init.toml` is the
/// fallback copy and defines a smaller set of services: faced and fprintd are
/// not in it, so asserting on them there would fail for a reason that is not a
/// regression. This one names `etc/raven/init.toml` alone, which is the file
/// stage2 copies to /etc and therefore the file a machine actually boots.
///
/// Every assertion here is a value somebody argued for in a comment beside it.
/// The test exists so that an edit which removes one -- a block rewritten, a
/// sub-table moved above a plain key so TOML quietly reparents it -- fails in
/// `cargo test` rather than on a laptop, where the evidence would be a core
/// file full of fingerprint templates or a compositor killed in place of a
/// face recogniser.
#[test]
fn the_shipped_config_carries_its_resource_policy() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../etc/raven/init.toml");
    let text = std::fs::read_to_string(&p).expect("readable");
    let cfg: config::InitConfig = toml::from_str(&text).expect("parses");

    let svc = |name: &str| {
        cfg.services
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("{name} is shipped"))
    };

    // Three daemons hold a credential in memory for as long as they are
    // answering: wireless passphrases, finger templates, face embeddings. A
    // core dump is a copy of that written to disk by the kernel with no say
    // from the program, so all three refuse to produce one.
    for name in ["cawd", "fprintd", "faced"] {
        assert_eq!(
            svc(name).limits.core.as_deref(),
            Some("0"),
            "{name} holds a credential; it must not be able to dump core"
        );
    }

    // faced optimises two ONNX graphs before it binds, which is both the
    // largest stretch of CPU and the largest resident allocation anything in
    // this file makes. The nice value keeps that off the compositor's back
    // during boot; the score says that losing face unlock is cheaper than
    // losing the session, which is what the kernel would otherwise take.
    let faced = svc("faced");
    assert_eq!(faced.nice, 5);
    assert!(
        faced.oom_score_adj > 0,
        "faced is the expendable one: the password still works without it"
    );
    assert!(faced.restart, "an expendable service must come back");

    // The one-shots. `restart = false` says the supervisor should not bring
    // them back; `type` is what says their exiting is success, which is what
    // keeps `raven-rc list` from printing the same word for a finished
    // coldplug and a daemon that died.
    for name in ["udev", "console-font", "network", "nftables"] {
        assert_eq!(
            svc(name).service_type,
            config::ServiceType::Oneshot,
            "{name} finishing is what success looks like for it"
        );
    }

    // Nothing here sets a memory ceiling, and that is a decision rather than
    // an omission: a limit guessed below a working set turns a daemon that
    // works into one killed under the load it was installed to handle, with a
    // bare SIGKILL as the only evidence. If one ever appears, it should arrive
    // with a measurement and this assertion should be the thing that makes
    // somebody write the measurement down.
    for s in &cfg.services {
        assert!(
            s.memory_max.is_none(),
            "{} has a memory_max; no working set here has been measured",
            s.name
        );
    }
}
