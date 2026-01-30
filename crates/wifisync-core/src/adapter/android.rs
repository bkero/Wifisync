//! Android WifiManager adapter
//!
//! This adapter provides WiFi credential management on Android using the
//! WifiNetworkSuggestion API (Android 10+, API 29+).
//!
//! Unlike the Linux NetworkManager adapter which uses D-Bus for all operations,
//! the Android adapter operates through JNI, with the actual WiFi operations
//! performed by Kotlin code on the Android side.
//!
//! # Architecture
//!
//! ```text
//! Rust Core (wifisync-core)
//!        │
//!        ▼
//!    AndroidAdapter (this module)
//!        │
//!        ▼ JNI calls
//!    Kotlin WifiManagerWrapper
//!        │
//!        ▼
//!    Android WifiManager API
//! ```
//!
//! # Credential Delivery
//!
//! Unlike Linux where passwords are provided on-demand via Secret Agent,
//! Android requires passwords to be embedded directly in WifiNetworkSuggestion
//! objects at creation time. This is a platform constraint.
//!
//! # Extraction Limitations
//!
//! - **Non-rooted devices**: Can only list Wifisync-managed suggestions
//! - **Rooted devices**: Can extract all credentials from WifiConfigStore.xml

use async_trait::async_trait;
use std::collections::HashMap;

use super::{NetworkAdapter, NetworkInfo, PlatformInfo};
use crate::models::{SecurityType, SourcePlatform, WifiCredential};
use crate::{Error, Result};

/// Callback interface for JNI operations
///
/// This trait defines the operations that must be implemented on the Kotlin side
/// and called through JNI. The actual JNI binding is in the `wifisync-jni` crate.
pub trait AndroidJniCallback: Send + Sync {
    /// Get the Android API level
    fn get_api_level(&self) -> i32;

    /// Check if the device has root access
    fn has_root_access(&self) -> bool;

    /// List currently installed network suggestions
    fn list_suggestions(&self) -> std::result::Result<Vec<SuggestionInfo>, String>;

    /// Add a network suggestion
    fn add_suggestion(&self, suggestion: SuggestionRequest) -> std::result::Result<String, String>;

    /// Remove a network suggestion by ID
    fn remove_suggestion(&self, suggestion_id: &str) -> std::result::Result<(), String>;

    /// Read WifiConfigStore.xml (requires root)
    fn read_wifi_config_store(&self) -> std::result::Result<String, String>;
}

/// Information about a network suggestion
#[derive(Debug, Clone)]
pub struct SuggestionInfo {
    /// Unique identifier for this suggestion
    pub id: String,
    /// Network SSID
    pub ssid: String,
    /// Security type
    pub security_type: SecurityType,
    /// Whether the network is hidden
    pub hidden: bool,
    /// Installation timestamp (milliseconds since epoch)
    pub installed_at: i64,
}

/// Request to create a network suggestion
#[derive(Debug, Clone)]
pub struct SuggestionRequest {
    /// Network SSID
    pub ssid: String,
    /// Security type (WPA2 or WPA3)
    pub security_type: SecurityType,
    /// Pre-shared key (password)
    pub password: String,
    /// Whether the network is hidden
    pub hidden: bool,
}

/// Android platform capabilities
#[derive(Debug, Clone)]
pub struct AndroidCapabilities {
    /// Android API level (e.g., 29 for Android 10)
    pub api_level: i32,
    /// Whether the device has root access
    pub has_root: bool,
    /// Number of currently installed suggestions
    pub suggestion_count: usize,
    /// Maximum suggestions allowed (typically ~50)
    pub suggestion_limit: usize,
    /// Whether credential extraction is supported (requires root)
    pub extraction_supported: bool,
}

impl Default for AndroidCapabilities {
    fn default() -> Self {
        Self {
            api_level: 0,
            has_root: false,
            suggestion_count: 0,
            suggestion_limit: 50, // Android's default limit
            extraction_supported: false,
        }
    }
}

