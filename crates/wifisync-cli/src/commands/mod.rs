//! CLI command implementations

pub mod agent;
pub mod collection;
pub mod exclude;
pub mod export;
pub mod import;
pub mod install;
pub mod list;
pub mod show;
pub mod status;
pub mod sync;
pub mod uninstall;

use anyhow::Result;
use directories::ProjectDirs;
use uuid::Uuid;
use wifisync_core::sync::{ChangeType, SyncStateManager};

/// Track a single sync change (no-op if sync is not configured).
pub fn track_sync_change(
    collection_id: Uuid,
    credential_id: Uuid,
    change_type: ChangeType,
) -> Result<()> {
    let dirs = ProjectDirs::from("org", "wifisync", "wifisync")
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    let data_dir = dirs.data_dir().to_path_buf();
    let state_manager = SyncStateManager::new(&data_dir);

    let config = match state_manager.load_config()? {
        Some(c) => c,
        None => return Ok(()), // sync not configured, nothing to do
    };

    let mut state = state_manager.load_state()?;
    state.record_change(collection_id, credential_id, change_type, &config.device_id);
    state_manager.save_state(&state)?;

    Ok(())
}

/// Track multiple sync changes at once (no-op if sync is not configured).
pub fn track_sync_changes(changes: &[(Uuid, Uuid, ChangeType)]) -> Result<()> {
    if changes.is_empty() {
        return Ok(());
    }

    let dirs = ProjectDirs::from("org", "wifisync", "wifisync")
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    let data_dir = dirs.data_dir().to_path_buf();
    let state_manager = SyncStateManager::new(&data_dir);

    let config = match state_manager.load_config()? {
        Some(c) => c,
        None => return Ok(()), // sync not configured, nothing to do
    };

    let mut state = state_manager.load_state()?;
    for (collection_id, credential_id, change_type) in changes {
        state.record_change(*collection_id, *credential_id, *change_type, &config.device_id);
    }
    state_manager.save_state(&state)?;

    Ok(())
}
