// Temporary verification: the configs this repo ships must deserialize with
// the ServiceConfig schema PID 1 actually uses.
use std::path::Path;

#[path = "../src/config.rs"]
mod config;

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
