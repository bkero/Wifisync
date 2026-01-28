//! Install command implementation
//!
//! Installs a credential from a collection to the system network store.
//! The profile is created WITHOUT a password; the Secret Agent daemon
//! provides the password on-demand when connecting.

use anyhow::{Context, Result};
use console::style;
use wifisync_core::adapter::detect_adapter;
use wifisync_core::management::ProfileManager;
use wifisync_core::storage::Storage;

pub async fn run(ssid: &str, json: bool) -> Result<()> {
    let adapter = detect_adapter().await?;
    let storage = Storage::new().context("Failed to initialize storage")?;

    // Find the credential in collections
    let mut collections = storage.load_collections()?;
    let mut found_cred = None;
    let mut found_collection_idx = None;

    for (idx, collection) in collections.iter().enumerate() {
        if let Some(cred) = collection.find_by_ssid(ssid) {
            found_cred = Some(cred.clone());
            found_collection_idx = Some(idx);
            break;
        }
    }

    let mut credential = found_cred
        .ok_or_else(|| anyhow::anyhow!(
            "Network '{}' not found in any collection. \
             First import or add it to a collection.",
            ssid
        ))?;

    let collection_idx = found_collection_idx.unwrap();

    // Check if already installed
    if credential.managed {
        if json {
            let output = serde_json::json!({
                "ssid": ssid,
                "installed": false,
                "error": "already_installed",
                "system_id": credential.system_id,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!(
                "{} '{}' is already installed (system ID: {})",
                style("!").yellow(),
                ssid,
                credential.system_id.as_deref().unwrap_or("unknown")
            );
        }
        return Ok(());
    }

    // Install the profile
    let manager = ProfileManager::new(adapter, storage);
    let profile = manager.install(&mut credential).await
        .context("Failed to install profile")?;

    // Update the credential in the collection
    let storage = Storage::new()?;
    if let Some(coll_cred) = collections[collection_idx].find_by_id_mut(credential.id) {
        coll_cred.set_managed(profile.system_id.clone());
    }
    storage.save_collections(&collections)?;

    if json {
        let output = serde_json::json!({
            "ssid": ssid,
            "installed": true,
            "system_id": profile.system_id,
            "credential_id": credential.id.to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Installed '{}' to system (UUID: {})",
            style("OK").green().bold(),
            ssid,
            profile.system_id
        );
        println!(
            "  {} Password will be provided by Secret Agent daemon",
            style("->").dim()
        );

        // Check if agent is running
        let config = wifisync_core::storage::StorageConfig::default_paths()?;
        let agent_status = wifisync_core::AgentService::status(&config.data_dir);
        match agent_status {
            Some(s) if s.running => {
                println!(
                    "  {} Agent is running (PID {})",
                    style("->").dim(),
                    s.pid
                );
            }
            _ => {
                println!();
                println!(
                    "  {} The Secret Agent daemon is not running!",
                    style("Warning:").yellow().bold()
                );
                println!(
                    "  {} Start it with: {}",
                    style("->").dim(),
                    style("wifisync agent start").cyan()
                );
                println!(
                    "  {} Or enable the service: {}",
                    style("->").dim(),
                    style("systemctl --user enable --now wifisync-agent").cyan()
                );
            }
        }
    }

    Ok(())
}
