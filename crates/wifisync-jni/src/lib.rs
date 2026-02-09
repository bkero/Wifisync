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
use wifisync_core::sync::{
    generate_salt, PushRequest, SyncChange, SyncClient, SyncConfig, SyncEncryption,
    SyncOperation, SyncStateManager,
};

/// Global state for the Wifisync core
static CORE: OnceLock<WifisyncCore> = OnceLock::new();

/// Tokio runtime for async operations
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Get or create the tokio runtime
fn get_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    })
}

/// Core state holding all Wifisync services
struct WifisyncCore {
    storage: Storage,
    data_dir: std::path::PathBuf,
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

    let core = WifisyncCore {
        storage,
        data_dir: std::path::PathBuf::from(&files_dir),
    };

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
// Sync JNI Exports
// ============================================================================

/// Login to a sync server
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeSyncLogin<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    server_url: JString<'local>,
    username: JString<'local>,
    password: JString<'local>,
) -> jstring {
    let server_url: String = match env.get_string(&server_url) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let username: String = match env.get_string(&username) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let password: String = match env.get_string(&password) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let result = sync_login_impl(&server_url, &username, &password);
    string_to_jstring(&mut env, &result)
}

fn sync_login_impl(server_url: &str, username: &str, password: &str) -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    let state_manager = SyncStateManager::new(&core.data_dir);

    // Check if already logged in
    if let Ok(Some(_existing)) = state_manager.load_config() {
        return json_error("Already logged in. Logout first.");
    }

    if password.is_empty() {
        return json_error("Password cannot be empty");
    }

    // Get device name
    let device_name = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Android Device".to_string());

    // Run async login in tokio runtime
    let runtime = get_runtime();
    let result = runtime.block_on(async {
        // Create client
        let mut client = match SyncClient::new(server_url) {
            Ok(c) => c,
            Err(e) => return Err((format!("Failed to create client: {e}"), None::<[u8; 32]>)),
        };

        // Try to get existing salt from server
        let (salt, is_new_user) = match client.get_salt(username).await {
            Ok(Some(salt_b64)) => {
                use base64::Engine;
                let salt_bytes = base64::engine::general_purpose::STANDARD
                    .decode(&salt_b64)
                    .map_err(|e| (format!("Invalid salt from server: {e}"), None))?;
                let salt: [u8; 32] = salt_bytes
                    .try_into()
                    .map_err(|_| ("Salt has wrong length".to_string(), None))?;
                (salt, false)
            }
            Ok(None) => (generate_salt(), true),
            Err(e) => return Err((format!("Failed to get salt: {e}"), None)),
        };

        // Derive keys
        let encryption = match SyncEncryption::from_password(password, &salt) {
            Ok(e) => e,
            Err(e) => return Err((format!("Key derivation failed: {e}"), None)),
        };
        let auth_proof = encryption.auth_proof();

        // Register if new user
        if is_new_user {
            use base64::Engine;
            let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);
            if let Err(e) = client.register(username, &auth_proof, &salt_b64).await {
                if !e.to_string().contains("already exists") {
                    tracing::debug!("Registration failed (may be expected): {}", e);
                }
            }
        }

        // Now login
        match client.login(username, &auth_proof, &device_name).await {
            Ok(resp) => Ok((resp, salt)),
            Err(e) => Err((format!("Login failed: {e}"), None)),
        }
    });

    match result {
        Ok((login_resp, salt)) => {
            // Save configuration
            let mut config = SyncConfig::new(
                server_url.to_string(),
                username.to_string(),
                login_resp.device_id.clone(),
                salt.to_vec(),
            );
            config.set_token(login_resp.token, login_resp.expires_at);

            if let Err(e) = state_manager.save_config(&config) {
                return json_error(&format!("Failed to save config: {e}"));
            }

            let response = SyncLoginResponseData {
                server_url: server_url.to_string(),
                username: username.to_string(),
                device_id: login_resp.device_id,
            };
            serde_json::to_string(&ApiResponse::success(response))
                .unwrap_or_else(|e| json_error(&e.to_string()))
        }
        Err((e, _)) => json_error(&e),
    }
}

