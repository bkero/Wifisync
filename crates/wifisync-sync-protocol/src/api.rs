//! API request and response types
//!
//! These types define the REST API contract between clients and the sync server.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{SyncChange, VectorClock};

// =============================================================================
// Authentication Types
// =============================================================================

/// Request to register a new user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    /// Username (unique identifier)
    pub username: String,
    /// Authentication proof (derived from master password via HKDF)
    /// Server stores hash of this, never the master password
    pub auth_proof: String,
    /// Base64-encoded salt used for key derivation
    /// Stored on server so the same auth_proof can be derived on re-login
    pub auth_salt: String,
}

/// Response containing the salt for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaltResponse {
    /// Base64-encoded salt used for key derivation
    pub auth_salt: String,
}

/// Response to user registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    /// Newly created user ID
    pub user_id: Uuid,
}

/// Request to login (authenticate a device)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    /// Username
    pub username: String,
    /// Authentication proof
    pub auth_proof: String,
    /// Human-readable device name (e.g., "My Laptop", "Work Phone")
    pub device_name: String,
}

/// Response to successful login
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    /// Device ID (unique per device per user)
    pub device_id: String,
    /// JWT token for API authentication
    pub token: String,
    /// Token expiration timestamp
    pub expires_at: DateTime<Utc>,
}

/// Request to refresh JWT token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshRequest {
    /// Current (valid) JWT token
    pub token: String,
}

/// Response to token refresh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshResponse {
    /// New JWT token
    pub token: String,
    /// New expiration timestamp
    pub expires_at: DateTime<Utc>,
}

// =============================================================================
// Sync Types
// =============================================================================

/// Request to push changes to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRequest {
    /// Device making the push
    pub device_id: String,
    /// Changes to push
    pub changes: Vec<SyncChange>,
}

/// Result of pushing a single change
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PushChangeResult {
    /// Change was accepted
    Accepted {
        /// The change ID that was accepted
        change_id: Uuid,
    },
    /// Change conflicted with another device's change
    Conflict {
        /// The change ID that conflicted
        change_id: Uuid,
        /// ID of the conflict record for resolution
        conflict_id: Uuid,
    },
}

/// Response to push request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResponse {
    /// Results for each pushed change
    pub results: Vec<PushChangeResult>,
    /// Number of changes accepted
    pub accepted_count: usize,
    /// Number of conflicts detected
    pub conflict_count: usize,
}

/// Request to pull changes from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    /// Device making the pull
    pub device_id: String,
    /// Vector clock of last known state (pull changes newer than this)
    /// If None, pull all changes
    pub since: Option<VectorClock>,
    /// Optional collection filter
    pub collection_id: Option<Uuid>,
}

/// Response to pull request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullResponse {
    /// Changes since the requested point
    pub changes: Vec<SyncChange>,
    /// Current server vector clock
    pub server_clock: VectorClock,
    /// Whether there are more changes (pagination)
    pub has_more: bool,
}

// =============================================================================
// Conflict Types
// =============================================================================

/// A sync conflict requiring user resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    /// Unique identifier for this conflict
    pub id: Uuid,
    /// Collection containing the conflicting credential
    pub collection_id: Uuid,
    /// Credential that has conflicting changes
    pub credential_id: Uuid,
    /// The local (this device's) version
    pub local_change: SyncChange,
    /// The remote (other device's) version
    pub remote_change: SyncChange,
    /// When the conflict was detected
    pub created_at: DateTime<Utc>,
}

/// List of pending conflicts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictsResponse {
    /// Pending conflicts
    pub conflicts: Vec<SyncConflict>,
}

/// How to resolve a conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Keep the local version, discard remote
    KeepLocal,
    /// Keep the remote version, discard local
    KeepRemote,
    /// Keep both (creates a duplicate credential)
    KeepBoth,
}

/// Request to resolve a conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveConflictRequest {
    /// Conflict ID to resolve
    pub conflict_id: Uuid,
    /// Resolution choice
    pub resolution: ConflictResolution,
    /// For KeepLocal or custom merge, the final payload to use
    /// (encrypted, so server cannot see content)
    pub merged_payload: Option<crate::ChangePayload>,
}

