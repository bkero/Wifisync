//! Wifisync Sync Server
//!
//! A REST API server for syncing WiFi credentials between devices.

mod config;
mod api;
mod db;
mod sync;
mod error;

use std::net::SocketAddr;

use sqlx::sqlite::SqlitePool;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::ServerConfig;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool
    pub db: SqlitePool,
    /// Server configuration
    pub config: ServerConfig,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wifisync_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = ServerConfig::from_env();

    tracing::info!("Starting Wifisync Sync Server");
    tracing::info!("Database: {}", config.database_url);

    // Initialize database
    let db = db::init_db(&config.database_url).await?;

    // Create application state
    let state = AppState {
        db,
        config: config.clone(),
    };

    // Build router
    let app = api::routes::create_router(state);

    // Start server
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
