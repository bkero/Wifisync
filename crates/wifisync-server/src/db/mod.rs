//! Database module

pub mod models;
pub mod queries;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

/// Initialize the database connection and run migrations
pub async fn init_db(database_url: &str) -> anyhow::Result<SqlitePool> {
    tracing::info!("Initializing database connection");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // Enable WAL mode for better concurrent read performance
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;
    tracing::info!("SQLite WAL mode enabled");

    // Run migrations
    run_migrations(&pool).await?;

    Ok(pool)
}

/// Run database migrations
async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    tracing::info!("Running database migrations");

    // Create tables if they don't exist
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            auth_key_hash TEXT NOT NULL,
            auth_salt TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        )
        ",
    )
    .execute(pool)
    .await?;

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
    .await?;

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS collections (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            encrypted_name BLOB NOT NULL,
            vector_clock TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        ",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS sync_records (
            id TEXT PRIMARY KEY,
            collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
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
    .await?;

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
    .await?;

    // Create indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_devices_user_id ON devices(user_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_collections_user_id ON collections(user_id)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_sync_records_collection_id ON sync_records(collection_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_conflicts_user_id ON conflicts(user_id)")
        .execute(pool)
        .await?;

    // Migration: add auth_salt column to existing users table
    let has_auth_salt: bool = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name='auth_salt'",
    )
    .fetch_one(pool)
    .await
    .map(|count| count > 0)?;

    if !has_auth_salt {
        sqlx::query("ALTER TABLE users ADD COLUMN auth_salt TEXT NOT NULL DEFAULT ''")
            .execute(pool)
            .await?;
        tracing::info!("Added auth_salt column to users table");
    }

    tracing::info!("Database migrations complete");
    Ok(())
}
