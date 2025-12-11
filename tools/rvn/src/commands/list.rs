use anyhow::Result;
use colored::Colorize;

pub async fn run(pattern: Option<&str>, explicit_only: bool) -> Result<()> {
    println!("{} Listing installed packages...", "::".bright_blue());

    if let Some(p) = pattern {
        println!("{} Filtering by pattern: {}", "::".bright_blue(), p);
    }

    if explicit_only {
        println!("{} Showing explicitly installed only", "::".bright_blue());
    }

    // TODO: Query installed packages database

    // Placeholder results
    let packages = vec![
        ("bash", "5.2.21", true),
        ("coreutils", "9.4", true),
        ("gcc", "14.2.0", true),
        ("glibc", "2.38", false),
        ("linux", "6.11", true),
        ("rust", "1.75.0", true),
    ];

    println!();
    for (name, version, explicit) in packages {
        if explicit_only && !explicit {
            continue;
        }

        let status = if explicit {
            "[explicit]".bright_green()
        } else {
            "[dependency]".dimmed()
        };

        println!(
            "{} {} {}",
            name.bright_white(),
            version.bright_blue(),
            status
        );
    }

    Ok(())
}
