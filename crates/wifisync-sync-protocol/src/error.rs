//! Sync protocol error types

use thiserror::Error;

/// Result type alias for sync operations
pub type SyncResult<T> = std::result::Result<T, SyncError>;

/// Errors that can occur during sync operations
#[derive(Debug, Error)]
pub enum SyncError {
    /// Network error during sync
    #[error("Network error: {0}")]
    Network(String),

    /// Server returned an error
    #[error("Server error: {code} - {message}")]
    Server { code: String, message: String },

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Not logged in
    #[error("Not logged in - please run 'wifisync sync login' first")]
    NotLoggedIn,

    /// Token expired
    #[error("Token expired - please login again")]
    TokenExpired,

    /// Conflict detected during sync
    #[error("Sync conflict detected for credential {credential_id}")]
    Conflict { credential_id: uuid::Uuid },

    /// Encryption/decryption error
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl SyncError {
    /// Create a network error
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network(message.into())
    }

    /// Create a server error from API response
    pub fn server(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Server {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Create an authentication error
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication(message.into())
    }

    /// Create an encryption error
    pub fn encryption(message: impl Into<String>) -> Self {
        Self::Encryption(message.into())
    }

    /// Create an invalid state error
    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::InvalidState(message.into())
    }

    /// Check if this is an authentication-related error
    pub fn is_auth_error(&self) -> bool {
        matches!(
            self,
            Self::Authentication(_) | Self::NotLoggedIn | Self::TokenExpired
        )
    }
}
