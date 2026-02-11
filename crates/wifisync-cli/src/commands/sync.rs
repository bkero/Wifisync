//! Sync command implementations

use anyhow::{bail, Result};
use console::style;
use directories::ProjectDirs;
use std::path::PathBuf;

use wifisync_core::sync::{
    generate_salt, ConflictResolution, PushRequest, SyncChange, SyncClient, SyncConfig,
    SyncEncryption, SyncOperation, SyncStateManager,
};
use wifisync_core::storage::Storage;

/// Get the data directory for sync configuration
fn get_data_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("org", "wifisync", "wifisync")
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    Ok(dirs.data_dir().to_path_buf())
}

/// Get password from user (hidden input)
fn get_password(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    std::io::Write::flush(&mut std::io::stdout())?;
    let password = rpassword::read_password()?;
    Ok(password)
}

/// Login to a sync server
pub async fn login(server_url: &str, username: &str, json: bool) -> Result<()> {
    let data_dir = get_data_dir()?;
    let state_manager = SyncStateManager::new(&data_dir);

    // Check if already logged in
    if let Some(existing) = state_manager.load_config()? {
        if !json {
            eprintln!(
                "{} Already logged in as {} on {}",
                style("Warning:").yellow(),
                existing.username,
                existing.server_url
            );
            eprintln!("Run 'wifisync sync logout' first to login with different credentials.");
        }
        bail!("Already logged in");
    }

    // Get password
    let password = if json {
        // In JSON mode, read from stdin
        let mut password = String::new();
        std::io::stdin().read_line(&mut password)?;
        password.trim().to_string()
    } else {
        get_password("Master password: ")?
    };

    if password.is_empty() {
        bail!("Password cannot be empty");
    }

    // Create client
    let mut client = SyncClient::new(server_url)?;

    // Try to get existing salt from server (user may already be registered)
    let (salt, is_new_user) = match client.get_salt(username).await? {
        Some(salt_b64) => {
            // User exists, decode the stored salt
            use base64::Engine;
            let salt_bytes = base64::engine::general_purpose::STANDARD
                .decode(&salt_b64)
                .map_err(|e| anyhow::anyhow!("Invalid salt from server: {e}"))?;
            let salt: [u8; 32] = salt_bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("Salt has wrong length"))?;
            (salt, false)
        }
        None => {
            // New user, generate fresh salt
            (generate_salt(), true)
        }
    };

    // Derive keys
    let encryption = SyncEncryption::from_password(&password, &salt)?;
    let auth_proof = encryption.auth_proof();

    // Get device name
    let device_name = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown Device".to_string());

    // Register if new user
    if is_new_user {
        use base64::Engine;
        let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);
        match client.register(username, &auth_proof, &salt_b64).await {
            Ok(_) => {
                if !json {
                    println!("{} Created new account", style("Info:").blue());
                }
            }
            Err(e) => {
                if !e.to_string().contains("already exists") {
                    return Err(e.into());
                }
            }
        }
    }

    // Now login
    let login_resp = client.login(username, &auth_proof, &device_name).await?;

    // Save configuration
    let mut config = SyncConfig::new(
        server_url.to_string(),
        username.to_string(),
        login_resp.device_id.clone(),
        salt.to_vec(),
    );
    config.set_auth_proof(auth_proof);
    config.set_token(login_resp.token, login_resp.expires_at);

    state_manager.save_config(&config)?;

    if json {
        let output = serde_json::json!({
            "status": "success",
            "server_url": server_url,
            "username": username,
            "device_id": login_resp.device_id
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Logged in to {} as {}",
            style("Success:").green(),
            server_url,
            username
        );
        println!("Device ID: {}", login_resp.device_id);
    }

    Ok(())
}

