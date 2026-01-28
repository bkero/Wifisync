//! Profile lifecycle management
//!
//! This module handles creating and removing network profiles in the system
//! network store. Profiles do NOT contain passwords - passwords are provided
//! by the Secret Agent daemon when NetworkManager requests them.

use uuid::Uuid;

use crate::adapter::NetworkAdapter;
use crate::error::Error;
use crate::models::{NetworkProfile, WifiCredential};
use crate::storage::Storage;
use crate::Result;

/// Report of an uninstall operation
#[derive(Debug, Default)]
pub struct UninstallReport {
    /// Successfully uninstalled profiles
    pub uninstalled: Vec<UninstalledProfile>,
    /// Profiles that failed to uninstall
    pub failed: Vec<FailedUninstall>,
    /// Profiles that were not found in the system
    pub not_found: Vec<String>,
}

/// Information about an uninstalled profile
#[derive(Debug)]
pub struct UninstalledProfile {
    /// SSID of the network
    pub ssid: String,
    /// System ID that was removed
    pub system_id: String,
}

/// Information about a failed uninstall
#[derive(Debug)]
pub struct FailedUninstall {
    /// SSID of the network
    pub ssid: String,
    /// Error message
    pub error: String,
}

impl UninstallReport {
    /// Total number of profiles processed
    pub fn total(&self) -> usize {
        self.uninstalled.len() + self.failed.len() + self.not_found.len()
    }

    /// Number of successful uninstalls
    pub fn success_count(&self) -> usize {
        self.uninstalled.len()
    }
}

/// Profile manager for lifecycle operations
///
/// Manages network profiles in the system. Profiles contain network
/// configuration but NOT passwords. Passwords are provided by the
/// Secret Agent daemon when connecting.
pub struct ProfileManager<A: NetworkAdapter> {
    adapter: A,
    storage: Storage,
}

impl<A: NetworkAdapter> ProfileManager<A> {
    /// Create a new profile manager
    pub fn new(adapter: A, storage: Storage) -> Self {
        Self { adapter, storage }
    }

    /// Install a profile for a credential to the system network store
    ///
    /// Creates a network profile WITHOUT the password. NetworkManager will
    /// query the Secret Agent for the password when connecting.
    ///
    /// Returns the network profile record on success.
    pub async fn install(&self, credential: &mut WifiCredential) -> Result<NetworkProfile> {
        // Check if already installed
        if credential.managed {
            if credential.system_id.is_some() {
                return Err(Error::AlreadyInstalled {
                    ssid: credential.ssid.clone(),
                });
            }
        }

        // Create profile in system (no password stored)
        let system_id = self.adapter.create_profile(credential).await?;

        // Update credential to track the profile
        credential.set_managed(system_id.clone());

        // Create tracking record
        let profile = NetworkProfile::new(
            credential.id,
            system_id,
            self.adapter.source_platform(),
        );

        // Save tracking
        self.storage.add_profile(profile.clone())?;

        tracing::info!(
            ssid = %credential.ssid,
            system_id = %profile.system_id,
            "Installed profile to system (password provided by Secret Agent)"
        );

        Ok(profile)
    }

    /// Uninstall a profile from the system network store
    pub async fn uninstall(&self, credential: &mut WifiCredential) -> Result<()> {
        // Check if managed
        if !credential.managed {
            return Err(Error::NotManaged { id: credential.id });
        }

        let system_id = credential.system_id.as_ref().ok_or_else(|| {
            Error::internal("Credential marked as managed but has no system_id")
        })?;

        // Delete from system
        self.adapter.delete_profile(system_id).await?;

        // Remove tracking
        self.storage.remove_profile(credential.id)?;

        // Update credential
        credential.clear_managed();

        tracing::info!(ssid = %credential.ssid, "Uninstalled profile from system");

        Ok(())
    }