/// Logout from sync server
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeSyncLogout<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let result = sync_logout_impl();
    string_to_jstring(&mut env, &result)
}

fn sync_logout_impl() -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    let state_manager = SyncStateManager::new(&core.data_dir);

    let config = match state_manager.load_config() {
        Ok(Some(c)) => c,
        Ok(None) => return json_error("Not logged in"),
        Err(e) => return json_error(&format!("Failed to load config: {e}")),
    };

    // Try to logout from server
    if config.has_valid_token() {
        let runtime = get_runtime();
        runtime.block_on(async {
            if let Ok(mut client) = SyncClient::from_config(&config) {
                if let Err(e) = client.logout().await {
                    tracing::warn!("Failed to logout from server: {}", e);
                }
            }
        });
    }

    // Delete local config
    if let Err(e) = state_manager.delete_config() {
        return json_error(&format!("Failed to delete config: {e}"));
    }

    serde_json::to_string(&ApiResponse::<()>::success_message("Logged out"))
        .unwrap_or_else(|e| json_error(&e.to_string()))
}

/// Get sync status
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeSyncStatus<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let result = sync_status_impl();
    string_to_jstring(&mut env, &result)
}

fn sync_status_impl() -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    let state_manager = SyncStateManager::new(&core.data_dir);

    let config = match state_manager.load_config() {
        Ok(c) => c,
        Err(e) => return json_error(&format!("Failed to load config: {e}")),
    };

    let state = match state_manager.load_state() {
        Ok(s) => s,
        Err(e) => return json_error(&format!("Failed to load state: {e}")),
    };

    // Calculate pending count - for first sync, count all credentials
    let pending_count = if state.last_sync.is_none() && !state.has_pending_changes() {
        // First sync: all local credentials are pending
        let collections = core.storage.load_collections().unwrap_or_default();
        collections.iter().map(|c| c.credentials.len()).sum()
    } else {
        state.pending_count()
    };

    let status = SyncStatusData {
        enabled: config.is_some(),
        server_url: config.as_ref().map(|c| c.server_url.clone()),
        username: config.as_ref().map(|c| c.username.clone()),
        device_id: config.as_ref().map(|c| c.device_id.clone()),
        last_sync: state.last_sync.map(|t| t.to_rfc3339()),
        pending_changes: pending_count,
        has_valid_token: config.as_ref().is_some_and(SyncConfig::has_valid_token),
    };

    serde_json::to_string(&ApiResponse::success(status))
        .unwrap_or_else(|e| json_error(&e.to_string()))
}

/// Push local changes to server
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeSyncPush<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    password: JString<'local>,
) -> jstring {
    let password: String = match env.get_string(&password) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let result = sync_push_impl(&password);
    string_to_jstring(&mut env, &result)
}

