use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;

#[derive(Debug, Clone)]
pub struct AlpinePackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub license: Option<String>,
    pub dependencies: Vec<String>,
    pub download_size: u64,
    pub installed_size: u64,
    pub repo: String,
    pub filename: String,
}

#[derive(Debug, Clone)]
pub struct AlpineClient {
    client: reqwest::Client,
    mirror: String,
    mirrors: Vec<String>,
    branch: String,
    branches: Vec<String>,
    arch: String,
    repos: Vec<String>,
    index: Option<AlpineIndex>,
}

#[derive(Debug, Clone)]
struct AlpineIndex {
    packages: HashMap<String, AlpinePackage>,
    provides: HashMap<String, String>,
}

impl AlpineClient {
    pub fn new() -> Self {
        let mirrors = std::env::var("RVN_ALPINE_MIRRORS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().trim_end_matches('/').to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .or_else(|| {
                std::env::var("RVN_ALPINE_MIRROR")
                    .ok()
                    .map(|m| vec![m.trim().trim_end_matches('/').to_string()])
            })
            .unwrap_or_else(|| vec!["https://dl-cdn.alpinelinux.org/alpine".to_string()]);

        // If user provided only https://..., try http://... as a fallback for environments
        // without working system time / CA store.
        let mut mirrors = add_http_fallback(mirrors);
        if mirrors.is_empty() {
            mirrors.push("https://dl-cdn.alpinelinux.org/alpine".to_string());
        }

        let branches = std::env::var("RVN_ALPINE_BRANCHES")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .or_else(|| std::env::var("RVN_ALPINE_BRANCH").ok().map(|b| vec![b]))
            .unwrap_or_else(|| vec!["v3.20".to_string(), "v3.19".to_string(), "edge".to_string()]);
        let branch = branches
            .first()
            .cloned()
            .unwrap_or_else(|| "v3.20".to_string());
        let arch = std::env::var("RVN_ALPINE_ARCH").unwrap_or_else(|_| default_arch());
        let repos = std::env::var("RVN_ALPINE_REPOS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec!["main".to_string(), "community".to_string()]);

        let client = reqwest::Client::builder()
            .user_agent("rvn/0.1.0 (alpine-fallback)")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            mirror: mirrors[0].clone(),
            mirrors,
            branch,
            branches,
            arch,
            repos,
            index: None,
        }
    }

    pub async fn ensure_loaded(&mut self) -> Result<()> {
        if self.index.is_some() {
            return Ok(());
        }

        let mut attempted = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for mirror in &self.mirrors {
            for branch in &self.branches {
                let mut packages: HashMap<String, AlpinePackage> = HashMap::new();
                let mut provides: HashMap<String, String> = HashMap::new();
                let mut loaded_any = false;

                for repo in &self.repos {
                    let url = format!(
                        "{}/{}/{}/{}/APKINDEX.tar.gz",
                        mirror, branch, repo, self.arch
                    );
                    attempted.push(url.clone());

                    let resp = match self.client.get(&url).send().await {
                        Ok(r) => r,
                        Err(e) => {
                            errors.push(format!("{}: {}", url, e));
                            continue;
                        }
                    };

                    if !resp.status().is_success() {
                        errors.push(format!("{}: HTTP {}", url, resp.status()));
                        continue;
                    }

                    let bytes = resp.bytes().await?;
                    let mut index_text = String::new();
                    let cursor = std::io::Cursor::new(bytes);
                    let decoder = GzDecoder::new(cursor);
                    let mut archive = Archive::new(decoder);
                    for entry in archive.entries()? {
                        let mut entry = entry?;
                        let path = entry.path()?;
                        if path.to_string_lossy().ends_with("APKINDEX") {
                            entry.read_to_string(&mut index_text)?;
                            break;
                        }
                    }

                    parse_apkindex(repo, &index_text, &mut packages, &mut provides);
                    if !index_text.trim().is_empty() {
                        loaded_any = true;
                    }
                }

                if loaded_any && !packages.is_empty() {
                    self.mirror = mirror.clone();
                    self.branch = branch.clone();
                    self.index = Some(AlpineIndex { packages, provides });
                    return Ok(());
                }
            }
        }

        let mut msg = format!(
            "Alpine fallback unavailable (could not load APKINDEX). Tried: {}",
            attempted.join(", ")
        );
        if !errors.is_empty() {
            let preview = errors.into_iter().take(3).collect::<Vec<_>>().join(" | ");
            msg.push_str(&format!(". Errors: {}", preview));
        }
        anyhow::bail!(msg)
    }

    pub async fn find(&mut self, name: &str) -> Result<Option<AlpinePackage>> {
        self.ensure_loaded().await?;
        Ok(self
            .index
            .as_ref()
            .and_then(|idx| idx.packages.get(name).cloned()))
    }

    pub async fn resolve_with_deps(&mut self, name: &str) -> Result<Vec<AlpinePackage>> {
        self.ensure_loaded().await?;
        let idx = self.index.as_ref().expect("loaded");

        let mut resolved = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        fn normalize_dep(token: &str, provides: &HashMap<String, String>) -> Option<String> {
            let t = token.trim();
            if t.is_empty() {
                return None;
            }
            if let Some(pkg) = provides.get(t) {
                return Some(pkg.clone());
            }
            let base = t
                .split(|c: char| c == '<' || c == '>' || c == '=')
                .next()
                .unwrap_or(t)
                .trim();
            if let Some(pkg) = provides.get(base) {
                return Some(pkg.clone());
            }
            if base.contains(':') {
                return provides.get(base).cloned();
            }
            Some(base.to_string())
        }

        fn dfs(
            name: &str,
            idx: &AlpineIndex,
            resolved: &mut Vec<AlpinePackage>,
            seen: &mut HashSet<String>,
        ) -> Result<()> {
            if seen.contains(name) {
                return Ok(());
            }
            seen.insert(name.to_string());

            let pkg = idx
                .packages
                .get(name)
                .cloned()
                .with_context(|| format!("Alpine package not found: {}", name))?;

            for dep in &pkg.dependencies {
                if let Some(dep_name) = normalize_dep(dep, &idx.provides) {
                    if idx.packages.contains_key(&dep_name) {
                        dfs(&dep_name, idx, resolved, seen)?;
                    }
                }
            }

            resolved.push(pkg);
            Ok(())
        }

        dfs(name, idx, &mut resolved, &mut seen)?;
        Ok(resolved)
    }

    pub fn download_url(&self, pkg: &AlpinePackage) -> String {
        format!(
            "{}/{}/{}/{}/{}",
            self.mirror, self.branch, pkg.repo, self.arch, pkg.filename
        )
    }

    pub async fn download(&self, pkg: &AlpinePackage, cache_dir: &Path) -> Result<PathBuf> {
        let dest = cache_dir.join(&pkg.filename);
        if dest.exists() {
            return Ok(dest);
        }

        let url = self.download_url(pkg);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to download {}", url))?;
        if !resp.status().is_success() {
            anyhow::bail!("Failed to download {}: HTTP {}", url, resp.status());
        }

        tokio::fs::create_dir_all(cache_dir).await.ok();
        let bytes = resp.bytes().await?;
        tokio::fs::write(&dest, &bytes).await?;
        Ok(dest)
    }

    pub fn extract_data_tar_gz(apk_path: &Path, dest_dir: &Path) -> Result<()> {
        let file = std::fs::File::open(apk_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        enum DataKind {
            Gz,
            Zst,
        }

        let mut data: Vec<u8> = Vec::new();
        let mut kind: Option<DataKind> = None;
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            match path.to_string_lossy().as_ref() {
                "data.tar.gz" => {
                    entry.read_to_end(&mut data)?;
                    kind = Some(DataKind::Gz);
                    break;
                }
                "data.tar.zst" => {
                    entry.read_to_end(&mut data)?;
                    kind = Some(DataKind::Zst);
                    break;
                }
                _ => {}
            }
        }

        let Some(kind) = kind else {
            anyhow::bail!("Invalid apk (missing data.tar.*): {}", apk_path.display());
        };

        if data.is_empty() {
            anyhow::bail!("Invalid apk (empty data.tar.*): {}", apk_path.display());
        }

        std::fs::create_dir_all(dest_dir)?;

        match kind {
            DataKind::Gz => {
                let cursor = std::io::Cursor::new(data);
                let decoder = GzDecoder::new(cursor);
                let mut data_archive = Archive::new(decoder);
                data_archive.unpack(dest_dir)?;
            }
            DataKind::Zst => {
                let cursor = std::io::Cursor::new(data);
                let decoder =
                    zstd::stream::read::Decoder::new(cursor).context("zstd decode failed")?;
                let mut data_archive = Archive::new(decoder);
                data_archive.unpack(dest_dir)?;
            }
        }
        Ok(())
    }
}

