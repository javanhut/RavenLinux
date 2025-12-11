use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub repositories: Vec<Repository>,
    pub build: BuildOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub cache_dir: PathBuf,
    pub database_dir: PathBuf,
    pub log_dir: PathBuf,
    pub parallel_downloads: usize,
    pub check_signatures: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildOptions {
    pub jobs: usize,
    pub ccache: bool,
    pub build_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                cache_dir: PathBuf::from("/var/cache/rvn"),
                database_dir: PathBuf::from("/var/lib/rvn"),
                log_dir: PathBuf::from("/var/log/rvn"),
                parallel_downloads: 5,
                check_signatures: true,
            },
            repositories: vec![
                Repository {
                    name: "core".to_string(),
                    url: "https://repo.ravenlinux.org/core".to_string(),
                    enabled: true,
                    priority: 1,
                },
                Repository {
                    name: "extra".to_string(),
                    url: "https://repo.ravenlinux.org/extra".to_string(),
                    enabled: true,
                    priority: 2,
                },
                Repository {
                    name: "community".to_string(),
                    url: "https://repo.ravenlinux.org/community".to_string(),
                    enabled: true,
                    priority: 3,
                },
            ],
            build: BuildOptions {
                jobs: num_cpus::get(),
                ccache: true,
                build_dir: PathBuf::from("/tmp/rvn-build"),
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = PathBuf::from("/etc/rvn/config.toml");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = PathBuf::from("/etc/rvn/config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.general.cache_dir.clone()
    }

    pub fn database_dir(&self) -> PathBuf {
        self.general.database_dir.clone()
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
