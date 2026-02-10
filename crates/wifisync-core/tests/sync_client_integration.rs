//! Integration tests for the sync client
//!
//! These tests verify that the wifisync-core sync client (used by Android JNI)
//! can properly communicate with a wifisync-server instance.
//!
//! Run with: cargo test -p wifisync-core --features sync --test sync_client_integration

use std::net::SocketAddr;

use tempfile::TempDir;
use tokio::sync::oneshot;
use uuid::Uuid;

use wifisync_core::sync::{
    generate_salt, PushRequest, SyncChange, SyncClient, SyncConfig, SyncEncryption,
    SyncOperation, SyncStateManager,
};
use wifisync_core::{CredentialCollection, SecurityType, SourcePlatform, WifiCredential};
use wifisync_sync_protocol::{LoginRequest, RegisterRequest, VectorClock};

// =============================================================================
// Test Server Setup
// =============================================================================

/// Test server configuration
struct TestServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    _temp_dir: TempDir,
}

impl TestServer {
    /// Start a test server on a random port
    async fn start() -> Self {
        use axum::{routing::{delete, get, post}, Router};
        use sqlx::sqlite::SqlitePoolOptions;
        use tokio::net::TcpListener;

        // Create temp directory for database
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create in-memory SQLite database
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        // Run migrations
        run_migrations(&pool).await;

        // Find a free port
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind to random port");
        let addr = listener.local_addr().unwrap();

        // Create app state
        let state = TestAppState {
            db: pool,
            jwt_secret: "test_secret_key_for_integration_tests_12345".to_string(),
        };

        // Create router with handlers
        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/api/v1/users/register", post(handle_register))
            .route("/api/v1/auth/login", post(handle_login))
            .route("/api/v1/auth/salt/:username", get(handle_get_salt))
            .route("/api/v1/auth/logout", delete(handle_logout))
            .route("/api/v1/sync/push", post(handle_push))
            .route("/api/v1/sync/pull", post(handle_pull))
            .route("/api/v1/collections", get(handle_list_collections))
            .route("/api/v1/collections", post(handle_create_collection))
            .route(
                "/api/v1/collections/:id",
                delete(handle_delete_collection),
            )
            .route("/api/v1/sync/conflicts", get(handle_list_conflicts))
            .with_state(state);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // Spawn server
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("Server failed");
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
            _temp_dir: temp_dir,
        }
    }

    /// Get the base URL for this test server
    fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

#[derive(Clone)]
struct TestAppState {
    db: sqlx::SqlitePool,
    jwt_secret: String,
}

