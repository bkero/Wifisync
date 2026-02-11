//! Import command implementation
//!
//! Imports a credential collection from a file, optionally installing
//! profiles to the system network store.

use std::path::Path;

use anyhow::{Context, Result};
use console::style;
use wifisync_core::storage::Storage;
use wifisync_core::sync::ChangeType;

use super::track_sync_changes;

pub async fn run(input: &Path, password: Option<&str>, install: bool, json: bool) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("File not found: {}", input.display());
    }

    let storage = Storage::new().context("Failed to initialize storage")?;

    let collection = storage.import_collection(input, password)
        .context("Failed to import collection")?;

    let cred_count = collection.credentials.len();
    let collection_name = collection.name.clone();

    // Check if a collection with this name already exists
    let existing = storage.load_collections()?;
    if existing.iter().any(|c| c.name == collection.name) {
        anyhow::bail!(
            "Collection '{}' already exists. Delete it first with: wifisync collection delete '{}'",
            collection.name,
            collection.name
        );
    }

    // Save the imported collection
    storage.save_collection(&collection)?;

    // Track all imported credentials as sync changes
    let import_changes: Vec<_> = collection
        .credentials
        .iter()
        .map(|c| (collection.id, c.id, ChangeType::Create))
        .collect();
    track_sync_changes(&import_changes)?;

    // Optionally install all credentials as profiles
    let mut installed_count = 0;
    if install {
        let adapter = wifisync_core::adapter::detect_adapter().await?;
        let install_storage = Storage::new()?;
        let manager = wifisync_core::management::ProfileManager::new(adapter, install_storage);

        let mut collections = storage.load_collections()?;
        let coll = collections.iter_mut()
            .find(|c| c.name == collection_name)
            .expect("just saved collection");

        for cred in &mut coll.credentials {
            if !cred.managed && cred.security_type.is_syncable() {
                match manager.install(cred).await {
                    Ok(profile) => {
                        cred.set_managed(profile.system_id);
                        installed_count += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            ssid = %cred.ssid,
                            error = %e,
                            "Failed to install profile"
                        );
                    }
                }
            }
        }

        // Save updated collection with managed flags
        let save_storage = Storage::new()?;
        save_storage.save_collections(&collections)?;
    }

    if json {
        let output = serde_json::json!({
            "collection": collection_name,
            "credentials": cred_count,
            "installed": installed_count,
            "path": input.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Imported '{}' ({} credentials) from {}",
            style("OK").green().bold(),
            collection_name,
            cred_count,
            input.display()
        );

        if install {
            println!(
                "  {} Installed {} profiles to system",
                style("->").dim(),
                installed_count
            );
            if installed_count < cred_count {
                println!(
                    "  {} {} credentials skipped (enterprise/open/already installed)",
                    style("->").dim(),
                    cred_count - installed_count
                );
            }
        } else {
            println!(
                "  {} Use {} to install profiles",
                style("Tip:").cyan(),
                style("wifisync install <ssid>").cyan()
            );
        }
    }

    Ok(())
}
