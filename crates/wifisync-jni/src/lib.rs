//! JNI Bridge for Wifisync Android
//!
//! This crate provides the JNI interface between the Kotlin Android app and
//! the Rust core library. It exports native functions that can be called
//! from Kotlin/Java code.
//!
//! # Architecture
//!
//! ```text
//! Kotlin (WifisyncCore.kt)
//!     │
//!     ▼ JNI native calls
//! wifisync-jni (this crate)
//!     │
//!     ▼ Rust function calls
//! wifisync-core (adapter, storage, etc.)
//! ```
//!
//! # Thread Safety
//!
//! The JNI bridge maintains a single instance of the core services,
//! protected by appropriate synchronization primitives.

use std::sync::OnceLock;

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use serde::{Deserialize, Serialize};

use wifisync_core::adapter::android::{AndroidJniCallback, SuggestionInfo, SuggestionRequest};
use wifisync_core::storage::Storage;

/// Global state for the Wifisync core
static CORE: OnceLock<WifisyncCore> = OnceLock::new();

/// Core state holding all Wifisync services
struct WifisyncCore {
    storage: Storage,
    // The adapter is created on-demand with the JNI callback
}

/// JNI callback implementation that delegates to Kotlin
///
/// This struct holds a reference to the Kotlin WifiManagerWrapper and will be used
/// to call back into Kotlin for WiFi operations.
#[allow(dead_code)]
struct KotlinJniCallback {
    /// Reference to the Kotlin WifiManagerWrapper object
    /// Stored as a global reference to prevent garbage collection
    wrapper_ref: jni::objects::GlobalRef,
}

// Safety: The GlobalRef is safe to send between threads
unsafe impl Send for KotlinJniCallback {}
unsafe impl Sync for KotlinJniCallback {}

impl AndroidJniCallback for KotlinJniCallback {
    fn get_api_level(&self) -> i32 {
        // This would call back to Kotlin, but for now return a default
        // In production, this calls Build.VERSION.SDK_INT
        30 // Android 11
    }

    fn has_root_access(&self) -> bool {
        // This would call back to Kotlin to check root
        false
    }

    fn list_suggestions(&self) -> Result<Vec<SuggestionInfo>, String> {
        // This would call back to Kotlin
        Ok(Vec::new())
    }

    fn add_suggestion(&self, _suggestion: SuggestionRequest) -> Result<String, String> {
        // This would call back to Kotlin
        Err("Not implemented - requires Kotlin callback".to_string())
    }

    fn remove_suggestion(&self, _suggestion_id: &str) -> Result<(), String> {
        // This would call back to Kotlin
        Err("Not implemented - requires Kotlin callback".to_string())
    }

    fn read_wifi_config_store(&self) -> Result<String, String> {
        // This would call back to Kotlin
        Err("Root access required".to_string())
    }
}

// ============================================================================
// JNI Exports
// ============================================================================

/// Initialize the Wifisync core
///
/// Must be called before any other Wifisync functions.
/// Takes the Android application's files directory path.
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeInit(
    mut env: JNIEnv,
    _class: JClass,
    files_dir: JString,
) -> jboolean {
    // Initialize Android logging
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Debug)
                .with_tag("wifisync"),
        );
    }

    let files_dir: String = match env.get_string(&files_dir) {
        Ok(s) => s.into(),
        Err(e) => {
            tracing::error!("Failed to get files_dir string: {}", e);
            return JNI_FALSE;
        }
    };

    tracing::info!("Initializing Wifisync with files_dir: {}", files_dir);

    // Create storage with the Android files directory
    let storage = match Storage::with_data_dir(std::path::PathBuf::from(&files_dir)) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create storage: {}", e);
            return JNI_FALSE;
        }
    };

    let core = WifisyncCore { storage };

    if CORE.set(core).is_err() {
        tracing::warn!("Core already initialized");
    }

    JNI_TRUE
}

