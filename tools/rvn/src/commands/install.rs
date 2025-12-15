//! Package installation command

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::PathBuf;
use std::{fs, path::Path};

use crate::aur::{AurClient, AurPackage};
use crate::config::Config;
use crate::database::Database;
use crate::package::archive::PackageArchive;
use crate::repository::client::MultiRepoClient;

pub async fn run(packages: &[String], _build_only: bool, dry_run: bool) -> Result<()> {
    if packages.is_empty() {
        println!("{}", "No packages specified".yellow());
        return Ok(());
    }

    // Load configuration
    let config = Config::load().unwrap_or_default();

    // Open package database
    let db = Database::open_default()?;

    // Create repository client (priority order)
    let mut repo_client = MultiRepoClient::new();
    let mut repos = config.repositories.clone();
    repos.sort_by_key(|r| r.priority);
    for repo in &repos {
        if repo.enabled {
            repo_client.add_repo(repo.name.clone(), repo.url.clone(), repo.repo_type.clone());
        }
    }
    repo_client.preload_indexes().await;

    // AUR fallback client (used when not found in configured repos)
    let aur_client = AurClient::with_config(config.aur.clone());

    println!(
        "{} Resolving dependencies for {} package(s)...",
        "::".bright_blue(),
        packages.len()
    );

    // Check what's already installed and what needs to be installed
    let mut to_install = Vec::new();
    let mut already_installed = Vec::new();

    for pkg_name in packages {
        if db.is_installed(pkg_name)? {
            already_installed.push(pkg_name.clone());
        } else {
            to_install.push(pkg_name.clone());
        }
    }

    // Show already installed packages
    if !already_installed.is_empty() {
        println!();
        println!("{} Already installed:", "::".bright_blue());
        for pkg in &already_installed {
            println!("   {} {}", pkg.bright_white(), "(installed)".bright_cyan());
        }
    }

    if to_install.is_empty() {
        println!();
        println!("{} All packages are already installed", "✓".bright_green());
        return Ok(());
    }

    // Resolve dependencies for packages to install
    let resolved = resolve_dependencies(&to_install, &db, &repo_client, &aur_client).await?;

    println!();
    println!(
        "{} Packages ({}):",
        "::".bright_blue(),
        resolved.len().to_string().bright_white()
    );

    let mut total_download_size = 0u64;
    let mut total_install_size = 0u64;

    for pkg in &resolved {
        let source_tag = match &pkg.source {
            PackageSource::Raven { .. } => "(raven)".bright_green(),
            PackageSource::Aur { .. } => "(aur)".bright_yellow(),
        };
        println!(
            "   {} {} {} {}",
            pkg.name.bright_white(),
            pkg.version.bright_cyan(),
            source_tag,
            if pkg.is_dependency {
                "(dependency)".dimmed()
            } else {
                "(explicit)".bright_green()
            }
        );
        total_download_size += pkg.download_size;
        total_install_size += pkg.install_size;
    }

    println!();
    println!(
        "   Download size: {}",
        format_size(total_download_size).bright_cyan()
    );
    println!(
        "   Installed size: {}",
        format_size(total_install_size).bright_cyan()
    );

    if dry_run {
        println!();
        println!("{}", "Dry run - no changes made".yellow());
        return Ok(());
    }

    // Confirm installation
    println!();
    if !confirm_action("Proceed with installation?")? {
        println!("{}", "Installation cancelled".yellow());
        return Ok(());
    }

    // Create cache directory
    let cache_dir = config.cache_dir();
    std::fs::create_dir_all(&cache_dir)?;

    // Download packages
    println!();
    println!("{} Downloading packages...", "::".bright_blue());

    let pb = ProgressBar::new(resolved.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("━━─"),
    );

    let mut downloaded_packages = Vec::new();

    for pkg in &resolved {
        pb.set_message(format!("Downloading {}", pkg.name));

        // Try to find and download the package
        let package_path = download_package(pkg, &cache_dir, &repo_client, &aur_client).await?;
        downloaded_packages.push((pkg.clone(), package_path));

        pb.inc(1);
    }
    pb.finish_with_message("Downloads complete");

    // Install packages
    println!();
    println!("{} Installing packages...", "::".bright_blue());

    let pb = ProgressBar::new(resolved.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("━━─"),
    );

    let root = PathBuf::from("/");

    for (pkg, package_path) in &downloaded_packages {
        pb.set_message(format!("Installing {}", pkg.name));

        // Extract and install package
        install_any_package(pkg, package_path, &root, &db, !pkg.is_dependency)?;

        pb.inc(1);
    }
    pb.finish_with_message("Installation complete");

    println!();
    println!(
        "{} Successfully installed {} package(s)",
        "✓".bright_green(),
        resolved.len()
    );

    Ok(())
}

