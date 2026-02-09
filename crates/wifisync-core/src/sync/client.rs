//! HTTP client for sync server communication

use chrono::{DateTime, Utc};
use reqwest::Client;
use wifisync_sync_protocol::{
    ApiError, CollectionsResponse, ConflictResolution, ConflictsResponse,
    CreateCollectionRequest, CreateCollectionResponse, LoginRequest, LoginResponse, PullRequest,
    PullResponse, PushRequest, PushResponse, RegisterRequest, RegisterResponse,
    ResolveConflictRequest, SaltResponse, VectorClock,
};

use crate::error::{Error, Result};

use super::state::SyncConfig;

/// HTTP client for communicating with the sync server
pub struct SyncClient {
    /// HTTP client
    client: Client,
    /// Server base URL
    base_url: String,
    /// Current JWT token
    token: Option<String>,
}

impl SyncClient {
    /// Create a new sync client
    pub fn new(base_url: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            token: None,
        })
    }

    /// Create a sync client from existing configuration
    pub fn from_config(config: &SyncConfig) -> Result<Self> {
        let mut client = Self::new(&config.server_url)?;
        if let Some(token) = &config.token {
            client.token = Some(token.clone());
        }
        Ok(client)
    }

    /// Set the authentication token
    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    /// Clear the authentication token
    pub fn clear_token(&mut self) {
        self.token = None;
    }

    /// Build a URL for an endpoint
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Make an authenticated request
    fn auth_header(&self) -> Option<String> {
        self.token.as_ref().map(|t| format!("Bearer {}", t))
    }

    /// Handle API error response
    async fn handle_error(response: reqwest::Response) -> Error {
        let status = response.status();
        match response.json::<ApiError>().await {
            Ok(api_error) => Error::internal(format!(
                "Server error ({}): {} - {}",
                status, api_error.code, api_error.message
            )),
            Err(_) => Error::internal(format!("Server error: {}", status)),
        }
    }

    // =========================================================================
    // Authentication
    // =========================================================================

    /// Register a new user
    pub async fn register(&self, username: &str, auth_proof: &str, auth_salt: &str) -> Result<RegisterResponse> {
        let url = self.url("/api/v1/users/register");
        let req = RegisterRequest {
            username: username.to_string(),
            auth_proof: auth_proof.to_string(),
            auth_salt: auth_salt.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse response: {e}")))
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    /// Get the auth salt for a user (returns None if user doesn't exist)
    pub async fn get_salt(&self, username: &str) -> Result<Option<String>> {
        // Build URL with proper percent-encoding of the username path segment
        let base = self.url("/api/v1/auth/salt");
        let mut url = reqwest::Url::parse(&base)
            .map_err(|e| Error::internal(format!("Invalid URL: {e}")))?;
        url.path_segments_mut()
            .map_err(|_| Error::internal("URL cannot-be-a-base"))?
            .push(username);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            let salt_resp: SaltResponse = response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse response: {e}")))?;
            Ok(Some(salt_resp.auth_salt))
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    /// Login (authenticate a device)
    pub async fn login(
        &mut self,
        username: &str,
        auth_proof: &str,
        device_name: &str,
    ) -> Result<LoginResponse> {
        let url = self.url("/api/v1/auth/login");
        let req = LoginRequest {
            username: username.to_string(),
            auth_proof: auth_proof.to_string(),
            device_name: device_name.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            let login_resp: LoginResponse = response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse response: {e}")))?;

            // Store the token
            self.token = Some(login_resp.token.clone());

            Ok(login_resp)
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    /// Refresh JWT token
    pub async fn refresh_token(&mut self) -> Result<(String, DateTime<Utc>)> {
        let url = self.url("/api/v1/auth/refresh");

        let token = self
            .token
            .as_ref()
            .ok_or_else(|| Error::internal("No token to refresh"))?
            .clone();

        let req = serde_json::json!({ "token": token });

        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            let resp: serde_json::Value = response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse response: {e}")))?;

            let new_token = resp["token"]
                .as_str()
                .ok_or_else(|| Error::internal("Missing token in response"))?
                .to_string();

            let expires_str = resp["expires_at"]
                .as_str()
                .ok_or_else(|| Error::internal("Missing expires_at in response"))?;

            let expires_at = DateTime::parse_from_rfc3339(expires_str)
                .map_err(|e| Error::internal(format!("Invalid expires_at: {e}")))?
                .with_timezone(&Utc);

            self.token = Some(new_token.clone());

            Ok((new_token, expires_at))
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    /// Logout (delete device from server)
    pub async fn logout(&mut self) -> Result<()> {
        let url = self.url("/api/v1/auth/logout");

        let auth = self
            .auth_header()
            .ok_or_else(|| Error::internal("Not logged in"))?;

        let response = self
            .client
            .delete(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            self.token = None;
            Ok(())
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    // =========================================================================
    // Sync Operations
    // =========================================================================

    /// Push changes to the server
    pub async fn push(&self, req: PushRequest) -> Result<PushResponse> {
        let url = self.url("/api/v1/sync/push");

        let auth = self
            .auth_header()
            .ok_or_else(|| Error::internal("Not logged in"))?;

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth)
            .json(&req)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse response: {e}")))
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    /// Pull changes from the server
    pub async fn pull(
        &self,
        device_id: &str,
        since: Option<VectorClock>,
        collection_id: Option<uuid::Uuid>,
    ) -> Result<PullResponse> {
        let url = self.url("/api/v1/sync/pull");

        let auth = self
            .auth_header()
            .ok_or_else(|| Error::internal("Not logged in"))?;

        let req = PullRequest {
            device_id: device_id.to_string(),
            since,
            collection_id,
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth)
            .json(&req)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse response: {e}")))
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    // =========================================================================
    // Conflicts
    // =========================================================================

    /// Get pending conflicts
    pub async fn get_conflicts(&self) -> Result<ConflictsResponse> {
        let url = self.url("/api/v1/sync/conflicts");

        let auth = self
            .auth_header()
            .ok_or_else(|| Error::internal("Not logged in"))?;

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse response: {e}")))
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    /// Resolve a conflict
    pub async fn resolve_conflict(
        &self,
        conflict_id: uuid::Uuid,
        resolution: ConflictResolution,
    ) -> Result<()> {
        let url = self.url(&format!("/api/v1/sync/conflicts/{}/resolve", conflict_id));

        let auth = self
            .auth_header()
            .ok_or_else(|| Error::internal("Not logged in"))?;

        let req = ResolveConflictRequest {
            conflict_id,
            resolution,
            merged_payload: None,
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth)
            .json(&req)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    // =========================================================================
    // Collections
    // =========================================================================

    /// List collections
    pub async fn list_collections(&self) -> Result<CollectionsResponse> {
        let url = self.url("/api/v1/collections");

        let auth = self
            .auth_header()
            .ok_or_else(|| Error::internal("Not logged in"))?;

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse response: {e}")))
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    /// Create a collection
    pub async fn create_collection(
        &self,
        id: Option<uuid::Uuid>,
        encrypted_name: Vec<u8>,
    ) -> Result<CreateCollectionResponse> {
        let url = self.url("/api/v1/collections");

        let auth = self
            .auth_header()
            .ok_or_else(|| Error::internal("Not logged in"))?;

        let req = CreateCollectionRequest { id, encrypted_name };

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth)
            .json(&req)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse response: {e}")))
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    /// Delete a collection
    pub async fn delete_collection(&self, collection_id: uuid::Uuid) -> Result<()> {
        let url = self.url(&format!("/api/v1/collections/{}", collection_id));

        let auth = self
            .auth_header()
            .ok_or_else(|| Error::internal("Not logged in"))?;

        let response = self
            .client
            .delete(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(Self::handle_error(response).await)
        }
    }

    // =========================================================================
    // Health Check
    // =========================================================================

    /// Check if the server is reachable
    pub async fn health_check(&self) -> Result<bool> {
        let url = self.url("/health");

        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|_| Error::internal("Server unreachable"))?;

        Ok(response.status().is_success())
    }
}
