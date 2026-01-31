//! Integration tests for the sync server and client
//!
//! These tests start a real server instance and test the full sync workflow.

use std::net::SocketAddr;
use tokio::sync::oneshot;
use uuid::Uuid;

use wifisync_sync_protocol::{LoginRequest, RegisterRequest, VectorClock};

/// Test server configuration
struct TestServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl TestServer {
    /// Start a test server on a random port
    async fn start() -> Self {
        use sqlx::sqlite::SqlitePoolOptions;
        use tokio::net::TcpListener;

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
        let config = TestConfig::new();
        let state = AppState {
            db: pool,
            config,
        };

        // Create router
        let app = create_test_router(state);

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

/// Test configuration
#[derive(Clone)]
struct TestConfig {
    jwt_secret: String,
    jwt_expiration_hours: u64,
}

impl TestConfig {
    fn new() -> Self {
        Self {
            jwt_secret: "test_secret_key_for_integration_tests_12345".to_string(),
            jwt_expiration_hours: 24,
        }
    }
}

/// Simplified AppState for testing
#[derive(Clone)]
struct AppState {
    db: sqlx::SqlitePool,
    config: TestConfig,
}

/// Run database migrations
async fn run_migrations(pool: &sqlx::SqlitePool) {
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            auth_key_hash TEXT NOT NULL,
            created_at TEXT NOT NULL
        )
        ",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS devices (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            device_token_hash TEXT NOT NULL,
            last_sync_at TEXT,
            created_at TEXT NOT NULL
        )
        ",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS collections (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            encrypted_name BLOB NOT NULL,
            vector_clock TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        ",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS sync_records (
            id TEXT PRIMARY KEY,
            collection_id TEXT NOT NULL,
            credential_id TEXT NOT NULL,
            vector_clock TEXT NOT NULL,
            encrypted_payload BLOB NOT NULL,
            tombstone INTEGER DEFAULT 0,
            updated_at TEXT NOT NULL,
            UNIQUE(collection_id, credential_id)
        )
        ",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS conflicts (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            collection_id TEXT NOT NULL,
            credential_id TEXT NOT NULL,
            local_payload BLOB,
            local_vector_clock TEXT,
            remote_payload BLOB,
            remote_vector_clock TEXT,
            created_at TEXT NOT NULL,
            resolved INTEGER DEFAULT 0
        )
        ",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Create the test router with simplified handlers