#[allow(clippy::too_many_lines)]
fn sync_push_impl(password: &str) -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    let state_manager = SyncStateManager::new(&core.data_dir);

    let config = match state_manager.load_config() {
        Ok(Some(c)) => c,
        Ok(None) => return json_error("Not logged in"),
        Err(e) => return json_error(&format!("Failed to load config: {e}")),
    };

    let mut state = match state_manager.load_state() {
        Ok(s) => s,
        Err(e) => return json_error(&format!("Failed to load state: {e}")),
    };

    let collections = match core.storage.load_collections() {
        Ok(c) => c,
        Err(e) => return json_error(&format!("Failed to load collections: {e}")),
    };

    // Check if this is a first sync
    let is_first_sync = state.last_sync.is_none() && !state.has_pending_changes();

    if is_first_sync {
        let total_creds: usize = collections.iter().map(|c| c.credentials.len()).sum();
        if total_creds == 0 {
            return serde_json::to_string(&ApiResponse::success(SyncPushResponseData {
                accepted: 0,
                conflicts: 0,
            })).unwrap_or_else(|e| json_error(&e.to_string()));
        }
    } else if !state.has_pending_changes() {
        return serde_json::to_string(&ApiResponse::success(SyncPushResponseData {
            accepted: 0,
            conflicts: 0,
        })).unwrap_or_else(|e| json_error(&e.to_string()));
    }

    // Derive encryption key
    let encryption = match SyncEncryption::from_password(password, &config.key_salt) {
        Ok(e) => e,
        Err(e) => return json_error(&format!("Key derivation failed: {e}")),
    };

    // Build changes
    let mut changes = Vec::new();

    let runtime = get_runtime();
    let result = runtime.block_on(async {
        let client = match SyncClient::from_config(&config) {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to create client: {e}")),
        };

        if is_first_sync {
            // First sync: ensure all collections exist on server
            let server_collections = match client.list_collections().await {
                Ok(c) => c,
                Err(e) => return Err(format!("Failed to list server collections: {e}")),
            };

            let server_collection_ids: std::collections::HashSet<_> = server_collections
                .collections
                .iter()
                .map(|c| c.id)
                .collect();

            for collection in &collections {
                if !server_collection_ids.contains(&collection.id) {
                    // Create collection on server
                    let encrypted_name_payload = match encryption.encrypt_payload(collection.name.as_bytes()) {
                        Ok(p) => p,
                        Err(e) => return Err(format!("Failed to encrypt collection name: {e}")),
                    };
                    let encrypted_name = match serde_json::to_vec(&encrypted_name_payload) {
                        Ok(n) => n,
                        Err(e) => return Err(format!("Failed to serialize: {e}")),
                    };
                    if let Err(e) = client.create_collection(Some(collection.id), encrypted_name).await {
                        if !e.to_string().contains("already exists") {
                            tracing::warn!("Failed to create collection {}: {}", collection.name, e);
                        }
                    }
                }
            }
        }

        Ok(client)
    });

    let client = match result {
        Ok(c) => c,
        Err(e) => return json_error(&e),
    };

    // Build the changes to push
    if is_first_sync {
        for collection in &collections {
            for credential in &collection.credentials {
                let credential_json = match serde_json::to_vec(credential) {
                    Ok(j) => j,
                    Err(e) => return json_error(&format!("Failed to serialize credential: {e}")),
                };
                let payload = match encryption.encrypt_payload(&credential_json) {
                    Ok(p) => p,
                    Err(e) => return json_error(&format!("Failed to encrypt credential: {e}")),
                };

                state.local_clock.increment(&config.device_id);

                let change = SyncChange::new(
                    collection.id,
                    credential.id,
                    SyncOperation::Create,
                    state.local_clock.clone(),
                    payload,
                    config.device_id.clone(),
                );
                changes.push(change);
            }
        }
    } else {
        for pending in &state.pending_changes {
            for collection in &collections {
                if collection.id == pending.collection_id {
                    if let Some(credential) = collection.credentials.iter().find(|c| c.id == pending.credential_id) {
                        let credential_json = match serde_json::to_vec(credential) {
                            Ok(j) => j,
                            Err(e) => return json_error(&format!("Failed to serialize credential: {e}")),
                        };
                        let payload = match encryption.encrypt_payload(&credential_json) {
                            Ok(p) => p,
                            Err(e) => return json_error(&format!("Failed to encrypt credential: {e}")),
                        };

                        let operation = match pending.change_type {
                            wifisync_core::sync::ChangeType::Create => SyncOperation::Create,
                            wifisync_core::sync::ChangeType::Update => SyncOperation::Update,
                            wifisync_core::sync::ChangeType::Delete => SyncOperation::Delete,
                        };

                        let change = SyncChange::new(
                            pending.collection_id,
                            pending.credential_id,
                            operation,
                            state.local_clock.clone(),
                            payload,
                            config.device_id.clone(),
                        );
                        changes.push(change);
                    }
                    break;
                }
            }
        }
    }

    if changes.is_empty() {
        return serde_json::to_string(&ApiResponse::success(SyncPushResponseData {
            accepted: 0,
            conflicts: 0,
        })).unwrap_or_else(|e| json_error(&e.to_string()));
    }

    let req = PushRequest {
        device_id: config.device_id.clone(),
        changes,
    };

    let result = runtime.block_on(async {
        client.push(req).await
    });

    match result {
        Ok(resp) => {
            // Update state
            let pushed_ids: Vec<_> = state.pending_changes.iter().map(|c| c.credential_id).collect();
            state.remove_pending(&pushed_ids);

            // Mark as synced if this was first sync
            if is_first_sync {
                state.last_sync = Some(chrono::Utc::now());
            }

            if let Err(e) = state_manager.save_state(&state) {
                tracing::warn!("Failed to save state: {}", e);
            }

            serde_json::to_string(&ApiResponse::success(SyncPushResponseData {
                accepted: resp.accepted_count,
                conflicts: resp.conflict_count,
            })).unwrap_or_else(|e| json_error(&e.to_string()))
        }
        Err(e) => json_error(&format!("Push failed: {e}")),
    }
}

