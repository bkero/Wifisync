//! Server configuration

use std::env;

/// Server configuration options
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,
    /// Port to listen on
    pub port: u16,
    /// SQLite database URL
    pub database_url: String,
    /// JWT secret key
    pub jwt_secret: String,
    /// JWT token expiration in hours
    pub jwt_expiration_hours: u64,
}

impl ServerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            host: env::var("WIFISYNC_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("WIFISYNC_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:wifisync.db?mode=rwc".to_string()),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| {
                // Generate a random secret if not provided (development only)
                use rand::Rng;
                let secret: String = rand::thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(64)
                    .map(char::from)
                    .collect();
                tracing::warn!("JWT_SECRET not set, using random secret (not suitable for production)");
                secret
            }),
            jwt_expiration_hours: env::var("JWT_EXPIRATION_HOURS")
                .ok()
                .and_then(|h| h.parse().ok())
                .unwrap_or(24 * 7), // 1 week default
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::from_env()
    }
}
