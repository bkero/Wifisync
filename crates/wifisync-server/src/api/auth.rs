//! JWT authentication middleware and utilities

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::{error::ServerError, AppState};

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Device ID
    pub device_id: String,
    /// Expiration timestamp (Unix epoch)
    pub exp: i64,
    /// Issued at timestamp (Unix epoch)
    pub iat: i64,
}

impl Claims {
    /// Create new claims for a user and device
    pub fn new(user_id: String, device_id: String, expiration_hours: u64) -> Self {
        let now = Utc::now();
        let exp = now + Duration::hours(expiration_hours as i64);

        Self {
            sub: user_id,
            device_id,
            exp: exp.timestamp(),
            iat: now.timestamp(),
        }
    }

}

/// Create a JWT token
pub fn create_token(
    user_id: &str,
    device_id: &str,
    secret: &str,
    expiration_hours: u64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = Claims::new(user_id.to_string(), device_id.to_string(), expiration_hours);
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

/// Validate and decode a JWT token
pub fn validate_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::default();
    // Explicitly enforce expiration validation (default in v9.x, but be explicit)
    validation.validate_exp = true;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

/// Authenticated user extracted from JWT
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// User ID
    pub user_id: String,
    /// Device ID
    pub device_id: String,
}

/// Extractor for authenticated requests
#[async_trait::async_trait]
impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ServerError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract Authorization header
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ServerError::Unauthorized)?;

        // Check for Bearer prefix
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(ServerError::Unauthorized)?;

        // Validate token (decode rejects expired tokens via validate_exp)
        let claims = validate_token(token, &state.config.jwt_secret)
            .map_err(|_| ServerError::TokenExpired)?;

        Ok(AuthenticatedUser {
            user_id: claims.sub,
            device_id: claims.device_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_validate_token() {
        let user_id = "user123";
        let device_id = "device456";
        let secret = "test_secret_key_123";

        let token = create_token(user_id, device_id, secret, 24).unwrap();
        let claims = validate_token(&token, secret).unwrap();

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.device_id, device_id);
        assert!(claims.exp > Utc::now().timestamp());
    }

    #[test]
    fn test_invalid_secret() {
        let token = create_token("user", "device", "secret1", 24).unwrap();
        let result = validate_token(&token, "secret2");
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_token_rejected() {
        // Create a token with 0 hours expiration (already expired)
        let claims = Claims {
            sub: "user".to_string(),
            device_id: "device".to_string(),
            exp: (Utc::now() - Duration::hours(1)).timestamp(),
            iat: (Utc::now() - Duration::hours(2)).timestamp(),
        };
        let secret = "test_secret";
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();

        let result = validate_token(&token, secret);
        assert!(result.is_err(), "Expired token must be rejected at decode level");
    }

    #[test]
    fn test_empty_secret_tokens_differ() {
        // Tokens signed with empty secret should not validate with a real secret
        let token = create_token("user", "device", "", 24).unwrap();
        let result = validate_token(&token, "real_secret");
        assert!(result.is_err());
    }
}