// =============================================================================
// Collection Types
// =============================================================================

/// Collection metadata (encrypted name, server doesn't see actual name)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    /// Collection ID
    pub id: Uuid,
    /// Encrypted collection name
    pub encrypted_name: Vec<u8>,
    /// Current vector clock
    pub vector_clock: VectorClock,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Response listing collections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionsResponse {
    /// User's collections
    pub collections: Vec<CollectionInfo>,
}

/// Request to create a new collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionRequest {
    /// Optional specific ID (if not provided, server generates one)
    pub id: Option<Uuid>,
    /// Encrypted collection name
    pub encrypted_name: Vec<u8>,
}

/// Response to collection creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionResponse {
    /// Created collection ID
    pub id: Uuid,
}

// =============================================================================
// Status Types
// =============================================================================

/// Sync status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Whether sync is enabled
    pub enabled: bool,
    /// Server URL if configured
    pub server_url: Option<String>,
    /// Current user if logged in
    pub username: Option<String>,
    /// Device ID if registered
    pub device_id: Option<String>,
    /// Last successful sync timestamp
    pub last_sync: Option<DateTime<Utc>>,
    /// Number of pending local changes
    pub pending_changes: usize,
    /// Number of unresolved conflicts
    pub pending_conflicts: usize,
}

// =============================================================================
// Error Types
// =============================================================================

/// API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    /// Error code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Optional additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    /// Create a new API error
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Add details to the error
    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    // Common error constructors

    /// Authentication required
    #[must_use]
    pub fn unauthorized() -> Self {
        Self::new("unauthorized", "Authentication required")
    }

    /// Invalid credentials
    #[must_use]
    pub fn invalid_credentials() -> Self {
        Self::new("invalid_credentials", "Invalid username or password")
    }

    /// Resource not found
    #[must_use]
    pub fn not_found(resource: &str) -> Self {
        Self::new("not_found", format!("{resource} not found"))
    }

    /// Conflict detected
    #[must_use]
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new("conflict", message)
    }

    /// Validation error
    #[must_use]
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new("validation_error", message)
    }

    /// Internal server error
    #[must_use]
    pub fn internal() -> Self {
        Self::new("internal_error", "An internal error occurred")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_request_serialization() {
        let req = LoginRequest {
            username: "testuser".to_string(),
            auth_proof: "proof123".to_string(),
            device_name: "My Laptop".to_string(),
        };

        let json = serde_json::to_string(&req).unwrap();
        let restored: LoginRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.username, "testuser");
        assert_eq!(restored.device_name, "My Laptop");
    }

    #[test]
    fn test_push_change_result_serialization() {
        let accepted = PushChangeResult::Accepted {
            change_id: Uuid::new_v4(),
        };
        let conflict = PushChangeResult::Conflict {
            change_id: Uuid::new_v4(),
            conflict_id: Uuid::new_v4(),
        };

        let json1 = serde_json::to_string(&accepted).unwrap();
        let json2 = serde_json::to_string(&conflict).unwrap();

        assert!(json1.contains("accepted"));
        assert!(json2.contains("conflict"));
    }

    #[test]
    fn test_conflict_resolution_serialization() {
        let resolutions = [
            ConflictResolution::KeepLocal,
            ConflictResolution::KeepRemote,
            ConflictResolution::KeepBoth,
        ];

        for res in resolutions {
            let json = serde_json::to_string(&res).unwrap();
            let restored: ConflictResolution = serde_json::from_str(&json).unwrap();
            assert_eq!(res, restored);
        }
    }

    #[test]
    fn test_api_error() {
        let err = ApiError::not_found("Collection");
        assert_eq!(err.code, "not_found");
        assert!(err.message.contains("Collection"));

        let err_with_details = ApiError::validation("Invalid email")
            .with_details(serde_json::json!({"field": "email"}));
        assert!(err_with_details.details.is_some());
    }
}