/// Android WifiManager adapter
///
/// This adapter manages WiFi credentials on Android through the WifiNetworkSuggestion API.
/// It requires a JNI callback implementation to communicate with the Kotlin layer.
pub struct AndroidAdapter {
    /// JNI callback for Android operations
    callback: Box<dyn AndroidJniCallback>,
    /// Cached capabilities
    capabilities: AndroidCapabilities,
    /// Local tracking of installed suggestions (SSID -> suggestion ID)
    /// Used for detecting user-removed suggestions and re-suggestion
    #[allow(dead_code)]
    suggestion_tracking: HashMap<String, String>,
}

impl std::fmt::Debug for AndroidAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AndroidAdapter")
            .field("capabilities", &self.capabilities)
            .field("suggestion_tracking", &self.suggestion_tracking)
            .finish_non_exhaustive()
    }
}

impl AndroidAdapter {
    /// Create a new Android adapter with the given JNI callback
    pub fn new(callback: Box<dyn AndroidJniCallback>) -> Result<Self> {
        let api_level = callback.get_api_level();

        // Require API 29+ (Android 10+)
        if api_level < 29 {
            return Err(Error::UnsupportedPlatform {
                platform: format!("Android API {} (minimum required: 29)", api_level),
            });
        }

        let has_root = callback.has_root_access();

        let capabilities = AndroidCapabilities {
            api_level,
            has_root,
            suggestion_count: 0,
            suggestion_limit: 50,
            extraction_supported: has_root,
        };

        Ok(Self {
            callback,
            capabilities,
            suggestion_tracking: HashMap::new(),
        })
    }

    /// Get the current capabilities
    pub fn capabilities(&self) -> &AndroidCapabilities {
        &self.capabilities
    }

    /// Refresh capabilities from the device
    pub fn refresh_capabilities(&mut self) {
        self.capabilities.has_root = self.callback.has_root_access();
        self.capabilities.extraction_supported = self.capabilities.has_root;

        if let Ok(suggestions) = self.callback.list_suggestions() {
            self.capabilities.suggestion_count = suggestions.len();
        }
    }

    /// Parse WifiConfigStore.xml to extract credentials (requires root)
    fn parse_wifi_config_store(&self, xml_content: &str) -> Result<Vec<WifiCredential>> {
        let mut credentials = Vec::new();

        // Parse the XML content
        // WifiConfigStore.xml structure (simplified):
        // <WifiConfigStoreData>
        //   <NetworkList>
        //     <Network>
        //       <WifiConfiguration>
        //         <string name="ConfigKey">SSID-WPA_PSK</string>
        //         <string name="SSID">"NetworkName"</string>
        //         <string name="PreSharedKey">"password"</string>
        //         <int name="SecurityType" value="2" />
        //       </WifiConfiguration>
        //     </Network>
        //   </NetworkList>
        // </WifiConfigStoreData>

        // Simple XML parsing - in production, use a proper XML parser
        let lines: Vec<&str> = xml_content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            if line.contains("<Network>") {
                // Parse this network entry
                let mut ssid: Option<String> = None;
                let mut password: Option<String> = None;
                let mut security_type = SecurityType::Unknown;
                let mut hidden = false;

                while i < lines.len() && !lines[i].contains("</Network>") {
                    let inner_line = lines[i].trim();

                    // Parse SSID
                    if inner_line.contains("name=\"SSID\"") {
                        if let Some(value) = extract_string_value(inner_line) {
                            // Remove surrounding quotes if present
                            ssid = Some(value.trim_matches('"').to_string());
                        }
                    }

                    // Parse PreSharedKey
                    if inner_line.contains("name=\"PreSharedKey\"") {
                        if let Some(value) = extract_string_value(inner_line) {
                            // Remove surrounding quotes if present
                            password = Some(value.trim_matches('"').to_string());
                        }
                    }

                    // Parse SecurityType
                    if inner_line.contains("name=\"SecurityType\"") {
                        if let Some(value) = extract_int_value(inner_line) {
                            security_type = android_security_type_to_enum(value);
                        }
                    }

                    // Parse HiddenSSID
                    if inner_line.contains("name=\"HiddenSSID\"") {
                        if let Some(value) = extract_boolean_value(inner_line) {
                            hidden = value;
                        }
                    }

                    i += 1;
                }

                // Only add if we have SSID and password, and it's not enterprise
                if let (Some(ssid), Some(password)) = (ssid, password) {
                    if !security_type.is_enterprise() && !security_type.is_open() {
                        let credential = WifiCredential::builder(&ssid)
                            .password(password)
                            .security_type(security_type)
                            .source_platform(SourcePlatform::Android)
                            .hidden(hidden)
                            .build();
                        credentials.push(credential);
                    }
                }
            }

            i += 1;
        }

