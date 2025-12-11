use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

mod commands;
mod config;
mod database;
mod package;
mod repository;
mod resolver;
mod workspace;

#[derive(Parser)]
#[command(name = "rvn")]
#[command(author = "RavenLinux Team")]
#[command(version = "0.1.0")]
#[command(about = "The Raven Linux Package Manager", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Run without making changes (dry-run)
    #[arg(short = 'n', long, global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install packages
    #[command(visible_alias = "i")]
    Install {
        /// Packages to install
        packages: Vec<String>,

        /// Install as build dependency only
        #[arg(long)]
        build: bool,
    },

    /// Remove packages
    #[command(visible_alias = "rm")]
    Remove {
        /// Packages to remove
        packages: Vec<String>,

        /// Also remove dependencies not required by other packages
        #[arg(long)]
        purge: bool,
    },

    /// Upgrade packages
    #[command(visible_alias = "up")]
    Upgrade {
        /// Specific packages to upgrade (all if none specified)
        packages: Vec<String>,
    },

    /// Search for packages
    #[command(visible_alias = "s")]
    Search {
        /// Search query
        query: String,

        /// Search in package descriptions too
        #[arg(short, long)]
        description: bool,
    },

    /// Show package information
    Info {
        /// Package name
        package: String,

        /// Show all versions
        #[arg(long)]
        versions: bool,
    },

    /// List installed packages
    List {
        /// Filter by pattern
        pattern: Option<String>,

        /// Show explicitly installed packages only
        #[arg(short, long)]
        explicit: bool,
    },

    /// Synchronize package database
    Sync {
        /// Force full refresh
        #[arg(short, long)]
        force: bool,
    },

    /// Clean package cache
    Clean {
        /// Remove all cached packages
        #[arg(short, long)]
        all: bool,
    },

    /// Build a package from source
    Build {
        /// Path to package definition or package name
        package: String,

        /// Install after building
        #[arg(short, long)]
        install: bool,
    },

    /// Developer workspace management
    #[command(subcommand)]
    Workspace(WorkspaceCommands),

    /// Developer tools management
    #[command(visible_alias = "d")]
    #[command(subcommand)]
    Dev(DevCommands),

    /// System management commands
    #[command(subcommand)]
    System(SystemCommands),
}

#[derive(Subcommand)]
enum WorkspaceCommands {
    /// Create a new development workspace
    Create {
        /// Workspace name
        name: String,

        /// Languages/tools to include
        #[arg(short, long, value_delimiter = ',')]
        lang: Vec<String>,

        /// Base template to use
        #[arg(short, long)]
        template: Option<String>,
    },

    /// Enter a workspace (activate environment)
    Enter {
        /// Workspace name
        name: String,
    },

    /// List workspaces
    List,

    /// Delete a workspace
    Delete {
        /// Workspace name
        name: String,

        /// Don't ask for confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Add package to current workspace
    Add {
        /// Packages to add
        packages: Vec<String>,
    },

    /// Export workspace configuration
    Export {
        /// Output file path
        output: Option<String>,
    },

    /// Import workspace from configuration
    Import {
        /// Configuration file path
        path: String,
    },
}

#[derive(Subcommand)]
enum DevCommands {
    /// Install/manage Rust toolchain
    Rust {
        /// Specific version (e.g., stable, nightly, 1.75.0)
        version: Option<String>,
    },

    /// Install/manage Node.js
    Node {
        /// Specific version (e.g., 20, 21, lts)
        version: Option<String>,
    },

    /// Install/manage Python
    Python {
        /// Specific version (e.g., 3.11, 3.12)
        version: Option<String>,
    },

    /// Install/manage Go
    Go {
        /// Specific version
        version: Option<String>,
    },

    /// Set up Docker/Podman
    Docker {
        /// Use podman instead of docker
        #[arg(long)]
        podman: bool,
    },

    /// Set up container development
    Containers,

    /// Set up virtual machine support
    Vm,

    /// List available dev tools
    List,
}

#[derive(Subcommand)]
enum SystemCommands {
    /// Create system snapshot
    Snapshot {
        /// Snapshot name/description
        name: Option<String>,
    },

    /// List snapshots
    Snapshots,

    /// Rollback to snapshot
    Rollback {
        /// Snapshot ID or name
        snapshot: String,
    },

    /// Check system health
    Health,

    /// View system information
    Info,
}

fn print_banner() {
    println!(
        "{}",
        r#"
  Raven Package Manager
"#
        .bright_blue()
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    if cli.verbose {
        print_banner();
    }

    match cli.command {
        Commands::Install { packages, build } => {
            commands::install::run(&packages, build, cli.dry_run).await
        }
        Commands::Remove { packages, purge } => {
            commands::remove::run(&packages, purge, cli.dry_run).await
        }
        Commands::Upgrade { packages } => {
            commands::upgrade::run(&packages, cli.dry_run).await
        }
        Commands::Search { query, description } => {
            commands::search::run(&query, description).await
        }
        Commands::Info { package, versions } => {
            commands::info::run(&package, versions).await
        }
        Commands::List { pattern, explicit } => {
            commands::list::run(pattern.as_deref(), explicit).await
        }
        Commands::Sync { force } => {
            commands::sync::run(force).await
        }
        Commands::Clean { all } => {
            commands::clean::run(all).await
        }
        Commands::Build { package, install } => {
            commands::build::run(&package, install).await
        }
        Commands::Workspace(cmd) => {
            commands::workspace::run(cmd).await
        }
        Commands::Dev(cmd) => {
            commands::dev::run(cmd).await
        }
        Commands::System(cmd) => {
            commands::system::run(cmd).await
        }
    }
}
