//! Wifisync CLI
//!
//! Command-line interface for Wifisync wifi credential synchronization.

use anyhow::Result;
use clap::{Parser, Subcommand};
use console::style;

mod commands;

/// Wifisync - Sync wifi credentials between devices
#[derive(Parser)]
#[command(name = "wifisync")]
#[command(version, about, long_about = None)]
struct Cli {
    /// Output in JSON format
    #[arg(long, global = true)]
    json: bool,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List saved wifi networks from the system
    List {
        /// Show only networks that can be synced (excludes enterprise/open)
        #[arg(long)]
        syncable: bool,
    },

    /// Show details of a specific network
    Show {
        /// Network SSID to show
        ssid: String,

        /// Show the password (requires confirmation)
        #[arg(long)]
        show_password: bool,
    },

    /// Export credentials to a file
    Export {
        /// Collection name to export (or "all" for all networks)
        name: String,

        /// Output file path
        #[arg(short, long)]
        output: std::path::PathBuf,

        /// Encrypt with a password
        #[arg(short, long)]
        password: bool,
    },

    /// Import credentials from a file
    Import {
        /// Input file path
        input: std::path::PathBuf,

        /// Password for encrypted files
        #[arg(short, long)]
        password: Option<String>,

        /// Install imported credentials to system
        #[arg(long)]
        install: bool,
    },

    /// Install a credential to the system network store
    Install {
        /// Network SSID to install
        ssid: String,
    },

    /// Uninstall a managed credential from the system
    Uninstall {
        /// Network SSID to uninstall (required unless --all)
        ssid: Option<String>,

        /// Uninstall all managed credentials
        #[arg(long)]
        all: bool,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Show sync status of managed credentials
    Status,

    /// Manage the exclusion list
    Exclude {
        #[command(subcommand)]
        action: ExcludeAction,
    },

    /// Manage credential collections
    Collection {
        #[command(subcommand)]
        action: CollectionAction,
    },

    /// Manage the Secret Agent daemon
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
}

#[derive(Subcommand)]
enum ExcludeAction {
    /// List all exclusions
    List,
    /// Add an exclusion (SSID or pattern like "Home*")
    Add {
        /// SSID or pattern to exclude
        pattern: String,
    },
    /// Remove an exclusion
    Remove {
        /// SSID or pattern to remove
        pattern: String,
    },
}

#[derive(Subcommand)]
enum CollectionAction {
    /// List all collections
    List,
    /// Create a new collection
    Create {
        /// Collection name
        name: String,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Delete a collection
    Delete {
        /// Collection name
        name: String,
        /// Skip confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Add a network to a collection
    Add {
        /// Collection name
        collection: String,
        /// Network SSID to add
        ssid: String,
    },
    /// Remove a network from a collection
    Remove {
        /// Collection name
        collection: String,
        /// Network SSID to remove
        ssid: String,
    },
}

#[derive(Subcommand)]
enum AgentAction {
    /// Start the Secret Agent daemon (runs in foreground)
    Start,
    /// Show the daemon status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        "wifisync=debug,wifisync_core=debug"
    } else {
        "wifisync=info,wifisync_core=info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Run the command
    let result = match cli.command {
        Commands::List { syncable } => commands::list::run(cli.json, syncable).await,
        Commands::Show { ssid, show_password } => {
            commands::show::run(&ssid, show_password, cli.json).await
        }
        Commands::Export { name, output, password } => {
            commands::export::run(&name, &output, password, cli.json).await
        }
        Commands::Import { input, password, install } => {
            commands::import::run(&input, password.as_deref(), install, cli.json).await
        }
        Commands::Install { ssid } => commands::install::run(&ssid, cli.json).await,
        Commands::Uninstall { ssid, all, yes } => {
            if all {
                commands::uninstall::run_all(yes, cli.json).await
            } else if let Some(ssid) = ssid {
                commands::uninstall::run(&ssid, cli.json).await
            } else {
                anyhow::bail!("Provide an SSID or use --all to uninstall all profiles");
            }
        }
        Commands::Status => commands::status::run(cli.json).await,
        Commands::Exclude { action } => match action {
            ExcludeAction::List => commands::exclude::list(cli.json).await,
            ExcludeAction::Add { pattern } => commands::exclude::add(&pattern, cli.json).await,
            ExcludeAction::Remove { pattern } => commands::exclude::remove(&pattern, cli.json).await,
        },
        Commands::Collection { action } => match action {
            CollectionAction::List => commands::collection::list(cli.json).await,
            CollectionAction::Create { name, description } => {
                commands::collection::create(&name, description.as_deref(), cli.json).await
            }
            CollectionAction::Delete { name, yes } => {
                commands::collection::delete(&name, yes, cli.json).await
            }
            CollectionAction::Add { collection, ssid } => {
                commands::collection::add(&collection, &ssid, cli.json).await
            }
            CollectionAction::Remove { collection, ssid } => {
                commands::collection::remove(&collection, &ssid, cli.json).await
            }
        },
        Commands::Agent { action } => match action {
            AgentAction::Start => commands::agent::start(cli.json).await,
            AgentAction::Status => commands::agent::status(cli.json).await,
        },
    };

    if let Err(e) = result {
        if cli.json {
            let error = serde_json::json!({
                "error": e.to_string()
            });
            println!("{}", serde_json::to_string_pretty(&error)?);
        } else {
            eprintln!("{} {}", style("Error:").red().bold(), e);
        }
        std::process::exit(1);
    }

    Ok(())
}
