//! Common test utilities for integration tests
//!
//! Provides skip macros, fixtures, and availability checks for D-Bus and NetworkManager.

#![allow(dead_code)] // Utilities may not all be used in every test file

use std::path::Path;
use tempfile::TempDir;

use wifisync_core::models::{SecurityType, SourcePlatform, WifiCredential};
use wifisync_core::storage::{Storage, StorageConfig};

// =============================================================================
// Skip Macros
// =============================================================================

/// Skip the test if D-Bus session bus is not available
#[macro_export]
macro_rules! skip_if_no_dbus_session {
    () => {
        if !$crate::common::dbus_session_available() {
            eprintln!("SKIPPED: D-Bus session bus not available");
            return;
        }
    };
}

/// Skip the test if D-Bus system bus is not available
#[macro_export]
macro_rules! skip_if_no_dbus_system {
    () => {
        if !$crate::common::dbus_system_available() {
            eprintln!("SKIPPED: D-Bus system bus not available");
            return;
        }
    };
}

/// Skip the test if NetworkManager is not available
#[macro_export]
macro_rules! skip_if_no_networkmanager {
    () => {
        if !$crate::common::networkmanager_available().await {
            eprintln!("SKIPPED: NetworkManager not available");
            return;
        }
    };
}

// =============================================================================
// Availability Checks
// =============================================================================

/// Check if D-Bus session bus is available
///
/// Checks both that DBUS_SESSION_BUS_ADDRESS is set and that the socket exists
pub fn dbus_session_available() -> bool {
    let Ok(addr) = std::env::var("DBUS_SESSION_BUS_ADDRESS") else {
        return false;
    };

    // Parse the address to find the socket path
    // Format is typically: unix:path=/run/user/1000/bus or unix:abstract=/tmp/dbus-xxx
    for part in addr.split(',') {
        if let Some(path) = part.strip_prefix("unix:path=") {
            return Path::new(path).exists();
        }
        if part.starts_with("unix:abstract=") {
            // Abstract sockets always "exist" if we can name them
            return true;
        }
    }

    // If we can't parse it, assume it might work
    true
}

/// Check if D-Bus system bus is available by attempting connection
pub fn dbus_system_available() -> bool {
    // System bus doesn't require an env var, but we can check if the socket exists
    let system_socket = Path::new("/run/dbus/system_bus_socket");
    let alt_socket = Path::new("/var/run/dbus/system_bus_socket");
    system_socket.exists() || alt_socket.exists()
}

/// Check if NetworkManager is running and accessible
pub async fn networkmanager_available() -> bool {
    if !dbus_system_available() {
        return false;
    }

    // Try to create the adapter - this will fail if NM isn't running
    match wifisync_core::NetworkManagerAdapter::new().await {
        Ok(_) => true,
        Err(e) => {
            eprintln!("NetworkManager check failed: {}", e);
            false
        }
    }
}

// =============================================================================
// Test Fixtures
// =============================================================================

/// Create an isolated test storage instance
///
/// Returns the Storage instance and the TempDir (keep TempDir alive for test duration)
pub fn test_storage() -> (Storage, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = StorageConfig::with_paths(
        temp_dir.path().join("data"),
        temp_dir.path().join("config"),
    );
    let storage = Storage::with_config(config).expect("Failed to create storage");
    (storage, temp_dir)
}

/// Create a test WifiCredential with the given SSID and password
pub fn test_credential(ssid: &str, password: &str) -> WifiCredential {
    WifiCredential::new(ssid, password, SecurityType::Wpa2Psk, SourcePlatform::Manual)
}

/// Create a test credential with WPA3 security
pub fn test_credential_wpa3(ssid: &str, password: &str) -> WifiCredential {
    WifiCredential::new(ssid, password, SecurityType::Wpa3Psk, SourcePlatform::Manual)
}

/// Generate a unique test SSID with a prefix
///
/// Uses UUID to avoid collisions between test runs
pub fn unique_test_ssid(prefix: &str) -> String {
    let suffix = uuid::Uuid::new_v4().to_string();
    // Truncate UUID to keep SSID reasonable length (max 32 bytes)
    format!("{}_{}", prefix, &suffix[..8])
}

// =============================================================================
// Test Cleanup Helpers
// =============================================================================

/// Track profiles created during tests for cleanup
pub struct TestProfileTracker {
    profiles: Vec<String>,
}

impl TestProfileTracker {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    /// Record a profile system_id for later cleanup
    pub fn track(&mut self, system_id: String) {
        self.profiles.push(system_id);
    }

    /// Get all tracked profile system_ids
    pub fn profiles(&self) -> &[String] {
        &self.profiles
    }
}

impl Default for TestProfileTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_ssid_generation() {
        let ssid1 = unique_test_ssid("test");
        let ssid2 = unique_test_ssid("test");

        // Should have the prefix
        assert!(ssid1.starts_with("test_"));
        assert!(ssid2.starts_with("test_"));

        // Should be unique
        assert_ne!(ssid1, ssid2);

        // Should be reasonable length
        assert!(ssid1.len() <= 32);
    }

    #[test]
    fn test_test_credential_creation() {
        let cred = test_credential("TestNet", "password123");
        assert_eq!(cred.ssid, "TestNet");
        assert_eq!(cred.security_type, SecurityType::Wpa2Psk);
        assert_eq!(cred.source_platform, SourcePlatform::Manual);
    }

    #[test]
    fn test_storage_fixture() {
        let (storage, _tmp) = test_storage();
        assert!(storage.data_dir().exists());
        assert!(storage.config_dir().exists());
    }
}