/// Pull changes from server
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeSyncPull<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    password: JString<'local>,
) -> jstring {
    let password: String = match env.get_string(&password) {
        Ok(s) => s.into(),
        Err(e) => return string_to_jstring(&mut env, &json_error(&e.to_string())),
    };

    let result = sync_pull_impl(&password);
    string_to_jstring(&mut env, &result)
}

fn sync_pull_impl(password: &str) -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    let state_manager = SyncStateManager::new(&core.data_dir);

    let config = match state_manager.load_config() {
        Ok(Some(c)) => c,
        Ok(None) => return json_error("Not logged in"),
        Err(e) => return json_error(&format!("Failed to load config: {e}")),
    };

    let mut state = match state_manager.load_state() {
        Ok(s) => s,
        Err(e) => return json_error(&format!("Failed to load state: {e}")),
    };

    // Derive encryption key
    let encryption = match SyncEncryption::from_password(password, &config.key_salt) {
        Ok(e) => e,
        Err(e) => return json_error(&format!("Key derivation failed: {e}")),
    };

    let runtime = get_runtime();
    let result = runtime.block_on(async {
        let client = match SyncClient::from_config(&config) {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to create client: {e}")),
        };

        let since = if state.server_clock.is_empty() {
            None
        } else {
            Some(state.server_clock.clone())
        };

        match client.pull(&config.device_id, since, None).await {
            Ok(resp) => Ok(resp),
            Err(e) => Err(format!("Pull failed: {e}")),
        }
    });

    match result {
        Ok(resp) => {
            if resp.changes.is_empty() {
                return serde_json::to_string(&ApiResponse::success(SyncPullResponseData {
                    applied: 0,
                    errors: 0,
                })).unwrap_or_else(|e| json_error(&e.to_string()));
            }

            // Apply changes to local storage
            let mut collections = match core.storage.load_collections() {
                Ok(c) => c,
                Err(e) => return json_error(&format!("Failed to load collections: {e}")),
            };

            let mut applied = 0;
            let mut errors = 0;

            for change in &resp.changes {
                match apply_sync_change(&mut collections, change, &encryption) {
                    Ok(()) => applied += 1,
                    Err(e) => {
                        tracing::warn!("Failed to apply change {}: {}", change.id, e);
                        errors += 1;
                    }
                }
            }

            // Save updated collections
            if let Err(e) = core.storage.save_collections(&collections) {
                return json_error(&format!("Failed to save collections: {e}"));
            }

            // Update sync state
            state.server_clock = resp.server_clock;
            state.last_sync = Some(chrono::Utc::now());
            if let Err(e) = state_manager.save_state(&state) {
                tracing::warn!("Failed to save state: {}", e);
            }

            serde_json::to_string(&ApiResponse::success(SyncPullResponseData {
                applied,
                errors,
            })).unwrap_or_else(|e| json_error(&e.to_string()))
        }
        Err(e) => json_error(&e),
    }
}

