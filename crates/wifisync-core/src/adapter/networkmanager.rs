//! NetworkManager adapter for Linux
//!
//! This module provides an implementation of `NetworkAdapter` for Linux systems
//! using NetworkManager via D-Bus.

use std::collections::HashMap;

use async_trait::async_trait;
use zbus::names::InterfaceName;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::Connection;

use super::{NetworkAdapter, NetworkInfo, PlatformInfo};
use crate::error::Error;
use crate::models::{SecurityType, SourcePlatform, WifiCredential};
use crate::Result;

/// NetworkManager D-Bus service name
const NM_SERVICE: &str = "org.freedesktop.NetworkManager";
/// NetworkManager settings path
const NM_SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";

/// Type alias for NetworkManager connection settings
type NMSettings = HashMap<String, HashMap<String, OwnedValue>>;

/// Adapter for Linux NetworkManager
pub struct NetworkManagerAdapter {
    connection: Connection,
    version: Option<String>,
}

impl NetworkManagerAdapter {
    /// Create a new NetworkManager adapter
    ///
    /// This will connect to the system D-Bus and verify that NetworkManager is available.
    pub async fn new() -> Result<Self> {
        let connection = Connection::system()
            .await
            .map_err(|e| Error::service_unavailable(format!("D-Bus connection failed: {e}")))?;

        // Check if NetworkManager is available and get version
        let version = Self::get_nm_version(&connection).await.ok();

        Ok(Self {
            connection,
            version,
        })
    }

    /// Get NetworkManager version
    async fn get_nm_version(connection: &Connection) -> Result<String> {
        let proxy = zbus::fdo::PropertiesProxy::builder(connection)
            .destination(NM_SERVICE)?
            .path("/org/freedesktop/NetworkManager")?
            .build()
            .await?;

        let interface = InterfaceName::from_static_str_unchecked(NM_SERVICE);
        let version_value: OwnedValue = proxy
            .get(interface, "Version")
            .await
            .map_err(|e| Error::service_unavailable(format!("Failed to get NM version: {e}")))?;

        let version: String = version_value
            .try_into()
            .map_err(|_| Error::service_unavailable("Invalid version format"))?;

        Ok(version)
    }

    /// Get all connection paths from NetworkManager Settings
    async fn get_connection_paths(&self) -> Result<Vec<OwnedObjectPath>> {
        let proxy = zbus::fdo::PropertiesProxy::builder(&self.connection)
            .destination(NM_SERVICE)?
            .path(NM_SETTINGS_PATH)?
            .build()
            .await?;

        let interface =
            InterfaceName::from_static_str_unchecked("org.freedesktop.NetworkManager.Settings");
        let connections_value: OwnedValue = proxy
            .get(interface, "Connections")
            .await
            .map_err(|e| Error::service_unavailable(format!("Failed to list connections: {e}")))?;

        let connections: Vec<OwnedObjectPath> = connections_value
            .try_into()
            .map_err(|_| Error::service_unavailable("Invalid connections format"))?;

        Ok(connections)
    }

    /// Get connection settings for a given path
    async fn get_connection_settings(&self, path: &str) -> Result<NMSettings> {
        let proxy = ConnectionProxy::builder(&self.connection)
            .destination(NM_SERVICE)?
            .path(path)?
            .build()
            .await?;

        let settings = proxy.get_settings().await?;
        Ok(settings)
    }

