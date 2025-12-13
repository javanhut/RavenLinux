use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn run(force: bool) -> Result<()> {
    println!("{} Synchronizing package database...", "::".bright_blue());

    if force {
        println!("{} Forcing full refresh", "::".bright_blue());
    }

    let config = crate::config::Config::load().unwrap_or_default();
    let db = crate::database::Database::open_default()?;

    let mut repos = config.repositories.clone();
    repos.sort_by_key(|r| r.priority);
    let repos: Vec<_> = repos.into_iter().filter(|r| r.enabled).collect();
    if repos.is_empty() {
        println!("{}", "No repositories enabled in config".yellow());
        return Ok(());
    }

    let pb = ProgressBar::new(repos.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("━━─"),
    );

    let mut ok = 0usize;
    let mut failed = 0usize;

    for repo in repos {
        pb.set_message(format!("Syncing {}", repo.name));

        let client = crate::repository::client::RepoClient::new(
            repo.name.clone(),
            repo.url.clone(),
            repo.repo_type.clone(),
        );

        match client.fetch_index().await {
            Ok(index) => {
                if force {
                    db.clear_repo_packages(&repo.name)?;
                }
                db.replace_repo_packages(&repo.name, &index.packages)?;
                ok += 1;
            }
            Err(e) => {
                failed += 1;
                eprintln!(
                    "Warning: Failed to sync {} ({}): {}",
                    repo.name, repo.url, e
                );
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message("Sync complete");

    println!();
    if ok == 0 {
        anyhow::bail!("Failed to sync any repositories");
    }
    if failed == 0 {
        println!("{} Package database is up to date", "✓".bright_green());
    } else {
        println!(
            "{} Synced {} repos ({} failed)",
            "✓".bright_green(),
            ok,
            failed
        );
    }

    Ok(())
}