/// Logout from sync server
pub async fn logout(json: bool) -> Result<()> {
    let data_dir = get_data_dir()?;
    let state_manager = SyncStateManager::new(&data_dir);

    let config = state_manager
        .load_config()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in"))?;

    // Try to logout from server
    if config.has_valid_token() {
        let mut client = SyncClient::from_config(&config)?;
        if let Err(e) = client.logout().await {
            tracing::warn!("Failed to logout from server: {}", e);
        }
    }

    // Delete local config
    state_manager.delete_config()?;

    if json {
        let output = serde_json::json!({
            "status": "success",
            "message": "Logged out"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{} Logged out", style("Success:").green());
    }

    Ok(())
}

/// Show sync status
pub async fn status(json: bool) -> Result<()> {
    let data_dir = get_data_dir()?;
    let state_manager = SyncStateManager::new(&data_dir);
    let storage = Storage::new()?;

    let config = state_manager.load_config()?;
    let state = state_manager.load_state()?;

    // Calculate pending count - for first sync, count all credentials
    let pending_count = if state.last_sync.is_none() && !state.has_pending_changes() {
        // First sync: all local credentials are pending
        let collections = storage.load_collections().unwrap_or_default();
        collections.iter().map(|c| c.credentials.len()).sum()
    } else {
        state.pending_count()
    };

    if json {
        let output = serde_json::json!({
            "enabled": config.is_some(),
            "server_url": config.as_ref().map(|c| &c.server_url),
            "username": config.as_ref().map(|c| &c.username),
            "device_id": config.as_ref().map(|c| &c.device_id),
            "last_sync": state.last_sync,
            "pending_changes": pending_count,
            "has_valid_token": config.as_ref().map(|c| c.has_valid_token()).unwrap_or(false)
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if let Some(config) = config {
        println!("{}", style("Sync Status").bold().underlined());
        println!();
        println!("  Server: {}", config.server_url);
        println!("  Username: {}", config.username);
        println!("  Device ID: {}", config.device_id);
        println!();
        if let Some(last_sync) = state.last_sync {
            println!("  Last sync: {}", last_sync.format("%Y-%m-%d %H:%M:%S UTC"));
        } else {
            println!("  Last sync: {} (first sync pending)", style("Never").yellow());
        }
        println!("  Pending changes: {}", pending_count);
        if config.has_valid_token() {
            println!("  Token: {} (expires {})",
                style("Valid").green(),
                config.token_expires.map(|e| e.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default()
            );
        } else {
            println!("  Token: {}", style("Expired").red());
        }
    } else {
        println!("Sync is not configured.");
        println!();
        println!("To enable sync, run:");
        println!("  wifisync sync login <server-url> <username>");
    }

    Ok(())
}

/// Push local changes to server
pub async fn push(json: bool) -> Result<()> {
    let data_dir = get_data_dir()?;
    let state_manager = SyncStateManager::new(&data_dir);
    let storage = Storage::new()?;

    let config = state_manager
        .load_config()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in - run 'wifisync sync login' first"))?;

    let mut state = state_manager.load_state()?;
    let collections = storage.load_collections()?;

    // Check if this is a first sync (no last_sync timestamp)
    // In that case, treat all existing credentials as pending changes
    let is_first_sync = state.last_sync.is_none() && !state.has_pending_changes();

    if is_first_sync {
        // Count total credentials
        let total_creds: usize = collections.iter().map(|c| c.credentials.len()).sum();
        if total_creds == 0 {
            if json {
                let output = serde_json::json!({
                    "status": "success",
                    "message": "No changes to push"
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("No changes to push.");
            }
            return Ok(());
        }
        if !json {
            println!(
                "{} First sync - pushing {} credentials from {} collections",
                style("Info:").blue(),
                total_creds,
                collections.len()
            );
        }
    } else if !state.has_pending_changes() {
        if json {
            let output = serde_json::json!({
                "status": "success",
                "message": "No changes to push"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!("No changes to push.");
        }
        return Ok(());
    }

    // Get password for encryption
    let password = if json {
        let mut password = String::new();
        std::io::stdin().read_line(&mut password)?;
        password.trim().to_string()
    } else {
        get_password("Master password: ")?
    };

    let encryption = SyncEncryption::from_password(&password, &config.key_salt)?;

    // Verify password matches the one used during login
    if let Err(e) = config.verify_auth_proof(&encryption.auth_proof()) {
        bail!("{e}");
    }

    // Create client
    let client = SyncClient::from_config(&config)?;

    // Build changes
    let mut changes = Vec::new();

    if is_first_sync {
        // First sync: push all credentials from all collections
        // First, ensure all collections exist on the server
        if !json {
            println!("{}", style("Creating collections on server...").dim());
        }

        let server_collections = client.list_collections().await?;
        let server_collection_ids: std::collections::HashSet<_> = server_collections
            .collections
            .iter()
            .map(|c| c.id)
            .collect();

        for collection in &collections {
            if !server_collection_ids.contains(&collection.id) {
                // Create collection on server
                // Encrypt the name and serialize the payload to bytes
                let encrypted_name_payload = encryption.encrypt_payload(collection.name.as_bytes())?;
                let encrypted_name = serde_json::to_vec(&encrypted_name_payload)?;
                match client.create_collection(Some(collection.id), encrypted_name).await {
                    Ok(_) => {
                        if !json {
                            println!("  Created collection: {}", collection.name);
                        }
                    }
                    Err(e) => {
                        // Collection creation can fail if it already exists on the server
                        // (e.g. from a prior sync). Treat as non-fatal — the push will
                        // give a proper error if the collection is truly inaccessible.
                        tracing::warn!("Failed to create collection {} (may already exist): {}", collection.name, e);
                    }
                }
            }
        }

        // Now push all credentials
        for collection in &collections {
            for credential in &collection.credentials {
                let credential_json = serde_json::to_vec(credential)?;
                let payload = encryption.encrypt_payload(&credential_json)?;

                // Increment clock for each change
                state.local_clock.increment(&config.device_id);

                let change = SyncChange::new(
                    collection.id,
                    credential.id,
                    SyncOperation::Create,
                    state.local_clock.clone(),
                    payload,
                    config.device_id.clone(),
                );
                changes.push(change);
            }
        }
    } else {
        // Subsequent sync: ensure any new collections exist on the server
        let server_collections = client.list_collections().await?;
        let server_collection_ids: std::collections::HashSet<_> = server_collections
            .collections
            .iter()
            .map(|c| c.id)
            .collect();

        for collection in &collections {
            if !server_collection_ids.contains(&collection.id) {
                let encrypted_name_payload = encryption.encrypt_payload(collection.name.as_bytes())?;
                let encrypted_name = serde_json::to_vec(&encrypted_name_payload)?;
                match client.create_collection(Some(collection.id), encrypted_name).await {
                    Ok(_) => {
                        if !json {
                            println!("  Created collection: {}", collection.name);
                        }
                    }
                    Err(e) => {
                        // Collection creation can fail if it already exists on the server
                        // (e.g. from a prior sync). Treat as non-fatal — the push will
                        // give a proper error if the collection is truly inaccessible.
                        tracing::warn!("Failed to create collection {} (may already exist): {}", collection.name, e);
                    }
                }
            }
        }

        // Push pending changes
        for pending in &state.pending_changes {
            if pending.change_type == wifisync_core::sync::ChangeType::Delete {
                // For deletes, the credential no longer exists locally; use a tombstone
                state.local_clock.increment(&config.device_id);
                let change = SyncChange::tombstone(
                    pending.collection_id,
                    pending.credential_id,
                    state.local_clock.clone(),
                    config.device_id.clone(),
                );
                changes.push(change);
            } else {
                // Find the credential locally
                let credential = collections.iter()
                    .find(|c| c.id == pending.collection_id)
                    .and_then(|c| c.credentials.iter().find(|cr| cr.id == pending.credential_id));

                if let Some(credential) = credential {
                    let credential_json = serde_json::to_vec(credential)?;
                    let payload = encryption.encrypt_payload(&credential_json)?;

                    let operation = match pending.change_type {
                        wifisync_core::sync::ChangeType::Create => SyncOperation::Create,
                        wifisync_core::sync::ChangeType::Update => SyncOperation::Update,
                        wifisync_core::sync::ChangeType::Delete => unreachable!(),
                    };

                    state.local_clock.increment(&config.device_id);
                    let change = SyncChange::new(
                        pending.collection_id,
                        pending.credential_id,
                        operation,
                        state.local_clock.clone(),
                        payload,
                        config.device_id.clone(),
                    );
                    changes.push(change);
                }
            }
        }
    }

    if changes.is_empty() {
        if !json {
            println!("No valid changes to push.");
        }
        return Ok(());
    }

    let req = PushRequest {
        device_id: config.device_id.clone(),
        changes,
    };

    let resp = client.push(req).await?;

    // Update state
    let pushed_ids: Vec<_> = state.pending_changes.iter().map(|c| c.credential_id).collect();
    state.remove_pending(&pushed_ids);
    state.last_sync = Some(chrono::Utc::now());
    state_manager.save_state(&state)?;

    if json {
        let output = serde_json::json!({
            "status": "success",
            "accepted": resp.accepted_count,
            "conflicts": resp.conflict_count
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Pushed {} changes ({} accepted, {} conflicts)",
            style("Success:").green(),
            resp.results.len(),
            resp.accepted_count,
            resp.conflict_count
        );
        if resp.conflict_count > 0 {
            println!();
            println!("Run 'wifisync sync conflicts' to view and resolve conflicts.");
        }
    }

    Ok(())
}

/// Pull changes from server
pub async fn pull(json: bool) -> Result<()> {
    let data_dir = get_data_dir()?;
    let state_manager = SyncStateManager::new(&data_dir);
    let storage = Storage::new()?;

    let config = state_manager
        .load_config()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in - run 'wifisync sync login' first"))?;

    let mut state = state_manager.load_state()?;

    // Get password for decryption
    let password = if json {
        let mut password = String::new();
        std::io::stdin().read_line(&mut password)?;
        password.trim().to_string()
    } else {
        get_password("Master password: ")?
    };

    let encryption = SyncEncryption::from_password(&password, &config.key_salt)?;

    // Verify password matches the one used during login
    if let Err(e) = config.verify_auth_proof(&encryption.auth_proof()) {
        bail!("{e}");
    }

    // Create client
    let client = SyncClient::from_config(&config)?;

    // Pull changes
    let since = if state.server_clock.is_empty() {
        None
    } else {
        Some(state.server_clock.clone())
    };

    let resp = client.pull(&config.device_id, since, None).await?;

    if resp.changes.is_empty() {
        if json {
            let output = serde_json::json!({
                "status": "success",
                "message": "No new changes"
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!("No new changes from server.");
        }
        return Ok(());
    }

    // Apply changes to local storage
    let mut collections = storage.load_collections()?;
    let mut applied = 0;
    let mut errors = 0;

    for change in &resp.changes {
        match apply_change(&mut collections, &change, &encryption) {
            Ok(_) => applied += 1,
            Err(e) => {
                tracing::warn!("Failed to apply change {}: {}", change.id, e);
                errors += 1;
            }
        }
    }

    // Save updated collections
    storage.save_collections(&collections)?;

    // Update sync state
    state.server_clock = resp.server_clock;
    state.last_sync = Some(chrono::Utc::now());
    state_manager.save_state(&state)?;

    if json {
        let output = serde_json::json!({
            "status": "success",
            "applied": applied,
            "errors": errors
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Pulled {} changes ({} applied, {} errors)",
            style("Success:").green(),
            resp.changes.len(),
            applied,
            errors
        );
    }

    Ok(())
}

/// Apply a single change to collections
fn apply_change(
    collections: &mut Vec<wifisync_core::CredentialCollection>,
    change: &SyncChange,
    encryption: &SyncEncryption,
) -> Result<()> {
    // Find or create collection
    let collection = collections
        .iter_mut()
        .find(|c| c.id == change.collection_id);

    match change.operation {
        SyncOperation::Delete => {
            if let Some(coll) = collection {
                coll.credentials.retain(|c| c.id != change.credential_id);
            }
        }
        SyncOperation::Create | SyncOperation::Update => {
            // Decrypt the payload
            let decrypted = encryption.decrypt_payload(&change.payload)?;
            let credential: wifisync_core::WifiCredential = serde_json::from_slice(&decrypted)?;

            if let Some(coll) = collection {
                // Update or add credential
                if let Some(existing) = coll.credentials.iter_mut().find(|c| c.id == credential.id) {
                    *existing = credential;
                } else {
                    coll.credentials.push(credential);
                }
            }
            // If collection doesn't exist locally, we skip (could also create it)
        }
    }

    Ok(())
}

/// Bidirectional sync (push then pull)
pub async fn sync_all(json: bool) -> Result<()> {
    // Push first
    if !json {
        println!("{}", style("Pushing local changes...").dim());
    }
    push(json).await?;

    // Then pull
    if !json {
        println!();
        println!("{}", style("Pulling remote changes...").dim());
    }
    pull(json).await?;

    Ok(())
}

/// List pending conflicts
pub async fn list_conflicts(json: bool) -> Result<()> {
    let data_dir = get_data_dir()?;
    let state_manager = SyncStateManager::new(&data_dir);

    let config = state_manager
        .load_config()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in - run 'wifisync sync login' first"))?;

    let client = SyncClient::from_config(&config)?;
    let resp = client.get_conflicts().await?;

    if json {
        let output = serde_json::json!({
            "conflicts": resp.conflicts.iter().map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "collection_id": c.collection_id,
                    "credential_id": c.credential_id,
                    "created_at": c.created_at
                })
            }).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if resp.conflicts.is_empty() {
        println!("No pending conflicts.");
    } else {
        println!("{}", style("Pending Conflicts").bold().underlined());
        println!();
        for conflict in &resp.conflicts {
            println!("  ID: {}", conflict.id);
            println!("  Collection: {}", conflict.collection_id);
            println!("  Credential: {}", conflict.credential_id);
            println!("  Created: {}", conflict.created_at.format("%Y-%m-%d %H:%M:%S"));
            println!();
        }
        println!("To resolve a conflict, run:");
        println!("  wifisync sync resolve <conflict-id> --keep-local");
        println!("  wifisync sync resolve <conflict-id> --keep-remote");
        println!("  wifisync sync resolve <conflict-id> --keep-both");
    }

    Ok(())
}

/// Resolve a conflict
pub async fn resolve_conflict(
    conflict_id: &str,
    keep_local: bool,
    keep_remote: bool,
    keep_both: bool,
    json: bool,
) -> Result<()> {
    let data_dir = get_data_dir()?;
    let state_manager = SyncStateManager::new(&data_dir);

    let config = state_manager
        .load_config()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in - run 'wifisync sync login' first"))?;

    // Determine resolution
    let resolution = if keep_local {
        ConflictResolution::KeepLocal
    } else if keep_remote {
        ConflictResolution::KeepRemote
    } else if keep_both {
        ConflictResolution::KeepBoth
    } else {
        bail!("Specify --keep-local, --keep-remote, or --keep-both");
    };

    let conflict_uuid = uuid::Uuid::parse_str(conflict_id)?;
    let client = SyncClient::from_config(&config)?;
    client.resolve_conflict(conflict_uuid, resolution).await?;

    if json {
        let output = serde_json::json!({
            "status": "success",
            "conflict_id": conflict_id,
            "resolution": format!("{:?}", resolution)
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Conflict {} resolved with {:?}",
            style("Success:").green(),
            conflict_id,
            resolution
        );
    }

    Ok(())
}
