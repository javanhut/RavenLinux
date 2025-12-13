//! Repository client for fetching packages and metadata

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

use super::{RepoIndex, RepoPackage};

/// Repository client configuration
#[derive(Debug, Clone)]
pub struct RepoClient {
    pub name: String,
    pub base_url: String,
    pub repo_type: Option<String>,
    pub client: reqwest::Client,
}

impl RepoClient {
    /// Create a new repository client
    pub fn new(name: String, base_url: String, repo_type: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("rvn/0.1.0")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            name,
            base_url: base_url.trim_end_matches('/').to_string(),
            repo_type,
            client,
        }
    }

    /// Fetch repository index
    pub async fn fetch_index(&self) -> Result<RepoIndex> {
        // Support older repo layouts that keep the index alongside packages.
        // Prefer the repo root, then fall back to /packages.
        let candidates = [
            format!("{}/index.json", self.base_url),
            format!("{}/packages/index.json", self.base_url),
        ];

        let mut last_err: Option<anyhow::Error> = None;
        for url in candidates {
            match self.fetch_index_from(&url).await {
                Ok(index) => return Ok(index),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("No index candidates tried")))
    }

    async fn fetch_index_from(&self, url: &str) -> Result<RepoIndex> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch index from {}", url))?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch index from {}: HTTP {}", url, response.status());
        }

        response
            .json()
            .await
            .with_context(|| format!("Failed to parse repository index from {}", url))
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
        let url = self.package_url(&package.filename);
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

    fn package_url(&self, filename: &str) -> String {
        let base = self
            .repo_type
            .as_deref()
            .map(|t| t.eq_ignore_ascii_case("github"))
            .unwrap_or(false);

        // For Git LFS, raw.githubusercontent.com may serve pointer files. Using the
        // github.com/{owner}/{repo}/raw/{ref}/... URL ensures redirects reach LFS storage.
        if base {
            if let Some(github_raw_base) = raw_github_to_github_raw(&self.base_url) {
                return format!("{}/packages/{}", github_raw_base, filename);
            }
        }

        format!("{}/packages/{}", self.base_url, filename)
    }
}

fn raw_github_to_github_raw(base_url: &str) -> Option<String> {
    // Transform:
    //   https://raw.githubusercontent.com/<owner>/<repo>/<ref>/<path...>
    // into:
    //   https://github.com/<owner>/<repo>/raw/<ref>/<path...>
    const PREFIX: &str = "https://raw.githubusercontent.com/";
    let rest = base_url.strip_prefix(PREFIX)?;
    let mut parts = rest.split('/').collect::<Vec<_>>();
    if parts.len() < 4 {
        return None;
    }
    let owner = parts.remove(0);
    let repo = parts.remove(0);
    let git_ref = parts.remove(0);
    let path = parts.join("/");
    if path.is_empty() {
        Some(format!(
            "https://github.com/{}/{}/raw/{}",
            owner, repo, git_ref
        ))
    } else {
        Some(format!(
            "https://github.com/{}/{}/raw/{}/{}",
            owner, repo, git_ref, path
        ))
    }
}

/// Fetch from multiple repositories
pub struct MultiRepoClient {
    repos: Vec<RepoClient>,
    indexes: RwLock<HashMap<String, RepoIndex>>,
    warned: RwLock<HashSet<String>>,
}

impl MultiRepoClient {
    pub fn new() -> Self {
        Self {
            repos: Vec::new(),
            indexes: RwLock::new(HashMap::new()),
            warned: RwLock::new(HashSet::new()),
        }
    }

    pub fn add_repo(&mut self, name: String, base_url: String, repo_type: Option<String>) {
        self.repos.push(RepoClient::new(name, base_url, repo_type));
    }

    pub async fn preload_indexes(&self) {
        use futures_util::future::join_all;

        let futures = self.repos.iter().cloned().map(|repo| async move {
            let res = repo.fetch_index().await;
            (repo.name.clone(), res)
        });

        let results = join_all(futures).await;
        for (repo_name, res) in results {
            match res {
                Ok(index) => {
                    self.indexes.write().await.insert(repo_name, index);
                }
                Err(e) => {
                    self.warn_once(&repo_name, &e).await;
                }
            }
        }
    }

    async fn warn_once(&self, repo_name: &str, err: &anyhow::Error) {
        let mut warned = self.warned.write().await;
        if warned.insert(repo_name.to_string()) {
            eprintln!("Warning: Failed to load repo index for {}: {}", repo_name, err);
        }
    }

    async fn get_index(&self, repo: &RepoClient) -> Option<RepoIndex> {
        if let Some(index) = self.indexes.read().await.get(&repo.name).cloned() {
            return Some(index);
        }

        match repo.fetch_index().await {
            Ok(index) => {
                self.indexes
                    .write()
                    .await
                    .insert(repo.name.clone(), index.clone());
                Some(index)
            }
            Err(e) => {
                self.warn_once(&repo.name, &e).await;
                None
            }
        }
    }

    /// Search all repositories
    pub async fn search(
        &self,
        query: &str,
        search_description: bool,
    ) -> Result<Vec<(String, RepoPackage)>> {
        let mut results = Vec::new();
        for repo in &self.repos {
            let Some(index) = self.get_index(repo).await else {
                continue;
            };
            let query_lower = query.to_lowercase();
            for pkg in index.packages.into_iter().filter(|pkg| {
                pkg.name.to_lowercase().contains(&query_lower)
                    || (search_description && pkg.description.to_lowercase().contains(&query_lower))
            }) {
                results.push((repo.name.clone(), pkg));
            }
        }
        Ok(results)
    }

    /// Find a package in any repository
    pub async fn find_package(&self, name: &str) -> Result<Option<(RepoClient, RepoPackage)>> {
        for repo in &self.repos {
            let Some(index) = self.get_index(repo).await else {
                continue;
            };
            if let Some(pkg) = index.packages.into_iter().find(|p| p.name == name) {
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
