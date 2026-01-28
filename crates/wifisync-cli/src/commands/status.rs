//! Status command implementation
//!
//! Shows sync status of managed profiles and Secret Agent daemon status.

use anyhow::{Context, Result};
use console::style;
use wifisync_core::adapter::detect_adapter;
use wifisync_core::management::ProfileManager;
use wifisync_core::storage::{Storage, StorageConfig};

pub async fn run(json: bool) -> Result<()> {
    let storage = Storage::new().context("Failed to initialize storage")?;
    let config = StorageConfig::default_paths()?;

    // Get profile data
    let profiles = storage.load_profiles()?;
    let collections = storage.load_collections()?;

    let total_credentials: usize = collections.iter().map(|c| c.credentials.len()).sum();
    let managed_credentials: usize = collections
        .iter()
        .flat_map(|c| &c.credentials)
        .filter(|c| c.managed)
        .count();

    // Get agent status
    let agent_status = wifisync_core::AgentService::status(&config.data_dir);

    // Get sync status if adapter is available
    let sync_status = if !profiles.is_empty() {
        match detect_adapter().await {
            Ok(adapter) => {
                let manager = ProfileManager::new(adapter, storage);
                match manager.sync_status().await {
                    Ok(status) => Some(status),
                    Err(e) => {
                        tracing::debug!("Failed to get sync status: {}", e);
                        None
                    }
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    if json {
        let agent_json = match &agent_status {
            Some(s) => serde_json::json!({
                "running": s.running,
                "pid": s.pid,
            }),
            None => serde_json::json!({
                "running": false,
            }),
        };

        let sync_json = match &sync_status {
            Some(s) => serde_json::json!({
                "synced": s.synced.len(),
                "orphaned_tracking": s.orphaned_tracking.len(),
                "is_synced": s.is_synced(),
            }),
            None => serde_json::json!(null),
        };

        let output = serde_json::json!({
            "collections": collections.len(),
            "total_credentials": total_credentials,
            "managed_credentials": managed_credentials,
            "installed_profiles": profiles.len(),
            "agent": agent_json,
            "sync": sync_json,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        // Header
        println!("{}", style("Wifisync Status").bold().underlined());
        println!();

        // Collections
        println!(
            "  {:<22} {}",
            style("Collections:").bold(),
            collections.len()
        );
        println!(
            "  {:<22} {}",
            style("Total credentials:").bold(),
            total_credentials
        );
        println!(
            "  {:<22} {}",
            style("Installed profiles:").bold(),
            profiles.len()
        );
        println!();

        // Agent status
        print!("  {:<22} ", style("Secret Agent:").bold());
        match &agent_status {
            Some(s) if s.running => {
                println!("{} (PID {})", style("running").green().bold(), s.pid);
            }
            Some(s) => {
                println!(
                    "{} (stale PID {})",
                    style("not running").red().bold(),
                    s.pid
                );
            }
            None => {
                println!("{}", style("not running").yellow().bold());
            }
        }

        // Sync status
        if let Some(sync) = &sync_status {
            println!();
            println!("  {}", style("Profile Sync:").bold());

            if sync.is_synced() {
                println!(
                    "    {} All {} profiles in sync",
                    style("OK").green().bold(),
                    sync.synced.len()
                );
            } else {
                if !sync.synced.is_empty() {
                    println!(
                        "    {} {} profiles synced",
                        style("OK").green(),
                        sync.synced.len()
                    );
                }
                if !sync.orphaned_tracking.is_empty() {
                    println!(
                        "    {} {} orphaned tracking records (system profiles missing)",
                        style("!").yellow(),
                        sync.orphaned_tracking.len()
                    );
                    for p in &sync.orphaned_tracking {
                        println!(
                            "      - credential {} (was system ID {})",
                            &p.credential_id.to_string()[..8],
                            &p.system_id[..8]
                        );
                    }
                }
            }
        }

        // Warnings
        if profiles.is_empty() && managed_credentials == 0 {
            println!();
            println!(
                "  {} No profiles installed. Use {} to install credentials.",
                style("Tip:").cyan(),
                style("wifisync install <ssid>").cyan()
            );
        } else if agent_status.map_or(true, |s| !s.running) && !profiles.is_empty() {
            println!();
            println!(
                "  {} Agent not running but {} profiles installed!",
                style("Warning:").yellow().bold(),
                profiles.len()
            );
            println!(
                "  {} Connections will fail without the agent. Start with:",
                style("->").dim()
            );
            println!(
                "           {}",
                style("wifisync agent start").cyan()
            );
        }
    }

    Ok(())
}