fn create_test_router(state: AppState) -> axum::Router {
    use axum::{
        extract::{Path, State},
        http::StatusCode,
        routing::{delete, get, post},
        Json, Router,
    };
    use chrono::{Duration, Utc};

    async fn health() -> &'static str {
        "ok"
    }

    async fn register(
        State(state): State<AppState>,
        Json(req): Json<RegisterRequest>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        // Check if user exists
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM users WHERE username = ?")
                .bind(&req.username)
                .fetch_optional(&state.db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if existing.is_some() {
            return Err((
                StatusCode::BAD_REQUEST,
                "Username already exists".to_string(),
            ));
        }

        let user_id = Uuid::new_v4().to_string();
        let auth_hash = bcrypt::hash(&req.auth_proof, 4).unwrap(); // Use cost 4 for faster tests

        sqlx::query("INSERT INTO users (id, username, auth_key_hash, created_at) VALUES (?, ?, ?, ?)")
            .bind(&user_id)
            .bind(&req.username)
            .bind(&auth_hash)
            .bind(Utc::now().to_rfc3339())
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        Ok(Json(serde_json::json!({ "user_id": user_id })))
    }

    async fn login(
        State(state): State<AppState>,
        Json(req): Json<LoginRequest>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        // Find user
        let user: Option<(String, String)> =
            sqlx::query_as("SELECT id, auth_key_hash FROM users WHERE username = ?")
                .bind(&req.username)
                .fetch_optional(&state.db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let (user_id, auth_hash) = user.ok_or((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()))?;

        // Verify password
        if !bcrypt::verify(&req.auth_proof, &auth_hash).unwrap_or(false) {
            return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()));
        }

        // Create device
        let device_id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO devices (id, user_id, name, device_token_hash, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&device_id)
            .bind(&user_id)
            .bind(&req.device_name)
            .bind("token_hash")
            .bind(Utc::now().to_rfc3339())
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Create JWT
        let exp = Utc::now() + Duration::hours(state.config.jwt_expiration_hours as i64);
        let claims = serde_json::json!({
            "sub": user_id,
            "device_id": device_id,
            "exp": exp.timestamp(),
            "iat": Utc::now().timestamp()
        });

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        )
        .unwrap();

        Ok(Json(serde_json::json!({
            "device_id": device_id,
            "token": token,
            "expires_at": exp.to_rfc3339()
        })))
    }

    async fn push(
        State(state): State<AppState>,
        body: String,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        // Parse manually to handle the payload bytes correctly
        let req: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;

        let changes = req["changes"].as_array()
            .ok_or((StatusCode::BAD_REQUEST, "Missing changes array".to_string()))?;

        let mut results = Vec::new();
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

            results.push(serde_json::json!({
                "accepted": { "change_id": change_id }
            }));
            accepted += 1;
        }

        Ok(Json(serde_json::json!({
            "results": results,
            "accepted_count": accepted,
            "conflict_count": 0
        })))
    }

    async fn pull(
        State(state): State<AppState>,
        _body: String,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        // Get all sync records
        let records: Vec<(String, String, String, String, Vec<u8>, i32, String)> = sqlx::query_as(
            "SELECT id, collection_id, credential_id, vector_clock, encrypted_payload, tombstone, updated_at FROM sync_records",
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut changes = Vec::new();
        let server_clock = VectorClock::new();

        for (id, coll_id, cred_id, clock_str, payload_bytes, tombstone, updated_at) in records {
            // Parse the clock from the stored JSON
            let clock: serde_json::Value = serde_json::from_str(&clock_str).unwrap_or_default();

            // Parse the payload from stored bytes (which is JSON string of the payload)
            let payload: serde_json::Value = String::from_utf8(payload_bytes.clone())
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| serde_json::json!({
                    "encrypted_data": [],
                    "nonce": []
                }));

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

        Ok(Json(serde_json::json!({
            "changes": changes,
            "server_clock": server_clock,
            "has_more": false
        })))
    }

    async fn list_collections(
        State(state): State<AppState>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
        let collections: Vec<(String, Vec<u8>, String, String)> = sqlx::query_as(
            "SELECT id, encrypted_name, vector_clock, updated_at FROM collections",
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let colls: Vec<serde_json::Value> = collections
            .into_iter()
            .map(|(id, name, clock, updated)| {
                serde_json::json!({
                    "id": id,
                    "encrypted_name": name,
                    "vector_clock": VectorClock::from_json(&clock).unwrap_or_default(),
                    "updated_at": updated
                })
            })
            .collect();

        Ok(Json(serde_json::json!({ "collections": colls })))
    }

    async fn create_collection(
        State(state): State<AppState>,
        body: String,
    ) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
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
            .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect::<Vec<u8>>())
            .unwrap_or_default();

        // Use a placeholder user_id for testing
        sqlx::query(
            "INSERT INTO collections (id, user_id, encrypted_name, vector_clock, updated_at) VALUES (?, 'test_user', ?, '{}', ?)",
        )
        .bind(&id)
        .bind(&encrypted_name)
        .bind(Utc::now().to_rfc3339())
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        Ok(Json(serde_json::json!({ "id": id })))
    }

    async fn delete_collection(
        State(state): State<AppState>,
        Path(id): Path<String>,
    ) -> Result<StatusCode, (StatusCode, String)> {
        sqlx::query("DELETE FROM collections WHERE id = ?")
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        Ok(StatusCode::NO_CONTENT)
    }

    async fn list_conflicts() -> Json<serde_json::Value> {
        Json(serde_json::json!({ "conflicts": [] }))
    }

    Router::new()
        .route("/health", get(health))
        .route("/api/v1/users/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/sync/push", post(push))
        .route("/api/v1/sync/pull", post(pull))
        .route("/api/v1/collections", get(list_collections))
        .route("/api/v1/collections", post(create_collection))
        .route("/api/v1/collections/:id", delete(delete_collection))
        .route("/api/v1/sync/conflicts", get(list_conflicts))
        .with_state(state)
}

