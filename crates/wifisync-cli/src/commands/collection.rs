//! Collection command implementation

use anyhow::Result;
use console::style;
use wifisync_core::models::CredentialCollection;
use wifisync_core::storage::Storage;
use wifisync_core::sync::ChangeType;

use super::{track_sync_change, track_sync_changes};

pub async fn list(json: bool) -> Result<()> {
    let storage = Storage::new()?;
    let collections = storage.load_collections()?;

    if json {
        let output: Vec<_> = collections
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id.to_string(),
                    "name": c.name,
                    "description": c.description,
                    "credential_count": c.credentials.len(),
                    "is_shared": c.is_shared,
                    "created_at": c.created_at.to_rfc3339(),
                    "updated_at": c.updated_at.to_rfc3339(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if collections.is_empty() {
            println!("{}", style("No collections found.").dim());
            println!();
            println!(
                "Use {} to create a collection",
                style("wifisync collection create <name>").cyan()
            );
        } else {
            println!(
                "{:<25} {:<10} {:<10} {}",
                style("Name").bold().underlined(),
                style("Networks").bold().underlined(),
                style("Shared").bold().underlined(),
                style("Description").bold().underlined()
            );

            for collection in &collections {
                let shared_str = if collection.is_shared { "yes" } else { "no" };
                let desc = collection
                    .description
                    .as_deref()
                    .unwrap_or("-");

                println!(
                    "{:<25} {:<10} {:<10} {}",
                    collection.name,
                    collection.credentials.len(),
                    shared_str,
                    desc
                );
            }

            println!();
            println!("{} {} collections", style("Total:").bold(), collections.len());
        }
    }

    Ok(())
}

pub async fn show(name: &str, json: bool) -> Result<()> {
    let storage = Storage::new()?;
    let collection = storage.load_collection(name)?;

    if json {
        let credentials: Vec<_> = collection
            .credentials
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id.to_string(),
                    "ssid": c.ssid,
                    "security_type": format!("{:?}", c.security_type),
                    "hidden": c.hidden,
                    "created_at": c.created_at.to_rfc3339(),
                })
            })
            .collect();

        let output = serde_json::json!({
            "id": collection.id.to_string(),
            "name": collection.name,
            "description": collection.description,
            "is_shared": collection.is_shared,
            "created_at": collection.created_at.to_rfc3339(),
            "updated_at": collection.updated_at.to_rfc3339(),
            "credentials": credentials,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} {}",
            style("Collection:").bold(),
            style(&collection.name).cyan()
        );

        if let Some(desc) = &collection.description {
            println!("{} {}", style("Description:").bold(), desc);
        }

        println!(
            "{} {}",
            style("Created:").bold(),
            collection.created_at.format("%Y-%m-%d %H:%M")
        );

        if collection.is_shared {
            println!("{} yes", style("Shared:").bold());
        }

        println!();

        if collection.credentials.is_empty() {
            println!("{}", style("No networks in this collection.").dim());
            println!();
            println!(
                "Use {} to add networks",
                style(format!("wifisync collection add {} <ssid>", name)).cyan()
            );
        } else {
            println!(
                "{:<30} {:<15} {}",
                style("SSID").bold().underlined(),
                style("Security").bold().underlined(),
                style("Added").bold().underlined()
            );

            for cred in &collection.credentials {
                let security_str = format!("{:?}", cred.security_type);
                let added = cred.created_at.format("%Y-%m-%d").to_string();

                println!("{:<30} {:<15} {}", cred.ssid, security_str, added);
            }

            println!();
            println!(
                "{} {} networks",
                style("Total:").bold(),
                collection.credentials.len()
            );
        }
    }

    Ok(())
}

