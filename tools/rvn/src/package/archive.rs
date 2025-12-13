//! Package archive creation and extraction (.rvn format)

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tar::{Archive, Builder};

use super::manifest::PackageManifest;
use super::PackageMetadata;

/// RVN package archive format
/// Structure:
/// - metadata.json (package metadata)
/// - manifest.json (file listing)
/// - data.tar.gz (actual files)
pub struct PackageArchive {
    pub metadata: PackageMetadata,
    pub manifest: PackageManifest,
}

impl PackageArchive {
    /// Create a new package archive
    pub fn new(metadata: PackageMetadata, manifest: PackageManifest) -> Self {
        Self { metadata, manifest }
    }

    /// Create a .rvn package file
    pub fn create(&self, source_dir: &Path, output_path: &Path) -> Result<()> {
        let file = File::create(output_path)
            .with_context(|| format!("Failed to create package: {}", output_path.display()))?;
        let encoder = GzEncoder::new(file, Compression::best());
        let mut tar = Builder::new(encoder);

        // Add metadata.json
        let metadata_json = serde_json::to_string_pretty(&self.metadata)?;
        let metadata_bytes = metadata_json.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path("metadata.json")?;
        header.set_size(metadata_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, metadata_bytes)?;

        // Add manifest.json
        let manifest_json = serde_json::to_string_pretty(&self.manifest)?;
        let manifest_bytes = manifest_json.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path("manifest.json")?;
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, manifest_bytes)?;

        // Add data directory
        tar.append_dir_all("data", source_dir)?;

        tar.finish()?;
        tar.into_inner()?.finish()?;
        Ok(())
    }

    /// Extract a .rvn package to a directory
    pub fn extract(archive_path: &Path, dest_dir: &Path) -> Result<Self> {
        let file = File::open(archive_path)
            .with_context(|| format!("Failed to open package: {}", archive_path.display()))?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        let mut metadata: Option<PackageMetadata> = None;
        let mut manifest: Option<PackageManifest> = None;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            let path_str = path.to_string_lossy().to_string();

            if path_str == "metadata.json" {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                metadata = Some(serde_json::from_str(&content)?);
            } else if path_str == "manifest.json" {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                manifest = Some(serde_json::from_str(&content)?);
            } else if path_str == "data" || path_str.starts_with("data/") {
                // Keep the `data/` prefix on-disk while unpacking so tar hardlinks/symlinks
                // inside the data tree remain valid. Callers can then install from `dest_dir/data`.
                entry.unpack_in(dest_dir)
                    .with_context(|| format!("Failed to unpack {}", path_str))?;
            }
        }

        Ok(Self {
            metadata: metadata.context("Package missing metadata.json")?,
            manifest: manifest.context("Package missing manifest.json")?,
        })
    }

    /// Get package info without full extraction
    pub fn info(archive_path: &Path) -> Result<PackageMetadata> {
        let file = File::open(archive_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            if path.to_string_lossy() == "metadata.json" {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                return Ok(serde_json::from_str(&content)?);
            }
        }

        anyhow::bail!("Package missing metadata.json")
    }
}

/// Calculate SHA256 hash of a file
pub fn hash_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};

    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn extract_preserves_hardlinks_inside_data_tree() -> Result<()> {
        let metadata = PackageMetadata {
            name: "git".to_string(),
            version: semver::Version::parse("1.0.0")?,
            description: "test".to_string(),
            license: None,
            homepage: None,
            repository: None,
            maintainers: Vec::new(),
            categories: Vec::new(),
        };
        let manifest = PackageManifest::new("git".to_string(), "1.0.0".to_string());
        let out_dir = tempfile::tempdir()?;
        let archive_path = out_dir.path().join("git.rvn");

        // Build a minimal .rvn archive that contains a tar hardlink inside `data/`.
        let file = File::create(&archive_path)?;
        let encoder = GzEncoder::new(file, Compression::best());
        let mut tar = Builder::new(encoder);

        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        let mut header = tar::Header::new_gnu();
        header.set_path("metadata.json")?;
        header.set_size(metadata_json.as_bytes().len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, metadata_json.as_bytes())?;

        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        let mut header = tar::Header::new_gnu();
        header.set_path("manifest.json")?;
        header.set_size(manifest_json.as_bytes().len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, manifest_json.as_bytes())?;

        for dir in ["data", "data/usr", "data/usr/bin"] {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
            header.set_path(dir)?;
            header.set_mode(0o755);
            header.set_size(0);
            header.set_cksum();
            tar.append(&header, std::io::empty())?;
        }

        let mut header = tar::Header::new_gnu();
        header.set_path("data/usr/bin/git-upload-archive")?;
        header.set_mode(0o755);
        header.set_size(4);
        header.set_cksum();
        tar.append(&header, b"test".as_slice())?;

        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Link);
        header.set_path("data/usr/bin/git")?;
        header.set_link_name("data/usr/bin/git-upload-archive")?;
        header.set_mode(0o755);
        header.set_size(0);
        header.set_cksum();
        tar.append(&header, std::io::empty())?;

        tar.finish()?;
        tar.into_inner()?.finish()?;

        let extract_dir = tempfile::tempdir()?;
        PackageArchive::extract(&archive_path, extract_dir.path())?;

        let extracted_git = extract_dir.path().join("data/usr/bin/git");
        let extracted_upload_archive = extract_dir.path().join("data/usr/bin/git-upload-archive");

        let git_meta = std::fs::metadata(&extracted_git)?;
        let upload_meta = std::fs::metadata(&extracted_upload_archive)?;
        assert_eq!(git_meta.ino(), upload_meta.ino(), "files should be hardlinks");

        Ok(())
    }
}