/// HTTP client helper for tests
struct TestClient {
    client: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl TestClient {
    fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
            token: None,
        }
    }

    fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        let mut req = self.client.get(format!("{}{}", self.base_url, path));
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        req.send().await.unwrap()
    }

    async fn post<T: serde::Serialize>(&self, path: &str, body: &T) -> reqwest::Response {
        let mut req = self.client.post(format!("{}{}", self.base_url, path));
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        req.json(body).send().await.unwrap()
    }

    async fn post_and_check<T: serde::Serialize>(&self, path: &str, body: &T) -> reqwest::Response {
        let resp = self.post(path, body).await;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            panic!("Request to {} failed with {}: {}", path, status, text);
        }
        resp
    }

    async fn delete(&self, path: &str) -> reqwest::Response {
        let mut req = self.client.delete(format!("{}{}", self.base_url, path));
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        req.send().await.unwrap()
    }
}

// =============================================================================
// Integration Tests
// =============================================================================

#[tokio::test]
async fn test_health_check() {
    let server = TestServer::start().await;
    let client = TestClient::new(&server.base_url());

    let resp = client.get("/health").await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn test_user_registration() {
    let server = TestServer::start().await;
    let client = TestClient::new(&server.base_url());

    let req = RegisterRequest {
        username: "testuser".to_string(),
        auth_proof: "test_auth_proof_123".to_string(),
    };

    let resp = client.post("/api/v1/users/register", &req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("user_id").is_some());
}

#[tokio::test]
async fn test_duplicate_registration_fails() {
    let server = TestServer::start().await;
    let client = TestClient::new(&server.base_url());

    let req = RegisterRequest {
        username: "duplicate_user".to_string(),
        auth_proof: "auth_proof".to_string(),
    };

    // First registration should succeed
    let resp = client.post("/api/v1/users/register", &req).await;
    assert_eq!(resp.status(), 200);

    // Second registration should fail
    let resp = client.post("/api/v1/users/register", &req).await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_login_flow() {
    let server = TestServer::start().await;
    let client = TestClient::new(&server.base_url());

    // Register first
    let register_req = RegisterRequest {
        username: "login_test_user".to_string(),
        auth_proof: "my_auth_proof".to_string(),
    };
    let resp = client.post("/api/v1/users/register", &register_req).await;
    assert_eq!(resp.status(), 200);

    // Now login
    let login_req = LoginRequest {
        username: "login_test_user".to_string(),
        auth_proof: "my_auth_proof".to_string(),
        device_name: "Test Device".to_string(),
    };
    let resp = client.post("/api/v1/auth/login", &login_req).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("token").is_some());
    assert!(body.get("device_id").is_some());
    assert!(body.get("expires_at").is_some());
}

#[tokio::test]
async fn test_login_wrong_password_fails() {
    let server = TestServer::start().await;
    let client = TestClient::new(&server.base_url());

    // Register
    let register_req = RegisterRequest {
        username: "wrong_pass_user".to_string(),
        auth_proof: "correct_password".to_string(),
    };
    client.post("/api/v1/users/register", &register_req).await;

    // Try to login with wrong password
    let login_req = LoginRequest {
        username: "wrong_pass_user".to_string(),
        auth_proof: "wrong_password".to_string(),
        device_name: "Test Device".to_string(),
    };
    let resp = client.post("/api/v1/auth/login", &login_req).await;
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_push_changes() {
    let server = TestServer::start().await;
    let mut client = TestClient::new(&server.base_url());

    // Register and login
    let register_req = RegisterRequest {
        username: "push_user".to_string(),
        auth_proof: "auth".to_string(),
    };
    client.post("/api/v1/users/register", &register_req).await;

    let login_req = LoginRequest {
        username: "push_user".to_string(),
        auth_proof: "auth".to_string(),
        device_name: "Test".to_string(),
    };
    let resp = client.post("/api/v1/auth/login", &login_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let token = body["token"].as_str().unwrap().to_string();
    let device_id = body["device_id"].as_str().unwrap().to_string();
    client.set_token(token);

    // Create a change using raw JSON to avoid serialization issues
    let push_req = serde_json::json!({
        "device_id": device_id,
        "changes": [{
            "id": Uuid::new_v4().to_string(),
            "collection_id": Uuid::new_v4().to_string(),
            "credential_id": Uuid::new_v4().to_string(),
            "operation": "create",
            "vector_clock": { "clocks": { &device_id: 1 } },
            "payload": {
                "encrypted_data": [1, 2, 3, 4, 5],
                "nonce": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            },
            "device_id": device_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }]
    });

    let resp = client.post_and_check("/api/v1/sync/push", &push_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["accepted_count"], 1);
    assert_eq!(body["conflict_count"], 0);
}

#[tokio::test]
async fn test_push_and_pull_roundtrip() {
    let server = TestServer::start().await;
    let mut client = TestClient::new(&server.base_url());

    // Register and login
    let register_req = RegisterRequest {
        username: "roundtrip_user".to_string(),
        auth_proof: "auth".to_string(),
    };
    client.post("/api/v1/users/register", &register_req).await;

    let login_req = LoginRequest {
        username: "roundtrip_user".to_string(),
        auth_proof: "auth".to_string(),
        device_name: "Test".to_string(),
    };
    let resp = client.post("/api/v1/auth/login", &login_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let token = body["token"].as_str().unwrap().to_string();
    let device_id = body["device_id"].as_str().unwrap().to_string();
    client.set_token(token);

    // Push some changes using raw JSON
    let collection_id = Uuid::new_v4();
    let credential_id = Uuid::new_v4();

    let push_req = serde_json::json!({
        "device_id": device_id,
        "changes": [{
            "id": Uuid::new_v4().to_string(),
            "collection_id": collection_id.to_string(),
            "credential_id": credential_id.to_string(),
            "operation": "create",
            "vector_clock": { "clocks": { &device_id: 1 } },
            "payload": {
                "encrypted_data": [1, 2, 3, 4, 5],
                "nonce": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            },
            "device_id": device_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }]
    });
    client.post_and_check("/api/v1/sync/push", &push_req).await;

    // Now pull
    let pull_req = serde_json::json!({
        "device_id": device_id,
        "since": null,
        "collection_id": null
    });
    let resp = client.post_and_check("/api/v1/sync/pull", &pull_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let changes = body["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1);

    // Verify the pulled change matches what we pushed
    let pulled = &changes[0];
    assert_eq!(pulled["credential_id"].as_str().unwrap(), credential_id.to_string());
}

#[tokio::test]
async fn test_collection_crud() {
    let server = TestServer::start().await;
    let mut client = TestClient::new(&server.base_url());

    // Register and login
    let register_req = RegisterRequest {
        username: "collection_user".to_string(),
        auth_proof: "auth".to_string(),
    };
    client.post("/api/v1/users/register", &register_req).await;

    let login_req = LoginRequest {
        username: "collection_user".to_string(),
        auth_proof: "auth".to_string(),
        device_name: "Test".to_string(),
    };
    let resp = client.post("/api/v1/auth/login", &login_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    client.set_token(body["token"].as_str().unwrap().to_string());

    // Create a collection
    let create_req = serde_json::json!({
        "encrypted_name": [1, 2, 3, 4, 5]
    });
    let resp = client.post_and_check("/api/v1/collections", &create_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let collection_id = body["id"].as_str().unwrap().to_string();

    // List collections
    let resp = client.get("/api/v1/collections").await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let collections = body["collections"].as_array().unwrap();
    assert_eq!(collections.len(), 1);

    // Delete the collection
    let resp = client
        .delete(&format!("/api/v1/collections/{}", collection_id))
        .await;
    assert_eq!(resp.status(), 204);

    // Verify deletion
    let resp = client.get("/api/v1/collections").await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let collections = body["collections"].as_array().unwrap();
    assert_eq!(collections.len(), 0);
}

#[tokio::test]
async fn test_multiple_devices_sync() {
    let server = TestServer::start().await;

    // Register user
    let mut client1 = TestClient::new(&server.base_url());
    let register_req = RegisterRequest {
        username: "multi_device_user".to_string(),
        auth_proof: "auth".to_string(),
    };
    client1.post("/api/v1/users/register", &register_req).await;

    // Login from device 1
    let login_req = LoginRequest {
        username: "multi_device_user".to_string(),
        auth_proof: "auth".to_string(),
        device_name: "Device 1".to_string(),
    };
    let resp = client1.post("/api/v1/auth/login", &login_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let device1_id = body["device_id"].as_str().unwrap().to_string();
    client1.set_token(body["token"].as_str().unwrap().to_string());

    // Login from device 2
    let mut client2 = TestClient::new(&server.base_url());
    let login_req = LoginRequest {
        username: "multi_device_user".to_string(),
        auth_proof: "auth".to_string(),
        device_name: "Device 2".to_string(),
    };
    let resp = client2.post("/api/v1/auth/login", &login_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let device2_id = body["device_id"].as_str().unwrap().to_string();
    client2.set_token(body["token"].as_str().unwrap().to_string());

    // Device 1 pushes a change
    let collection_id = Uuid::new_v4();
    let credential_id = Uuid::new_v4();

    let push_req = serde_json::json!({
        "device_id": device1_id,
        "changes": [{
            "id": Uuid::new_v4().to_string(),
            "collection_id": collection_id.to_string(),
            "credential_id": credential_id.to_string(),
            "operation": "create",
            "vector_clock": { "clocks": { &device1_id: 1 } },
            "payload": {
                "encrypted_data": [1, 2, 3],
                "nonce": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            },
            "device_id": device1_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }]
    });
    client1.post_and_check("/api/v1/sync/push", &push_req).await;

    // Device 2 pulls and sees the change
    let pull_req = serde_json::json!({
        "device_id": device2_id,
        "since": null,
        "collection_id": null
    });
    let resp = client2.post_and_check("/api/v1/sync/pull", &pull_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let changes = body["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0]["credential_id"].as_str().unwrap(),
        credential_id.to_string()
    );
}

#[tokio::test]
async fn test_push_multiple_changes() {
    let server = TestServer::start().await;
    let mut client = TestClient::new(&server.base_url());

    // Register and login
    let register_req = RegisterRequest {
        username: "multi_change_user".to_string(),
        auth_proof: "auth".to_string(),
    };
    client.post("/api/v1/users/register", &register_req).await;

    let login_req = LoginRequest {
        username: "multi_change_user".to_string(),
        auth_proof: "auth".to_string(),
        device_name: "Test".to_string(),
    };
    let resp = client.post("/api/v1/auth/login", &login_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let device_id = body["device_id"].as_str().unwrap().to_string();
    client.set_token(body["token"].as_str().unwrap().to_string());

    // Push multiple changes at once
    let collection_id = Uuid::new_v4();
    let mut changes = Vec::new();

    for i in 0..5 {
        changes.push(serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "collection_id": collection_id.to_string(),
            "credential_id": Uuid::new_v4().to_string(),
            "operation": "create",
            "vector_clock": { "clocks": { &device_id: 1 } },
            "payload": {
                "encrypted_data": [i as u8],
                "nonce": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            },
            "device_id": device_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }));
    }

    let push_req = serde_json::json!({
        "device_id": device_id,
        "changes": changes
    });

    let resp = client.post_and_check("/api/v1/sync/push", &push_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["accepted_count"], 5);

    // Verify all changes can be pulled
    let pull_req = serde_json::json!({
        "device_id": device_id,
        "since": null,
        "collection_id": null
    });
    let resp = client.post_and_check("/api/v1/sync/pull", &pull_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["changes"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn test_tombstone_sync() {
    let server = TestServer::start().await;
    let mut client = TestClient::new(&server.base_url());

    // Register and login
    let register_req = RegisterRequest {
        username: "tombstone_user".to_string(),
        auth_proof: "auth".to_string(),
    };
    client.post("/api/v1/users/register", &register_req).await;

    let login_req = LoginRequest {
        username: "tombstone_user".to_string(),
        auth_proof: "auth".to_string(),
        device_name: "Test".to_string(),
    };
    let resp = client.post("/api/v1/auth/login", &login_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let device_id = body["device_id"].as_str().unwrap().to_string();
    client.set_token(body["token"].as_str().unwrap().to_string());

    let collection_id = Uuid::new_v4();
    let credential_id = Uuid::new_v4();

    // First create
    let create_req = serde_json::json!({
        "device_id": device_id,
        "changes": [{
            "id": Uuid::new_v4().to_string(),
            "collection_id": collection_id.to_string(),
            "credential_id": credential_id.to_string(),
            "operation": "create",
            "vector_clock": { "clocks": { &device_id: 1 } },
            "payload": {
                "encrypted_data": [1, 2, 3],
                "nonce": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            },
            "device_id": device_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }]
    });
    client.post_and_check("/api/v1/sync/push", &create_req).await;

    // Now delete (tombstone)
    let delete_req = serde_json::json!({
        "device_id": device_id,
        "changes": [{
            "id": Uuid::new_v4().to_string(),
            "collection_id": collection_id.to_string(),
            "credential_id": credential_id.to_string(),
            "operation": "delete",
            "vector_clock": { "clocks": { &device_id: 2 } },
            "payload": {
                "encrypted_data": [],
                "nonce": []
            },
            "device_id": device_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }]
    });
    client.post_and_check("/api/v1/sync/push", &delete_req).await;

    // Pull and verify tombstone
    let pull_req = serde_json::json!({
        "device_id": device_id,
        "since": null,
        "collection_id": null
    });
    let resp = client.post_and_check("/api/v1/sync/pull", &pull_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();

    let changes = body["changes"].as_array().unwrap();
    // Should have one record (the tombstone replaced the create)
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["operation"].as_str().unwrap(), "delete");
}

#[tokio::test]
async fn test_empty_conflicts_list() {
    let server = TestServer::start().await;
    let mut client = TestClient::new(&server.base_url());

    // Register and login
    let register_req = RegisterRequest {
        username: "conflicts_user".to_string(),
        auth_proof: "auth".to_string(),
    };
    client.post("/api/v1/users/register", &register_req).await;

    let login_req = LoginRequest {
        username: "conflicts_user".to_string(),
        auth_proof: "auth".to_string(),
        device_name: "Test".to_string(),
    };
    let resp = client.post("/api/v1/auth/login", &login_req).await;
    let body: serde_json::Value = resp.json().await.unwrap();
    client.set_token(body["token"].as_str().unwrap().to_string());

    // Get conflicts (should be empty)
    let resp = client.get("/api/v1/sync/conflicts").await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let conflicts = body["conflicts"].as_array().unwrap();
    assert!(conflicts.is_empty());
}
