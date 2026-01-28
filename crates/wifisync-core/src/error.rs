//! Error types for Wifisync

use thiserror::Error;

/// Result type alias using Wifisync's Error type
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in Wifisync operations
#[derive(Debug, Error)]
pub enum Error {
    /// The requested network was not found
    #[error("Network not found: {ssid}")]
    NetworkNotFound { ssid: String },

    /// The credential is not managed by Wifisync
    #[error("Credential is not managed: {id}")]
    NotManaged { id: uuid::Uuid },

    /// The credential is already installed to the system
    #[error("Credential already installed: {ssid}")]
    AlreadyInstalled { ssid: String },

    /// Permission denied when accessing network manager
    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },

    /// The network manager service is not available
    #[error("Network manager not available: {message}")]
    ServiceUnavailable { message: String },

    /// Encryption or decryption failed
    #[error("Encryption error: {message}")]
    Encryption { message: String },

    /// Invalid password provided for decryption
    #[error("Invalid password")]
    InvalidPassword,

    /// Data corruption detected
    #[error("Data corrupted: {message}")]
    DataCorrupted { message: String },

    /// Collection not found
    #[error("Collection not found: {name}")]
    CollectionNotFound { name: String },

    /// Collection already exists
    #[error("Collection already exists: {name}")]
    CollectionExists { name: String },

    /// Invalid credential data
    #[error("Invalid credential: {message}")]
    InvalidCredential { message: String },

    /// File I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML serialization error
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    /// D-Bus error (Linux only)
    #[cfg(feature = "networkmanager")]
    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),

    /// D-Bus variant error (Linux only)
    #[cfg(feature = "networkmanager")]
    #[error("D-Bus variant error: {0}")]
    DBusVariant(#[from] zbus::zvariant::Error),

    /// Platform not supported
    #[error("Platform not supported: {platform}")]
    UnsupportedPlatform { platform: String },

    /// Generic internal error
    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl Error {
    /// Create a permission denied error
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::PermissionDenied {
            message: message.into(),
        }
    }

    /// Create a service unavailable error
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::ServiceUnavailable {
            message: message.into(),
        }
    }

    /// Create an encryption error
    pub fn encryption(message: impl Into<String>) -> Self {
        Self::Encryption {
            message: message.into(),
        }
    }

    /// Create a data corrupted error
    pub fn data_corrupted(message: impl Into<String>) -> Self {
        Self::DataCorrupted {
            message: message.into(),
        }
    }

    /// Create an invalid credential error
    pub fn invalid_credential(message: impl Into<String>) -> Self {
        Self::InvalidCredential {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}
