pub mod archive;
pub mod definition;
pub mod manifest;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Package identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageId {
    pub name: String,
    pub version: semver::Version,
}

impl std::fmt::Display for PackageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.name, self.version)
    }
}

/// Package metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: semver::Version,
    pub description: String,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub maintainers: Vec<String>,
    pub categories: Vec<String>,
}

/// Build system type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BuildSystem {
    #[default]
    Make,
    CMake,
    Meson,
    Cargo,
    Autotools,
    Custom,
}

/// Build configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildConfig {
    pub system: BuildSystem,
    pub configure_flags: Vec<String>,
    pub build_flags: Vec<String>,
    pub install_flags: Vec<String>,
    pub env: HashMap<String, String>,
    pub pre_build: Vec<String>,
    pub post_build: Vec<String>,
    pub pre_install: Vec<String>,
    pub post_install: Vec<String>,
}

/// Package source
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PackageSource {
    Tarball {
        url: String,
        sha256: String,
    },
    Git {
        url: String,
        #[serde(rename = "ref")]
        git_ref: Option<String>,
        commit: Option<String>,
    },
    Local {
        path: PathBuf,
    },
}

/// Dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Package dependencies
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Dependencies {
    #[serde(default)]
    pub runtime: Vec<Dependency>,
    #[serde(default)]
    pub build: Vec<Dependency>,
    #[serde(default)]
    pub optional: Vec<Dependency>,
}

/// Complete package definition (from package.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDefinition {
    pub package: PackageMetadata,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub dependencies: Dependencies,
    pub source: PackageSource,
    #[serde(default)]
    pub install: InstallConfig,
}

/// Installation configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallConfig {
    #[serde(default)]
    pub files: Vec<FileMapping>,
    #[serde(default)]
    pub directories: Vec<String>,
    #[serde(default)]
    pub symlinks: Vec<SymlinkMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMapping {
    pub src: String,
    pub dest: String,
    #[serde(default)]
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymlinkMapping {
    pub target: String,
    pub link: String,
}

/// Installed package record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub id: PackageId,
    pub metadata: PackageMetadata,
    pub files: Vec<PathBuf>,
    pub install_date: chrono::DateTime<chrono::Utc>,
    pub explicit: bool,
    pub build_info: Option<BuildInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    pub build_date: chrono::DateTime<chrono::Utc>,
    pub build_host: String,
    pub compiler: String,
    pub flags: Vec<String>,
}

impl PackageDefinition {
    pub fn id(&self) -> PackageId {
        PackageId {
            name: self.package.name.clone(),
            version: self.package.version.clone(),
        }
    }
}