    /// Uninstall a profile by credential ID without modifying the credential object
    pub async fn uninstall_by_id(&self, credential_id: Uuid) -> Result<()> {
        // Find tracking record
        let profile = self.storage.find_profile(credential_id)?.ok_or_else(|| {
            Error::NotManaged { id: credential_id }
        })?;

        // Delete from system
        self.adapter.delete_profile(&profile.system_id).await?;

        // Remove tracking
        self.storage.remove_profile(credential_id)?;

        tracing::info!(credential_id = %credential_id, "Uninstalled profile from system");

        Ok(())
    }

    /// Uninstall all managed profiles
    pub async fn uninstall_all(&self) -> Result<UninstallReport> {
        let profiles = self.storage.load_profiles()?;
        let mut report = UninstallReport::default();

        for profile in profiles {
            match self.adapter.delete_profile(&profile.system_id).await {
                Ok(()) => {
                    report.uninstalled.push(UninstalledProfile {
                        ssid: format!("credential:{}", profile.credential_id),
                        system_id: profile.system_id.clone(),
                    });
                }
                Err(e) => {
                    // Check if it was just not found
                    if matches!(e, Error::NetworkNotFound { .. }) {
                        report.not_found.push(profile.system_id.clone());
                    } else {
                        report.failed.push(FailedUninstall {
                            ssid: format!("credential:{}", profile.credential_id),
                            error: e.to_string(),
                        });
                    }
                }
            }
        }

        // Clear all tracking
        self.storage.save_profiles(&[])?;

        tracing::info!(
            uninstalled = report.uninstalled.len(),
            failed = report.failed.len(),
            not_found = report.not_found.len(),
            "Completed uninstall all"
        );

        Ok(report)
    }

    /// Get sync status - find orphaned profiles
    pub async fn sync_status(&self) -> Result<SyncStatus> {
        let profiles = self.storage.load_profiles()?;
        let system_networks = self.adapter.list_networks().await?;

        let mut status = SyncStatus::default();

        // Check each profile against system
        for profile in &profiles {
            let in_system = system_networks
                .iter()
                .any(|n| n.system_id.as_ref() == Some(&profile.system_id));

            if in_system {
                status.synced.push(profile.clone());
            } else {
                status.orphaned_tracking.push(profile.clone());
            }
        }

        Ok(status)
    }
}

/// Sync status showing profile state
#[derive(Debug, Default)]
pub struct SyncStatus {
    /// Profiles properly synced between tracking and system
    pub synced: Vec<NetworkProfile>,
    /// Tracking records without corresponding system entry
    pub orphaned_tracking: Vec<NetworkProfile>,
    /// System entries that might be wifisync-managed but not tracked
    pub potential_orphaned_system: Vec<String>,
}

