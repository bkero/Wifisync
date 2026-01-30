//! Platform adapters for network managers
//!
//! This module defines the `NetworkAdapter` trait that abstracts over different
//! platform-specific network managers, and provides implementations for each
//! supported platform.
//!
//! # Supported Platforms
//!
//! - **Linux**: NetworkManager via D-Bus (feature: `networkmanager`)
//! - **Android**: WifiManager via JNI (feature: `android`)
//!
//! # Credential Delivery Patterns
//!
//! Different platforms use different credential delivery mechanisms:
//!
//! - **Linux (NetworkManager)**: Secret Agent pattern - profiles are created WITHOUT
//!   passwords, and a daemon provides passwords on-demand via D-Bus GetSecrets()
//!
//! - **Android**: Direct embedding - passwords are embedded directly in
//!   WifiNetworkSuggestion objects at creation time (platform constraint)

#[cfg(feature = "networkmanager")]
pub mod networkmanager;

#[cfg(feature = "android")]
pub mod android;

use async_trait::async_trait;

use crate::models::{SecurityType, SourcePlatform, WifiCredential};
use crate::Result;

/// Information about a network visible to the adapter
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    /// Network SSID
    pub ssid: String,
    /// Security type
    pub security_type: SecurityType,
    /// Whether the network is hidden
    pub hidden: bool,
    /// Platform-specific connection ID (if saved)
    pub system_id: Option<String>,
}

/// Information about the platform adapter
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Platform name
    pub name: String,
    /// Platform version (e.g., NetworkManager version)
    pub version: Option<String>,
    /// Supported features
    pub features: Vec<String>,
}

/// Trait for platform-specific network manager adapters
///
/// Implementations of this trait provide access to the platform's network manager
/// for reading, writing, and deleting wifi credentials.
///
/// # Secret Agent Pattern
///
/// This adapter creates network profiles WITHOUT storing passwords. Passwords are
/// provided on-demand by a Secret Agent daemon that responds to D-Bus GetSecrets()
/// calls from NetworkManager.
#[async_trait]
pub trait NetworkAdapter: Send + Sync {
    /// List all saved wifi networks
    async fn list_networks(&self) -> Result<Vec<NetworkInfo>>;

    /// Get credentials for a specific network by SSID
    ///
    /// This is used for extracting credentials from the system for backup/export.
    async fn get_credentials(&self, ssid: &str) -> Result<WifiCredential>;

    /// Create a network profile WITHOUT storing the password
    ///
    /// The profile will have psk-flags=1 (agent-owned secret), meaning NetworkManager
    /// will query a Secret Agent for the password when connecting.
    ///
    /// Returns the system-specific connection ID (UUID) for tracking.
    async fn create_profile(&self, credential: &WifiCredential) -> Result<String>;

    /// Delete a network profile from the system by its system ID
    async fn delete_profile(&self, system_id: &str) -> Result<()>;

    /// Get information about this platform adapter
    fn platform_info(&self) -> PlatformInfo;

    /// Get the source platform enum for this adapter
    fn source_platform(&self) -> SourcePlatform;
}

/// Blanket implementation of `NetworkAdapter` for boxed trait objects
///
/// This allows `Box<dyn NetworkAdapter>` (returned by `detect_adapter()`)
/// to be used directly with `ProfileManager` and other generic consumers.
#[async_trait]
impl NetworkAdapter for Box<dyn NetworkAdapter> {
    async fn list_networks(&self) -> Result<Vec<NetworkInfo>> {
        (**self).list_networks().await
    }

    async fn get_credentials(&self, ssid: &str) -> Result<WifiCredential> {
        (**self).get_credentials(ssid).await
    }

    async fn create_profile(&self, credential: &WifiCredential) -> Result<String> {
        (**self).create_profile(credential).await
    }

    async fn delete_profile(&self, system_id: &str) -> Result<()> {
        (**self).delete_profile(system_id).await
    }

    fn platform_info(&self) -> PlatformInfo {
        (**self).platform_info()
    }

    fn source_platform(&self) -> SourcePlatform {
        (**self).source_platform()
    }
}

/// Detect and create the appropriate adapter for the current platform
///
/// This function attempts to create an adapter in the following order:
/// 1. On Linux: Try NetworkManager
/// 2. On Android: The adapter must be created explicitly with `AndroidAdapter::new()`
///    since it requires a JNI callback
///
/// # Errors
///
/// Returns `UnsupportedPlatform` if no suitable adapter is found.
pub async fn detect_adapter() -> Result<Box<dyn NetworkAdapter>> {
    #[cfg(feature = "networkmanager")]
    {
        // Try NetworkManager first on Linux
        if cfg!(target_os = "linux") {
            match networkmanager::NetworkManagerAdapter::new().await {
                Ok(adapter) => return Ok(Box::new(adapter)),
                Err(e) => {
                    tracing::debug!("NetworkManager not available: {}", e);
                }
            }
        }
    }

    // Note: Android adapter cannot be auto-detected because it requires
    // a JNI callback to be provided. Use AndroidAdapter::new() directly
    // from the Android app with the appropriate callback.
    #[cfg(feature = "android")]
    {
        if cfg!(target_os = "android") {
            return Err(crate::Error::Internal {
                message: "Android adapter requires JNI callback. \
                          Use AndroidAdapter::new() with a callback implementation."
                    .to_string(),
            });
        }
    }

    Err(crate::Error::UnsupportedPlatform {
        platform: std::env::consts::OS.to_string(),
    })
}

// Re-export Android types when the feature is enabled
#[cfg(feature = "android")]
pub use android::{
    AndroidAdapter, AndroidCapabilities, AndroidJniCallback, SuggestionInfo, SuggestionRequest,
};
