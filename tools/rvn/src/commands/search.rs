use anyhow::Result;
use colored::Colorize;

pub async fn run(query: &str, search_description: bool) -> Result<()> {
    println!(
        "{} Searching for '{}'...",
        "::".bright_blue(),
        query.bright_white()
    );

    if search_description {
        println!(
            "{} Including package descriptions in search",
            "::".bright_blue()
        );
    }

    // TODO: Search package database

    // Placeholder results
    let results = vec![
        ("rust", "1.75.0", "The Rust programming language"),
        ("rust-analyzer", "0.3.1800", "Rust language server"),
        ("rustfmt", "1.6.0", "Rust code formatter"),
    ];

    if results.is_empty() {
        println!("{}", "No packages found".yellow());
        return Ok(());
    }

    println!();
    for (name, version, description) in results {
        println!(
            "{}/{} {}",
            "raven".bright_blue(),
            name.bright_white(),
            version.bright_green()
        );
        println!("    {}", description.dimmed());
    }

    Ok(())
}
