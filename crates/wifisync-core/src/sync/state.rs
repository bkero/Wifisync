//! Local sync state management
//!
//! Manages sync configuration, device credentials, and local sync state.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use wifisync_sync_protocol::VectorClock;

use crate::error::Result;

/// Sync configuration stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Sync server URL
    pub server_url: String,
    /// Username for authentication
    pub username: String,
    /// Device ID assigned by server
    pub device_id: String,
    /// Salt for key derivation (stored locally, used with password)
    pub key_salt: Vec<u8>,
    /// Auth proof from login (used to verify password before push/pull)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_proof: Option<String>,
    /// Current JWT token (optional, refreshed on each sync)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Token expiration timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_expires: Option<DateTime<Utc>>,
}

impl SyncConfig {
    /// Create a new sync configuration
    pub fn new(
        server_url: String,
        username: String,
        device_id: String,
        key_salt: Vec<u8>,
    ) -> Self {
        Self {
            server_url,
            username,
            device_id,
            key_salt,
            auth_proof: None,
            token: None,
            token_expires: None,
        }
    }

    /// Set the auth proof (stored during login for password verification)
    pub fn set_auth_proof(&mut self, auth_proof: String) {
        self.auth_proof = Some(auth_proof);
    }

    /// Verify that a password-derived auth proof matches the stored one.
    /// Returns Ok(()) if there's no stored proof (legacy config) or if it matches.
    /// Returns Err with a message if it doesn't match.
    pub fn verify_auth_proof(&self, auth_proof: &str) -> std::result::Result<(), String> {
        if let Some(stored) = &self.auth_proof {
            if stored != auth_proof {
                return Err("Password does not match the one used during login. Please use the same master password.".to_string());
            }
        }
        Ok(())
    }

    /// Check if we have a valid (non-expired) token
    pub fn has_valid_token(&self) -> bool {
        match (&self.token, &self.token_expires) {
            (Some(_token), Some(expires)) => Utc::now() < *expires,
            _ => false,
        }
    }

    /// Update the token
    pub fn set_token(&mut self, token: String, expires: DateTime<Utc>) {
        self.token = Some(token);
        self.token_expires = Some(expires);
    }

    /// Clear the token (logout)
    pub fn clear_token(&mut self) {
        self.token = None;
        self.token_expires = None;
    }
}

/// Local sync state tracking changes and conflicts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncState {
    /// Current local vector clock
    pub local_clock: VectorClock,
    /// Last synced server clock
    pub server_clock: VectorClock,
    /// Last successful sync timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<DateTime<Utc>>,
    /// Pending local changes (credential IDs that have been modified)
    pub pending_changes: Vec<PendingChange>,
}

/// A pending local change that needs to be synced
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingChange {
    /// Collection ID
    pub collection_id: uuid::Uuid,
    /// Credential ID
    pub credential_id: uuid::Uuid,
    /// Type of change
    pub change_type: ChangeType,
    /// Timestamp of change
    pub timestamp: DateTime<Utc>,
}

/// Type of local change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    /// Credential was created
    Create,
    /// Credential was updated
    Update,
    /// Credential was deleted
    Delete,
}

impl SyncState {
    /// Create a new empty sync state
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a local change
    pub fn record_change(
        &mut self,
        collection_id: uuid::Uuid,
        credential_id: uuid::Uuid,
        change_type: ChangeType,
        device_id: &str,
    ) {
        // Increment local clock
        self.local_clock.increment(device_id);

        // Add pending change
        self.pending_changes.push(PendingChange {
            collection_id,
            credential_id,
            change_type,
            timestamp: Utc::now(),
        });
    }

    /// Mark changes as synced
    pub fn mark_synced(&mut self, server_clock: VectorClock) {
        self.pending_changes.clear();
        self.server_clock = server_clock;
        self.last_sync = Some(Utc::now());
    }

    /// Remove specific changes from pending (after push)
    pub fn remove_pending(&mut self, credential_ids: &[uuid::Uuid]) {
        self.pending_changes
            .retain(|c| !credential_ids.contains(&c.credential_id));
    }

    /// Check if there are pending changes
    pub fn has_pending_changes(&self) -> bool {
        !self.pending_changes.is_empty()
    }

    /// Get the number of pending changes
    pub fn pending_count(&self) -> usize {
        self.pending_changes.len()
    }
}

/// Manager for sync configuration and state persistence
pub struct SyncStateManager {
    /// Directory for storing sync files
    data_dir: PathBuf,
}

