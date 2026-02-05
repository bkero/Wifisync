//! Local storage for credentials and collections
//!
//! This module handles persisting credentials and collections to disk.

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::crypto;
use crate::error::Error;
use crate::models::{CredentialCollection, NetworkProfile, WifiCredential};
use crate::Result;

/// Default application name for directory paths
const APP_NAME: &str = "wifisync";
/// Collections file name
const COLLECTIONS_FILE: &str = "collections.json";
/// Network profiles tracking file (profiles WITHOUT passwords)
const PROFILES_FILE: &str = "profiles.json";
/// Exclusion list file
const EXCLUSIONS_FILE: &str = "exclusions.json";

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Data directory path
    pub data_dir: PathBuf,
    /// Config directory path
    pub config_dir: PathBuf,
}

impl StorageConfig {
    /// Create storage config with default paths
    pub fn default_paths() -> Result<Self> {
        let dirs = ProjectDirs::from("", "", APP_NAME)
            .ok_or_else(|| Error::internal("Could not determine home directory"))?;

        Ok(Self {
            data_dir: dirs.data_dir().to_path_buf(),
            config_dir: dirs.config_dir().to_path_buf(),
        })
    }

    /// Create storage config with custom paths
    pub fn with_paths(data_dir: impl Into<PathBuf>, config_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            config_dir: config_dir.into(),
        }
    }
}

/// Local storage for Wifisync data
pub struct Storage {
    config: StorageConfig,
}

impl Storage {
    /// Create a new storage instance with default paths
    pub fn new() -> Result<Self> {
        let config = StorageConfig::default_paths()?;
        Self::with_config(config)
    }

    /// Create a new storage instance with custom config
    pub fn with_config(config: StorageConfig) -> Result<Self> {
        // Ensure directories exist
        fs::create_dir_all(&config.data_dir)?;
        fs::create_dir_all(&config.config_dir)?;

        Ok(Self { config })
    }

    /// Create a new storage instance with a custom data directory
    ///
    /// This is useful for Android where the data directory is provided by the app context.
    /// The config directory is set to the same as the data directory.
    pub fn with_data_dir(data_dir: PathBuf) -> Result<Self> {
        let config = StorageConfig::with_paths(data_dir.clone(), data_dir);
        Self::with_config(config)
    }

    /// Get the data directory path
    pub fn data_dir(&self) -> &Path {
        &self.config.data_dir
    }

    /// Get the config directory path
    pub fn config_dir(&self) -> &Path {
        &self.config.config_dir
    }

    // --- Collections ---

    /// Load all collections from storage
    pub fn load_collections(&self) -> Result<Vec<CredentialCollection>> {
        let path = self.config.data_dir.join(COLLECTIONS_FILE);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let data = fs::read_to_string(&path)?;
        let collections: Vec<CredentialCollection> = serde_json::from_str(&data)?;
        Ok(collections)
    }

