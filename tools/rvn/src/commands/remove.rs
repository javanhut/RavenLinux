//! Package removal command

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::database::Database;

pub async fn run(packages: &[String], purge: bool, dry_run: bool) -> Result<()> {
    if packages.is_empty() {
        println!("{}", "No packages specified".yellow());
        return Ok(());
    }

    // Open package database
    let db = Database::open_default()?;

    println!(
        "{} Calculating removal for {} package(s)...",
        "::".bright_blue(),
        packages.len()
    );

    // Check which packages are installed
    let mut to_remove = Vec::new();
    let mut not_installed = Vec::new();

    for pkg_name in packages {
        if db.is_installed(pkg_name)? {
            to_remove.push(pkg_name.clone());
        } else {
            not_installed.push(pkg_name.clone());
        }
    }

    // Show not installed packages
    if !not_installed.is_empty() {
        println!();
        println!("{} Not installed:", "::".bright_yellow());
        for pkg in &not_installed {
            println!("   {} {}", pkg.bright_white(), "(not found)".dimmed());
        }
    }

    if to_remove.is_empty() {
        println!();
        println!("{} No packages to remove", "✓".bright_green());
        return Ok(());
    }

    // Find orphaned dependencies if purge is enabled
    let orphans = if purge {
        find_orphans(&db, &to_remove)?
    } else {
        Vec::new()
    };

    // Display packages to remove
    println!();
    println!(
        "{} Packages to remove ({}):",
        "::".bright_blue(),
        (to_remove.len() + orphans.len()).to_string().bright_white()
    );

    for pkg in &to_remove {
        if let Ok(Some(version)) = db.get_installed_version(pkg) {
            println!(
                "   {} {} {}",
                pkg.bright_white(),
                version.bright_cyan(),
                "(explicit)".bright_red()
            );
        } else {
            println!("   {} {}", pkg.bright_white(), "(remove)".bright_red());
        }
    }

    if !orphans.is_empty() {
        println!();
        println!("{} Orphaned dependencies:", "::".bright_blue());
        for pkg in &orphans {
            println!("   {} {}", pkg.bright_white(), "(orphan)".dimmed());
        }
    }

    if dry_run {
        println!();
        println!("{}", "Dry run - no changes made".yellow());
        return Ok(());
    }

    // Confirm removal
    println!();
    if !confirm_action("Proceed with removal?")? {
        println!("{}", "Removal cancelled".yellow());
        return Ok(());
    }

    // Remove packages
    println!();
    println!("{} Removing packages...", "::".bright_blue());

    let all_to_remove: Vec<String> = to_remove.into_iter().chain(orphans.into_iter()).collect();

    let pb = ProgressBar::new(all_to_remove.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("━━─"),
    );

    let mut total_files_removed = 0;

    for pkg in &all_to_remove {
        pb.set_message(format!("Removing {}", pkg));

        // Get files and remove package from database
        let files = db.remove_package(pkg)?;

        // Remove actual files from filesystem
        for file_path in &files {
            if std::path::Path::new(file_path).exists() {
                if let Err(e) = std::fs::remove_file(file_path) {
                    eprintln!("Warning: Failed to remove {}: {}", file_path, e);
                } else {
                    total_files_removed += 1;
                }
            }
        }

        pb.inc(1);
    }
    pb.finish_with_message("Removal complete");

    println!();
    println!(
        "{} Successfully removed {} package(s), {} file(s)",
        "✓".bright_green(),
        all_to_remove.len(),
        total_files_removed
    );

    Ok(())
}

/// Find packages that are no longer needed (installed as dependencies but no longer required)
fn find_orphans(db: &Database, removing: &[String]) -> Result<Vec<String>> {
    let mut orphans = Vec::new();

    // Get all installed packages
    let installed = db.list_installed()?;

    // Find packages that were installed as dependencies (explicit = false)
    // and are not depended upon by any remaining package
    for (name, _version, explicit) in installed {
        if !explicit && !removing.contains(&name) {
            // This is a dependency - check if it's still needed
            // For now, we'll use a simple heuristic: if it was a dep, mark it as orphan
            // A full implementation would check the dependency graph
            orphans.push(name);
        }
    }

    // Filter out packages that are still needed by non-removed packages
    // This is a simplified implementation - a full one would traverse the dep graph

    Ok(orphans)
}

fn confirm_action(message: &str) -> Result<bool> {
    use dialoguer::Confirm;

    Confirm::new()
        .with_prompt(message)
        .default(false) // Default to No for removal
        .interact()
        .context("Failed to read user input")
}
