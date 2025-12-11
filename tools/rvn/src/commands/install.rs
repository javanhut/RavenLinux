//! Package installation command

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

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

    // Create repository client
    let mut repo_client = MultiRepoClient::new();
    for repo in &config.repositories {
        if repo.enabled {
            repo_client.add_repo(repo.name.clone(), repo.url.clone());
        }
    }

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
    let resolved = resolve_dependencies(&to_install, &db, &repo_client).await?;

    println!();
    println!(
        "{} Packages ({}):",
        "::".bright_blue(),
        resolved.len().to_string().bright_white()
    );

    let mut total_download_size = 0u64;
    let mut total_install_size = 0u64;

    for pkg in &resolved {
        println!(
            "   {} {} {}",
            pkg.name.bright_white(),
            pkg.version.bright_cyan(),
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
        let package_path = download_package(pkg, &cache_dir, &repo_client).await?;
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
        install_package(package_path, &root, &db, !pkg.is_dependency)?;

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
    download_size: u64,
    install_size: u64,
    is_dependency: bool,
    filename: String,
    sha256: String,
}

async fn resolve_dependencies(
    packages: &[String],
    db: &Database,
    repo_client: &MultiRepoClient,
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
        resolve_package_async(pkg_name, false, &installed, repo_client, &mut resolved, &mut seen).await?;
    }

    Ok(resolved)
}

async fn resolve_package_async(
    name: &str,
    is_dependency: bool,
    installed: &std::collections::HashSet<String>,
    repo_client: &MultiRepoClient,
    resolved: &mut Vec<PackageToInstall>,
    seen: &mut std::collections::HashSet<String>,
) -> Result<()> {
    if seen.contains(name) || installed.contains(name) {
        return Ok(());
    }
    seen.insert(name.to_string());

    // Find package in repositories
    if let Some((_repo, pkg)) = repo_client.find_package(name).await? {
        // Resolve dependencies first (use Box::pin for recursive async)
        for dep in &pkg.dependencies {
            Box::pin(resolve_package_async(dep, true, installed, repo_client, resolved, seen)).await?;
        }

        // Add this package
        resolved.push(PackageToInstall {
            name: pkg.name,
            version: pkg.version,
            download_size: pkg.download_size,
            install_size: pkg.installed_size,
            is_dependency,
            filename: pkg.filename,
            sha256: pkg.sha256,
        });
    } else {
        anyhow::bail!("Package '{}' not found in any repository", name);
    }

    Ok(())
}

async fn download_package(
    pkg: &PackageToInstall,
    cache_dir: &PathBuf,
    repo_client: &MultiRepoClient,
) -> Result<PathBuf> {
    let package_path = cache_dir.join(&pkg.filename);

    // Check if already cached
    if package_path.exists() {
        let hash = crate::package::archive::hash_file(&package_path)?;
        if hash == pkg.sha256 {
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

fn install_package(
    package_path: &PathBuf,
    root: &PathBuf,
    db: &Database,
    explicit: bool,
) -> Result<()> {
    // Extract package to a temp directory
    let temp_dir = tempfile::tempdir()?;
    let archive = PackageArchive::extract(package_path, temp_dir.path())?;

    // Copy files to destination
    let mut installed_files = Vec::new();

    for entry in walkdir::WalkDir::new(temp_dir.path()) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let relative = entry.path().strip_prefix(temp_dir.path())?;
            let dest = root.join(relative);

            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::copy(entry.path(), &dest)?;
            installed_files.push(dest.to_string_lossy().to_string());
        }
    }

    // Record installation in database
    let file_refs: Vec<&str> = installed_files.iter().map(|s| s.as_str()).collect();
    db.record_installation(
        &archive.metadata.name,
        &archive.metadata.version.to_string(),
        Some(&archive.metadata.description),
        explicit,
        &file_refs,
    )?;

    Ok(())
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
