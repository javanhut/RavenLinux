pub mod client;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIndex {
    pub name: String,
    pub timestamp: u64,
    pub packages: Vec<RepoPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub license: Option<String>,
    pub dependencies: Vec<String>,
    pub build_deps: Vec<String>,
    pub download_size: u64,
    pub installed_size: u64,
    pub filename: String,
    pub sha256: String,
}
