//! Repository client for fetching packages and metadata

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use super::{RepoIndex, RepoPackage};

/// Repository client configuration
#[derive(Debug, Clone)]
pub struct RepoClient {
    pub name: String,
    pub base_url: String,
    pub client: reqwest::Client,
}

impl RepoClient {
    /// Create a new repository client
    pub fn new(name: String, base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("rvn/0.1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            name,
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    /// Fetch repository index
    pub async fn fetch_index(&self) -> Result<RepoIndex> {
        let url = format!("{}/index.json", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch index from {}", url))?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch index: HTTP {}", response.status());
        }

        let index: RepoIndex = response
            .json()
            .await
            .context("Failed to parse repository index")?;

        Ok(index)
    }

    /// Search for packages
    pub async fn search(&self, query: &str, search_description: bool) -> Result<Vec<RepoPackage>> {
        let index = self.fetch_index().await?;
        let query_lower = query.to_lowercase();

        let matches: Vec<RepoPackage> = index
            .packages
            .into_iter()
            .filter(|pkg| {
                pkg.name.to_lowercase().contains(&query_lower)
                    || (search_description && pkg.description.to_lowercase().contains(&query_lower))
            })
            .collect();

        Ok(matches)
    }

    /// Get package info
    pub async fn get_package(&self, name: &str) -> Result<Option<RepoPackage>> {
        let index = self.fetch_index().await?;
        Ok(index.packages.into_iter().find(|p| p.name == name))
    }

    /// Download a package file
    pub async fn download_package(
        &self,
        package: &RepoPackage,
        dest_dir: &Path,
        show_progress: bool,
    ) -> Result<std::path::PathBuf> {
        let url = format!("{}/packages/{}", self.base_url, package.filename);
        let dest_path = dest_dir.join(&package.filename);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to download {}", url))?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download package: HTTP {}", response.status());
        }

        let total_size = response.content_length().unwrap_or(package.download_size);

        let pb = if show_progress {
            let pb = ProgressBar::new(total_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            Some(pb)
        } else {
            None
        };

        let mut file = File::create(&dest_path)
            .await
            .with_context(|| format!("Failed to create file: {}", dest_path.display()))?;

        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading download stream")?;
            file.write_all(&chunk).await?;
            if let Some(ref pb) = pb {
                pb.inc(chunk.len() as u64);
            }
        }

        if let Some(pb) = pb {
            pb.finish_with_message("Downloaded");
        }

        // Verify checksum
        let hash = crate::package::archive::hash_file(&dest_path)?;
        if hash != package.sha256 {
            std::fs::remove_file(&dest_path)?;
            anyhow::bail!(
                "Checksum mismatch for {}: expected {}, got {}",
                package.name,
                package.sha256,
                hash
            );
        }

        Ok(dest_path)
    }
}

/// Fetch from multiple repositories
pub struct MultiRepoClient {
    repos: Vec<RepoClient>,
}

impl MultiRepoClient {
    pub fn new() -> Self {
        Self { repos: Vec::new() }
    }

    pub fn add_repo(&mut self, name: String, base_url: String) {
        self.repos.push(RepoClient::new(name, base_url));
    }

    /// Search all repositories
    pub async fn search(
        &self,
        query: &str,
        search_description: bool,
    ) -> Result<Vec<(String, RepoPackage)>> {
        let mut results = Vec::new();
        for repo in &self.repos {
            match repo.search(query, search_description).await {
                Ok(packages) => {
                    for pkg in packages {
                        results.push((repo.name.clone(), pkg));
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to search {}: {}", repo.name, e);
                }
            }
        }
        Ok(results)
    }

    /// Find a package in any repository
    pub async fn find_package(&self, name: &str) -> Result<Option<(RepoClient, RepoPackage)>> {
        for repo in &self.repos {
            if let Ok(Some(pkg)) = repo.get_package(name).await {
                return Ok(Some((repo.clone(), pkg)));
            }
        }
        Ok(None)
    }
}

impl Default for MultiRepoClient {
    fn default() -> Self {
        Self::new()
    }
}
