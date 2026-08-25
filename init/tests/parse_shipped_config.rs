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

        let cawd = cfg.services.iter().find(|s| s.name == "cawd").expect("cawd present");
        assert_eq!(cawd.stop_exec.as_deref(), Some("/usr/bin/caw"));
        assert_eq!(cawd.stop_args, vec!["shutdown".to_string()]);
        assert_eq!(cawd.stop_timeout, 5);
        assert!(cawd.enabled);

        // A service with no stop fields must still parse, and default sanely.
        let getty = cfg.services.iter().find(|s| s.name == "getty-tty1").expect("getty present");
        assert_eq!(getty.stop_exec, None);
        assert!(getty.stop_args.is_empty());
        assert_eq!(getty.stop_timeout, 5, "serde default must apply, not 0");

        let iwd = cfg.services.iter().find(|s| s.name == "iwd").expect("iwd present");
        assert!(!iwd.enabled, "iwd must stay disabled: it fights cawd for the wiphy");

        println!("{} ok: {} services", rel, cfg.services.len());
    }
}