fn parse_apkindex(
    repo: &str,
    text: &str,
    packages: &mut HashMap<String, AlpinePackage>,
    provides: &mut HashMap<String, String>,
) {
    for stanza in text.split("\n\n") {
        let mut name = None::<String>;
        let mut version = None::<String>;
        let mut desc = String::new();
        let mut license = None::<String>;
        let mut deps: Vec<String> = Vec::new();
        let mut download_size: u64 = 0;
        let mut installed_size: u64 = 0;
        let mut prov: Vec<String> = Vec::new();

        for line in stanza.lines() {
            if line.len() < 2 || !line.as_bytes().get(1).is_some_and(|b| *b == b':') {
                continue;
            }
            let key = &line[0..1];
            let val = line[2..].trim();
            match key {
                "P" => name = Some(val.to_string()),
                "V" => version = Some(val.to_string()),
                "T" => desc = val.to_string(),
                "L" => license = Some(val.to_string()),
                "D" => deps = val.split_whitespace().map(|s| s.to_string()).collect(),
                "S" => download_size = val.parse().unwrap_or(0),
                "I" => installed_size = val.parse().unwrap_or(0),
                "p" => prov = val.split_whitespace().map(|s| s.to_string()).collect(),
                _ => {}
            }
        }

        let (Some(name), Some(version)) = (name, version) else {
            continue;
        };

        let filename = format!("{}-{}.apk", name, version);
        let pkg = AlpinePackage {
            name: name.clone(),
            version,
            description: desc,
            license,
            dependencies: deps,
            download_size,
            installed_size,
            repo: repo.to_string(),
            filename,
        };

        for p in prov {
            provides.insert(p, name.clone());
        }
        packages.entry(name).or_insert(pkg);
    }
}

fn default_arch() -> String {
    match std::env::consts::ARCH {
        "x86_64" => "x86_64".to_string(),
        "aarch64" => "aarch64".to_string(),
        other => other.to_string(),
    }
}

fn add_http_fallback(mirrors: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for m in mirrors {
        let m = m.trim_end_matches('/').to_string();
        if m.is_empty() {
            continue;
        }
        out.push(m.clone());
        if m.starts_with("https://") {
            let http = m.replacen("https://", "http://", 1);
            if !out.contains(&http) {
                out.push(http);
            }
        }
    }
    out
}
