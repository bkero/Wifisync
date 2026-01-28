//! Uninstall command implementation
//!
//! Removes managed network profiles from the system network store.

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
            "Network '{}' not found in any collection",
            ssid
        ))?;

    let collection_idx = found_collection_idx.unwrap();

    if !credential.managed {
        if json {
            let output = serde_json::json!({
                "ssid": ssid,
                "uninstalled": false,
                "error": "not_installed",
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!(
                "{} '{}' is not installed to the system",
                style("!").yellow(),
                ssid
            );
        }
        return Ok(());
    }

    let system_id = credential.system_id.clone().unwrap_or_default();

    // Uninstall the profile
    let manager = ProfileManager::new(adapter, storage);
    manager.uninstall(&mut credential).await
        .context("Failed to uninstall profile")?;

    // Update the credential in the collection
    let storage = Storage::new()?;
    if let Some(coll_cred) = collections[collection_idx].find_by_id_mut(credential.id) {
        coll_cred.clear_managed();
    }
    storage.save_collections(&collections)?;

    if json {
        let output = serde_json::json!({
            "ssid": ssid,
            "uninstalled": true,
            "system_id": system_id,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Uninstalled '{}' from system (was UUID: {})",
            style("OK").green().bold(),
            ssid,
            system_id
        );
    }

    Ok(())
}

pub async fn run_all(yes: bool, json: bool) -> Result<()> {
    let storage = Storage::new().context("Failed to initialize storage")?;
    let profiles = storage.load_profiles()?;

    if profiles.is_empty() {
        if json {
            let output = serde_json::json!({
                "uninstalled": 0,
                "message": "No profiles installed",
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!("{}", style("No profiles are installed.").dim());
        }
        return Ok(());
    }

    if !yes && !json {
        println!(
            "{} This will uninstall {} managed profiles from the system.",
            style("Warning:").yellow().bold(),
            profiles.len()
        );
        anyhow::bail!(
            "Use --yes to confirm. Run 'wifisync status' to review profiles first."
        );
    }

    let adapter = detect_adapter().await?;
    let manager = ProfileManager::new(adapter, storage);
    let report = manager.uninstall_all().await
        .context("Failed to uninstall profiles")?;

    // Clear managed flags in collections
    let storage = Storage::new()?;
    let mut collections = storage.load_collections()?;
    for collection in &mut collections {
        for cred in &mut collection.credentials {
            if cred.managed {
                cred.clear_managed();
            }
        }
    }
    storage.save_collections(&collections)?;

    if json {
        let output = serde_json::json!({
            "uninstalled": report.success_count(),
            "failed": report.failed.len(),
            "not_found": report.not_found.len(),
            "total": report.total(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Uninstalled {} profiles",
            style("OK").green().bold(),
            report.success_count()
        );

        if !report.failed.is_empty() {
            println!(
                "{} {} profiles failed to uninstall:",
                style("!").yellow(),
                report.failed.len()
            );
            for fail in &report.failed {
                println!("  - {}: {}", fail.ssid, fail.error);
            }
        }

        if !report.not_found.is_empty() {
            println!(
                "{} {} profiles were already removed from system",
                style("*").dim(),
                report.not_found.len()
            );
        }
    }

    Ok(())
}
