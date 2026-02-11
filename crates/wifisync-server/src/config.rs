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
    /// JWT token expiration in seconds.
    ///
    /// Loaded from `JWT_EXPIRATION_SECONDS` if set, otherwise from
    /// `JWT_EXPIRATION_HOURS` (converted to seconds). Default: 7 days.
    pub jwt_expiration_secs: u64,
    /// Bcrypt cost factor (4-31, default 12)
    pub bcrypt_cost: u32,
    /// Allowed CORS origins (empty = allow all)
    pub cors_allowed_origins: Vec<String>,
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
            jwt_secret: env::var("JWT_SECRET")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    // Generate a random secret if not provided or empty (development only)
                    use rand::Rng;
                    let secret: String = rand::thread_rng()
                        .sample_iter(&rand::distributions::Alphanumeric)
                        .take(64)
                        .map(char::from)
                        .collect();
                    tracing::warn!("JWT_SECRET not set or empty, using random secret (not suitable for production)");
                    secret
                }),
            jwt_expiration_secs: env::var("JWT_EXPIRATION_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .or_else(|| {
                    env::var("JWT_EXPIRATION_HOURS")
                        .ok()
                        .and_then(|h| h.parse::<u64>().ok())
                        .map(|h| h * 3600)
                })
                .unwrap_or(24 * 7 * 3600), // 1 week default
            bcrypt_cost: {
                let cost = env::var("BCRYPT_COST")
                    .ok()
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(bcrypt::DEFAULT_COST);
                if cost < 4 || cost > 31 {
                    tracing::warn!("BCRYPT_COST={cost} out of range (4-31), using default 12");
                    bcrypt::DEFAULT_COST
                } else {
                    cost
                }
            },
            cors_allowed_origins: env::var("CORS_ALLOWED_ORIGINS")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|s| s.split(',').map(|o| o.trim().to_string()).collect())
                .unwrap_or_default(), // empty = allow all (backward compat for mobile/CLI)
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::from_env()
    }
}
