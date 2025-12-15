//! AUR HTTP client for fetching package information and sources

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{AurConfig, AurPackage, AurResponse};

/// Client for interacting with the AUR
#[derive(Debug, Clone)]
pub struct AurClient {
    http: reqwest::Client,
    config: AurConfig,
}

impl AurClient {
    /// Create a new AUR client with default configuration
    pub fn new() -> Self {
        Self::with_config(AurConfig::default())
    }

    /// Create a new AUR client with custom configuration
    pub fn with_config(config: AurConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("rvn/0.1.0 (ravenlinux-aur-compat)")
            .build()
            .expect("Failed to create HTTP client");

        Self { http, config }
    }

    /// Check if AUR fallback is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Search for packages in AUR
    pub async fn search(&self, query: &str) -> Result<Vec<AurPackage>> {
        let url = format!("{}?v=5&type=search&arg={}", self.config.rpc_url, query);

        let response: AurResponse = self
            .http
            .get(&url)
            .send()
            .await
            .context("Failed to query AUR")?
            .json()
            .await
            .context("Failed to parse AUR response")?;

        if let Some(error) = response.error {
            anyhow::bail!("AUR error: {}", error);
        }

        Ok(response.results)
    }

    /// Get detailed info about a specific package
    pub async fn info(&self, name: &str) -> Result<Option<AurPackage>> {
        let url = format!("{}?v=5&type=info&arg={}", self.config.rpc_url, name);

        let response: AurResponse = self
            .http
            .get(&url)
            .send()
            .await
            .context("Failed to query AUR")?
            .json()
            .await
            .context("Failed to parse AUR response")?;

        if let Some(error) = response.error {
            anyhow::bail!("AUR error: {}", error);
        }

        Ok(response.results.into_iter().next())
    }

    /// Get info for multiple packages at once
    pub async fn info_multi(&self, names: &[&str]) -> Result<Vec<AurPackage>> {
        if names.is_empty() {
            return Ok(Vec::new());
        }

        let args: String = names.iter().map(|n| format!("&arg[]={}", n)).collect();
        let url = format!("{}?v=5&type=info{}", self.config.rpc_url, args);

        let response: AurResponse = self
            .http
            .get(&url)
            .send()
            .await
            .context("Failed to query AUR")?
            .json()
            .await
            .context("Failed to parse AUR response")?;

        if let Some(error) = response.error {
            anyhow::bail!("AUR error: {}", error);
        }

        Ok(response.results)
    }

    /// Find a package by exact name
    pub async fn find(&self, name: &str) -> Result<Option<AurPackage>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let pkg = self.info(name).await?;

        // Check if out of date and we should skip
        if let Some(ref p) = pkg {
            if self.config.skip_out_of_date && p.out_of_date.is_some() {
                eprintln!(
                    "Warning: AUR package '{}' is flagged out of date, skipping",
                    name
                );
                return Ok(None);
            }
        }