/// Run database migrations
async fn run_migrations(pool: &sqlx::SqlitePool) {
    let migrations = [
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            auth_key_hash TEXT NOT NULL,
            auth_salt TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS devices (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            device_token_hash TEXT NOT NULL,
            last_sync_at TEXT,
            created_at TEXT NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS collections (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            encrypted_name BLOB NOT NULL,
            vector_clock TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS sync_records (
            id TEXT PRIMARY KEY,
            collection_id TEXT NOT NULL,
            credential_id TEXT NOT NULL,
            vector_clock TEXT NOT NULL,
            encrypted_payload BLOB NOT NULL,
            tombstone INTEGER DEFAULT 0,
            updated_at TEXT NOT NULL,
            UNIQUE(collection_id, credential_id)
        )",
    ];

    for sql in migrations {
        sqlx::query(sql).execute(pool).await.unwrap();
    }
}

// =============================================================================
// Request Handlers (minimal test implementations)
// =============================================================================

async fn handle_register(
    axum::extract::State(state): axum::extract::State<TestAppState>,
    axum::Json(req): axum::Json<RegisterRequest>,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;
    use chrono::Utc;

    // Check if user exists
    let existing: Option<(String, String)> =
        sqlx::query_as("SELECT id, auth_salt FROM users WHERE username = ?")
            .bind(&req.username)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some((user_id, auth_salt)) = existing {
        if auth_salt.is_empty() {
            // Legacy user — allow re-registration to upgrade credentials
            let auth_hash = bcrypt::hash(&req.auth_proof, 4).unwrap();
            sqlx::query("UPDATE users SET auth_key_hash = ?, auth_salt = ? WHERE username = ?")
                .bind(&auth_hash)
                .bind(&req.auth_salt)
                .bind(&req.username)
                .execute(&state.db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            return Ok(axum::Json(serde_json::json!({ "user_id": user_id })));
        }
        return Err((
            StatusCode::BAD_REQUEST,
            "Username already exists".to_string(),
        ));
    }

    let user_id = Uuid::new_v4().to_string();
    let auth_hash = bcrypt::hash(&req.auth_proof, 4).unwrap();

    sqlx::query("INSERT INTO users (id, username, auth_key_hash, auth_salt, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&user_id)
        .bind(&req.username)
        .bind(&auth_hash)
        .bind(&req.auth_salt)
        .bind(Utc::now().to_rfc3339())
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(serde_json::json!({ "user_id": user_id })))
}

async fn handle_login(
    axum::extract::State(state): axum::extract::State<TestAppState>,
    axum::Json(req): axum::Json<LoginRequest>,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;
    use chrono::{Duration, Utc};

    // Find user
    let user: Option<(String, String)> =
        sqlx::query_as("SELECT id, auth_key_hash FROM users WHERE username = ?")
            .bind(&req.username)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (user_id, auth_hash) =
        user.ok_or((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()))?;

    // Verify password
    if !bcrypt::verify(&req.auth_proof, &auth_hash).unwrap_or(false) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()));
    }

    // Create device
    let device_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO devices (id, user_id, name, device_token_hash, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&device_id)
    .bind(&user_id)
    .bind(&req.device_name)
    .bind("token_hash")
    .bind(Utc::now().to_rfc3339())
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Create JWT
    let exp = Utc::now() + Duration::hours(24);
    let claims = serde_json::json!({
        "sub": user_id,
        "device_id": device_id,
        "exp": exp.timestamp(),
        "iat": Utc::now().timestamp()
    });

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .unwrap();

    Ok(axum::Json(serde_json::json!({
        "device_id": device_id,
        "token": token,
        "expires_at": exp.to_rfc3339()
    })))
}

async fn handle_logout() -> axum::http::StatusCode {
    axum::http::StatusCode::NO_CONTENT
}

async fn handle_get_salt(
    axum::extract::State(state): axum::extract::State<TestAppState>,
    axum::extract::Path(username): axum::extract::Path<String>,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;

    let salt: Option<(String,)> = sqlx::query_as("SELECT auth_salt FROM users WHERE username = ?")
        .bind(&username)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match salt {
        Some((auth_salt,)) if !auth_salt.is_empty() => {
            Ok(axum::Json(serde_json::json!({ "auth_salt": auth_salt })))
        }
        _ => Err((StatusCode::NOT_FOUND, "User not found".to_string())),
    }
}