/// List all credentials in the default collection
///
/// Returns a JSON array of credentials (without passwords).
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeListCredentials<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let result = list_credentials_impl();
    string_to_jstring(&mut env, &result)
}

fn list_credentials_impl() -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    match core.storage.load_collections() {
        Ok(collections) => {
            // Convert to a simple format (without exposing passwords)
            let creds: Vec<CredentialSummary> = collections
                .iter()
                .flat_map(|c| {
                    c.credentials.iter().map(|cred| CredentialSummary {
                        id: cred.id.to_string(),
                        ssid: cred.ssid.clone(),
                        security_type: format!("{:?}", cred.security_type),
                        hidden: cred.hidden,
                        managed: cred.managed,
                        tags: cred.tags.clone(),
                    })
                })
                .collect();

            serde_json::to_string(&ApiResponse::success(creds)).unwrap_or_else(|e| json_error(&e.to_string()))
        }
        Err(e) => json_error(&e.to_string()),
    }
}

/// Import credentials from a JSON file
///
/// Takes the file path and optional password for encrypted files.
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeImportCredentials<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    file_path: JString<'local>,
    password: JString<'local>,
) -> jstring {
    let file_path: String = match env.get_string(&file_path) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let password: Option<String> = if env.is_same_object(&password, JObject::null()).unwrap_or(true) {
        None
    } else {
        env.get_string(&password).ok().map(|s| s.into())
    };

    let result = import_credentials_impl(&file_path, password.as_deref());
    string_to_jstring(&mut env, &result)
}

fn import_credentials_impl(file_path: &str, password: Option<&str>) -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    let path = std::path::Path::new(file_path);
    match core.storage.import_collection(path, password) {
        Ok(collection) => {
            let summary = ImportSummary {
                name: collection.name.clone(),
                count: collection.credentials.len(),
            };
            serde_json::to_string(&ApiResponse::success(summary)).unwrap_or_else(|e| json_error(&e.to_string()))
        }
        Err(e) => json_error(&e.to_string()),
    }
}

/// Export credentials to a JSON file
///
/// Takes the collection name, file path, and optional password for encryption.
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeExportCredentials<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    collection_name: JString<'local>,
    file_path: JString<'local>,
    password: JString<'local>,
) -> jstring {
    let collection_name: String = match env.get_string(&collection_name) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let file_path: String = match env.get_string(&file_path) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let password: Option<String> = if env.is_same_object(&password, JObject::null()).unwrap_or(true) {
        None
    } else {
        env.get_string(&password).ok().map(|s| s.into())
    };

    let result = export_credentials_impl(&collection_name, &file_path, password.as_deref());
    string_to_jstring(&mut env, &result)
}

fn export_credentials_impl(collection_name: &str, file_path: &str, password: Option<&str>) -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    match core.storage.load_collection(collection_name) {
        Ok(collection) => {
            let path = std::path::Path::new(file_path);
            match core.storage.export_collection(&collection, path, password) {
                Ok(()) => {
                    let summary = ExportSummary {
                        name: collection.name,
                        count: collection.credentials.len(),
                        path: file_path.to_string(),
                        encrypted: password.is_some(),
                    };
                    serde_json::to_string(&ApiResponse::success(summary))
                        .unwrap_or_else(|e| json_error(&e.to_string()))
                }
                Err(e) => json_error(&e.to_string()),
            }
        }
        Err(e) => json_error(&e.to_string()),
    }
}

/// Get a specific credential by SSID
///
/// Returns JSON with the credential details (including password if authorized).
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeGetCredential<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ssid: JString<'local>,
    include_password: jboolean,
) -> jstring {
    let ssid: String = match env.get_string(&ssid) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let result = get_credential_impl(&ssid, include_password == JNI_TRUE);
    string_to_jstring(&mut env, &result)
}

