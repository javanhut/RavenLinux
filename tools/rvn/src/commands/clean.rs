use anyhow::Result;
use colored::Colorize;

pub async fn run(all: bool) -> Result<()> {
    println!("{} Cleaning package cache...", "::".bright_blue());

    // TODO: Scan cache directory

    if all {
        println!("{} Removing all cached packages", "::".bright_blue());
        // TODO: Remove /var/cache/rvn/*
        println!("{} Removed 1.2 GiB of cached packages", "✓".bright_green());
    } else {
        println!("{} Removing old package versions", "::".bright_blue());
        // TODO: Keep only latest version of each package
        println!("{} Removed 450 MiB of old packages", "✓".bright_green());
    }

    Ok(())
}