async fn handle_push(
    axum::extract::State(state): axum::extract::State<TestAppState>,
    body: String,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;
    use chrono::Utc;

    let req: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;

    let changes = req["changes"]
        .as_array()
        .ok_or((StatusCode::BAD_REQUEST, "Missing changes array".to_string()))?;

    let mut accepted = 0;

    for change in changes {
        let change_id = change["id"].as_str().unwrap_or_default();
        let collection_id = change["collection_id"].as_str().unwrap_or_default();
        let credential_id = change["credential_id"].as_str().unwrap_or_default();
        let clock_json = serde_json::to_string(&change["vector_clock"]).unwrap_or_default();
        let payload_json = serde_json::to_string(&change["payload"]).unwrap_or_default();
        let is_tombstone = change["operation"].as_str() == Some("delete");

        sqlx::query(
            r"
            INSERT INTO sync_records (id, collection_id, credential_id, vector_clock, encrypted_payload, tombstone, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(collection_id, credential_id) DO UPDATE SET
                vector_clock = excluded.vector_clock,
                encrypted_payload = excluded.encrypted_payload,
                tombstone = excluded.tombstone,
                updated_at = excluded.updated_at
            ",
        )
        .bind(change_id)
        .bind(collection_id)
        .bind(credential_id)
        .bind(&clock_json)
        .bind(payload_json.as_bytes())
        .bind(if is_tombstone { 1 } else { 0 })
        .bind(Utc::now().to_rfc3339())
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        accepted += 1;
    }

    Ok(axum::Json(serde_json::json!({
        "results": [],
        "accepted_count": accepted,
        "conflict_count": 0
    })))
}

async fn handle_pull(
    axum::extract::State(state): axum::extract::State<TestAppState>,
    _body: String,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;

    let records: Vec<(String, String, String, String, Vec<u8>, i32, String)> = sqlx::query_as(
        "SELECT id, collection_id, credential_id, vector_clock, encrypted_payload, tombstone, updated_at FROM sync_records",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut changes = Vec::new();
    let server_clock = VectorClock::new();

    for (id, coll_id, cred_id, clock_str, payload_bytes, tombstone, updated_at) in records {
        let clock: serde_json::Value = serde_json::from_str(&clock_str).unwrap_or_default();
        let payload: serde_json::Value = String::from_utf8(payload_bytes.clone())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| {
                serde_json::json!({
                    "encrypted_data": [],
                    "nonce": []
                })
            });

        changes.push(serde_json::json!({
            "id": id,
            "collection_id": coll_id,
            "credential_id": cred_id,
            "operation": if tombstone != 0 { "delete" } else { "update" },
            "vector_clock": clock,
            "payload": payload,
            "device_id": "server",
            "timestamp": updated_at
        }));
    }

    Ok(axum::Json(serde_json::json!({
        "changes": changes,
        "server_clock": server_clock,
        "has_more": false
    })))
}

async fn handle_list_collections(
    axum::extract::State(state): axum::extract::State<TestAppState>,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;

    let collections: Vec<(String, Vec<u8>, String, String)> =
        sqlx::query_as("SELECT id, encrypted_name, vector_clock, updated_at FROM collections")
            .fetch_all(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let colls: Vec<serde_json::Value> = collections
        .into_iter()
        .map(|(id, _name, _clock, updated)| {
            serde_json::json!({
                "id": id,
                "updated_at": updated
            })
        })
        .collect();

    Ok(axum::Json(serde_json::json!({ "collections": colls })))
}

async fn handle_create_collection(
    axum::extract::State(state): axum::extract::State<TestAppState>,
    body: String,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;
    use chrono::Utc;

    let req: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;

    let id = req
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let encrypted_name = req
        .get("encrypted_name")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect::<Vec<u8>>()
        })
        .unwrap_or_default();

    sqlx::query(
        "INSERT INTO collections (id, user_id, encrypted_name, vector_clock, updated_at) VALUES (?, 'test_user', ?, '{}', ?)",
    )
    .bind(&id)
    .bind(&encrypted_name)
    .bind(Utc::now().to_rfc3339())
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(serde_json::json!({ "id": id })))
}