        Ok(credentials)
    }
}

#[async_trait]
impl NetworkAdapter for AndroidAdapter {
    async fn list_networks(&self) -> Result<Vec<NetworkInfo>> {
        // On Android, we can only list Wifisync-managed suggestions (non-root)
        // or all networks (root via WifiConfigStore.xml)

        let suggestions = self.callback.list_suggestions().map_err(|e| {
            Error::service_unavailable(format!("Failed to list suggestions: {}", e))
        })?;

        let networks: Vec<NetworkInfo> = suggestions
            .into_iter()
            .map(|s| NetworkInfo {
                ssid: s.ssid,
                security_type: s.security_type,
                hidden: s.hidden,
                system_id: Some(s.id),
            })
            .collect();

        Ok(networks)
    }

    async fn get_credentials(&self, ssid: &str) -> Result<WifiCredential> {
        // Credential extraction requires root access on Android
        if !self.capabilities.has_root {
            return Err(Error::permission_denied(
                "Credential extraction on Android requires root access. \
                 On non-rooted devices, you can only import credentials from other sources.",
            ));
        }

        // Read WifiConfigStore.xml
        let xml_content = self.callback.read_wifi_config_store().map_err(|e| {
            Error::permission_denied(format!(
                "Failed to read WifiConfigStore.xml: {}. Ensure the app has root access.",
                e
            ))
        })?;

        // Parse and find the requested network
        let credentials = self.parse_wifi_config_store(&xml_content)?;

        credentials
            .into_iter()
            .find(|c| c.ssid == ssid)
            .ok_or_else(|| Error::NetworkNotFound {
                ssid: ssid.to_string(),
            })
    }

    async fn create_profile(&self, credential: &WifiCredential) -> Result<String> {
        // On Android, we create a WifiNetworkSuggestion WITH the password
        // (unlike Linux where password is provided on-demand via Secret Agent)

        use secrecy::ExposeSecret;

        let request = SuggestionRequest {
            ssid: credential.ssid.clone(),
            security_type: credential.security_type,
            password: credential.password.expose_secret().to_string(),
            hidden: credential.hidden,
        };

        let suggestion_id = self.callback.add_suggestion(request).map_err(|e| {
            // Map Android error codes to user-friendly messages
            if e.contains("ADD_DUPLICATE") {
                Error::AlreadyInstalled {
                    ssid: credential.ssid.clone(),
                }
            } else if e.contains("ADD_EXCEEDS_MAX") {
                Error::Internal {
                    message: format!(
                        "Cannot add more network suggestions. \
                         Android limits apps to {} suggestions. \
                         Remove some networks first.",
                        self.capabilities.suggestion_limit
                    ),
                }
            } else {
                Error::Internal {
                    message: format!("Failed to add network suggestion: {}", e),
                }
            }
        })?;

        Ok(suggestion_id)
    }

    async fn delete_profile(&self, system_id: &str) -> Result<()> {
        self.callback.remove_suggestion(system_id).map_err(|e| {
            Error::Internal {
                message: format!("Failed to remove network suggestion: {}", e),
            }
        })?;

        Ok(())
    }

