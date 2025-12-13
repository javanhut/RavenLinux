use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};

use crate::package::archive::hash_file;
use crate::package::manifest::PackageManifest;
use crate::package::PackageMetadata;
use crate::repository::{RepoIndex, RepoPackage};

pub async fn index(repo_root: &Path, name: Option<&str>, output: Option<&Path>) -> Result<()> {
    if let Some(s) = repo_root.to_str() {
        if s.contains("://") {
            anyhow::bail!(
                "repo index expects a local directory path (not a URL). Run this on the machine that has the repo files (expects <repo_root>/packages/*.rvn), then upload the generated index.json."
            );
        }
    }

    let packages_dir = repo_root.join("packages");
    if !packages_dir.is_dir() {
        anyhow::bail!(
            "Expected packages directory at {}",
            packages_dir.display()
        );
    }

    let repo_name = name
        .map(str::to_string)
        .or_else(|| {
            repo_root
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "raven".to_string());

    let output_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("index.json"));

    let mut packages = Vec::new();
    for entry in std::fs::read_dir(&packages_dir)
        .with_context(|| format!("Failed to read {}", packages_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rvn") {
            continue;
        }
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .context("Package path missing filename")?;

        let (metadata, manifest) = read_metadata_and_manifest(&path)?;
        let download_size = std::fs::metadata(&path)?.len();
        let installed_size = manifest.total_size();
        let sha256 = hash_file(&path)?;

        packages.push(RepoPackage {
            name: metadata.name,
            version: metadata.version.to_string(),
            description: metadata.description,
            license: metadata.license,
            dependencies: Vec::new(),
            build_deps: Vec::new(),
            download_size,
            installed_size,
            filename,
            sha256,
        });
    }

    packages.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));

    let index = RepoIndex {
        name: repo_name,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        packages,
    };

    let json = serde_json::to_string_pretty(&index)?;
    std::fs::write(&output_path, json)
        .with_context(|| format!("Failed to write {}", output_path.display()))?;

    println!(
        "{} Wrote repo index: {}",
        "✓".bright_green(),
        output_path.display().to_string().bright_white()
    );
    Ok(())
}

fn read_metadata_and_manifest(path: &Path) -> Result<(PackageMetadata, PackageManifest)> {
    use flate2::read::GzDecoder;
    use std::fs::File;
    use std::io::Read;
    use tar::Archive;

    let file = File::open(path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    let mut metadata: Option<PackageMetadata> = None;
    let mut manifest: Option<PackageManifest> = None;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?;
        match entry_path.to_string_lossy().as_ref() {
            "metadata.json" => {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                metadata = Some(serde_json::from_str(&content)?);
            }
            "manifest.json" => {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                manifest = Some(serde_json::from_str(&content)?);
            }
            _ => {}
        }
        if metadata.is_some() && manifest.is_some() {
            break;
        }
    }

    Ok((
        metadata.context("Package missing metadata.json")?,
        manifest.context("Package missing manifest.json")?,
    ))
}