async fn handle_delete_collection(
    axum::extract::State(state): axum::extract::State<TestAppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;

    sqlx::query("DELETE FROM collections WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

async fn handle_list_conflicts() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({ "conflicts": [] }))
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a test credential
fn create_test_credential(ssid: &str) -> WifiCredential {
    WifiCredential::new(
        ssid.to_string(),
        "TestPassword123".to_string(),
        SecurityType::Wpa2Psk,
        SourcePlatform::Manual,
    )
}

/// Create a test collection with credentials
fn create_test_collection(name: &str, credentials: Vec<WifiCredential>) -> CredentialCollection {
    let mut collection = CredentialCollection::new(name);
    for cred in credentials {
        collection.credentials.push(cred);
    }
    collection
}

// =============================================================================
// Integration Tests - SyncClient API
// =============================================================================

/// Test that SyncClient can connect to a server and perform health check
#[tokio::test]
async fn test_sync_client_connection() {
    let server = TestServer::start().await;
    let client = SyncClient::new(&server.base_url()).expect("Failed to create client");

    // Health check via a simple request
    let result = client.register("test_user", "test_auth_proof", "").await;
    assert!(result.is_ok(), "Failed to register: {:?}", result.err());
}

/// Test full login flow using SyncClient
#[tokio::test]
async fn test_sync_client_login() {
    let server = TestServer::start().await;
    let mut client = SyncClient::new(&server.base_url()).expect("Failed to create client");

    // Register
    let result = client.register("login_test_user", "my_auth_proof", "").await;
    assert!(result.is_ok(), "Registration failed: {:?}", result.err());

    // Login
    let result = client
        .login("login_test_user", "my_auth_proof", "Test Device")
        .await;
    assert!(result.is_ok(), "Login failed: {:?}", result.err());

    let login_response = result.unwrap();
    assert!(!login_response.device_id.is_empty());
    assert!(!login_response.token.is_empty());
}

/// Test login with wrong credentials fails
#[tokio::test]
async fn test_sync_client_login_wrong_password() {
    let server = TestServer::start().await;
    let mut client = SyncClient::new(&server.base_url()).expect("Failed to create client");

    // Register
    client
        .register("wrong_pass_user", "correct_password", "")
        .await
        .unwrap();

    // Try to login with wrong password
    let result = client
        .login("wrong_pass_user", "wrong_password", "Test Device")
        .await;
    assert!(result.is_err(), "Login should have failed");
}

/// Test SyncEncryption key derivation and encryption roundtrip
#[tokio::test]
async fn test_sync_encryption_roundtrip() {
    let password = "my_master_password";
    let salt = generate_salt();

    // Derive keys
    let encryption = SyncEncryption::from_password(password, &salt).expect("Key derivation failed");

    // Test encryption roundtrip
    let plaintext = b"Hello, this is a test credential!";
    let payload = encryption
        .encrypt_payload(plaintext)
        .expect("Encryption failed");
    let decrypted = encryption
        .decrypt_payload(&payload)
        .expect("Decryption failed");

    assert_eq!(plaintext.to_vec(), decrypted);
}

/// Test that same password + salt produces same keys
#[tokio::test]
async fn test_sync_encryption_deterministic() {
    let password = "my_master_password";
    let salt = generate_salt();

    let enc1 = SyncEncryption::from_password(password, &salt).unwrap();
    let enc2 = SyncEncryption::from_password(password, &salt).unwrap();

    // Auth proofs should be identical
    assert_eq!(enc1.auth_proof(), enc2.auth_proof());

    // Encrypt with one, decrypt with other
    let plaintext = b"Test data";
    let payload = enc1.encrypt_payload(plaintext).unwrap();
    let decrypted = enc2.decrypt_payload(&payload).unwrap();
    assert_eq!(plaintext.to_vec(), decrypted);
}

/// Test full sync workflow: register, login, push, pull
#[tokio::test]
async fn test_sync_client_full_workflow() {
    let server = TestServer::start().await;
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let password = "my_master_password";
    let salt = generate_salt();

    // Create encryption helper
    let encryption = SyncEncryption::from_password(password, &salt).expect("Key derivation failed");
    let auth_proof = encryption.auth_proof();

    // Create client and register/login
    let mut client = SyncClient::new(&server.base_url()).expect("Failed to create client");

    use base64::Engine;
    let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);
    client
        .register("sync_workflow_user", &auth_proof, &salt_b64)
        .await
        .expect("Registration failed");

    let login_resp = client
        .login("sync_workflow_user", &auth_proof, "Test Device")
        .await
        .expect("Login failed");

    let device_id = login_resp.device_id.clone();

    // Save config (simulating what Android app does)
    let state_manager = SyncStateManager::new(temp_dir.path());
    let config = SyncConfig::new(
        server.base_url(),
        "sync_workflow_user".to_string(),
        device_id.clone(),
        salt.to_vec(),
    );
    state_manager.save_config(&config).expect("Failed to save config");

    // Create a test collection and credential
    let collection = create_test_collection("Home", vec![create_test_credential("MyHomeWifi")]);

    // Create collection on server
    let encrypted_name = encryption
        .encrypt_payload(collection.name.as_bytes())
        .expect("Failed to encrypt name");
    let encrypted_name_bytes = serde_json::to_vec(&encrypted_name).unwrap();

    client
        .create_collection(Some(collection.id), encrypted_name_bytes)
        .await
        .expect("Failed to create collection");

    // Build push request
    let credential = &collection.credentials[0];
    let credential_json = serde_json::to_vec(credential).expect("Failed to serialize credential");
    let encrypted_payload = encryption
        .encrypt_payload(&credential_json)
        .expect("Failed to encrypt");

    let mut clock = VectorClock::new();
    clock.increment(&device_id);

    let change = SyncChange::new(
        collection.id,
        credential.id,
        SyncOperation::Create,
        clock,
        encrypted_payload,
        device_id.clone(),
    );

    let push_req = PushRequest {
        device_id: device_id.clone(),
        changes: vec![change],
    };

    // Push
    let push_resp = client.push(push_req).await.expect("Push failed");
    assert_eq!(push_resp.accepted_count, 1);
    assert_eq!(push_resp.conflict_count, 0);

    // Pull and verify
    let pull_resp = client
        .pull(&device_id, None, None)
        .await
        .expect("Pull failed");

    assert_eq!(pull_resp.changes.len(), 1);

    // Decrypt the pulled credential
    let pulled_change = &pull_resp.changes[0];
    let decrypted_bytes = encryption
        .decrypt_payload(&pulled_change.payload)
        .expect("Failed to decrypt pulled credential");
    let pulled_credential: WifiCredential =
        serde_json::from_slice(&decrypted_bytes).expect("Failed to parse credential");

    assert_eq!(pulled_credential.ssid, "MyHomeWifi");
}

