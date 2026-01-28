//! Data models for Wifisync
//!
//! This module contains the core data structures used throughout Wifisync.

use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of wifi security protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityType {
    /// Open network (no security) - excluded from sync
    Open,
    /// WEP (legacy, insecure)
    Wep,
    /// WPA Personal (PSK)
    WpaPsk,
    /// WPA2 Personal (PSK)
    Wpa2Psk,
    /// WPA3 Personal (SAE)
    Wpa3Psk,
    /// WPA/WPA2 mixed mode Personal
    WpaWpa2Psk,
    /// WPA2/WPA3 mixed mode Personal
    Wpa2Wpa3Psk,
    /// WPA Enterprise (802.1X) - excluded from sync
    WpaEnterprise,
    /// WPA2 Enterprise (802.1X) - excluded from sync
    Wpa2Enterprise,
    /// WPA3 Enterprise (802.1X) - excluded from sync
    Wpa3Enterprise,
    /// Unknown security type
    Unknown,
}

impl SecurityType {
    /// Returns true if this security type uses enterprise (802.1X) authentication
    pub fn is_enterprise(&self) -> bool {
        matches!(
            self,
            Self::WpaEnterprise | Self::Wpa2Enterprise | Self::Wpa3Enterprise
        )
    }

    /// Returns true if this is an open (no password) network
    pub fn is_open(&self) -> bool {
        matches!(self, Self::Open)
    }

    /// Returns true if this security type is syncable (has a PSK password)
    pub fn is_syncable(&self) -> bool {
        !self.is_enterprise() && !self.is_open()
    }
}

/// Platform/source where a credential was extracted from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourcePlatform {
    /// Linux NetworkManager
    NetworkManager,
    /// Android WifiManager
    Android,
    /// Windows WLAN API
    Windows,
    /// iOS/macOS
    Apple,
    /// Imported from file
    Import,
    /// Manually created
    Manual,
}

impl std::fmt::Display for SourcePlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetworkManager => write!(f, "NetworkManager"),
            Self::Android => write!(f, "Android"),
            Self::Windows => write!(f, "Windows"),
            Self::Apple => write!(f, "Apple"),
            Self::Import => write!(f, "Import"),
            Self::Manual => write!(f, "Manual"),
        }
    }
}

/// A wifi credential (network configuration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiCredential {
    /// Unique identifier for this credential
    pub id: Uuid,

    /// Network SSID (name)
    pub ssid: String,

    /// Security type
    pub security_type: SecurityType,

    /// Pre-shared key (password) - stored securely
    #[serde(serialize_with = "serialize_secret", deserialize_with = "deserialize_secret")]
    pub password: SecretString,

    /// Whether the network is hidden (doesn't broadcast SSID)
    #[serde(default)]
    pub hidden: bool,

    /// Platform this credential was extracted from
    pub source_platform: SourcePlatform,

    /// When this credential was added to Wifisync
    pub created_at: DateTime<Utc>,

    /// User-defined tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Whether this credential is managed (installed to system)
    #[serde(default)]
    pub managed: bool,

    /// Platform-specific system ID (e.g., NetworkManager connection UUID)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_id: Option<String>,
}

impl WifiCredential {
    /// Create a new wifi credential
    pub fn new(
        ssid: impl Into<String>,
        password: impl Into<String>,
        security_type: SecurityType,
        source_platform: SourcePlatform,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            ssid: ssid.into(),
            security_type,
            password: SecretString::new(password.into()),
            hidden: false,
            source_platform,
            created_at: Utc::now(),
            tags: Vec::new(),
            managed: false,
            system_id: None,
        }
    }

    /// Create a credential builder for more complex construction
    pub fn builder(ssid: impl Into<String>) -> WifiCredentialBuilder {
        WifiCredentialBuilder::new(ssid)
    }

    /// Add a tag to this credential
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Remove a tag from this credential
    pub fn remove_tag(&mut self, tag: &str) -> bool {
        if let Some(pos) = self.tags.iter().position(|t| t == tag) {
            self.tags.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if this credential has a specific tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Mark this credential as managed with the given system ID
    pub fn set_managed(&mut self, system_id: String) {
        self.managed = true;
        self.system_id = Some(system_id);
    }

    /// Clear the managed status
    pub fn clear_managed(&mut self) {
        self.managed = false;
        self.system_id = None;
    }
}

/// Builder for constructing `WifiCredential` instances
#[derive(Debug)]
pub struct WifiCredentialBuilder {
    ssid: String,
    password: Option<SecretString>,
    security_type: SecurityType,
    source_platform: SourcePlatform,
    hidden: bool,
    tags: Vec<String>,
}

impl WifiCredentialBuilder {
    /// Create a new builder with the given SSID
    pub fn new(ssid: impl Into<String>) -> Self {
        Self {
            ssid: ssid.into(),
            password: None,
            security_type: SecurityType::Unknown,
            source_platform: SourcePlatform::Manual,
            hidden: false,
            tags: Vec::new(),
        }
    }

    /// Set the password
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(SecretString::new(password.into()));
        self
    }

    /// Set the security type
    pub fn security_type(mut self, security_type: SecurityType) -> Self {
        self.security_type = security_type;
        self
    }

    /// Set the source platform
    pub fn source_platform(mut self, platform: SourcePlatform) -> Self {
        self.source_platform = platform;
        self
    }

    /// Set whether the network is hidden
    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Add a tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Build the credential
    ///
    /// # Panics
    ///
    /// Panics if no password was set
    pub fn build(self) -> WifiCredential {
        WifiCredential {
            id: Uuid::new_v4(),
            ssid: self.ssid,
            security_type: self.security_type,
            password: self.password.expect("password is required"),
            hidden: self.hidden,
            source_platform: self.source_platform,
            created_at: Utc::now(),
            tags: self.tags,
            managed: false,
            system_id: None,
        }
    }
}