pub async fn create(name: &str, description: Option<&str>, json: bool) -> Result<()> {
    let storage = Storage::new()?;

    // Check if collection already exists
    let collections = storage.load_collections()?;
    if collections.iter().any(|c| c.name == name) {
        anyhow::bail!("Collection '{}' already exists", name);
    }

    let mut collection = CredentialCollection::new(name);
    collection.description = description.map(String::from);

    storage.save_collection(&collection)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "id": collection.id.to_string(),
                "name": collection.name,
                "created": true
            })
        );
    } else {
        println!(
            "{} Created collection '{}'",
            style("✓").green(),
            name
        );
    }

    Ok(())
}

pub async fn delete(name: &str, yes: bool, json: bool) -> Result<()> {
    let storage = Storage::new()?;

    // Check if collection exists
    let collection = storage.load_collection(name)?;

    if !yes && !json {
        // Prompt for confirmation
        println!(
            "{}",
            style(format!(
                "This will delete collection '{}' with {} credentials.",
                name,
                collection.credentials.len()
            ))
            .yellow()
        );

        // In a real implementation, we'd prompt here
        // For now, require --yes flag
        anyhow::bail!("Use --yes to confirm deletion");
    }

    // Track deletions for all credentials in this collection before deleting
    let delete_changes: Vec<_> = collection
        .credentials
        .iter()
        .map(|c| (collection.id, c.id, ChangeType::Delete))
        .collect();
    track_sync_changes(&delete_changes)?;

    let deleted = storage.delete_collection(name)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "name": name,
                "deleted": deleted
            })
        );
    } else if deleted {
        println!("{} Deleted collection '{}'", style("✓").green(), name);
    } else {
        println!(
            "{} Collection '{}' not found",
            style("!").yellow(),
            name
        );
    }

    Ok(())
}

pub async fn add(collection_name: &str, ssid: &str, json: bool) -> Result<()> {
    let adapter = wifisync_core::adapter::detect_adapter().await?;
    let storage = Storage::new()?;

    // Get the credential from the system
    let credential = adapter.get_credentials(ssid).await
        .map_err(|_| anyhow::anyhow!(
            "Network '{}' not found in system or cannot read its credentials", ssid
        ))?;

    // Load the collection
    let mut collection = storage.load_collection(collection_name)?;

    // Check if already in collection
    if collection.find_by_ssid(ssid).is_some() {
        if json {
            let output = serde_json::json!({
                "collection": collection_name,
                "ssid": ssid,
                "added": false,
                "error": "already_exists",
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!(
                "{} '{}' is already in collection '{}'",
                style("!").yellow(),
                ssid,
                collection_name
            );
        }
        return Ok(());
    }

    let cred_id = credential.id;
    collection.add(credential);
    storage.save_collection(&collection)?;
    track_sync_change(collection.id, cred_id, ChangeType::Create)?;

    if json {
        let output = serde_json::json!({
            "collection": collection_name,
            "ssid": ssid,
            "credential_id": cred_id.to_string(),
            "added": true,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Added '{}' to collection '{}'",
            style("OK").green().bold(),
            ssid,
            collection_name
        );
    }

    Ok(())
}

pub async fn remove(collection_name: &str, ssid: &str, json: bool) -> Result<()> {
    let storage = Storage::new()?;

    let mut collection = storage.load_collection(collection_name)?;

    // Find the credential by SSID
    let cred = collection.find_by_ssid(ssid)
        .ok_or_else(|| anyhow::anyhow!(
            "Network '{}' not found in collection '{}'", ssid, collection_name
        ))?;

    let cred_id = cred.id;
    let coll_id = collection.id;
    collection.remove(cred_id);
    storage.save_collection(&collection)?;
    track_sync_change(coll_id, cred_id, ChangeType::Delete)?;

    if json {
        let output = serde_json::json!({
            "collection": collection_name,
            "ssid": ssid,
            "credential_id": cred_id.to_string(),
            "removed": true,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Removed '{}' from collection '{}'",
            style("OK").green().bold(),
            ssid,
            collection_name
        );
    }

    Ok(())
}