impl SyncStatus {
    /// Check if everything is in sync
    pub fn is_synced(&self) -> bool {
        self.orphaned_tracking.is_empty() && self.potential_orphaned_system.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{NetworkAdapter, NetworkInfo, PlatformInfo};
    use crate::models::{SecurityType, SourcePlatform, WifiCredential};
    use crate::storage::StorageConfig;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Mock adapter that tracks create/delete calls
    struct MockAdapter {
        created: Mutex<Vec<String>>,
        deleted: Mutex<Vec<String>>,
    }

    impl MockAdapter {
        fn new() -> Self {
            Self {
                created: Mutex::new(Vec::new()),
                deleted: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl NetworkAdapter for MockAdapter {
        async fn list_networks(&self) -> crate::Result<Vec<NetworkInfo>> {
            let created = self.created.lock().unwrap();
            Ok(created
                .iter()
                .map(|uuid| NetworkInfo {
                    ssid: format!("mock-{}", &uuid[..8]),
                    security_type: SecurityType::Wpa2Psk,
                    hidden: false,
                    system_id: Some(uuid.clone()),
                })
                .collect())
        }

        async fn get_credentials(&self, ssid: &str) -> crate::Result<WifiCredential> {
            Ok(WifiCredential::new(
                ssid,
                "mock_password",
                SecurityType::Wpa2Psk,
                SourcePlatform::NetworkManager,
            ))
        }

        async fn create_profile(&self, _credential: &WifiCredential) -> crate::Result<String> {
            let uuid = uuid::Uuid::new_v4().to_string();
            self.created.lock().unwrap().push(uuid.clone());
            Ok(uuid)
        }

        async fn delete_profile(&self, system_id: &str) -> crate::Result<()> {
            self.deleted.lock().unwrap().push(system_id.to_string());
            Ok(())
        }

        fn platform_info(&self) -> PlatformInfo {
            PlatformInfo {
                name: "Mock".to_string(),
                version: Some("1.0".to_string()),
                features: vec![],
            }
        }

        fn source_platform(&self) -> SourcePlatform {
            SourcePlatform::NetworkManager
        }
    }

    fn test_storage() -> (crate::storage::Storage, TempDir) {
        let tmp = TempDir::new().unwrap();
        let config = StorageConfig::with_paths(
            tmp.path().join("data"),
            tmp.path().join("config"),
        );
        (crate::storage::Storage::with_config(config).unwrap(), tmp)
    }

    #[tokio::test]
    async fn test_install_and_uninstall() {
        let (storage, _tmp) = test_storage();
        let adapter = MockAdapter::new();
        let manager = ProfileManager::new(adapter, storage);

        let mut cred = WifiCredential::new(
            "TestNetwork",
            "password123",
            SecurityType::Wpa2Psk,
            SourcePlatform::Manual,
        );

        // Install
        let profile = manager.install(&mut cred).await.unwrap();
        assert!(cred.managed);
        assert!(cred.system_id.is_some());
        assert_eq!(cred.system_id.as_ref().unwrap(), &profile.system_id);

        // Can't install again
        let result = manager.install(&mut cred).await;
        assert!(result.is_err());

        // Uninstall
        manager.uninstall(&mut cred).await.unwrap();
        assert!(!cred.managed);
        assert!(cred.system_id.is_none());
    }

    #[tokio::test]
    async fn test_install_tracks_profile() {
        let (storage, tmp) = test_storage();
        let adapter = MockAdapter::new();
        let manager = ProfileManager::new(adapter, storage);

        let mut cred = WifiCredential::new(
            "TrackedNet",
            "pass",
            SecurityType::Wpa2Psk,
            SourcePlatform::Manual,
        );

        let profile = manager.install(&mut cred).await.unwrap();

        // Verify profile was persisted
        let storage2 = crate::storage::Storage::with_config(
            StorageConfig::with_paths(tmp.path().join("data"), tmp.path().join("config")),
        )
        .unwrap();
        let profiles = storage2.load_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].system_id, profile.system_id);
        assert_eq!(profiles[0].credential_id, cred.id);
    }

    #[tokio::test]
    async fn test_uninstall_all() {
        let (storage, _tmp) = test_storage();
        let adapter = MockAdapter::new();
        let manager = ProfileManager::new(adapter, storage);

        // Install two profiles
        let mut cred1 = WifiCredential::new("Net1", "p1", SecurityType::Wpa2Psk, SourcePlatform::Manual);
        let mut cred2 = WifiCredential::new("Net2", "p2", SecurityType::Wpa3Psk, SourcePlatform::Manual);

        manager.install(&mut cred1).await.unwrap();
        manager.install(&mut cred2).await.unwrap();

        // Uninstall all
        let report = manager.uninstall_all().await.unwrap();
        assert_eq!(report.success_count(), 2);
        assert!(report.failed.is_empty());
    }

    #[tokio::test]
    async fn test_sync_status() {
        let (storage, _tmp) = test_storage();
        let adapter = MockAdapter::new();
        let manager = ProfileManager::new(adapter, storage);

        let mut cred = WifiCredential::new("SyncNet", "pass", SecurityType::Wpa2Psk, SourcePlatform::Manual);
        manager.install(&mut cred).await.unwrap();

        let status = manager.sync_status().await.unwrap();
        // The mock adapter returns the profile in list_networks, so it should be synced
        assert_eq!(status.synced.len(), 1);
        assert!(status.orphaned_tracking.is_empty());
        assert!(status.is_synced());
    }
}