    /// Get connection secrets (passwords) for a given path
    async fn get_connection_secrets(&self, path: &str, setting_name: &str) -> Result<NMSettings> {
        let proxy = ConnectionProxy::builder(&self.connection)
            .destination(NM_SERVICE)?
            .path(path)?
            .build()
            .await?;

        let secrets = proxy.get_secrets(setting_name).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("PermissionDenied") || err_str.contains("NoSecrets") {
                Error::permission_denied(
                    "Cannot read wifi secrets. You may need to run as root or configure polkit.",
                )
            } else {
                Error::from(e)
            }
        })?;

        Ok(secrets)
    }

    /// Helper to extract a string from an OwnedValue
    fn value_to_string(value: &OwnedValue) -> Option<String> {
        value
            .downcast_ref::<zbus::zvariant::Str>()
            .ok()
            .map(|s| s.to_string())
    }

    /// Helper to extract bytes from an OwnedValue
    fn value_to_bytes(value: &OwnedValue) -> Option<Vec<u8>> {
        // Try to get it as Array of u8
        value
            .downcast_ref::<zbus::zvariant::Array>()
            .ok()
            .and_then(|arr| {
                let bytes: Vec<u8> = arr
                    .iter()
                    .filter_map(|v| v.downcast_ref::<u8>().ok())
                    .collect();
                if bytes.is_empty() {
                    None
                } else {
                    Some(bytes)
                }
            })
    }

    /// Helper to extract a bool from an OwnedValue
    fn value_to_bool(value: &OwnedValue) -> Option<bool> {
        value.downcast_ref::<bool>().ok()
    }

    /// Parse security type from connection settings
    fn parse_security_type(settings: &NMSettings) -> SecurityType {
        let Some(wifi_sec) = settings.get("802-11-wireless-security") else {
            return SecurityType::Open;
        };

        let key_mgmt = wifi_sec
            .get("key-mgmt")
            .and_then(Self::value_to_string)
            .unwrap_or_default();

        match key_mgmt.as_str() {
            "none" => SecurityType::Open,
            "wep" => SecurityType::Wep,
            "wpa-psk" => SecurityType::Wpa2Psk,
            "sae" => SecurityType::Wpa3Psk,
            "wpa-eap" | "wpa-eap-suite-b-192" => SecurityType::Wpa2Enterprise,
            _ => SecurityType::Unknown,
        }
    }

    /// Extract SSID from connection settings
    fn extract_ssid(settings: &NMSettings) -> Option<String> {
        let wireless = settings.get("802-11-wireless")?;
        let ssid_value = wireless.get("ssid")?;

        // SSID is stored as an array of bytes
        let ssid_bytes = Self::value_to_bytes(ssid_value)?;
        String::from_utf8(ssid_bytes).ok()
    }

    /// Extract password from secrets
    fn extract_password(secrets: &NMSettings) -> Option<String> {
        let wifi_sec = secrets.get("802-11-wireless-security")?;

        // Try psk first (WPA/WPA2/WPA3)
        if let Some(psk) = wifi_sec.get("psk") {
            if let Some(s) = Self::value_to_string(psk) {
                return Some(s);
            }
        }

        // Try wep-key0 for WEP
        if let Some(wep) = wifi_sec.get("wep-key0") {
            if let Some(s) = Self::value_to_string(wep) {
                return Some(s);
            }
        }

        None
    }

    /// Check if this is a wifi connection
    fn is_wifi_connection(settings: &NMSettings) -> bool {
        if let Some(connection) = settings.get("connection") {
            if let Some(conn_type) = connection.get("type") {
                if let Some(t) = Self::value_to_string(conn_type) {
                    return t == "802-11-wireless";
                }
            }
        }
        false
    }

    /// Get connection UUID from settings
    fn get_connection_uuid(settings: &NMSettings) -> Option<String> {
        let connection = settings.get("connection")?;
        let uuid = connection.get("uuid")?;
        Self::value_to_string(uuid)
    }

    /// Check if network is hidden
    fn is_hidden(settings: &NMSettings) -> bool {
        if let Some(wireless) = settings.get("802-11-wireless") {
            if let Some(hidden) = wireless.get("hidden") {
                return Self::value_to_bool(hidden).unwrap_or(false);
            }
        }
        false
    }

    /// Build connection settings for a profile WITHOUT password
    ///
    /// The profile uses psk-flags=1 (NM_SETTING_SECRET_FLAG_AGENT_OWNED) which tells
    /// NetworkManager to query a Secret Agent for the password instead of storing it.
    fn build_profile_settings(credential: &WifiCredential) -> (NMSettings, String) {
        use zbus::zvariant::Value;

        let mut settings = NMSettings::new();
        let conn_uuid = uuid::Uuid::new_v4().to_string();

        // Helper to create OwnedValue from string
        let str_val = |s: &str| -> OwnedValue { Value::from(s).try_to_owned().unwrap() };

        // Helper to create OwnedValue from u32
        let u32_val = |n: u32| -> OwnedValue { Value::from(n).try_to_owned().unwrap() };

        // Connection settings
        let mut connection = HashMap::new();
        connection.insert("id".to_string(), str_val(&credential.ssid));
        connection.insert("type".to_string(), str_val("802-11-wireless"));
        connection.insert("uuid".to_string(), str_val(&conn_uuid));
        settings.insert("connection".to_string(), connection);

        // Wireless settings
        let mut wireless = HashMap::new();
        let ssid_bytes: Vec<u8> = credential.ssid.as_bytes().to_vec();
        wireless.insert(
            "ssid".to_string(),
            Value::from(ssid_bytes).try_to_owned().unwrap(),
        );
        if credential.hidden {
            wireless.insert(
                "hidden".to_string(),
                Value::from(true).try_to_owned().unwrap(),
            );
        }
        settings.insert("802-11-wireless".to_string(), wireless);

        // Security settings - NO password, use psk-flags=1 (agent-owned)
        let mut security = HashMap::new();
        let key_mgmt = match credential.security_type {
            SecurityType::Wpa3Psk => "sae",
            SecurityType::Wpa2Psk
            | SecurityType::WpaPsk
            | SecurityType::WpaWpa2Psk
            | SecurityType::Wpa2Wpa3Psk => "wpa-psk",
            SecurityType::Wep => "none",
            _ => "wpa-psk",
        };
        security.insert("key-mgmt".to_string(), str_val(key_mgmt));
        // psk-flags=1 means NM_SETTING_SECRET_FLAG_AGENT_OWNED
        // NetworkManager will call GetSecrets() on registered Secret Agents
        security.insert("psk-flags".to_string(), u32_val(1));
        // NOTE: We do NOT include "psk" - password is provided by Secret Agent
        settings.insert("802-11-wireless-security".to_string(), security);

        // IPv4 settings
        let mut ipv4 = HashMap::new();
        ipv4.insert("method".to_string(), str_val("auto"));
        settings.insert("ipv4".to_string(), ipv4);

        // IPv6 settings
        let mut ipv6 = HashMap::new();
        ipv6.insert("method".to_string(), str_val("auto"));
        settings.insert("ipv6".to_string(), ipv6);

        (settings, conn_uuid)
    }
}