/// Test syncing multiple credentials
#[tokio::test]
async fn test_sync_multiple_credentials() {
    let server = TestServer::start().await;

    let password = "test_password";
    let salt = generate_salt();
    let encryption = SyncEncryption::from_password(password, &salt).unwrap();
    let auth_proof = encryption.auth_proof();

    let mut client = SyncClient::new(&server.base_url()).unwrap();
    client.register("multi_cred_user", &auth_proof, "").await.unwrap();
    let login_resp = client
        .login("multi_cred_user", &auth_proof, "Test Device")
        .await
        .unwrap();
    let device_id = login_resp.device_id;

    // Create collection with multiple credentials
    let collection = create_test_collection(
        "Networks",
        vec![
            create_test_credential("Network1"),
            create_test_credential("Network2"),
            create_test_credential("Network3"),
        ],
    );

    // Create collection on server
    let encrypted_name = encryption.encrypt_payload(collection.name.as_bytes()).unwrap();
    client
        .create_collection(Some(collection.id), serde_json::to_vec(&encrypted_name).unwrap())
        .await
        .unwrap();

    // Push all credentials
    let mut clock = VectorClock::new();
    let mut changes = Vec::new();

    for credential in &collection.credentials {
        clock.increment(&device_id);
        let credential_json = serde_json::to_vec(credential).unwrap();
        let encrypted_payload = encryption.encrypt_payload(&credential_json).unwrap();

        changes.push(SyncChange::new(
            collection.id,
            credential.id,
            SyncOperation::Create,
            clock.clone(),
            encrypted_payload,
            device_id.clone(),
        ));
    }

    let push_req = PushRequest {
        device_id: device_id.clone(),
        changes,
    };

    let push_resp = client.push(push_req).await.unwrap();
    assert_eq!(push_resp.accepted_count, 3);

    // Pull and verify all credentials
    let pull_resp = client.pull(&device_id, None, None).await.unwrap();
    assert_eq!(pull_resp.changes.len(), 3);

    // Decrypt and verify each credential
    let mut ssids: Vec<String> = pull_resp
        .changes
        .iter()
        .map(|change| {
            let decrypted = encryption.decrypt_payload(&change.payload).unwrap();
            let cred: WifiCredential = serde_json::from_slice(&decrypted).unwrap();
            cred.ssid
        })
        .collect();
    ssids.sort();

    assert_eq!(ssids, vec!["Network1", "Network2", "Network3"]);
}

/// Test syncing between two simulated devices
#[tokio::test]
async fn test_sync_between_devices() {
    let server = TestServer::start().await;

    let password = "shared_password";
    let salt = generate_salt();
    let encryption = SyncEncryption::from_password(password, &salt).unwrap();
    let auth_proof = encryption.auth_proof();

    // Device 1: Register and login
    let mut client1 = SyncClient::new(&server.base_url()).unwrap();
    client1.register("multi_device_user", &auth_proof, "").await.unwrap();
    let login1 = client1
        .login("multi_device_user", &auth_proof, "Device 1")
        .await
        .unwrap();
    let device1_id = login1.device_id;

    // Device 2: Login (same user)
    let mut client2 = SyncClient::new(&server.base_url()).unwrap();
    let login2 = client2
        .login("multi_device_user", &auth_proof, "Device 2")
        .await
        .unwrap();
    let device2_id = login2.device_id;

    // Device 1: Create and push a credential
    let collection = create_test_collection("SharedNetworks", vec![create_test_credential("SharedWifi")]);

    let encrypted_name = encryption.encrypt_payload(collection.name.as_bytes()).unwrap();
    client1
        .create_collection(Some(collection.id), serde_json::to_vec(&encrypted_name).unwrap())
        .await
        .unwrap();

    let credential = &collection.credentials[0];
    let credential_json = serde_json::to_vec(credential).unwrap();
    let encrypted_payload = encryption.encrypt_payload(&credential_json).unwrap();

    let mut clock = VectorClock::new();
    clock.increment(&device1_id);

    let push_req = PushRequest {
        device_id: device1_id.clone(),
        changes: vec![SyncChange::new(
            collection.id,
            credential.id,
            SyncOperation::Create,
            clock,
            encrypted_payload,
            device1_id.clone(),
        )],
    };

    client1.push(push_req).await.unwrap();

    // Device 2: Pull and verify it receives Device 1's credential
    let pull_resp = client2.pull(&device2_id, None, None).await.unwrap();
    assert_eq!(pull_resp.changes.len(), 1);

    let decrypted = encryption
        .decrypt_payload(&pull_resp.changes[0].payload)
        .unwrap();
    let pulled_cred: WifiCredential = serde_json::from_slice(&decrypted).unwrap();
    assert_eq!(pulled_cred.ssid, "SharedWifi");
}