        Ok(pkg)
    }

    /// Resolve package with all dependencies from AUR
    pub async fn resolve_with_deps(&self, name: &str) -> Result<Vec<AurPackage>> {
        let mut resolved = Vec::new();
        let mut seen = HashSet::new();
        let mut queue = vec![name.to_string()];

        while let Some(pkg_name) = queue.pop() {
            if seen.contains(&pkg_name) {
                continue;
            }

            if let Some(pkg) = self.info(&pkg_name).await? {
                seen.insert(pkg_name.clone());

                // Add dependencies to queue
                for dep in pkg.all_dependencies() {
                    let dep_name = AurPackage::parse_dep_name(&dep);
                    if !seen.contains(&dep_name) {
                        queue.push(dep_name);
                    }
                }

                resolved.push(pkg);
            }
        }

        // Reverse so dependencies come first
        resolved.reverse();
        Ok(resolved)
    }

    /// Download package source (git clone or snapshot)
    pub async fn download_source(&self, pkg: &AurPackage, dest_dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dest_dir)?;

        let pkg_dir = dest_dir.join(&pkg.package_base);

        // If already exists, try to update
        if pkg_dir.exists() {
            let status = Command::new("git")
                .args(["pull", "--ff-only"])
                .current_dir(&pkg_dir)
                .status();

            if status.is_ok() && status.unwrap().success() {
                return Ok(pkg_dir);
            }
            // If pull failed, remove and re-clone
            std::fs::remove_dir_all(&pkg_dir)?;
        }

        // Clone the repository
        let git_url = pkg.git_url(&self.config.base_url);
        let status = Command::new("git")
            .args(["clone", "--depth=1", &git_url, pkg_dir.to_str().unwrap()])
            .status()
            .context("Failed to run git clone")?;

        if !status.success() {
            // Fallback to snapshot download
            return self.download_snapshot(pkg, dest_dir).await;
        }

        Ok(pkg_dir)
    }

    /// Download package snapshot tarball
    async fn download_snapshot(&self, pkg: &AurPackage, dest_dir: &Path) -> Result<PathBuf> {
        let snapshot_url = pkg.snapshot_url(&self.config.base_url);
        let filename = format!("{}.tar.gz", pkg.package_base);
        let tarball_path = dest_dir.join(&filename);

        let response = self
            .http
            .get(&snapshot_url)
            .send()
            .await
            .context("Failed to download AUR snapshot")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to download AUR snapshot: HTTP {}",
                response.status()
            );
        }

        let bytes = response.bytes().await?;
        tokio::fs::write(&tarball_path, &bytes).await?;

        // Extract tarball
        let pkg_dir = dest_dir.join(&pkg.package_base);
        std::fs::create_dir_all(&pkg_dir)?;

        let status = Command::new("tar")
            .args([
                "-xf",
                tarball_path.to_str().unwrap(),
                "-C",
                dest_dir.to_str().unwrap(),
            ])
            .status()
            .context("Failed to extract AUR snapshot")?;

        if !status.success() {
            anyhow::bail!("Failed to extract AUR snapshot tarball");
        }

        // Clean up tarball
        std::fs::remove_file(&tarball_path)?;

        Ok(pkg_dir)
    }

    /// Build an AUR package for RavenLinux
    pub async fn build_package(
        &self,
        pkg: &AurPackage,
        source_dir: &Path,
        output_dir: &Path,
    ) -> Result<PathBuf> {
        std::fs::create_dir_all(output_dir)?;

        let pkgbuild_path = source_dir.join("PKGBUILD");
        if !pkgbuild_path.exists() {
            anyhow::bail!("PKGBUILD not found in {}", source_dir.display());
        }

        // Parse PKGBUILD
        let pkgbuild = super::pkgbuild::PkgBuild::parse(&pkgbuild_path)?;

        // Create build script that adapts PKGBUILD for RavenLinux
        let build_script = self.generate_build_script(&pkgbuild, source_dir)?;

        let script_path = source_dir.join("rvn-build.sh");
        std::fs::write(&script_path, &build_script)?;

        // Run build
        let status = Command::new("bash")
            .arg(&script_path)
            .current_dir(source_dir)
            .env("PKGDEST", output_dir)
            .env("MAKEFLAGS", format!("-j{}", num_cpus::get()))
            .status()
            .context("Failed to run build script")?;

        if !status.success() {
            anyhow::bail!("AUR package build failed for {}", pkg.name);
        }

        // Find the built package
        let rvn_file = output_dir.join(format!("{}-{}.rvn", pkg.name, pkg.version));
        if rvn_file.exists() {
            Ok(rvn_file)
        } else {
            // Look for any .rvn file
            for entry in std::fs::read_dir(output_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|e| e == "rvn").unwrap_or(false) {
                    return Ok(path);
                }
            }
            anyhow::bail!("No .rvn package found after build");
        }
    }

    /// Generate a build script that adapts PKGBUILD for RavenLinux
    fn generate_build_script(
        &self,
        pkgbuild: &super::pkgbuild::PkgBuild,
        source_dir: &Path,
    ) -> Result<String> {
        let script = format!(
            r#"#!/bin/bash
set -e

# RavenLinux AUR Build Script
# Generated for: {name} {version}

export CFLAGS="${{CFLAGS:--O2 -pipe}}"
export CXXFLAGS="${{CXXFLAGS:--O2 -pipe}}"
export LDFLAGS="${{LDFLAGS:-}}"
export MAKEFLAGS="${{MAKEFLAGS:--j$(nproc)}}"

srcdir="{source_dir}"
pkgdir="${{PKGDEST:-/tmp/rvn-pkg}}/{name}"

mkdir -p "$pkgdir"
cd "$srcdir"

# Source the PKGBUILD
source ./PKGBUILD

# Run prepare if exists
if declare -f prepare >/dev/null; then
    echo "==> Running prepare()..."
    prepare
fi

# Run build if exists
if declare -f build >/dev/null; then
    echo "==> Running build()..."
    build
fi

# Run package
if declare -f package >/dev/null; then
    echo "==> Running package()..."
    package
elif declare -f package_{name} >/dev/null; then
    echo "==> Running package_{name}()..."
    package_{name}
fi

# Create .rvn package
echo "==> Creating .rvn package..."
cd "$pkgdir"

# Create metadata
cat > metadata.json << 'METADATA_EOF'
{{
    "name": "{name}",
    "version": "{version}",
    "description": "{description}",
    "license": "{license}",
    "source": "aur"
}}
METADATA_EOF

# Create manifest
find . -type f -printf '%P\n' | sort > manifest.txt

# Create package archive
mkdir -p "${{PKGDEST}}"
tar -czf "${{PKGDEST}}/{name}-{version}.rvn" .

echo "==> Package created: ${{PKGDEST}}/{name}-{version}.rvn"
"#,
            name = pkgbuild.name,
            version = pkgbuild.version,
            description = pkgbuild.description.as_deref().unwrap_or(""),
            license = pkgbuild.license.first().map(|s| s.as_str()).unwrap_or("unknown"),
            source_dir = source_dir.display(),
        );

        Ok(script)
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.config.cache_dir
    }

    /// Get build directory
    pub fn build_dir(&self) -> &Path {
        &self.config.build_dir
    }
}

impl Default for AurClient {
    fn default() -> Self {
        Self::new()
    }
}
