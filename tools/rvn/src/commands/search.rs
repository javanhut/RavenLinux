use anyhow::Result;
use colored::Colorize;

use rvn::aur::AurClient;

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

    let config = rvn::config::Config::load().unwrap_or_default();
    let db = rvn::database::Database::open_default()?;
    let results = db.search(query)?;

    let mut found_any = false;
    let has_local_results = !results.is_empty();

    // Show local results first
    if has_local_results {
        found_any = true;
        println!();
        for (name, version, description) in results {
            println!(
                "{}/{} {}",
                "installed".bright_magenta(),
                name.bright_white(),
                version.bright_green()
            );
            println!("    {}", description.dimmed());
        }
    }

    // Search remote repositories
    let mut repo_client = rvn::repository::client::MultiRepoClient::new();
    let mut repos = config.repositories.clone();
    repos.sort_by_key(|r| r.priority);
    for repo in &repos {
        if repo.enabled {
            repo_client.add_repo(repo.name.clone(), repo.url.clone(), repo.repo_type.clone());
        }
    }
    repo_client.preload_indexes().await;
    let remote = repo_client.search(query, search_description).await?;

    if !remote.is_empty() {
        found_any = true;
        if !has_local_results {
            println!();
        }
        for (repo, pkg) in &remote {
            println!(
                "{}/{} {}",
                repo.bright_blue(),
                pkg.name.bright_white(),
                pkg.version.bright_green()
            );
            println!("    {}", pkg.description.dimmed());
        }
    }

    // Search AUR as fallback
    let aur_client = AurClient::with_config(config.aur.clone());
    if aur_client.is_enabled() {
        match aur_client.search(query).await {
            Ok(aur_results) if !aur_results.is_empty() => {
                found_any = true;
                println!();
                println!("{} AUR results:", "::".bright_blue());
                for pkg in aur_results.iter().take(10) {
                    println!(
                        "{}/{} {}",
                        "aur".bright_yellow(),
                        pkg.name.bright_white(),
                        pkg.version.bright_green()
                    );
                    println!(
                        "    {}",
                        pkg.description.as_deref().unwrap_or("No description").dimmed()
                    );
                }
                if aur_results.len() > 10 {
                    println!(
                        "    {} ... and {} more AUR packages",
                        "->".bright_blue(),
                        aur_results.len() - 10
                    );
                }
            }
            Ok(_) => {} // No AUR results
            Err(e) => {
                eprintln!(
                    "{} AUR search failed: {}",
                    "::".bright_yellow(),
                    e
                );
            }
        }
    }

    if !found_any {
        println!("{}", "No packages found".yellow());
    }

    Ok(())
}
