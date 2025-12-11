//! Package manifest - tracks installed files and metadata

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Manifest of installed package files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub files: Vec<InstalledFile>,
    pub directories: Vec<PathBuf>,
    pub symlinks: Vec<InstalledSymlink>,
    pub config_files: HashSet<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledFile {
    pub path: PathBuf,
    pub sha256: String,
    pub mode: u32,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSymlink {
    pub path: PathBuf,
    pub target: PathBuf,
}

impl PackageManifest {
    pub fn new(name: String, version: String) -> Self {
        Self {
            name,
            version,
            files: Vec::new(),
            directories: Vec::new(),
            symlinks: Vec::new(),
            config_files: HashSet::new(),
        }
    }

    /// Load manifest from file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read manifest: {}", path.display()))?;
        serde_json::from_str(&content).context("Failed to parse manifest")
    }

    /// Save manifest to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write manifest: {}", path.display()))?;
        Ok(())
    }

    /// Add a file to the manifest
    pub fn add_file(&mut self, path: PathBuf, sha256: String, mode: u32, size: u64) {
        self.files.push(InstalledFile {
            path,
            sha256,
            mode,
            size,
        });
    }

    /// Add a symlink to the manifest
    pub fn add_symlink(&mut self, path: PathBuf, target: PathBuf) {
        self.symlinks.push(InstalledSymlink { path, target });
    }

    /// Add a directory to the manifest
    pub fn add_directory(&mut self, path: PathBuf) {
        self.directories.push(path);
    }

    /// Mark a file as a config file (won't be removed on uninstall if modified)
    pub fn mark_config(&mut self, path: PathBuf) {
        self.config_files.insert(path);
    }

    /// Get all paths (files + symlinks)
    pub fn all_paths(&self) -> Vec<&Path> {
        let mut paths: Vec<&Path> = self.files.iter().map(|f| f.path.as_path()).collect();
        paths.extend(self.symlinks.iter().map(|s| s.path.as_path()));
        paths
    }

    /// Total installed size
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}
