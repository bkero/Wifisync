//! Database models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User record in the database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbUser {
    /// Unique user ID
    pub id: String,
    /// Username (unique)
    pub username: String,
    /// Bcrypt hash of the auth key
    pub auth_key_hash: String,
    /// Account creation timestamp
    pub created_at: String,
}

impl DbUser {
    /// Create a new user record
    pub fn new(username: String, auth_key_hash: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            username,
            auth_key_hash,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Get the user ID as UUID
    pub fn uuid(&self) -> Uuid {
        Uuid::parse_str(&self.id).expect("Invalid UUID in database")
    }

    /// Get creation time as DateTime
    #[allow(dead_code)]
    pub fn created_at_dt(&self) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(&self.created_at)
            .expect("Invalid datetime in database")
            .with_timezone(&Utc)
    }
}

/// Device record in the database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbDevice {
    /// Unique device ID
    pub id: String,
    /// Owner user ID
    pub user_id: String,
    /// Human-readable device name
    pub name: String,
    /// Hash of device token (for revocation)
    pub device_token_hash: String,
    /// Last sync timestamp
    pub last_sync_at: Option<String>,
    /// Device registration timestamp
    pub created_at: String,
}

impl DbDevice {
    /// Create a new device record
    pub fn new(user_id: String, name: String, device_token_hash: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            name,
            device_token_hash,
            last_sync_at: None,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

/// Collection record in the database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbCollection {
    /// Collection ID
    pub id: String,
    /// Owner user ID
    pub user_id: String,
    /// Encrypted collection name
    pub encrypted_name: Vec<u8>,
    /// JSON-serialized vector clock
    pub vector_clock: String,
    /// Last update timestamp
    pub updated_at: String,
}

impl DbCollection {
    /// Create a new collection record
    pub fn new(id: Option<Uuid>, user_id: String, encrypted_name: Vec<u8>) -> Self {
        Self {
            id: id.unwrap_or_else(Uuid::new_v4).to_string(),
            user_id,
            encrypted_name,
            vector_clock: "{}".to_string(),
            updated_at: Utc::now().to_rfc3339(),
        }
    }

    /// Get the collection ID as UUID
    pub fn uuid(&self) -> Uuid {
        Uuid::parse_str(&self.id).expect("Invalid UUID in database")
    }
}

/// Sync record (credential) in the database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbSyncRecord {
    /// Record ID
    pub id: String,
    /// Parent collection ID
    pub collection_id: String,
    /// Credential ID
    pub credential_id: String,
    /// JSON-serialized vector clock
    pub vector_clock: String,
    /// Encrypted credential payload
    pub encrypted_payload: Vec<u8>,
    /// Whether this is a tombstone (deleted)
    pub tombstone: i32,
    /// Last update timestamp
    pub updated_at: String,
}

impl DbSyncRecord {
    /// Check if this record is a tombstone
    pub fn is_tombstone(&self) -> bool {
        self.tombstone != 0
    }
}

/// Conflict record in the database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbConflict {
    /// Conflict ID
    pub id: String,
    /// User ID
    pub user_id: String,
    /// Collection ID
    pub collection_id: String,
    /// Credential ID
    pub credential_id: String,
    /// Local (this device's) encrypted payload
    pub local_payload: Option<Vec<u8>>,
    /// Local vector clock
    pub local_vector_clock: Option<String>,
    /// Remote (other device's) encrypted payload
    pub remote_payload: Option<Vec<u8>>,
    /// Remote vector clock
    pub remote_vector_clock: Option<String>,
    /// When conflict was detected
    pub created_at: String,
    /// Whether conflict has been resolved
    pub resolved: i32,
}

impl DbConflict {
    /// Create a new conflict record
    pub fn new(
        user_id: String,
        collection_id: String,
        credential_id: String,
        local_payload: Option<Vec<u8>>,
        local_vector_clock: Option<String>,
        remote_payload: Option<Vec<u8>>,
        remote_vector_clock: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            collection_id,
            credential_id,
            local_payload,
            local_vector_clock,
            remote_payload,
            remote_vector_clock,
            created_at: Utc::now().to_rfc3339(),
            resolved: 0,
        }
    }

    /// Get the conflict ID as UUID
    pub fn uuid(&self) -> Uuid {
        Uuid::parse_str(&self.id).expect("Invalid UUID in database")
    }
}