#[async_trait]
impl NetworkAdapter for NetworkManagerAdapter {
    async fn list_networks(&self) -> Result<Vec<NetworkInfo>> {
        let paths = self.get_connection_paths().await?;
        let mut networks = Vec::new();

        for path in paths {
            let path_str = path.as_str();
            let settings = match self.get_connection_settings(path_str).await {
                Ok(s) => s,
                Err(_) => continue,
            };

            if !Self::is_wifi_connection(&settings) {
                continue;
            }

            let Some(ssid) = Self::extract_ssid(&settings) else {
                continue;
            };

            let security_type = Self::parse_security_type(&settings);
            let system_id = Self::get_connection_uuid(&settings);
            let hidden = Self::is_hidden(&settings);

            networks.push(NetworkInfo {
                ssid,
                security_type,
                hidden,
                system_id,
            });
        }

        Ok(networks)
    }

    async fn get_credentials(&self, ssid: &str) -> Result<WifiCredential> {
        let paths = self.get_connection_paths().await?;

        for path in paths {
            let path_str = path.as_str();
            let settings = match self.get_connection_settings(path_str).await {
                Ok(s) => s,
                Err(_) => continue,
            };

            if !Self::is_wifi_connection(&settings) {
                continue;
            }

            let Some(found_ssid) = Self::extract_ssid(&settings) else {
                continue;
            };

            if found_ssid != ssid {
                continue;
            }

            let security_type = Self::parse_security_type(&settings);

            // Skip open and enterprise networks
            if !security_type.is_syncable() {
                return Err(Error::invalid_credential(format!(
                    "Network '{ssid}' has unsyncable security type: {security_type:?}"
                )));
            }

            // Get secrets
            let secrets = self
                .get_connection_secrets(path_str, "802-11-wireless-security")
                .await?;

            let password = Self::extract_password(&secrets).ok_or_else(|| {
                Error::invalid_credential(format!("No password found for network '{ssid}'"))
            })?;

            let system_id = Self::get_connection_uuid(&settings);
            let hidden = Self::is_hidden(&settings);

            let mut credential =
                WifiCredential::new(ssid, password, security_type, SourcePlatform::NetworkManager);
            credential.hidden = hidden;

            if let Some(id) = system_id {
                credential.set_managed(id);
            }

            return Ok(credential);
        }

        Err(Error::NetworkNotFound {
            ssid: ssid.to_string(),
        })
    }

