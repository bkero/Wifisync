//! Collection command implementation

use anyhow::Result;
use console::style;
use wifisync_core::models::CredentialCollection;
use wifisync_core::storage::Storage;

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
    collection.remove(cred_id);
    storage.save_collection(&collection)?;

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
