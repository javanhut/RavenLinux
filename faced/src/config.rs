//! `/etc/raven/face.toml`, and every number in it a default.
//!
//! The file is optional and so is every field, so a machine needs none of it.
//! It exists because two of the things this daemon decides -- how alike two
//! faces have to be, and how hard the liveness check is -- are judgements that
//! cannot be right for every camera in every room, and a machine where they
//! are wrong should be fixable without a rebuild.
//!
//! A file that is present and does not parse is a hard error: the daemon
//! refuses to start rather than silently falling back to defaults, because
//! falling back would quietly ignore a policy somebody wrote down. That is
//! `ravend`'s rule for `login.toml` and it is the right one here for the same
//! reason -- more so, because one of these numbers is how easily somebody gets
//! in.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::liveness::Limits;

pub const PATH: &str = "/etc/raven/face.toml";

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The camera to use, instead of the one this would have found.
    ///
    /// For a machine with several where the wrong one wins -- an external
    /// webcam pointed at a wall, or an infrared node this cannot recognise.
    pub device: Option<PathBuf>,

    /// Where the models are.
    pub models: Option<PathBuf>,

    /// How alike two faces have to be, as a cosine similarity between their
    /// embeddings. 1.0 is identical.
    ///
    /// OpenCV's own recommendation for SFace is 0.363, which is where the
    /// model does best at answering "are these two photographs of the same
    /// person". This is not that question. This is "should this open somebody's
    /// machine", where a wrong yes is much worse than a wrong no and there is
    /// a password sitting underneath either way -- so the default is well
    /// above OpenCV's.
    pub threshold: f32,

    /// How confident the detector has to be that something is a face at all.
    pub detect_threshold: f32,

    /// How much of the frame's short side the face has to fill. Under the
    /// first, somebody is too far away for the screen's light to reach them
    /// and for the recogniser to have much to work with; over the second they
    /// are close enough that the camera is looking at a nose.
    pub min_fill: f32,
    pub max_fill: f32,

    /// The liveness check's numbers. See [`crate::liveness`] -- every one of
    /// them is a judgement, and the module says which way each errs.
    pub min_response: f32,
    pub min_correlation: f32,
    pub min_relief: f32,
    pub max_glare: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device: None,
            models: None,
            threshold: 0.45,
            detect_threshold: 0.8,
            min_fill: 0.22,
            max_fill: 0.95,
            min_response: Limits::default().min_response,
            min_correlation: Limits::default().min_correlation,
            min_relief: Limits::default().min_relief,
            max_glare: Limits::default().max_glare,
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(Path::new(PATH))
    }

    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(anyhow::anyhow!("cannot read {}: {e}", path.display())),
        };
        let config: Self = toml::from_str(&text)
            .map_err(|e| anyhow::anyhow!("{} does not parse: {e}", path.display()))?;
        config.check()?;
        Ok(config)
    }

    /// Refuse a file that would turn face unlock into a formality.
    ///
    /// A threshold of zero matches everybody, and a correlation of zero passes
    /// a photograph. Somebody who wants face unlock off should turn it off in
    /// Settings; somebody who has written a 0 here has made a mistake, and a
    /// daemon that started anyway would be one that let the next person who
    /// walked past into this machine.
    fn check(&self) -> anyhow::Result<()> {
        if !(0.2..=1.0).contains(&self.threshold) {
            anyhow::bail!(
                "threshold = {} is not a similarity face unlock can be trusted with \
                 (0.2 to 1.0, and 0.45 is the default)",
                self.threshold
            );
        }
        if !(0.1..=1.0).contains(&self.detect_threshold) {
            anyhow::bail!("detect_threshold = {} is out of range", self.detect_threshold);
        }
        if !(0.0..1.0).contains(&self.min_fill) || !(self.min_fill..=1.0).contains(&self.max_fill) {
            anyhow::bail!("min_fill and max_fill do not describe a range");
        }
        if self.min_correlation < 0.2 {
            anyhow::bail!(
                "min_correlation = {} would let a photograph past",
                self.min_correlation
            );
        }
        if self.min_relief <= 0.0 {
            anyhow::bail!(
                "min_relief = {} would let a flat photograph past",
                self.min_relief
            );
        }
        Ok(())
    }

    pub fn limits(&self) -> Limits {
        Limits {
            min_response: self.min_response,
            min_correlation: self.min_correlation,
            min_relief: self.min_relief,
            max_glare: self.max_glare,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("raven-faced-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join(name);
        std::fs::write(&path, text).expect("write");
        path
    }

    #[test]
    fn a_missing_file_is_the_defaults() {
        let config = Config::load_from(Path::new("/nonexistent/face.toml")).expect("defaults");
        assert!((config.threshold - 0.45).abs() < 1e-6);
    }

    #[test]
    fn a_partial_file_leaves_the_rest_alone() {
        let path = write("partial.toml", "threshold = 0.5\n");
        let config = Config::load_from(&path).expect("loads");
        assert!((config.threshold - 0.5).abs() < 1e-6);
        assert!((config.min_relief - Limits::default().min_relief).abs() < 1e-6);
    }

    /// A file that does not parse stops the daemon rather than being ignored.
    #[test]
    fn a_damaged_file_is_refused() {
        let path = write("damaged.toml", "threshold = yes\n");
        assert!(Config::load_from(&path).is_err());
    }

    /// A key nobody recognises is a typo, and a typo in this file is a setting
    /// somebody believes is in force and is not.
    #[test]
    fn an_unknown_key_is_refused() {
        let path = write("typo.toml", "treshold = 0.5\n");
        assert!(Config::load_from(&path).is_err());
    }

    /// The numbers that would make this a formality are refused outright.
    #[test]
    fn settings_that_would_let_anybody_in_are_refused() {
        for bad in [
            "threshold = 0.0",
            "threshold = 0.1",
            "min_correlation = 0.0",
            "min_relief = 0.0",
            "detect_threshold = 0.0",
        ] {
            let path = write("bad.toml", bad);
            assert!(Config::load_from(&path).is_err(), "{bad} was accepted");
        }
    }

    /// ...but a machine that needs tuning can still be tuned.
    #[test]
    fn a_stricter_file_is_accepted() {
        let path = write(
            "strict.toml",
            "threshold = 0.6\nmin_relief = 0.2\nmin_correlation = 0.7\n",
        );
        let config = Config::load_from(&path).expect("loads");
        assert!((config.limits().min_relief - 0.2).abs() < 1e-6);
    }
}