/// Apply a single sync change to collections
fn apply_sync_change(
    collections: &mut [wifisync_core::CredentialCollection],
    change: &SyncChange,
    encryption: &SyncEncryption,
) -> Result<(), String> {
    let collection = collections
        .iter_mut()
        .find(|c| c.id == change.collection_id);

    match change.operation {
        SyncOperation::Delete => {
            if let Some(coll) = collection {
                coll.credentials.retain(|c| c.id != change.credential_id);
            }
        }
        SyncOperation::Create | SyncOperation::Update => {
            let decrypted = encryption.decrypt_payload(&change.payload)
                .map_err(|e| format!("Decryption failed: {e}"))?;
            let credential: wifisync_core::WifiCredential = serde_json::from_slice(&decrypted)
                .map_err(|e| format!("Parse failed: {e}"))?;

            if let Some(coll) = collection {
                if let Some(existing) = coll.credentials.iter_mut().find(|c| c.id == credential.id) {
                    *existing = credential;
                } else {
                    coll.credentials.push(credential);
                }
            }
        }
    }

    Ok(())
}

/// List devices (stub - returns empty list as there's no server endpoint yet)
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeListDevices<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let result = list_devices_impl();
    string_to_jstring(&mut env, &result)
}

fn list_devices_impl() -> String {
    let Some(core) = CORE.get() else {
        return json_error("Core not initialized");
    };

    let state_manager = SyncStateManager::new(&core.data_dir);

    let config = match state_manager.load_config() {
        Ok(Some(c)) => c,
        Ok(None) => return json_error("Not logged in"),
        Err(e) => return json_error(&format!("Failed to load config: {e}")),
    };

    // For now, just return the current device since there's no server endpoint for listing devices
    let device_name = hostname::get()
        .map_or_else(|_| "Android Device".to_string(), |h| h.to_string_lossy().to_string());

    let devices = vec![DeviceInfoData {
        id: config.device_id.clone(),
        name: device_name,
        last_sync_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        is_current_device: true,
    }];

    serde_json::to_string(&ApiResponse::success(devices))
        .unwrap_or_else(|e| json_error(&e.to_string()))
}

/// Get version information
///
/// # Safety
///
/// This function is called from Java/Kotlin via JNI.
#[no_mangle]
pub extern "system" fn Java_com_wifisync_android_WifisyncCore_nativeGetVersion<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let result = get_version_impl();
    string_to_jstring(&mut env, &result)
}