    async fn create_profile(&self, credential: &WifiCredential) -> Result<String> {
        let (settings, conn_uuid) = Self::build_profile_settings(credential);

        let settings_proxy = SettingsProxy::builder(&self.connection)
            .destination(NM_SERVICE)?
            .path(NM_SETTINGS_PATH)?
            .build()
            .await?;

        let _path = settings_proxy
            .add_connection2(settings, 0, HashMap::new())
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("PermissionDenied") {
                    Error::permission_denied("Cannot add connection. You may need root privileges.")
                } else {
                    Error::internal(format!("Failed to add connection: {e}"))
                }
            })?;

        tracing::info!(
            "Created profile for '{}' with UUID {} (password provided by Secret Agent)",
            credential.ssid,
            conn_uuid
        );

        Ok(conn_uuid)
    }

    async fn delete_profile(&self, system_id: &str) -> Result<()> {
        let paths = self.get_connection_paths().await?;

        for path in paths {
            let path_str = path.as_str();
            let settings = match self.get_connection_settings(path_str).await {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Some(uuid) = Self::get_connection_uuid(&settings) {
                if uuid == system_id {
                    let proxy = ConnectionProxy::builder(&self.connection)
                        .destination(NM_SERVICE)?
                        .path(path_str)?
                        .build()
                        .await?;

                    proxy.delete().await.map_err(|e| {
                        let err_str = e.to_string();
                        if err_str.contains("PermissionDenied") {
                            Error::permission_denied(
                                "Cannot delete connection. You may need root privileges.",
                            )
                        } else {
                            Error::internal(format!("Failed to delete connection: {e}"))
                        }
                    })?;

                    return Ok(());
                }
            }
        }

        // Not found - this is OK (idempotent delete)
        tracing::warn!("Connection with UUID {} not found for deletion", system_id);
        Ok(())
    }

    fn platform_info(&self) -> PlatformInfo {
        PlatformInfo {
            name: "NetworkManager".to_string(),
            version: self.version.clone(),
            features: vec![
                "list_networks".to_string(),
                "get_credentials".to_string(),
                "create_profile".to_string(),
                "delete_profile".to_string(),
                "secret_agent".to_string(),
            ],
        }
    }

    fn source_platform(&self) -> SourcePlatform {
        SourcePlatform::NetworkManager
    }
}

// D-Bus proxy definitions for NetworkManager

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Connection {
    fn get_settings(&self) -> zbus::Result<NMSettings>;

    fn get_secrets(&self, setting_name: &str) -> zbus::Result<NMSettings>;

    fn delete(&self) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
trait Settings {
    fn add_connection2(
        &self,
        settings: NMSettings,
        flags: u32,
        args: HashMap<String, OwnedValue>,
    ) -> zbus::Result<OwnedObjectPath>;
}
