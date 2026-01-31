//! Sync change records
//!
//! These types represent individual changes that can be synced between clients and server.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::VectorClock;

/// Type of sync operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOperation {
    /// Create a new credential
    Create,
    /// Update an existing credential
    Update,
    /// Delete a credential (tombstone)
    Delete,
}

impl std::fmt::Display for SyncOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create => write!(f, "create"),
            Self::Update => write!(f, "update"),
            Self::Delete => write!(f, "delete"),
        }
    }
}

/// Encrypted payload for a sync change
///
/// The server never sees the plaintext - only the encrypted blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePayload {
    /// Encrypted credential data (ChaCha20-Poly1305)
    /// For DELETE operations, this may be empty
    pub encrypted_data: Vec<u8>,

    /// Nonce used for encryption
    pub nonce: Vec<u8>,
}

impl ChangePayload {
    /// Create a new payload
    #[must_use]
    pub fn new(encrypted_data: Vec<u8>, nonce: Vec<u8>) -> Self {
        Self {
            encrypted_data,
            nonce,
        }
    }

    /// Create an empty payload (for tombstones)
    #[must_use]
    pub fn empty() -> Self {
        Self {
            encrypted_data: Vec::new(),
            nonce: Vec::new(),
        }
    }

    /// Check if payload is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.encrypted_data.is_empty()
    }
}

/// A single sync change record
///
/// Represents a change to a credential that needs to be synced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncChange {
    /// Unique identifier for this change
    pub id: Uuid,

    /// Collection this change belongs to
    pub collection_id: Uuid,

    /// Credential being changed
    pub credential_id: Uuid,

    /// Type of operation
    pub operation: SyncOperation,

    /// Vector clock at time of change
    pub vector_clock: VectorClock,

    /// Encrypted payload
    pub payload: ChangePayload,

    /// Device that made this change
    pub device_id: String,

    /// Timestamp of the change
    pub timestamp: DateTime<Utc>,
}

impl SyncChange {
    /// Create a new sync change
    #[must_use]
    pub fn new(
        collection_id: Uuid,
        credential_id: Uuid,
        operation: SyncOperation,
        vector_clock: VectorClock,
        payload: ChangePayload,
        device_id: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            collection_id,
            credential_id,
            operation,
            vector_clock,
            payload,
            device_id,
            timestamp: Utc::now(),
        }
    }

    /// Create a tombstone (delete marker)
    #[must_use]
    pub fn tombstone(
        collection_id: Uuid,
        credential_id: Uuid,
        vector_clock: VectorClock,
        device_id: String,
    ) -> Self {
        Self::new(
            collection_id,
            credential_id,
            SyncOperation::Delete,
            vector_clock,
            ChangePayload::empty(),
            device_id,
        )
    }

    /// Check if this change is a tombstone
    #[must_use]
    pub fn is_tombstone(&self) -> bool {
        self.operation == SyncOperation::Delete
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_operation_display() {
        assert_eq!(format!("{}", SyncOperation::Create), "create");
        assert_eq!(format!("{}", SyncOperation::Update), "update");
        assert_eq!(format!("{}", SyncOperation::Delete), "delete");
    }

    #[test]
    fn test_empty_payload() {
        let payload = ChangePayload::empty();
        assert!(payload.is_empty());
    }

    #[test]
    fn test_sync_change_creation() {
        let collection_id = Uuid::new_v4();
        let credential_id = Uuid::new_v4();
        let mut clock = VectorClock::new();
        clock.increment("device1");

        let change = SyncChange::new(
            collection_id,
            credential_id,
            SyncOperation::Create,
            clock,
            ChangePayload::new(vec![1, 2, 3], vec![0; 12]),
            "device1".to_string(),
        );

        assert_eq!(change.collection_id, collection_id);
        assert_eq!(change.credential_id, credential_id);
        assert_eq!(change.operation, SyncOperation::Create);
        assert!(!change.is_tombstone());
    }

    #[test]
    fn test_tombstone() {
        let tombstone = SyncChange::tombstone(
            Uuid::new_v4(),
            Uuid::new_v4(),
            VectorClock::new(),
            "device1".to_string(),
        );

        assert!(tombstone.is_tombstone());
        assert!(tombstone.payload.is_empty());
    }

    #[test]
    fn test_serialization() {
        let change = SyncChange::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            SyncOperation::Update,
            VectorClock::new(),
            ChangePayload::new(vec![1, 2, 3], vec![0; 12]),
            "device1".to_string(),
        );

        let json = serde_json::to_string(&change).unwrap();
        let restored: SyncChange = serde_json::from_str(&json).unwrap();

        assert_eq!(change.id, restored.id);
        assert_eq!(change.operation, restored.operation);
    }
}