fn get_version_impl() -> String {
    let version_info = VersionInfoData {
        version: env!("CARGO_PKG_VERSION").to_string(),
        rust_version: None, // Could add this if needed
        build_date: None,   // Could add this if needed
    };

    serde_json::to_string(&ApiResponse::success(version_info))
        .unwrap_or_else(|e| json_error(&e.to_string()))
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

// ============================================================================
// Sync-related Data Structures
// ============================================================================

/// Sync login response data
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncLoginResponseData {
    server_url: String,
    username: String,
    device_id: String,
}

/// Sync status data
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncStatusData {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_sync: Option<String>,
    pending_changes: usize,
    has_valid_token: bool,
}

/// Sync push response data
#[derive(Serialize, Deserialize)]
struct SyncPushResponseData {
    accepted: usize,
    conflicts: usize,
}

/// Sync pull response data
#[derive(Serialize, Deserialize)]
struct SyncPullResponseData {
    applied: usize,
    errors: usize,
}

/// Device info data
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceInfoData {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_sync_at: Option<String>,
    created_at: String,
    is_current_device: bool,
}

/// Version info data
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionInfoData {
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rust_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    build_date: Option<String>,
}

/// Create a JSON error response
fn json_error(message: &str) -> String {
    format!(r#"{{"success":false,"error":"{}"}}"#, message.replace('"', "\\\""))
}

/// Convert a Rust string to a JNI jstring
fn string_to_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // JSON Error Response Tests
    // =========================================================================

    #[test]
    fn test_json_error_basic() {
        let error = json_error("Test error");
        assert!(error.contains(r#""success":false"#));
        assert!(error.contains(r#""error":"Test error""#));
    }

    #[test]
    fn test_json_error_escapes_quotes() {
        let error = json_error(r#"Error with "quotes""#);
        assert!(error.contains(r#"\"quotes\""#));
        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&error).unwrap();
        assert!(!parsed["success"].as_bool().unwrap());
    }

    #[test]
    fn test_json_error_empty_message() {
        let error = json_error("");
        let parsed: serde_json::Value = serde_json::from_str(&error).unwrap();
        assert!(!parsed["success"].as_bool().unwrap());
        assert_eq!(parsed["error"].as_str().unwrap(), "");
    }

    #[test]
    fn test_json_error_special_characters() {
        let error = json_error("Error: path/to/file\nNew line");
        // Should be parseable (newlines are valid in JSON strings)
        assert!(error.contains("success"));
        assert!(error.contains("false"));
    }

    // =========================================================================
    // ApiResponse Tests
    // =========================================================================

    #[test]
    fn test_api_response_success_with_string() {
        let response = ApiResponse::success("test data");
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains(r#""success":true"#));
        assert!(json.contains(r#""data":"test data""#));
        assert!(!json.contains("error"));
        assert!(!json.contains("message"));
    }

    #[test]
    fn test_api_response_success_with_struct() {
        let summary = CredentialSummary {
            id: "abc-123".to_string(),
            ssid: "TestNetwork".to_string(),
            security_type: "Wpa2Psk".to_string(),
            hidden: false,
            managed: true,
            tags: vec!["home".to_string(), "trusted".to_string()],
        };

        let response = ApiResponse::success(summary);
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains(r#""success":true"#));
        assert!(json.contains(r#""ssid":"TestNetwork""#));
        assert!(json.contains(r#""security_type":"Wpa2Psk""#));
        assert!(json.contains(r#""managed":true"#));
        assert!(json.contains(r#""tags":["home","trusted"]"#));
    }

    #[test]
    fn test_api_response_success_with_vec() {
        let summaries = vec![
            CredentialSummary {
                id: "1".to_string(),
                ssid: "Net1".to_string(),
                security_type: "Wpa2Psk".to_string(),
                hidden: false,
                managed: false,
                tags: vec![],
            },
            CredentialSummary {
                id: "2".to_string(),
                ssid: "Net2".to_string(),
                security_type: "Wpa3Psk".to_string(),
                hidden: true,
                managed: true,
                tags: vec!["office".to_string()],
            },
        ];

        let response = ApiResponse::success(summaries);
        let json = serde_json::to_string(&response).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["success"].as_bool().unwrap());
        assert_eq!(parsed["data"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_api_response_success_message() {
        let response = ApiResponse::<()>::success_message("Operation completed");
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains(r#""success":true"#));
        assert!(json.contains(r#""message":"Operation completed""#));
        assert!(!json.contains("data"));
        assert!(!json.contains("error"));
    }

    // =========================================================================
    // CredentialSummary Tests
    // =========================================================================

    #[test]
    fn test_credential_summary_serialization() {
        let summary = CredentialSummary {
            id: "uuid-here".to_string(),
            ssid: "MyWifi".to_string(),
            security_type: "Wpa2Psk".to_string(),
            hidden: false,
            managed: false,
            tags: vec![],
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: CredentialSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "uuid-here");
        assert_eq!(parsed.ssid, "MyWifi");
        assert_eq!(parsed.security_type, "Wpa2Psk");
        assert!(!parsed.hidden);
        assert!(!parsed.managed);
        assert!(parsed.tags.is_empty());
    }

    #[test]
    fn test_credential_summary_with_tags() {
        let summary = CredentialSummary {
            id: "id".to_string(),
            ssid: "TaggedNet".to_string(),
            security_type: "Wpa3Psk".to_string(),
            hidden: true,
            managed: true,
            tags: vec!["work".to_string(), "secure".to_string(), "vpn".to_string()],
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains(r#""tags":["work","secure","vpn"]"#));
    }

    // =========================================================================
    // CredentialDetail Tests
    // =========================================================================

    #[test]
    fn test_credential_detail_without_password() {
        let detail = CredentialDetail {
            id: "id".to_string(),
            ssid: "SecureNet".to_string(),
            security_type: "Wpa3Psk".to_string(),
            password: None,
            hidden: false,
            managed: false,
            tags: vec![],
        };

        let json = serde_json::to_string(&detail).unwrap();
        assert!(!json.contains("password"));
    }

    #[test]
    fn test_credential_detail_with_password() {
        let detail = CredentialDetail {
            id: "id".to_string(),
            ssid: "SecureNet".to_string(),
            security_type: "Wpa3Psk".to_string(),
            password: Some("supersecret".to_string()),
            hidden: false,
            managed: false,
            tags: vec![],
        };

        let json = serde_json::to_string(&detail).unwrap();
        assert!(json.contains(r#""password":"supersecret""#));
    }

    // =========================================================================
    // CollectionSummary Tests
    // =========================================================================

    #[test]
    fn test_collection_summary_serialization() {
        let summary = CollectionSummary {
            id: "col-id".to_string(),
            name: "My Networks".to_string(),
            credential_count: 5,
            is_shared: false,
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: CollectionSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "col-id");
        assert_eq!(parsed.name, "My Networks");
        assert_eq!(parsed.credential_count, 5);
        assert!(!parsed.is_shared);
    }

    #[test]
    fn test_collection_summary_shared() {
        let summary = CollectionSummary {
            id: "shared-col".to_string(),
            name: "Shared Collection".to_string(),
            credential_count: 10,
            is_shared: true,
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains(r#""is_shared":true"#));
    }

    // =========================================================================
    // ImportSummary Tests
    // =========================================================================

    #[test]
    fn test_import_summary_serialization() {
        let summary = ImportSummary {
            name: "Imported Collection".to_string(),
            count: 15,
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: ImportSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "Imported Collection");
        assert_eq!(parsed.count, 15);
    }

    // =========================================================================
    // ExportSummary Tests
    // =========================================================================

    #[test]
    fn test_export_summary_unencrypted() {
        let summary = ExportSummary {
            name: "Backup".to_string(),
            count: 8,
            path: "/storage/emulated/0/Download/backup.json".to_string(),
            encrypted: false,
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains(r#""encrypted":false"#));
        assert!(json.contains(r#""path":"/storage/emulated/0/Download/backup.json""#));
    }

    #[test]
    fn test_export_summary_encrypted() {
        let summary = ExportSummary {
            name: "Secure Backup".to_string(),
            count: 12,
            path: "/storage/emulated/0/Download/backup.json.enc".to_string(),
            encrypted: true,
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: ExportSummary = serde_json::from_str(&json).unwrap();

        assert!(parsed.encrypted);
        assert!(parsed.path.ends_with(".enc"));
    }

    // =========================================================================
    // Round-trip Serialization Tests
    // =========================================================================

    #[test]
    fn test_api_response_roundtrip() {
        let original = ApiResponse::success(vec![
            CredentialSummary {
                id: "1".to_string(),
                ssid: "Network1".to_string(),
                security_type: "Wpa2Psk".to_string(),
                hidden: false,
                managed: true,
                tags: vec!["home".to_string()],
            },
        ]);

        let json = serde_json::to_string(&original).unwrap();
        let parsed: ApiResponse<Vec<CredentialSummary>> = serde_json::from_str(&json).unwrap();

        assert!(parsed.success);
        assert!(parsed.data.is_some());
        let data = parsed.data.unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].ssid, "Network1");
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_empty_tags_serialization() {
        let summary = CredentialSummary {
            id: "id".to_string(),
            ssid: "Net".to_string(),
            security_type: "Wpa2Psk".to_string(),
            hidden: false,
            managed: false,
            tags: vec![],
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains(r#""tags":[]"#));
    }

    #[test]
    fn test_special_ssid_characters() {
        let summary = CredentialSummary {
            id: "id".to_string(),
            ssid: "Net with spaces & \"quotes\"".to_string(),
            security_type: "Wpa2Psk".to_string(),
            hidden: false,
            managed: false,
            tags: vec![],
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: CredentialSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ssid, "Net with spaces & \"quotes\"");
    }

    #[test]
    fn test_unicode_ssid() {
        let summary = CredentialSummary {
            id: "id".to_string(),
            ssid: "カフェWiFi_咖啡".to_string(),
            security_type: "Wpa2Psk".to_string(),
            hidden: false,
            managed: false,
            tags: vec!["日本".to_string()],
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: CredentialSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ssid, "カフェWiFi_咖啡");
        assert_eq!(parsed.tags[0], "日本");
    }
}
