//! AUR (Arch User Repository) compatibility layer for RavenLinux
//!
//! This module provides the ability to fetch, parse, and build packages from the AUR,
//! cross-compiling them for RavenLinux when they don't exist in the official Raven repos.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod client;
pub mod pkgbuild;

pub use client::AurClient;
pub use pkgbuild::PkgBuild;

/// AUR package metadata from the RPC API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AurPackage {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "URL")]
    pub url: Option<String>,
    #[serde(rename = "License")]
    pub license: Option<Vec<String>>,
    #[serde(rename = "Maintainer")]
    pub maintainer: Option<String>,
    #[serde(rename = "NumVotes")]
    pub num_votes: Option<i32>,
    #[serde(rename = "Popularity")]
    pub popularity: Option<f64>,
    #[serde(rename = "OutOfDate")]
    pub out_of_date: Option<i64>,
    #[serde(rename = "PackageBase")]
    pub package_base: String,
    #[serde(rename = "URLPath")]
    pub url_path: String,
    #[serde(rename = "Depends")]
    pub depends: Option<Vec<String>>,
    #[serde(rename = "MakeDepends")]
    pub makedepends: Option<Vec<String>>,
    #[serde(rename = "OptDepends")]
    pub optdepends: Option<Vec<String>>,
    #[serde(rename = "CheckDepends")]
    pub checkdepends: Option<Vec<String>>,
    #[serde(rename = "Provides")]
    pub provides: Option<Vec<String>>,
    #[serde(rename = "Conflicts")]
    pub conflicts: Option<Vec<String>>,
    #[serde(rename = "Replaces")]
    pub replaces: Option<Vec<String>>,
}

/// AUR RPC API response
#[derive(Debug, Clone, Deserialize)]
pub struct AurResponse {
    pub version: i32,
    #[serde(rename = "type")]
    pub response_type: String,
    pub resultcount: i32,
    pub results: Vec<AurPackage>,
    pub error: Option<String>,
}

/// Build result from compiling an AUR package
#[derive(Debug, Clone)]
pub struct AurBuildResult {
    pub name: String,
    pub version: String,
    pub package_path: PathBuf,
    pub installed_files: Vec<String>,
    pub install_size: u64,
}

/// Configuration for AUR integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AurConfig {
    /// Enable AUR fallback when package not found in Raven repos
    pub enabled: bool,
    /// AUR base URL
    pub base_url: String,
    /// AUR RPC API URL
    pub rpc_url: String,
    /// Cache directory for PKGBUILD and source files
    pub cache_dir: PathBuf,
    /// Build directory for compiling packages
    pub build_dir: PathBuf,
    /// Whether to clean build directory after successful build
    pub clean_build: bool,
    /// Skip packages that have been flagged out of date
    pub skip_out_of_date: bool,
}

impl Default for AurConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: "https://aur.archlinux.org".to_string(),
            rpc_url: "https://aur.archlinux.org/rpc/".to_string(),
            cache_dir: PathBuf::from("/var/cache/rvn/aur"),
            build_dir: PathBuf::from("/tmp/rvn-aur-build"),
            clean_build: true,
            skip_out_of_date: false,
        }
    }
}

impl AurPackage {
    /// Get the git clone URL for this package
    pub fn git_url(&self, base_url: &str) -> String {
        format!("{}/{}.git", base_url, self.package_base)
    }

    /// Get the snapshot/tarball URL
    pub fn snapshot_url(&self, base_url: &str) -> String {
        format!("{}{}", base_url, self.url_path)
    }

    /// Get all dependencies (runtime + make)
    pub fn all_dependencies(&self) -> Vec<String> {
        let mut deps = Vec::new();
        if let Some(d) = &self.depends {
            deps.extend(d.iter().cloned());
        }
        if let Some(d) = &self.makedepends {
            deps.extend(d.iter().cloned());
        }
        deps
    }

    /// Parse dependency string to extract package name (strips version constraints)
    pub fn parse_dep_name(dep: &str) -> String {
        dep.split(|c: char| c == '<' || c == '>' || c == '=' || c == ':')
            .next()
            .unwrap_or(dep)
            .trim()
            .to_string()
    }

    /// Estimated download size (AUR doesn't provide this, so we estimate)
    pub fn estimated_download_size(&self) -> u64 {
        // Average source tarball size estimate
        5 * 1024 * 1024 // 5 MiB default estimate
    }

    /// Estimated install size (rough estimate based on package type)
    pub fn estimated_install_size(&self) -> u64 {
        // Rough estimate - actual size determined after build
        10 * 1024 * 1024 // 10 MiB default estimate
    }
}