/// A collection of wifi credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialCollection {
    /// Unique identifier for this collection
    pub id: Uuid,

    /// Collection name (e.g., "Coffee Shops", "Work Networks")
    pub name: String,

    /// Optional description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Credentials in this collection
    pub credentials: Vec<WifiCredential>,

    /// Whether this collection is published for sharing
    #[serde(default)]
    pub is_shared: bool,

    /// When this collection was created
    pub created_at: DateTime<Utc>,

    /// When this collection was last modified
    pub updated_at: DateTime<Utc>,
}

impl CredentialCollection {
    /// Create a new empty collection
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            credentials: Vec::new(),
            is_shared: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a credential to this collection
    pub fn add(&mut self, credential: WifiCredential) {
        self.credentials.push(credential);
        self.updated_at = Utc::now();
    }

    /// Remove a credential by ID
    pub fn remove(&mut self, id: Uuid) -> Option<WifiCredential> {
        if let Some(pos) = self.credentials.iter().position(|c| c.id == id) {
            self.updated_at = Utc::now();
            Some(self.credentials.remove(pos))
        } else {
            None
        }
    }

    /// Find a credential by SSID
    pub fn find_by_ssid(&self, ssid: &str) -> Option<&WifiCredential> {
        self.credentials.iter().find(|c| c.ssid == ssid)
    }

    /// Find a credential by ID
    pub fn find_by_id(&self, id: Uuid) -> Option<&WifiCredential> {
        self.credentials.iter().find(|c| c.id == id)
    }

    /// Get mutable reference to a credential by ID
    pub fn find_by_id_mut(&mut self, id: Uuid) -> Option<&mut WifiCredential> {
        self.credentials.iter_mut().find(|c| c.id == id)
    }

    /// Get the number of credentials in this collection
    pub fn len(&self) -> usize {
        self.credentials.len()
    }

    /// Check if this collection is empty
    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }
}

/// Tracking record for a network profile installed to the system
///
/// A network profile contains network configuration (SSID, security type)
/// but NOT the password. The password is provided on-demand by the
/// Secret Agent daemon when NetworkManager requests it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProfile {
    /// Reference to the Wifisync credential
    pub credential_id: Uuid,

    /// Platform-specific system connection ID (e.g., NetworkManager UUID)
    pub system_id: String,

    /// Which platform this profile exists on
    pub platform: SourcePlatform,

    /// When the profile was created in the system
    pub created_at: DateTime<Utc>,
}

impl NetworkProfile {
    /// Create a new network profile record
    pub fn new(credential_id: Uuid, system_id: impl Into<String>, platform: SourcePlatform) -> Self {
        Self {
            credential_id,
            system_id: system_id.into(),
            platform,
            created_at: Utc::now(),
        }
    }
}

/// Alias for backward compatibility
#[deprecated(note = "Use NetworkProfile instead")]
pub type ManagedNetwork = NetworkProfile;

// Serialization helpers for SecretString
fn serialize_secret<S>(secret: &SecretString, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(secret.expose_secret())
}

fn deserialize_secret<'de, D>(deserializer: D) -> Result<SecretString, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(SecretString::new(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_type_checks() {
        assert!(SecurityType::Open.is_open());
        assert!(!SecurityType::Wpa2Psk.is_open());

        assert!(SecurityType::Wpa2Enterprise.is_enterprise());
        assert!(!SecurityType::Wpa2Psk.is_enterprise());

        assert!(SecurityType::Wpa2Psk.is_syncable());
        assert!(!SecurityType::Open.is_syncable());
        assert!(!SecurityType::Wpa2Enterprise.is_syncable());
    }

    #[test]
    fn test_credential_creation() {
        let cred = WifiCredential::new(
            "TestNetwork",
            "password123",
            SecurityType::Wpa2Psk,
            SourcePlatform::Manual,
        );

        assert_eq!(cred.ssid, "TestNetwork");
        assert_eq!(cred.security_type, SecurityType::Wpa2Psk);
        assert!(!cred.managed);
        assert!(cred.system_id.is_none());
    }

    #[test]
    fn test_credential_builder() {
        let cred = WifiCredential::builder("CoffeeShop")
            .password("espresso")
            .security_type(SecurityType::Wpa3Psk)
            .source_platform(SourcePlatform::Import)
            .hidden(true)
            .tag("coffee")
            .tag("favorite")
            .build();

        assert_eq!(cred.ssid, "CoffeeShop");
        assert!(cred.hidden);
        assert_eq!(cred.tags.len(), 2);
        assert!(cred.has_tag("coffee"));
    }

    #[test]
    fn test_collection_operations() {
        let mut collection = CredentialCollection::new("Test Collection");
        assert!(collection.is_empty());

        let cred = WifiCredential::new(
            "Network1",
            "pass1",
            SecurityType::Wpa2Psk,
            SourcePlatform::Manual,
        );
        let cred_id = cred.id;

        collection.add(cred);
        assert_eq!(collection.len(), 1);
        assert!(collection.find_by_ssid("Network1").is_some());
        assert!(collection.find_by_id(cred_id).is_some());

        let removed = collection.remove(cred_id);
        assert!(removed.is_some());
        assert!(collection.is_empty());
    }

    #[test]
    fn test_credential_serialization() {
        let cred = WifiCredential::new(
            "TestNet",
            "secret123",
            SecurityType::Wpa2Psk,
            SourcePlatform::NetworkManager,
        );

        let json = serde_json::to_string(&cred).unwrap();
        let deserialized: WifiCredential = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.ssid, cred.ssid);
        assert_eq!(
            deserialized.password.expose_secret(),
            cred.password.expose_secret()
        );
    }
}
