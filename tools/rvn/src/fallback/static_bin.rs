use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use tar::Archive;

#[derive(Debug, Clone)]
pub struct StaticPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub url: String,
    pub archive: StaticArchive,
    pub installed_size: u64,
    pub download_size: u64,
}

#[derive(Debug, Clone)]
pub enum StaticArchive {
    /// A tar.gz containing a single binary at `bin_path`, installed to `/usr/bin/<name>`.
    TarGz { bin_path: String },
    /// A raw binary file, installed to `/usr/bin/<name>`.
    Raw,
}

#[derive(Debug, Clone)]
pub struct StaticClient {
    client: reqwest::Client,
}

impl StaticClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("rvn/0.1.0 (static-fallback)")
            .build()
            .expect("Failed to create HTTP client");
        Self { client }
    }

    pub fn find(&self, name: &str) -> Option<StaticPackage> {
        static_packages().into_iter().find(|p| p.name == name)
    }

    pub async fn download(&self, pkg: &StaticPackage, cache_dir: &Path) -> Result<PathBuf> {
        let filename = pkg.url.split('/').last().unwrap_or(&pkg.name).to_string();
        let dest = cache_dir.join(filename);
        if dest.exists() {
            return Ok(dest);
        }

        let resp = self
            .client
            .get(&pkg.url)
            .send()
            .await
            .with_context(|| format!("Failed to download {}", pkg.url))?;
        if !resp.status().is_success() {
            anyhow::bail!("Failed to download {}: HTTP {}", pkg.url, resp.status());
        }

        tokio::fs::create_dir_all(cache_dir).await.ok();
        let bytes = resp.bytes().await?;
        tokio::fs::write(&dest, &bytes).await?;
        Ok(dest)
    }

    pub fn extract_to_staging(
        pkg: &StaticPackage,
        downloaded: &Path,
        staging_dir: &Path,
    ) -> Result<()> {
        std::fs::create_dir_all(staging_dir)?;

        match &pkg.archive {
            StaticArchive::Raw => {
                let out = staging_dir.join("usr/bin").join(&pkg.name);
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(downloaded, &out)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755));
                }
            }
            StaticArchive::TarGz { bin_path } => {
                let file = std::fs::File::open(downloaded)?;
                let decoder = GzDecoder::new(file);
                let mut archive = Archive::new(decoder);

                let mut found = false;
                for entry in archive.entries()? {
                    let mut entry = entry?;
                    let path = entry.path()?.to_string_lossy().to_string();
                    if path == *bin_path {
                        let out = staging_dir.join("usr/bin").join(&pkg.name);
                        if let Some(parent) = out.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        entry.unpack(&out)?;
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = std::fs::set_permissions(
                                &out,
                                std::fs::Permissions::from_mode(0o755),
                            );
                        }
                        found = true;
                        break;
                    }
                }
                if !found {
                    anyhow::bail!(
                        "Static package archive missing expected binary '{}' ({})",
                        pkg.name,
                        bin_path
                    );
                }
            }
        }

        Ok(())
    }
}

fn static_packages() -> Vec<StaticPackage> {
    // These are meant as a convenience for common CLI tools that publish musl builds.
    // Prefer Alpine for complex dependency graphs / libraries.
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => "x86_64",
    };

    let mut pkgs = Vec::new();

    // ripgrep (rg)
    if arch == "x86_64" {
        pkgs.push(StaticPackage {
            name: "rg".to_string(),
            version: "14.1.0".to_string(),
            description: "ripgrep (static musl)".to_string(),
            url: "https://github.com/BurntSushi/ripgrep/releases/download/14.1.0/ripgrep-14.1.0-x86_64-unknown-linux-musl.tar.gz".to_string(),
            archive: StaticArchive::TarGz {
                bin_path: "ripgrep-14.1.0-x86_64-unknown-linux-musl/rg".to_string(),
            },
            installed_size: 0,
            download_size: 0,
        });
    }

    // fd
    if arch == "x86_64" {
        pkgs.push(StaticPackage {
            name: "fd".to_string(),
            version: "10.2.0".to_string(),
            description: "fd (static musl)".to_string(),
            url: "https://github.com/sharkdp/fd/releases/download/v10.2.0/fd-v10.2.0-x86_64-unknown-linux-musl.tar.gz".to_string(),
            archive: StaticArchive::TarGz {
                bin_path: "fd-v10.2.0-x86_64-unknown-linux-musl/fd".to_string(),
            },
            installed_size: 0,
            download_size: 0,
        });
    }

    pkgs
}
