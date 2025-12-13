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

    let db = crate::database::Database::open_default()?;
    let results = db.search(query)?;

    if results.is_empty() {
        println!(
            "{} No matches in local database; try {}",
            "::".bright_blue(),
            "rvn sync".bright_white()
        );

        // Best-effort remote search if the local DB is empty/stale.
        let config = crate::config::Config::load().unwrap_or_default();
        let mut repo_client = crate::repository::client::MultiRepoClient::new();
        let mut repos = config.repositories.clone();
        repos.sort_by_key(|r| r.priority);
        for repo in &repos {
            if repo.enabled {
                repo_client.add_repo(repo.name.clone(), repo.url.clone(), repo.repo_type.clone());
            }
        }
        repo_client.preload_indexes().await;
        let remote = repo_client.search(query, search_description).await?;
        if remote.is_empty() {
            println!("{}", "No packages found".yellow());
            return Ok(());
        }

        println!();
        for (repo, pkg) in remote {
            println!(
                "{}/{} {}",
                repo.bright_blue(),
                pkg.name.bright_white(),
                pkg.version.bright_green()
            );
            println!("    {}", pkg.description.dimmed());
        }
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