    fn platform_info(&self) -> PlatformInfo {
        let mut features = vec![
            format!("api_level:{}", self.capabilities.api_level),
            format!("suggestion_limit:{}", self.capabilities.suggestion_limit),
        ];

        if self.capabilities.has_root {
            features.push("root:true".to_string());
            features.push("extraction:supported".to_string());
        } else {
            features.push("root:false".to_string());
            features.push("extraction:unsupported".to_string());
        }

        // Android does NOT support Secret Agent pattern
        features.push("secret_agent:unsupported".to_string());

        PlatformInfo {
            name: "Android WifiManager".to_string(),
            version: Some(format!("API {}", self.capabilities.api_level)),
            features,
        }
    }

    fn source_platform(&self) -> SourcePlatform {
        SourcePlatform::Android
    }
}

// Helper functions for XML parsing

fn extract_string_value(line: &str) -> Option<String> {
    // Parse: <string name="Key">value</string>
    let start = line.find('>')? + 1;
    let end = line.rfind('<')?;
    if start < end {
        Some(line[start..end].to_string())
    } else {
        None
    }
}

fn extract_int_value(line: &str) -> Option<i32> {
    // Parse: <int name="Key" value="123" />
    let value_start = line.find("value=\"")? + 7;
    let value_end = line[value_start..].find('"')? + value_start;
    line[value_start..value_end].parse().ok()
}

