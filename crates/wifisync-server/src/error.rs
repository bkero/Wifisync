//! Server error handling

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;
use wifisync_sync_protocol::ApiError;

/// Server error type
#[derive(Debug, Error)]
pub enum ServerError {
    /// Authentication required
    #[error("Authentication required")]
    Unauthorized,

    /// Invalid credentials
    #[error("Invalid credentials")]
    InvalidCredentials,

    /// Token expired or invalid
    #[error("Token expired or invalid")]
    TokenExpired,

    /// Resource not found
    #[error("{resource} not found")]
    NotFound { resource: String },

    /// Conflict detected
    #[error("Conflict: {message}")]
    Conflict { message: String },

    /// Validation error
    #[error("Validation error: {message}")]
    Validation { message: String },

    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// JWT error
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    /// Password hashing error
    #[error("Password error: {0}")]
    Password(#[from] bcrypt::BcryptError),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ServerError {
    /// Create a not found error
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    /// Create a conflict error
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict {
            message: message.into(),
        }
    }

    /// Create a validation error
    #[allow(dead_code)]
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, api_error) = match &self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, ApiError::unauthorized()),
            Self::InvalidCredentials => (StatusCode::UNAUTHORIZED, ApiError::invalid_credentials()),
            Self::TokenExpired => (
                StatusCode::UNAUTHORIZED,
                ApiError::new("token_expired", "Token expired or invalid"),
            ),
            Self::NotFound { resource } => (StatusCode::NOT_FOUND, ApiError::not_found(resource)),
            Self::Conflict { message } => (StatusCode::CONFLICT, ApiError::conflict(message)),
            Self::Validation { message } => (StatusCode::BAD_REQUEST, ApiError::validation(message)),
            Self::Database(e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, ApiError::internal())
            }
            Self::Jwt(e) => {
                tracing::error!("JWT error: {}", e);
                (StatusCode::UNAUTHORIZED, ApiError::new("jwt_error", e.to_string()))
            }
            Self::Password(e) => {
                tracing::error!("Password error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, ApiError::internal())
            }
            Self::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, ApiError::internal())
            }
        };

        (status, Json(api_error)).into_response()
    }
}

/// Result type alias for server handlers
pub type ServerResult<T> = Result<T, ServerError>;