/// Test that SyncStateManager correctly tracks sync state
#[tokio::test]
async fn test_sync_state_manager() {
    let temp_dir = TempDir::new().unwrap();
    let state_manager = SyncStateManager::new(temp_dir.path());

    // Initially not configured
    assert!(!state_manager.is_configured());

    // Save config
    let salt = generate_salt();
    let config = SyncConfig::new(
        "http://localhost:8080".to_string(),
        "testuser".to_string(),
        "device123".to_string(),
        salt.to_vec(),
    );
    state_manager.save_config(&config).unwrap();

    // Now configured
    assert!(state_manager.is_configured());

    // Load config
    let loaded = state_manager.load_config().unwrap().unwrap();
    assert_eq!(loaded.server_url, "http://localhost:8080");
    assert_eq!(loaded.username, "testuser");
    assert_eq!(loaded.device_id, "device123");

    // Delete config (logout)
    state_manager.delete_config().unwrap();
    assert!(!state_manager.is_configured());
}

/// Test first sync detection (all credentials should be pushed)
#[tokio::test]
async fn test_first_sync_detection() {
    use wifisync_core::sync::SyncState;

    // New state with no history = first sync
    let state = SyncState::new();
    assert!(state.last_sync.is_none());
    assert!(!state.has_pending_changes());

    // This combination indicates first sync
    let is_first_sync = state.last_sync.is_none() && !state.has_pending_changes();
    assert!(is_first_sync);
}

/// Test credential deletion sync (tombstone)
#[tokio::test]
async fn test_sync_credential_deletion() {
    let server = TestServer::start().await;

    let password = "test_password";
    let salt = generate_salt();
    let encryption = SyncEncryption::from_password(password, &salt).unwrap();
    let auth_proof = encryption.auth_proof();

    let mut client = SyncClient::new(&server.base_url()).unwrap();
    client.register("deletion_user", &auth_proof, "").await.unwrap();
    let login_resp = client
        .login("deletion_user", &auth_proof, "Test Device")
        .await
        .unwrap();
    let device_id = login_resp.device_id;

    let collection = create_test_collection("ToDelete", vec![create_test_credential("WillBeDeleted")]);

    // Create collection
    let encrypted_name = encryption.encrypt_payload(collection.name.as_bytes()).unwrap();
    client
        .create_collection(Some(collection.id), serde_json::to_vec(&encrypted_name).unwrap())
        .await
        .unwrap();

    // Push create
    let credential = &collection.credentials[0];
    let credential_json = serde_json::to_vec(credential).unwrap();
    let encrypted_payload = encryption.encrypt_payload(&credential_json).unwrap();

    let mut clock = VectorClock::new();
    clock.increment(&device_id);

    client
        .push(PushRequest {
            device_id: device_id.clone(),
            changes: vec![SyncChange::new(
                collection.id,
                credential.id,
                SyncOperation::Create,
                clock.clone(),
                encrypted_payload,
                device_id.clone(),
            )],
        })
        .await
        .unwrap();

    // Push delete (tombstone)
    clock.increment(&device_id);
    let empty_payload = encryption.encrypt_payload(&[]).unwrap();

    client
        .push(PushRequest {
            device_id: device_id.clone(),
            changes: vec![SyncChange::new(
                collection.id,
                credential.id,
                SyncOperation::Delete,
                clock,
                empty_payload,
                device_id.clone(),
            )],
        })
        .await
        .unwrap();

    // Pull and verify tombstone
    let pull_resp = client.pull(&device_id, None, None).await.unwrap();
    assert_eq!(pull_resp.changes.len(), 1);
    assert_eq!(pull_resp.changes[0].operation, SyncOperation::Delete);
}