    /// Save all collections to storage
    pub fn save_collections(&self, collections: &[CredentialCollection]) -> Result<()> {
        let path = self.config.data_dir.join(COLLECTIONS_FILE);
        let data = serde_json::to_string_pretty(collections)?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Load a specific collection by name
    pub fn load_collection(&self, name: &str) -> Result<CredentialCollection> {
        let collections = self.load_collections()?;
        collections
            .into_iter()
            .find(|c| c.name == name)
            .ok_or_else(|| Error::CollectionNotFound {
                name: name.to_string(),
            })
    }

    /// Save a collection (creates or updates)
    pub fn save_collection(&self, collection: &CredentialCollection) -> Result<()> {
        let mut collections = self.load_collections()?;

        if let Some(pos) = collections.iter().position(|c| c.id == collection.id) {
            collections[pos] = collection.clone();
        } else {
            collections.push(collection.clone());
        }

        self.save_collections(&collections)
    }

    /// Delete a collection by name
    pub fn delete_collection(&self, name: &str) -> Result<bool> {
        let mut collections = self.load_collections()?;
        let len_before = collections.len();
        collections.retain(|c| c.name != name);

        if collections.len() < len_before {
            self.save_collections(&collections)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // --- Network Profiles ---
    //
    // Profiles track network configurations in the system WITHOUT passwords.
    // Passwords are provided by the Secret Agent daemon when connecting.

    /// Load network profile tracking data
    pub fn load_profiles(&self) -> Result<Vec<NetworkProfile>> {
        let path = self.config.data_dir.join(PROFILES_FILE);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let data = fs::read_to_string(&path)?;
        let profiles: Vec<NetworkProfile> = serde_json::from_str(&data)?;
        Ok(profiles)
    }

    /// Save network profile tracking data
    pub fn save_profiles(&self, profiles: &[NetworkProfile]) -> Result<()> {
        let path = self.config.data_dir.join(PROFILES_FILE);
        let data = serde_json::to_string_pretty(profiles)?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Add a network profile record
    pub fn add_profile(&self, record: NetworkProfile) -> Result<()> {
        let mut profiles = self.load_profiles()?;
        profiles.push(record);
        self.save_profiles(&profiles)
    }

    /// Remove a network profile record by credential ID
    pub fn remove_profile(&self, credential_id: uuid::Uuid) -> Result<Option<NetworkProfile>> {
        let mut profiles = self.load_profiles()?;

        if let Some(pos) = profiles.iter().position(|p| p.credential_id == credential_id) {
            let removed = profiles.remove(pos);
            self.save_profiles(&profiles)?;
            Ok(Some(removed))
        } else {
            Ok(None)
        }
    }

    /// Find a network profile by credential ID
    pub fn find_profile(&self, credential_id: uuid::Uuid) -> Result<Option<NetworkProfile>> {
        let profiles = self.load_profiles()?;
        Ok(profiles.into_iter().find(|p| p.credential_id == credential_id))
    }

    /// Find a network profile by system ID
    pub fn find_profile_by_system_id(&self, system_id: &str) -> Result<Option<NetworkProfile>> {
        let profiles = self.load_profiles()?;
        Ok(profiles.into_iter().find(|p| p.system_id == system_id))
    }

    /// Find a credential by ID across all collections
    pub fn find_credential(&self, credential_id: uuid::Uuid) -> Result<Option<WifiCredential>> {
        let collections = self.load_collections()?;
        for collection in &collections {
            if let Some(cred) = collection.find_by_id(credential_id) {
                return Ok(Some(cred.clone()));
            }
        }
        Ok(None)
    }

    /// Find a credential by SSID across all collections
    pub fn find_credential_by_ssid(&self, ssid: &str) -> Result<Option<WifiCredential>> {
        let collections = self.load_collections()?;
        for collection in &collections {
            if let Some(cred) = collection.find_by_ssid(ssid) {
                return Ok(Some(cred.clone()));
            }
        }
        Ok(None)
    }

    // --- Exclusions ---

    /// Load the exclusion list
    pub fn load_exclusions(&self) -> Result<Vec<String>> {
        let path = self.config.config_dir.join(EXCLUSIONS_FILE);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let data = fs::read_to_string(&path)?;
        let exclusions: Vec<String> = serde_json::from_str(&data)?;
        Ok(exclusions)
    }

    /// Save the exclusion list
    pub fn save_exclusions(&self, exclusions: &[String]) -> Result<()> {
        let path = self.config.config_dir.join(EXCLUSIONS_FILE);
        let data = serde_json::to_string_pretty(exclusions)?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Add an exclusion
    pub fn add_exclusion(&self, exclusion: &str) -> Result<bool> {
        let mut exclusions = self.load_exclusions()?;

        if exclusions.contains(&exclusion.to_string()) {
            return Ok(false);
        }

        exclusions.push(exclusion.to_string());
        self.save_exclusions(&exclusions)?;
        Ok(true)
    }

    /// Remove an exclusion
    pub fn remove_exclusion(&self, exclusion: &str) -> Result<bool> {
        let mut exclusions = self.load_exclusions()?;
        let len_before = exclusions.len();
        exclusions.retain(|e| e != exclusion);

        if exclusions.len() < len_before {
            self.save_exclusions(&exclusions)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // --- Export/Import ---

    /// Export a collection to a file
    pub fn export_collection(
        &self,
        collection: &CredentialCollection,
        path: &Path,
        password: Option<&str>,
    ) -> Result<()> {
        let export = ExportedCollection::from_collection(collection);
        let data = serde_json::to_string_pretty(&export)?;

        if let Some(pwd) = password {
            let encrypted = crypto::encrypt(data.as_bytes(), pwd)?;
            fs::write(path.with_extension("json.enc"), encrypted.to_bytes())?;
        } else {
            fs::write(path, data)?;
        }

        Ok(())
    }

    /// Import a collection from a file
    pub fn import_collection(&self, path: &Path, password: Option<&str>) -> Result<CredentialCollection> {
        let data = fs::read(path)?;

        let json_data = if path.extension().map_or(false, |e| e == "enc") {
            let password = password.ok_or_else(|| Error::InvalidPassword)?;
            let encrypted = crypto::EncryptedData::from_bytes(&data)?;
            let decrypted = crypto::decrypt(&encrypted, password)?;
            String::from_utf8(decrypted)
                .map_err(|e| Error::data_corrupted(format!("Invalid UTF-8: {e}")))?
        } else {
            String::from_utf8(data)
                .map_err(|e| Error::data_corrupted(format!("Invalid UTF-8: {e}")))?
        };

        let export: ExportedCollection = serde_json::from_str(&json_data)?;
        Ok(export.into_collection())
    }
}

/// Portable format for exported collections
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedCollection {
    /// Format version
    pub version: String,
    /// Application that created this export
    pub created_by: String,
    /// When exported
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The collection data
    pub collection: CredentialCollection,
}

impl ExportedCollection {
    /// Create an export from a collection
    pub fn from_collection(collection: &CredentialCollection) -> Self {
        Self {
            version: "1.0".to_string(),
            created_by: format!("wifisync/{}", env!("CARGO_PKG_VERSION")),
            created_at: chrono::Utc::now(),
            collection: collection.clone(),
        }
    }

    /// Convert back to a collection
    pub fn into_collection(self) -> CredentialCollection {
        self.collection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_storage() -> (Storage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig::with_paths(
            temp_dir.path().join("data"),
            temp_dir.path().join("config"),
        );
        let storage = Storage::with_config(config).unwrap();
        (storage, temp_dir)
    }

    #[test]
    fn test_collection_crud() {
        let (storage, _tmp) = test_storage();

        // Create
        let collection = CredentialCollection::new("Test Collection");
        storage.save_collection(&collection).unwrap();

        // Read
        let loaded = storage.load_collection("Test Collection").unwrap();
        assert_eq!(loaded.name, "Test Collection");

        // Update
        let mut updated = loaded;
        updated.description = Some("Updated description".to_string());
        storage.save_collection(&updated).unwrap();

        let reloaded = storage.load_collection("Test Collection").unwrap();
        assert_eq!(reloaded.description, Some("Updated description".to_string()));

        // Delete
        let deleted = storage.delete_collection("Test Collection").unwrap();
        assert!(deleted);

        let result = storage.load_collection("Test Collection");
        assert!(result.is_err());
    }

    #[test]
    fn test_profile_tracking() {
        use crate::models::{NetworkProfile, SourcePlatform};

        let (storage, _tmp) = test_storage();

        // Empty initially
        let profiles = storage.load_profiles().unwrap();
        assert!(profiles.is_empty());

        // Add a profile
        let cred_id = uuid::Uuid::new_v4();
        let profile = NetworkProfile::new(cred_id, "test-uuid-1234", SourcePlatform::NetworkManager);
        storage.add_profile(profile).unwrap();

        let profiles = storage.load_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].system_id, "test-uuid-1234");

        // Find by credential ID
        let found = storage.find_profile(cred_id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().credential_id, cred_id);

        // Find by system ID
        let found = storage.find_profile_by_system_id("test-uuid-1234").unwrap();
        assert!(found.is_some());

        // Not found
        let not_found = storage.find_profile_by_system_id("nonexistent").unwrap();
        assert!(not_found.is_none());

        // Remove
        let removed = storage.remove_profile(cred_id).unwrap();
        assert!(removed.is_some());
        assert!(storage.load_profiles().unwrap().is_empty());
    }

    #[test]
    fn test_find_credential_across_collections() {
        use crate::models::{SecurityType, SourcePlatform};

        let (storage, _tmp) = test_storage();

        // Create two collections with credentials
        let mut col1 = CredentialCollection::new("Collection 1");
        let cred1 = WifiCredential::new("Net1", "pass1", SecurityType::Wpa2Psk, SourcePlatform::Manual);
        let cred1_id = cred1.id;
        col1.add(cred1);
        storage.save_collection(&col1).unwrap();

        let mut col2 = CredentialCollection::new("Collection 2");
        let cred2 = WifiCredential::new("Net2", "pass2", SecurityType::Wpa3Psk, SourcePlatform::Manual);
        let cred2_id = cred2.id;
        col2.add(cred2);
        storage.save_collection(&col2).unwrap();

        // Find credential in first collection
        let found = storage.find_credential(cred1_id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().ssid, "Net1");

        // Find credential in second collection
        let found = storage.find_credential(cred2_id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().ssid, "Net2");

        // Not found
        let not_found = storage.find_credential(uuid::Uuid::new_v4()).unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_export_import_roundtrip() {
        use crate::models::{SecurityType, SourcePlatform};

        let (storage, tmp) = test_storage();

        // Create a collection with credentials
        let mut collection = CredentialCollection::new("Export Test");
        collection.add(WifiCredential::new(
            "ExportNet", "secret", SecurityType::Wpa2Psk, SourcePlatform::Manual,
        ));
        storage.save_collection(&collection).unwrap();

        // Export unencrypted
        let export_path = tmp.path().join("export.json");
        storage.export_collection(&collection, &export_path, None).unwrap();
        assert!(export_path.exists());

        // Import
        let imported = storage.import_collection(&export_path, None).unwrap();
        assert_eq!(imported.name, "Export Test");
        assert_eq!(imported.credentials.len(), 1);
        assert_eq!(imported.credentials[0].ssid, "ExportNet");

        // Export encrypted
        let enc_path = tmp.path().join("export_enc");
        storage.export_collection(&collection, &enc_path, Some("testpassword")).unwrap();

        let enc_file = enc_path.with_extension("json.enc");
        assert!(enc_file.exists());

        // Import encrypted
        let imported_enc = storage.import_collection(&enc_file, Some("testpassword")).unwrap();
        assert_eq!(imported_enc.name, "Export Test");
        assert_eq!(imported_enc.credentials.len(), 1);
    }

    #[test]
    fn test_exclusions() {
        let (storage, _tmp) = test_storage();

        assert!(storage.add_exclusion("HomeNetwork").unwrap());
        assert!(!storage.add_exclusion("HomeNetwork").unwrap()); // Duplicate

        let exclusions = storage.load_exclusions().unwrap();
        assert_eq!(exclusions, vec!["HomeNetwork"]);

        assert!(storage.remove_exclusion("HomeNetwork").unwrap());
        assert!(!storage.remove_exclusion("HomeNetwork").unwrap()); // Already removed

        let exclusions = storage.load_exclusions().unwrap();
        assert!(exclusions.is_empty());
    }

    /// Test SSID lookup across collections
    ///
    /// This tests the fallback mechanism used by the Secret Agent when a
    /// NetworkManager connection is deleted and recreated with a new UUID.
    /// The agent should be able to find credentials by SSID even when the
    /// UUID-based profile lookup fails.
    #[test]
    fn test_find_credential_by_ssid() {
        use crate::models::{SecurityType, SourcePlatform};

        let (storage, _tmp) = test_storage();

        // Create a collection with multiple credentials
        let mut collection = CredentialCollection::new("WiFi Networks");

        let cred1 = WifiCredential::new(
            "CoffeeShop",
            "coffee123",
            SecurityType::Wpa2Psk,
            SourcePlatform::NetworkManager,
        );
        let cred2 = WifiCredential::new(
            "HomeNetwork",
            "home456",
            SecurityType::Wpa3Psk,
            SourcePlatform::NetworkManager,
        );
        let cred3 = WifiCredential::new(
            "OfficeWifi",
            "work789",
            SecurityType::Wpa2Psk,
            SourcePlatform::Manual,
        );

        collection.add(cred1);
        collection.add(cred2);
        collection.add(cred3);
        storage.save_collection(&collection).unwrap();

        // Find by exact SSID match
        let found = storage.find_credential_by_ssid("HomeNetwork").unwrap();
        assert!(found.is_some());
        let cred = found.unwrap();
        assert_eq!(cred.ssid, "HomeNetwork");
        use secrecy::ExposeSecret;
        assert_eq!(cred.password.expose_secret(), "home456");

        // Find another credential
        let found = storage.find_credential_by_ssid("CoffeeShop").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().ssid, "CoffeeShop");

        // Not found - nonexistent SSID
        let not_found = storage.find_credential_by_ssid("NonexistentNetwork").unwrap();
        assert!(not_found.is_none());

        // Not found - case sensitive
        let not_found = storage.find_credential_by_ssid("homenetwork").unwrap();
        assert!(not_found.is_none());
    }

    /// Test SSID lookup across multiple collections
    ///
    /// Verifies that find_credential_by_ssid searches all collections,
    /// not just the first one.
    #[test]
    fn test_find_credential_by_ssid_multiple_collections() {
        use crate::models::{SecurityType, SourcePlatform};

        let (storage, _tmp) = test_storage();

        // Create first collection
        let mut col1 = CredentialCollection::new("Personal");
        col1.add(WifiCredential::new(
            "HomeWifi",
            "pass1",
            SecurityType::Wpa2Psk,
            SourcePlatform::Manual,
        ));
        storage.save_collection(&col1).unwrap();

        // Create second collection
        let mut col2 = CredentialCollection::new("Work");
        col2.add(WifiCredential::new(
            "OfficeWifi",
            "pass2",
            SecurityType::Wpa2Psk,
            SourcePlatform::Manual,
        ));
        storage.save_collection(&col2).unwrap();

        // Should find credential in first collection
        let found = storage.find_credential_by_ssid("HomeWifi").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().ssid, "HomeWifi");

        // Should find credential in second collection
        let found = storage.find_credential_by_ssid("OfficeWifi").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().ssid, "OfficeWifi");
    }
}