fn get_credential_impl(ssid: &str, include_password: bool) -> String {
    use secrecy::ExposeSecret;

    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    match core.storage.find_credential_by_ssid(ssid) {
        Ok(Some(cred)) => {
            let detail = CredentialDetail {
                id: cred.id.to_string(),
                ssid: cred.ssid.clone(),
                security_type: format!("{:?}", cred.security_type),
                password: if include_password {
                    Some(cred.password.expose_secret().to_string())
                } else {
                    None
                },
                hidden: cred.hidden,
                managed: cred.managed,
                tags: cred.tags.clone(),
            };
            serde_json::to_string(&ApiResponse::success(detail)).unwrap_or_else(|e| json_error(&e.to_string()))
        }
        Ok(None) => json_error(&format!("Network not found: {}", ssid)),
        Err(e) => json_error(&e.to_string()),
    }
}

/// Create a new collection
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeCreateCollection<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    name: JString<'local>,
) -> jstring {
    let name: String = match env.get_string(&name) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let result = create_collection_impl(&name);
    string_to_jstring(&mut env, &result)
}

fn create_collection_impl(name: &str) -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    let collection = wifisync_core::CredentialCollection::new(name);
    match core.storage.save_collection(&collection) {
        Ok(()) => {
            serde_json::to_string(&ApiResponse::<()>::success_message("Collection created"))
                .unwrap_or_else(|e| json_error(&e.to_string()))
        }
        Err(e) => json_error(&e.to_string()),
    }
}

/// List all collections
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeListCollections<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let result = list_collections_impl();
    string_to_jstring(&mut env, &result)
}

fn list_collections_impl() -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    match core.storage.load_collections() {
        Ok(collections) => {
            let summaries: Vec<CollectionSummary> = collections
                .iter()
                .map(|c| CollectionSummary {
                    id: c.id.to_string(),
                    name: c.name.clone(),
                    credential_count: c.credentials.len(),
                    is_shared: c.is_shared,
                })
                .collect();
            serde_json::to_string(&ApiResponse::success(summaries)).unwrap_or_else(|e| json_error(&e.to_string()))
        }
        Err(e) => json_error(&e.to_string()),
    }
}

// ============================================================================
// Helper Types and Functions
// ============================================================================

/// API response wrapper
#[derive(Serialize, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            message: None,
        }
    }

    fn success_message(message: &str) -> Self {
        Self {
            success: true,
            data: None,
            error: None,
            message: Some(message.to_string()),
        }
    }
}

/// Credential summary (without password)
#[derive(Serialize, Deserialize)]
struct CredentialSummary {
    id: String,
    ssid: String,
    security_type: String,
    hidden: bool,
    managed: bool,
    tags: Vec<String>,
}

/// Credential detail (optionally with password)
#[derive(Serialize, Deserialize)]
struct CredentialDetail {
    id: String,
    ssid: String,
    security_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    hidden: bool,
    managed: bool,
    tags: Vec<String>,
}

/// Collection summary
#[derive(Serialize, Deserialize)]
struct CollectionSummary {
    id: String,
    name: String,
    credential_count: usize,
    is_shared: bool,
}

/// Import operation summary
#[derive(Serialize, Deserialize)]
struct ImportSummary {
    name: String,
    count: usize,
}

/// Export operation summary
#[derive(Serialize, Deserialize)]
struct ExportSummary {
    name: String,
    count: usize,
    path: String,
    encrypted: bool,
}

/// Create a JSON error response
fn json_error(message: &str) -> String {
    format!(r#"{{"success":false,"error":"{}"}}"#, message.replace('"', "\\\""))
}

/// Convert a Rust string to a JNI jstring
fn string_to_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s)
        .map(|js| js.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_error() {
        let error = json_error("Test error");
        assert!(error.contains("success"));
        assert!(error.contains("false"));
        assert!(error.contains("Test error"));
    }

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse::success("test data");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("true"));
        assert!(json.contains("test data"));
    }
}