impl SyncStateManager {
    /// Create a new state manager
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
        }
    }

    /// Get path to sync config file
    fn config_path(&self) -> PathBuf {
        self.data_dir.join("sync_config.json")
    }

    /// Get path to sync state file
    fn state_path(&self) -> PathBuf {
        self.data_dir.join("sync_state.json")
    }

    /// Load sync configuration
    pub fn load_config(&self) -> Result<Option<SyncConfig>> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read_to_string(&path)?;
        let config: SyncConfig = serde_json::from_str(&contents)?;
        Ok(Some(config))
    }

    /// Save sync configuration
    pub fn save_config(&self, config: &SyncConfig) -> Result<()> {
        // Ensure directory exists
        std::fs::create_dir_all(&self.data_dir)?;

        let contents = serde_json::to_string_pretty(config)?;
        let path = self.config_path();

        // Write atomically using a temp file
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, &contents)?;
        std::fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Delete sync configuration (logout)
    pub fn delete_config(&self) -> Result<()> {
        let path = self.config_path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Load sync state
    pub fn load_state(&self) -> Result<SyncState> {
        let path = self.state_path();
        if !path.exists() {
            return Ok(SyncState::new());
        }

        let contents = std::fs::read_to_string(&path)?;
        let state: SyncState = serde_json::from_str(&contents)?;
        Ok(state)
    }

    /// Save sync state
    pub fn save_state(&self, state: &SyncState) -> Result<()> {
        // Ensure directory exists
        std::fs::create_dir_all(&self.data_dir)?;

        let contents = serde_json::to_string_pretty(state)?;
        let path = self.state_path();

        // Write atomically
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, &contents)?;
        std::fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Check if sync is configured
    pub fn is_configured(&self) -> bool {
        self.config_path().exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sync_config() {
        let config = SyncConfig::new(
            "https://sync.example.com".to_string(),
            "testuser".to_string(),
            "device123".to_string(),
            vec![1, 2, 3, 4],
        );

        assert!(!config.has_valid_token());

        let mut config = config;
        config.set_token("token123".to_string(), Utc::now() + chrono::Duration::hours(1));
        assert!(config.has_valid_token());

        config.clear_token();
        assert!(!config.has_valid_token());
    }

    #[test]
    fn test_sync_state() {
        let mut state = SyncState::new();

        let coll_id = uuid::Uuid::new_v4();
        let cred_id = uuid::Uuid::new_v4();

        state.record_change(coll_id, cred_id, ChangeType::Create, "device1");

        assert!(state.has_pending_changes());
        assert_eq!(state.pending_count(), 1);

        state.mark_synced(VectorClock::new());
        assert!(!state.has_pending_changes());
    }

    #[test]
    fn test_state_manager_persistence() {
        let dir = tempdir().unwrap();
        let manager = SyncStateManager::new(dir.path());

        // Save and load config
        let config = SyncConfig::new(
            "https://test.example.com".to_string(),
            "user1".to_string(),
            "dev1".to_string(),
            vec![0; 32],
        );

        manager.save_config(&config).unwrap();
        let loaded = manager.load_config().unwrap().unwrap();

        assert_eq!(loaded.server_url, config.server_url);
        assert_eq!(loaded.username, config.username);

        // Save and load state
        let mut state = SyncState::new();
        state.record_change(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            ChangeType::Update,
            "device1",
        );

        manager.save_state(&state).unwrap();
        let loaded_state = manager.load_state().unwrap();

        assert_eq!(loaded_state.pending_count(), 1);
    }

    /// Test first sync detection
    ///
    /// When a user first logs in to sync, they may have existing local
    /// credentials that need to be pushed. The first sync is detected when:
    /// - last_sync is None (never synced)
    /// - pending_changes is empty (no changes recorded yet)
    ///
    /// In this case, all existing local credentials should be pushed.
    #[test]
    fn test_first_sync_detection() {
        // New state with no history = first sync
        let state = SyncState::new();
        assert!(state.last_sync.is_none());
        assert!(!state.has_pending_changes());

        // This combination indicates first sync
        let is_first_sync = state.last_sync.is_none() && !state.has_pending_changes();
        assert!(is_first_sync);
    }

    /// Test that after syncing, it's no longer considered first sync
    #[test]
    fn test_after_first_sync() {
        let mut state = SyncState::new();

        // Mark as synced
        state.mark_synced(VectorClock::new());

        // Now last_sync is set, so not a first sync
        assert!(state.last_sync.is_some());
        let is_first_sync = state.last_sync.is_none() && !state.has_pending_changes();
        assert!(!is_first_sync);
    }

    /// Test pending changes after first sync
    ///
    /// After the first sync, new changes should be tracked normally.
    #[test]
    fn test_pending_changes_after_first_sync() {
        let mut state = SyncState::new();

        // First sync
        state.mark_synced(VectorClock::new());
        assert!(!state.has_pending_changes());

        // Make a change
        let coll_id = uuid::Uuid::new_v4();
        let cred_id = uuid::Uuid::new_v4();
        state.record_change(coll_id, cred_id, ChangeType::Create, "device1");

        // Now has pending changes, but not first sync
        assert!(state.has_pending_changes());
        assert_eq!(state.pending_count(), 1);
        let is_first_sync = state.last_sync.is_none() && !state.has_pending_changes();
        assert!(!is_first_sync);
    }

    /// Test state without sync_state.json file
    ///
    /// When there's no sync_state.json file, load_state() should return
    /// a new empty state (first sync scenario).
    #[test]
    fn test_missing_state_file_is_first_sync() {
        let dir = tempdir().unwrap();
        let manager = SyncStateManager::new(dir.path());

        // No state file exists
        assert!(!dir.path().join("sync_state.json").exists());

        // Loading state returns new empty state
        let state = manager.load_state().unwrap();
        assert!(state.last_sync.is_none());
        assert!(!state.has_pending_changes());

        // This is a first sync scenario
        let is_first_sync = state.last_sync.is_none() && !state.has_pending_changes();
        assert!(is_first_sync);
    }
}
