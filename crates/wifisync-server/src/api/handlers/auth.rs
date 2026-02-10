//! Authentication handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{Duration, Utc};
use wifisync_sync_protocol::{
    LoginRequest, LoginResponse, RefreshRequest, RefreshResponse, RegisterRequest,
    RegisterResponse, SaltResponse,
};

use crate::{
    api::auth::{create_token, validate_token, AuthenticatedUser},
    db::{
        models::{DbDevice, DbUser},
        queries,
    },
    error::{ServerError, ServerResult},
    AppState,
};

/// Register a new user
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> ServerResult<Json<RegisterResponse>> {
    tracing::info!("User registration request for: {}", req.username);

    // Check if username already exists
    if let Some(existing) = queries::find_user_by_username(&state.db, &req.username).await? {
        if existing.auth_salt.is_empty() {
            // Legacy user without salt — allow re-registration to upgrade credentials
            tracing::info!("Re-registering legacy user: {}", req.username);
            let auth_key_hash = bcrypt::hash(&req.auth_proof, state.config.bcrypt_cost)?;
            queries::update_user_credentials(
                &state.db,
                &req.username,
                &auth_key_hash,
                &req.auth_salt,
            )
            .await?;
            return Ok(Json(RegisterResponse {
                user_id: existing.uuid(),
            }));
        }
        return Err(ServerError::Validation {
            message: "Username already exists".to_string(),
        });
    }

    // Hash the auth proof (the client has already derived this from their master password)
    let auth_key_hash = bcrypt::hash(&req.auth_proof, state.config.bcrypt_cost)?;

    // Create user
    let user = DbUser::new(req.username, auth_key_hash, req.auth_salt);
    queries::create_user(&state.db, &user).await?;

    tracing::info!("User registered: {}", user.id);

    Ok(Json(RegisterResponse {
        user_id: user.uuid(),
    }))
}

/// Login (authenticate a device)
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ServerResult<Json<LoginResponse>> {
    tracing::info!("Login request for user: {}", req.username);

    // Find user
    let user = queries::find_user_by_username(&state.db, &req.username)
        .await?
        .ok_or(ServerError::InvalidCredentials)?;

    // Verify auth proof
    if !bcrypt::verify(&req.auth_proof, &user.auth_key_hash)? {
        return Err(ServerError::InvalidCredentials);
    }

    // Create or update device
    let device_token_hash = bcrypt::hash(&req.auth_proof, state.config.bcrypt_cost)?;
    let device = DbDevice::new(user.id.clone(), req.device_name, device_token_hash);
    queries::create_device(&state.db, &device).await?;

    // Create JWT
    let token = create_token(
        &user.id,
        &device.id,
        &state.config.jwt_secret,
        state.config.jwt_expiration_hours,
    )?;

    let expires_at = Utc::now() + Duration::hours(state.config.jwt_expiration_hours as i64);

    tracing::info!("User logged in: {} (device: {})", user.id, device.id);

    Ok(Json(LoginResponse {
        device_id: device.id,
        token,
        expires_at,
    }))
}

/// Refresh JWT token
pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> ServerResult<Json<RefreshResponse>> {
    // Validate current token
    let claims = validate_token(&req.token, &state.config.jwt_secret)
        .map_err(|_| ServerError::TokenExpired)?;

    // Check if device still exists
    queries::find_device_by_id(&state.db, &claims.device_id)
        .await?
        .ok_or(ServerError::not_found("Device"))?;

    // Create new token
    let token = create_token(
        &claims.sub,
        &claims.device_id,
        &state.config.jwt_secret,
        state.config.jwt_expiration_hours,
    )?;

    let expires_at = Utc::now() + Duration::hours(state.config.jwt_expiration_hours as i64);

    Ok(Json(RefreshResponse { token, expires_at }))
}

/// Get the auth salt for a user (used during re-login)
pub async fn get_salt(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> ServerResult<Json<SaltResponse>> {
    tracing::info!("Salt request for user: {}", username);

    let salt = queries::find_salt_by_username(&state.db, &username)
        .await?
        .filter(|s| !s.is_empty()) // Treat empty salt as non-existent (legacy user)
        .ok_or(ServerError::not_found("User"))?;

    Ok(Json(SaltResponse { auth_salt: salt }))
}

/// Logout (delete device)
pub async fn logout(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> ServerResult<StatusCode> {
    tracing::info!("Logout request for device: {}", user.device_id);

    queries::delete_device(&state.db, &user.device_id).await?;

    Ok(StatusCode::NO_CONTENT)
}
