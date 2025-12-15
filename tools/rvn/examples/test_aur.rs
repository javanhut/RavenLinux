//! Test AUR client functionality
//! Run with: cargo run --example test_aur

use rvn::aur::{AurClient, AurConfig};

#[tokio::main]
async fn main() {
    println!("Testing AUR client...\n");

    let client = AurClient::new();

    // Test 1: Search for packages
    println!("==> Searching AUR for 'yay'...");
    match client.search("yay").await {
        Ok(results) => {
            println!("Found {} packages:\n", results.len());
            for pkg in results.iter().take(5) {
                println!(
                    "  {} {} - {}",
                    pkg.name,
                    pkg.version,
                    pkg.description.as_deref().unwrap_or("No description")
                );
            }
        }
        Err(e) => println!("Search failed: {}", e),
    }

    println!();

    // Test 2: Get package info
    println!("==> Getting info for 'paru'...");
    match client.info("paru").await {
        Ok(Some(pkg)) => {
            println!("Package: {}", pkg.name);
            println!("Version: {}", pkg.version);
            println!("Description: {}", pkg.description.as_deref().unwrap_or("N/A"));
            println!("Maintainer: {}", pkg.maintainer.as_deref().unwrap_or("N/A"));
            println!("Votes: {}", pkg.num_votes.unwrap_or(0));
            println!("Dependencies: {:?}", pkg.depends.as_deref().unwrap_or(&[]));
            println!("Git URL: {}", pkg.git_url("https://aur.archlinux.org"));
        }
        Ok(None) => println!("Package not found"),
        Err(e) => println!("Info failed: {}", e),
    }

    println!();

    // Test 3: Find a package (uses enabled check)
    println!("==> Finding 'neofetch'...");
    match client.find("neofetch").await {
        Ok(Some(pkg)) => {
            println!("Found: {} {}", pkg.name, pkg.version);
            println!("All deps: {:?}", pkg.all_dependencies());
        }
        Ok(None) => println!("Package not found or disabled"),
        Err(e) => println!("Find failed: {}", e),
    }

    println!("\n==> AUR client test complete!");
}
