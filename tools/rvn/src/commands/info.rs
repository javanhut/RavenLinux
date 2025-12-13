use anyhow::Result;
use colored::Colorize;

pub async fn run(package: &str, show_versions: bool) -> Result<()> {
    println!(
        "{} Fetching info for '{}'...",
        "::".bright_blue(),
        package.bright_white()
    );

    // TODO: Lookup package in database

    // Placeholder info
    println!();
    println!("{}: {}", "Name".bright_white(), package);
    println!("{}: {}", "Version".bright_white(), "1.75.0");
    println!(
        "{}: {}",
        "Description".bright_white(),
        "The Rust programming language"
    );
    println!("{}: {}", "License".bright_white(), "Apache-2.0 / MIT");
    println!(
        "{}: {}",
        "Homepage".bright_white(),
        "https://www.rust-lang.org/"
    );
    println!(
        "{}: {}",
        "Repository".bright_white(),
        "https://github.com/rust-lang/rust"
    );
    println!("{}: {}", "Installed Size".bright_white(), "850 MiB");
    println!("{}: {}", "Download Size".bright_white(), "125 MiB");
    println!();
    println!(
        "{}: {}",
        "Dependencies".bright_white(),
        "libc, llvm, openssl"
    );
    println!(
        "{}: {}",
        "Build Deps".bright_white(),
        "cmake, python, ninja"
    );
    println!();
    println!(
        "{}: {}",
        "Status".bright_white(),
        "Installed".bright_green()
    );
    println!(
        "{}: {}",
        "Install Date".bright_white(),
        "2025-12-01 14:30:00"
    );
    println!(
        "{}: {}",
        "Install Reason".bright_white(),
        "Explicitly installed"
    );

    if show_versions {
        println!();
        println!("{}", "Available Versions:".bright_white());
        println!("  {} {}", "1.75.0".bright_green(), "(installed)");
        println!("  {}", "1.74.1");
        println!("  {}", "1.74.0");
        println!("  {}", "1.73.0");
    }

    Ok(())
}
