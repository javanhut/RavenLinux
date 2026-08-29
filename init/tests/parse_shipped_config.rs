// Temporary verification: the configs this repo ships must deserialize with
// the ServiceConfig schema PID 1 actually uses.
use std::path::Path;

#[path = "../src/config.rs"]
mod config;

/// Every service template under `configs/raven/services` must parse with the
/// same schema as `init.toml`, because that is what copying one into
/// `/etc/raven/init.d` asks init to do with it. A template that does not parse
/// is a service that silently does not exist, and the instructions for using it
/// are in a comment at the top of the file that no test reads.
#[test]
fn shipped_service_templates_parse() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../configs/raven/services");
    let mut seen = 0;

    for entry in std::fs::read_dir(&dir).expect("the template directory exists") {
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
        let cawd = cfg
            .services
            .iter()
            .find(|s| s.name == "cawd")
            .expect("cawd present");
        assert!(cawd.enabled, "cawd must be enabled at boot");

        println!("{} ok: {} services", rel, cfg.services.len());
    }
}
