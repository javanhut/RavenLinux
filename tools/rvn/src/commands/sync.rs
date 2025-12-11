use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn run(force: bool) -> Result<()> {
    println!("{} Synchronizing package database...", "::".bright_blue());

    if force {
        println!("{} Forcing full refresh", "::".bright_blue());
    }

    // TODO: Fetch repository metadata

    let repos = vec!["core", "extra", "community"];

    let pb = ProgressBar::new(repos.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("━━─"),
    );

    for repo in repos {
        pb.set_message(format!("Syncing {}", repo));
        // TODO: Actually fetch repository data
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        pb.inc(1);
    }

    pb.finish_with_message("Sync complete");

    println!();
    println!("{} Package database is up to date", "✓".bright_green());

    Ok(())
}