/// Package to be installed
#[derive(Debug, Clone)]
struct PackageToInstall {
    name: String,
    version: String,
    description: String,
    download_size: u64,
    install_size: u64,
    is_dependency: bool,
    source: PackageSource,
}

#[derive(Debug, Clone)]
enum PackageSource {
    Raven { filename: String, sha256: String },
    Aur { pkg: AurPackage },
}

async fn resolve_dependencies(
    packages: &[String],
    db: &Database,
    repo_client: &MultiRepoClient,
    aur_client: &AurClient,
) -> Result<Vec<PackageToInstall>> {
    // First, get set of installed packages (sync operation)
    let installed: std::collections::HashSet<String> = db
        .list_installed()?
        .into_iter()
        .map(|(name, _, _)| name)
        .collect();

    let mut resolved = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for pkg_name in packages {
        resolve_package_async(
            pkg_name,
            false,
            &installed,
            repo_client,
            aur_client,
            &mut resolved,
            &mut seen,
        )
        .await?;
    }

    Ok(resolved)
}

async fn resolve_package_async(
    name: &str,
    is_dependency: bool,
    installed: &std::collections::HashSet<String>,
    repo_client: &MultiRepoClient,
    aur_client: &AurClient,
    resolved: &mut Vec<PackageToInstall>,
    seen: &mut std::collections::HashSet<String>,
) -> Result<()> {
    if seen.contains(name) || installed.contains(name) {
        return Ok(());
    }

    // First, try to find in Raven repositories
    if let Some((_repo, pkg)) = repo_client.find_package(name).await? {
        seen.insert(name.to_string());
        // Resolve dependencies first (use Box::pin for recursive async)
        for dep in &pkg.dependencies {
            Box::pin(resolve_package_async(
                dep,
                true,
                installed,
                repo_client,
                aur_client,
                resolved,
                seen,
            ))
            .await?;
        }

        // Add this package
        resolved.push(PackageToInstall {
            name: pkg.name,
            version: pkg.version,
            description: pkg.description,
            download_size: pkg.download_size,
            install_size: pkg.installed_size,
            is_dependency,
            source: PackageSource::Raven {
                filename: pkg.filename,
                sha256: pkg.sha256,
            },
        });
        return Ok(());
    }

    // Fallback: AUR (Arch User Repository)
    if aur_client.is_enabled() {
        match aur_client.find(name).await {
            Ok(Some(aur_pkg)) => {
                seen.insert(name.to_string());

                // Resolve AUR dependencies
                for dep in aur_pkg.all_dependencies() {
                    let dep_name = AurPackage::parse_dep_name(&dep);
                    Box::pin(resolve_package_async(
                        &dep_name,
                        true,
                        installed,
                        repo_client,
                        aur_client,
                        resolved,
                        seen,
                    ))
                    .await
                    .ok(); // Best effort for AUR deps - they might be in Raven or need mapping
                }

                resolved.push(PackageToInstall {
                    name: aur_pkg.name.clone(),
                    version: aur_pkg.version.clone(),
                    description: aur_pkg.description.clone().unwrap_or_default(),
                    download_size: aur_pkg.estimated_download_size(),
                    install_size: aur_pkg.estimated_install_size(),
                    is_dependency,
                    source: PackageSource::Aur { pkg: aur_pkg },
                });
                return Ok(());
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("Warning: AUR lookup failed for '{}': {}", name, e);
            }
        }
    }

    anyhow::bail!(
        "Package '{}' not found in Raven repos (theravenlinux.org) or AUR",
        name
    )
}