fn extract_boolean_value(line: &str) -> Option<bool> {
    // Parse: <boolean name="Key" value="true" />
    let value_start = line.find("value=\"")? + 7;
    let value_end = line[value_start..].find('"')? + value_start;
    match &line[value_start..value_end] {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn android_security_type_to_enum(security_type: i32) -> SecurityType {
    // Android security type constants (from WifiConfiguration)
    // https://developer.android.com/reference/android/net/wifi/WifiConfiguration
    match security_type {
        0 => SecurityType::Open,           // SECURITY_TYPE_OPEN
        1 => SecurityType::Wep,            // SECURITY_TYPE_WEP
        2 => SecurityType::Wpa2Psk,        // SECURITY_TYPE_PSK (WPA/WPA2)
        3 => SecurityType::Wpa2Enterprise, // SECURITY_TYPE_EAP
        4 => SecurityType::Wpa3Psk,        // SECURITY_TYPE_SAE (WPA3)
        5 => SecurityType::Wpa2Wpa3Psk,    // SECURITY_TYPE_PSK_SAE_TRANSITION
        6 => SecurityType::Wpa3Enterprise, // SECURITY_TYPE_EAP_WPA3_ENTERPRISE
        _ => SecurityType::Unknown,
    }
}

/// Mock JNI callback for testing
#[cfg(test)]
pub struct MockAndroidCallback {
    pub api_level: i32,
    pub has_root: bool,
    pub suggestions: Vec<SuggestionInfo>,
    pub wifi_config_xml: Option<String>,
}

#[cfg(test)]
impl Default for MockAndroidCallback {
    fn default() -> Self {
        Self {
            api_level: 30, // Android 11
            has_root: false,
            suggestions: Vec::new(),
            wifi_config_xml: None,
        }
    }
}

#[cfg(test)]
impl AndroidJniCallback for MockAndroidCallback {
    fn get_api_level(&self) -> i32 {
        self.api_level
    }

    fn has_root_access(&self) -> bool {
        self.has_root
    }

    fn list_suggestions(&self) -> std::result::Result<Vec<SuggestionInfo>, String> {
        Ok(self.suggestions.clone())
    }

    fn add_suggestion(&self, suggestion: SuggestionRequest) -> std::result::Result<String, String> {
        Ok(format!("suggestion_{}", suggestion.ssid))
    }

    fn remove_suggestion(&self, _suggestion_id: &str) -> std::result::Result<(), String> {
        Ok(())
    }

    fn read_wifi_config_store(&self) -> std::result::Result<String, String> {
        self.wifi_config_xml
            .clone()
            .ok_or_else(|| "Root access required".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    // =========================================================================
    // Adapter Creation Tests
    // =========================================================================

    #[test]
    fn test_api_level_check_rejects_old_versions() {
        let callback = MockAndroidCallback {
            api_level: 28, // Android 9 - too old
            ..Default::default()
        };

        let result = AndroidAdapter::new(Box::new(callback));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("minimum required: 29"));
    }

    #[test]
    fn test_api_level_check_accepts_api_29() {
        let callback = MockAndroidCallback {
            api_level: 29, // Android 10 - minimum supported
            ..Default::default()
        };

        let result = AndroidAdapter::new(Box::new(callback));
        assert!(result.is_ok());
    }

    #[test]
    fn test_api_level_check_accepts_newer_versions() {
        for api_level in [30, 31, 32, 33, 34] {
            let callback = MockAndroidCallback {
                api_level,
                ..Default::default()
            };

            let result = AndroidAdapter::new(Box::new(callback));
            assert!(result.is_ok(), "API level {} should be accepted", api_level);
        }
    }

    #[test]
    fn test_adapter_creation_non_rooted() {
        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        assert_eq!(adapter.capabilities().api_level, 30);
        assert!(!adapter.capabilities().has_root);
        assert!(!adapter.capabilities().extraction_supported);
        assert_eq!(adapter.capabilities().suggestion_limit, 50);
    }

    #[test]
    fn test_adapter_creation_rooted() {
        let callback = MockAndroidCallback {
            has_root: true,
            ..Default::default()
        };
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        assert!(adapter.capabilities().has_root);
        assert!(adapter.capabilities().extraction_supported);
    }

    // =========================================================================
    // Platform Info Tests
    // =========================================================================

    #[test]
    fn test_platform_info_basic() {
        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        let info = adapter.platform_info();
        assert_eq!(info.name, "Android WifiManager");
        assert_eq!(info.version, Some("API 30".to_string()));
    }

    #[test]
    fn test_platform_info_features_non_rooted() {
        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        let info = adapter.platform_info();
        assert!(info.features.contains(&"secret_agent:unsupported".to_string()));
        assert!(info.features.contains(&"root:false".to_string()));
        assert!(info.features.contains(&"extraction:unsupported".to_string()));
        assert!(info.features.contains(&"api_level:30".to_string()));
    }

    #[test]
    fn test_platform_info_features_rooted() {
        let callback = MockAndroidCallback {
            has_root: true,
            ..Default::default()
        };
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        let info = adapter.platform_info();
        assert!(info.features.contains(&"root:true".to_string()));
        assert!(info.features.contains(&"extraction:supported".to_string()));
    }

    #[test]
    fn test_source_platform() {
        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        assert_eq!(adapter.source_platform(), SourcePlatform::Android);
    }

    // =========================================================================
    // Security Type Conversion Tests
    // =========================================================================

    #[test]
    fn test_security_type_conversion_all_types() {
        assert_eq!(android_security_type_to_enum(0), SecurityType::Open);
        assert_eq!(android_security_type_to_enum(1), SecurityType::Wep);
        assert_eq!(android_security_type_to_enum(2), SecurityType::Wpa2Psk);
        assert_eq!(android_security_type_to_enum(3), SecurityType::Wpa2Enterprise);
        assert_eq!(android_security_type_to_enum(4), SecurityType::Wpa3Psk);
        assert_eq!(android_security_type_to_enum(5), SecurityType::Wpa2Wpa3Psk);
        assert_eq!(android_security_type_to_enum(6), SecurityType::Wpa3Enterprise);
    }

    #[test]
    fn test_security_type_conversion_unknown() {
        assert_eq!(android_security_type_to_enum(7), SecurityType::Unknown);
        assert_eq!(android_security_type_to_enum(100), SecurityType::Unknown);
        assert_eq!(android_security_type_to_enum(-1), SecurityType::Unknown);
    }

    // =========================================================================
    // XML Helper Function Tests
    // =========================================================================

    #[test]
    fn test_extract_string_value() {
        assert_eq!(
            extract_string_value(r#"<string name="SSID">"MyNetwork"</string>"#),
            Some("\"MyNetwork\"".to_string())
        );
        assert_eq!(
            extract_string_value(r#"<string name="Key">value</string>"#),
            Some("value".to_string())
        );
        assert_eq!(extract_string_value(r#"<string name="Empty"></string>"#), None);
        assert_eq!(extract_string_value(r#"invalid"#), None);
    }

    #[test]
    fn test_extract_int_value() {
        assert_eq!(
            extract_int_value(r#"<int name="SecurityType" value="2" />"#),
            Some(2)
        );
        assert_eq!(
            extract_int_value(r#"<int name="Priority" value="100" />"#),
            Some(100)
        );
        assert_eq!(
            extract_int_value(r#"<int name="Negative" value="-5" />"#),
            Some(-5)
        );
        assert_eq!(extract_int_value(r#"<int name="Invalid" value="abc" />"#), None);
        assert_eq!(extract_int_value(r#"no value attribute"#), None);
    }

    #[test]
    fn test_extract_boolean_value() {
        assert_eq!(
            extract_boolean_value(r#"<boolean name="Hidden" value="true" />"#),
            Some(true)
        );
        assert_eq!(
            extract_boolean_value(r#"<boolean name="Hidden" value="false" />"#),
            Some(false)
        );
        assert_eq!(
            extract_boolean_value(r#"<boolean name="Invalid" value="yes" />"#),
            None
        );
        assert_eq!(extract_boolean_value(r#"no value"#), None);
    }

    // =========================================================================
    // XML Parsing Tests
    // =========================================================================

    #[test]
    fn test_xml_parsing_single_network() {
        let xml = r#"
<WifiConfigStoreData>
  <NetworkList>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"TestNetwork"</string>
        <string name="PreSharedKey">"password123"</string>
        <int name="SecurityType" value="2" />
        <boolean name="HiddenSSID" value="false" />
      </WifiConfiguration>
    </Network>
  </NetworkList>
</WifiConfigStoreData>
"#;

        let callback = MockAndroidCallback {
            has_root: true,
            wifi_config_xml: Some(xml.to_string()),
            ..Default::default()
        };

        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();
        let credentials = adapter.parse_wifi_config_store(xml).unwrap();

        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].ssid, "TestNetwork");
        assert_eq!(credentials[0].password.expose_secret(), "password123");
        assert_eq!(credentials[0].security_type, SecurityType::Wpa2Psk);
        assert!(!credentials[0].hidden);
        assert_eq!(credentials[0].source_platform, SourcePlatform::Android);
    }

    #[test]
    fn test_xml_parsing_multiple_networks() {
        let xml = r#"
<WifiConfigStoreData>
  <NetworkList>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"HomeWifi"</string>
        <string name="PreSharedKey">"homepass"</string>
        <int name="SecurityType" value="2" />
      </WifiConfiguration>
    </Network>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"OfficeWifi"</string>
        <string name="PreSharedKey">"officepass"</string>
        <int name="SecurityType" value="4" />
      </WifiConfiguration>
    </Network>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"CafeWifi"</string>
        <string name="PreSharedKey">"cafepass"</string>
        <int name="SecurityType" value="5" />
      </WifiConfiguration>
    </Network>
  </NetworkList>
</WifiConfigStoreData>
"#;

        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();
        let credentials = adapter.parse_wifi_config_store(xml).unwrap();

        assert_eq!(credentials.len(), 3);

        assert_eq!(credentials[0].ssid, "HomeWifi");
        assert_eq!(credentials[0].security_type, SecurityType::Wpa2Psk);

        assert_eq!(credentials[1].ssid, "OfficeWifi");
        assert_eq!(credentials[1].security_type, SecurityType::Wpa3Psk);

        assert_eq!(credentials[2].ssid, "CafeWifi");
        assert_eq!(credentials[2].security_type, SecurityType::Wpa2Wpa3Psk);
    }

    #[test]
    fn test_xml_parsing_hidden_network() {
        let xml = r#"
<WifiConfigStoreData>
  <NetworkList>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"HiddenNetwork"</string>
        <string name="PreSharedKey">"secretpass"</string>
        <int name="SecurityType" value="2" />
        <boolean name="HiddenSSID" value="true" />
      </WifiConfiguration>
    </Network>
  </NetworkList>
</WifiConfigStoreData>
"#;

        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();
        let credentials = adapter.parse_wifi_config_store(xml).unwrap();

        assert_eq!(credentials.len(), 1);
        assert!(credentials[0].hidden);
    }

    #[test]
    fn test_xml_parsing_skips_enterprise_networks() {
        let xml = r#"
<WifiConfigStoreData>
  <NetworkList>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"PersonalWifi"</string>
        <string name="PreSharedKey">"personalpass"</string>
        <int name="SecurityType" value="2" />
      </WifiConfiguration>
    </Network>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"CorpWifi"</string>
        <string name="PreSharedKey">"corppass"</string>
        <int name="SecurityType" value="3" />
      </WifiConfiguration>
    </Network>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"WPA3Enterprise"</string>
        <string name="PreSharedKey">"wpa3pass"</string>
        <int name="SecurityType" value="6" />
      </WifiConfiguration>
    </Network>
  </NetworkList>
</WifiConfigStoreData>
"#;

        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();
        let credentials = adapter.parse_wifi_config_store(xml).unwrap();

        // Should only get the personal network, enterprise networks are skipped
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].ssid, "PersonalWifi");
    }

    #[test]
    fn test_xml_parsing_skips_open_networks() {
        let xml = r#"
<WifiConfigStoreData>
  <NetworkList>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"SecureWifi"</string>
        <string name="PreSharedKey">"securepass"</string>
        <int name="SecurityType" value="2" />
      </WifiConfiguration>
    </Network>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"FreeWifi"</string>
        <string name="PreSharedKey">""</string>
        <int name="SecurityType" value="0" />
      </WifiConfiguration>
    </Network>
  </NetworkList>
</WifiConfigStoreData>
"#;

        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();
        let credentials = adapter.parse_wifi_config_store(xml).unwrap();

        // Should only get the secure network, open network is skipped
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].ssid, "SecureWifi");
    }

    #[test]
    fn test_xml_parsing_empty_network_list() {
        let xml = r#"
<WifiConfigStoreData>
  <NetworkList>
  </NetworkList>
</WifiConfigStoreData>
"#;

        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();
        let credentials = adapter.parse_wifi_config_store(xml).unwrap();

        assert!(credentials.is_empty());
    }

    #[test]
    fn test_xml_parsing_network_without_password() {
        let xml = r#"
<WifiConfigStoreData>
  <NetworkList>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"NoPassword"</string>
        <int name="SecurityType" value="2" />
      </WifiConfiguration>
    </Network>
  </NetworkList>
</WifiConfigStoreData>
"#;

        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();
        let credentials = adapter.parse_wifi_config_store(xml).unwrap();

        // Network without password should be skipped
        assert!(credentials.is_empty());
    }

    // =========================================================================
    // Async Operation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_list_networks_with_suggestions() {
        let callback = MockAndroidCallback {
            suggestions: vec![
                SuggestionInfo {
                    id: "id1".to_string(),
                    ssid: "Network1".to_string(),
                    security_type: SecurityType::Wpa2Psk,
                    hidden: false,
                    installed_at: 1000,
                },
                SuggestionInfo {
                    id: "id2".to_string(),
                    ssid: "Network2".to_string(),
                    security_type: SecurityType::Wpa3Psk,
                    hidden: true,
                    installed_at: 2000,
                },
            ],
            ..Default::default()
        };

        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();
        let networks = adapter.list_networks().await.unwrap();

        assert_eq!(networks.len(), 2);
        assert_eq!(networks[0].ssid, "Network1");
        assert_eq!(networks[0].security_type, SecurityType::Wpa2Psk);
        assert!(!networks[0].hidden);
        assert_eq!(networks[0].system_id, Some("id1".to_string()));

        assert_eq!(networks[1].ssid, "Network2");
        assert!(networks[1].hidden);
    }

    #[tokio::test]
    async fn test_list_networks_empty() {
        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();
        let networks = adapter.list_networks().await.unwrap();

        assert!(networks.is_empty());
    }

    #[tokio::test]
    async fn test_create_profile() {
        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        let credential = crate::WifiCredential::new(
            "TestNetwork",
            "testpassword",
            SecurityType::Wpa2Psk,
            SourcePlatform::Manual,
        );

        let result = adapter.create_profile(&credential).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "suggestion_TestNetwork");
    }

    #[tokio::test]
    async fn test_delete_profile() {
        let callback = MockAndroidCallback::default();
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        let result = adapter.delete_profile("some_id").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_credentials_without_root() {
        let callback = MockAndroidCallback {
            has_root: false,
            ..Default::default()
        };
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        let result = adapter.get_credentials("AnyNetwork").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("root access"));
    }

    #[tokio::test]
    async fn test_get_credentials_with_root() {
        let xml = r#"
<WifiConfigStoreData>
  <NetworkList>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"TargetNetwork"</string>
        <string name="PreSharedKey">"targetpass"</string>
        <int name="SecurityType" value="2" />
      </WifiConfiguration>
    </Network>
  </NetworkList>
</WifiConfigStoreData>
"#;

        let callback = MockAndroidCallback {
            has_root: true,
            wifi_config_xml: Some(xml.to_string()),
            ..Default::default()
        };
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        let result = adapter.get_credentials("TargetNetwork").await;
        assert!(result.is_ok());

        let cred = result.unwrap();
        assert_eq!(cred.ssid, "TargetNetwork");
        assert_eq!(cred.password.expose_secret(), "targetpass");
    }

    #[tokio::test]
    async fn test_get_credentials_network_not_found() {
        let xml = r#"
<WifiConfigStoreData>
  <NetworkList>
    <Network>
      <WifiConfiguration>
        <string name="SSID">"OtherNetwork"</string>
        <string name="PreSharedKey">"otherpass"</string>
        <int name="SecurityType" value="2" />
      </WifiConfiguration>
    </Network>
  </NetworkList>
</WifiConfigStoreData>
"#;

        let callback = MockAndroidCallback {
            has_root: true,
            wifi_config_xml: Some(xml.to_string()),
            ..Default::default()
        };
        let adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        let result = adapter.get_credentials("NonExistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    // =========================================================================
    // Capability Refresh Tests
    // =========================================================================

    #[test]
    fn test_refresh_capabilities() {
        let callback = MockAndroidCallback {
            suggestions: vec![
                SuggestionInfo {
                    id: "id1".to_string(),
                    ssid: "Net1".to_string(),
                    security_type: SecurityType::Wpa2Psk,
                    hidden: false,
                    installed_at: 1000,
                },
            ],
            ..Default::default()
        };

        let mut adapter = AndroidAdapter::new(Box::new(callback)).unwrap();

        // Initially suggestion_count is 0 (not refreshed yet)
        assert_eq!(adapter.capabilities().suggestion_count, 0);

        // After refresh, should reflect actual count
        adapter.refresh_capabilities();
        assert_eq!(adapter.capabilities().suggestion_count, 1);
    }
}