/// Test logout clears local config
#[tokio::test]
async fn test_sync_logout_clears_config() {
    let server = TestServer::start().await;
    let temp_dir = TempDir::new().unwrap();

    let password = "test_password";
    let salt = generate_salt();
    let encryption = SyncEncryption::from_password(password, &salt).unwrap();
    let auth_proof = encryption.auth_proof();

    // Login
    let mut client = SyncClient::new(&server.base_url()).unwrap();
    client.register("logout_test_user", &auth_proof, "").await.unwrap();
    let login_resp = client
        .login("logout_test_user", &auth_proof, "Test Device")
        .await
        .unwrap();

    // Save config
    let state_manager = SyncStateManager::new(temp_dir.path());
    let mut config = SyncConfig::new(
        server.base_url(),
        "logout_test_user".to_string(),
        login_resp.device_id.clone(),
        salt.to_vec(),
    );
    config.set_token(login_resp.token, login_resp.expires_at);
    state_manager.save_config(&config).unwrap();

    assert!(state_manager.is_configured());

    // Logout
    client.logout().await.unwrap();
    state_manager.delete_config().unwrap();

    assert!(!state_manager.is_configured());
}

/// Test re-login: register, login, get_salt, derive same proof, login again
#[tokio::test]
async fn test_sync_client_relogin() {
    let server = TestServer::start().await;

    let password = "my_master_password";
    let salt = generate_salt();

    // First login: register with salt
    let encryption = SyncEncryption::from_password(password, &salt).unwrap();
    let auth_proof = encryption.auth_proof();

    use base64::Engine;
    let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);

    let mut client = SyncClient::new(&server.base_url()).unwrap();
    client
        .register("relogin_user", &auth_proof, &salt_b64)
        .await
        .unwrap();
    let login1 = client
        .login("relogin_user", &auth_proof, "Device 1")
        .await
        .unwrap();
    assert!(!login1.device_id.is_empty());

    // Simulate re-login: get salt from server, derive same keys
    let retrieved_salt_b64 = client
        .get_salt("relogin_user")
        .await
        .unwrap()
        .expect("Salt should exist for registered user");
    assert_eq!(retrieved_salt_b64, salt_b64);

    let retrieved_salt_bytes = base64::engine::general_purpose::STANDARD
        .decode(&retrieved_salt_b64)
        .unwrap();
    let retrieved_salt: [u8; 32] = retrieved_salt_bytes.try_into().unwrap();

    let encryption2 = SyncEncryption::from_password(password, &retrieved_salt).unwrap();
    let auth_proof2 = encryption2.auth_proof();

    // Auth proofs should be identical
    assert_eq!(auth_proof, auth_proof2);

    // Re-login should succeed
    let mut client2 = SyncClient::new(&server.base_url()).unwrap();
    let login2 = client2
        .login("relogin_user", &auth_proof2, "Device 2")
        .await
        .unwrap();
    assert!(!login2.device_id.is_empty());
}

/// Test get_salt returns None for nonexistent user
#[tokio::test]
async fn test_sync_client_get_salt_nonexistent() {
    let server = TestServer::start().await;
    let client = SyncClient::new(&server.base_url()).unwrap();

    let result = client.get_salt("nonexistent_user").await.unwrap();
    assert!(result.is_none());
}

/// Test get_salt works with usernames containing URL-special characters
/// (spaces, @, #, etc.) — these must be percent-encoded in the URL path
#[tokio::test]
async fn test_sync_client_get_salt_special_chars_username() {
    let server = TestServer::start().await;
    let mut client = SyncClient::new(&server.base_url()).unwrap();

    let special_username = "user @name#test";
    let auth_proof = "some_proof";
    let auth_salt = "c29tZV9zYWx0";

    // Register user with special characters in username
    client
        .register(special_username, auth_proof, auth_salt)
        .await
        .unwrap();

    // get_salt must not fail with "builder error" — the username must be percent-encoded
    let result = client.get_salt(special_username).await.unwrap();
    assert_eq!(result.unwrap(), auth_salt);

    // Login should also succeed
    let login = client
        .login(special_username, auth_proof, "Test Device")
        .await
        .unwrap();
    assert!(!login.device_id.is_empty());
}