async fn download_package(
    pkg: &PackageToInstall,
    cache_dir: &PathBuf,
    repo_client: &MultiRepoClient,
    aur_client: &AurClient,
) -> Result<PathBuf> {
    match &pkg.source {
        PackageSource::Raven { filename, sha256 } => {
            let package_path = cache_dir.join(filename);

            // Check if already cached
            if package_path.exists() {
                let hash = crate::package::archive::hash_file(&package_path)?;
                if hash == *sha256 {
                    return Ok(package_path);
                }
                // Hash mismatch, redownload
                std::fs::remove_file(&package_path)?;
            }

            // Find package in repos and download
            if let Some((repo, repo_pkg)) = repo_client.find_package(&pkg.name).await? {
                repo.download_package(&repo_pkg, cache_dir, false).await?;
                return Ok(package_path);
            }

            anyhow::bail!("Failed to download package: {}", pkg.name)
        }
        PackageSource::Aur { pkg: aur_pkg } => {
            // Download and build AUR package
            let aur_cache = cache_dir.join("aur");
            std::fs::create_dir_all(&aur_cache)?;

            // Download source
            println!(
                "   {} Fetching AUR source for {}...",
                "->".bright_blue(),
                aur_pkg.name
            );
            let source_dir = aur_client.download_source(aur_pkg, &aur_cache).await?;

            // Build package
            let build_dir = cache_dir.join("aur-build");
            std::fs::create_dir_all(&build_dir)?;

            println!(
                "   {} Building {} from AUR...",
                "->".bright_blue(),
                aur_pkg.name
            );
            let built_pkg = aur_client
                .build_package(aur_pkg, &source_dir, &build_dir)
                .await?;

            Ok(built_pkg)
        }
    }
}

fn install_any_package(
    pkg: &PackageToInstall,
    downloaded: &PathBuf,
    root: &PathBuf,
    db: &Database,
    explicit: bool,
) -> Result<()> {
    match &pkg.source {
        PackageSource::Raven { .. } => install_rvn_package(downloaded, root, db, explicit),
        PackageSource::Aur { pkg: aur_pkg } => {
            // AUR packages are built into .rvn format, so same install process
            if downloaded.extension().map(|e| e == "rvn").unwrap_or(false) {
                install_rvn_package(downloaded, root, db, explicit)
            } else {
                // Fallback: extract built package directly
                let temp_dir = tempfile::tempdir()?;
                let status = std::process::Command::new("tar")
                    .args(["-xf", downloaded.to_str().unwrap(), "-C", temp_dir.path().to_str().unwrap()])
                    .status()?;

                if !status.success() {
                    anyhow::bail!("Failed to extract AUR package");
                }

                let installed_files = install_tree(temp_dir.path(), root)?;
                record_install(
                    db,
                    &aur_pkg.name,
                    &aur_pkg.version,
                    aur_pkg.description.as_deref().unwrap_or(""),
                    explicit,
                    &installed_files,
                )
            }
        }
    }
}

fn install_rvn_package(
    package_path: &PathBuf,
    root: &PathBuf,
    db: &Database,
    explicit: bool,
) -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let archive = PackageArchive::extract(package_path, temp_dir.path())?;
    let data_root = temp_dir.path().join("data");
    let installed_files = if data_root.exists() {
        install_tree(&data_root, root)?
    } else {
        install_tree(temp_dir.path(), root)?
    };
    record_install(
        db,
        &archive.metadata.name,
        &archive.metadata.version.to_string(),
        &archive.metadata.description,
        explicit,
        &installed_files,
    )
}

fn record_install(
    db: &Database,
    name: &str,
    version: &str,
    description: &str,
    explicit: bool,
    installed_files: &[String],
) -> Result<()> {
    let file_refs: Vec<&str> = installed_files.iter().map(|s| s.as_str()).collect();
    db.record_installation(name, version, Some(description), explicit, &file_refs)?;
    Ok(())
}

fn validate_relative_path(rel: &Path) -> Result<()> {
    if rel.is_absolute() {
        anyhow::bail!("Refusing to install absolute path: {}", rel.display());
    }
    for component in rel.components() {
        if matches!(component, std::path::Component::ParentDir) {
            anyhow::bail!("Refusing to install path with '..': {}", rel.display());
        }
    }
    Ok(())
}

fn install_tree(src_root: &Path, dst_root: &Path) -> Result<Vec<String>> {
    let mut installed_files = Vec::new();

    for entry in walkdir::WalkDir::new(src_root).follow_links(false) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src_root)?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        validate_relative_path(rel)?;

        let dest = dst_root.join(rel);
        let ft = entry.file_type();

        if ft.is_dir() {
            fs::create_dir_all(&dest)?;
            continue;
        }

        if ft.is_symlink() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let target = fs::read_link(entry.path())?;
            let _ = fs::remove_file(&dest);
            symlink(&target, &dest)?;
            installed_files.push(dest.to_string_lossy().to_string());
            continue;
        }

        if ft.is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &dest)?;
            let mode = fs::symlink_metadata(entry.path())?.permissions().mode();
            let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(mode));
            installed_files.push(dest.to_string_lossy().to_string());
        }
    }

    Ok(installed_files)
}

fn confirm_action(message: &str) -> Result<bool> {
    use dialoguer::Confirm;

    Confirm::new()
        .with_prompt(message)
        .default(true)
        .interact()
        .context("Failed to read user input")
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
