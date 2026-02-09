//! Database query functions

use chrono::Utc;
use sqlx::SqlitePool;

use super::models::{DbCollection, DbConflict, DbDevice, DbSyncRecord, DbUser};
use crate::error::ServerResult;

// =============================================================================
// User Queries
// =============================================================================

/// Find a user by username
pub async fn find_user_by_username(
    pool: &SqlitePool,
    username: &str,
) -> ServerResult<Option<DbUser>> {
    let user = sqlx::query_as::<_, DbUser>("SELECT * FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

/// Find a user by ID
#[allow(dead_code)]
pub async fn find_user_by_id(pool: &SqlitePool, user_id: &str) -> ServerResult<Option<DbUser>> {
    let user = sqlx::query_as::<_, DbUser>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

/// Create a new user
pub async fn create_user(pool: &SqlitePool, user: &DbUser) -> ServerResult<()> {
    sqlx::query("INSERT INTO users (id, username, auth_key_hash, auth_salt, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&user.id)
        .bind(&user.username)
        .bind(&user.auth_key_hash)
        .bind(&user.auth_salt)
        .bind(&user.created_at)
        .execute(pool)
        .await?;
    Ok(())
}

/// Find the auth salt for a user by username
pub async fn find_salt_by_username(
    pool: &SqlitePool,
    username: &str,
) -> ServerResult<Option<String>> {
    let salt = sqlx::query_scalar::<_, String>("SELECT auth_salt FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?;
    Ok(salt)
}

// =============================================================================
// Device Queries
// =============================================================================

/// Find a device by ID
pub async fn find_device_by_id(
    pool: &SqlitePool,
    device_id: &str,
) -> ServerResult<Option<DbDevice>> {
    let device = sqlx::query_as::<_, DbDevice>("SELECT * FROM devices WHERE id = ?")
        .bind(device_id)
        .fetch_optional(pool)
        .await?;
    Ok(device)
}

/// Find devices for a user
#[allow(dead_code)]
pub async fn find_devices_by_user(
    pool: &SqlitePool,
    user_id: &str,
) -> ServerResult<Vec<DbDevice>> {
    let devices = sqlx::query_as::<_, DbDevice>("SELECT * FROM devices WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(devices)
}

/// Create a new device
pub async fn create_device(pool: &SqlitePool, device: &DbDevice) -> ServerResult<()> {
    sqlx::query(
        "INSERT INTO devices (id, user_id, name, device_token_hash, last_sync_at, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&device.id)
    .bind(&device.user_id)
    .bind(&device.name)
    .bind(&device.device_token_hash)
    .bind(&device.last_sync_at)
    .bind(&device.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update device last sync time
pub async fn update_device_last_sync(pool: &SqlitePool, device_id: &str) -> ServerResult<()> {
    sqlx::query("UPDATE devices SET last_sync_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(device_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete a device
pub async fn delete_device(pool: &SqlitePool, device_id: &str) -> ServerResult<()> {
    sqlx::query("DELETE FROM devices WHERE id = ?")
        .bind(device_id)
        .execute(pool)
        .await?;
    Ok(())
}

// =============================================================================
// Collection Queries
// =============================================================================

/// Find a collection by ID
pub async fn find_collection_by_id(
    pool: &SqlitePool,
    collection_id: &str,
) -> ServerResult<Option<DbCollection>> {
    let collection =
        sqlx::query_as::<_, DbCollection>("SELECT * FROM collections WHERE id = ?")
            .bind(collection_id)
            .fetch_optional(pool)
            .await?;
    Ok(collection)
}

/// Find collections for a user
pub async fn find_collections_by_user(
    pool: &SqlitePool,
    user_id: &str,
) -> ServerResult<Vec<DbCollection>> {
    let collections =
        sqlx::query_as::<_, DbCollection>("SELECT * FROM collections WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(collections)
}

/// Create a new collection
pub async fn create_collection(pool: &SqlitePool, collection: &DbCollection) -> ServerResult<()> {
    sqlx::query(
        "INSERT INTO collections (id, user_id, encrypted_name, vector_clock, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&collection.id)
    .bind(&collection.user_id)
    .bind(&collection.encrypted_name)
    .bind(&collection.vector_clock)
    .bind(&collection.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update collection vector clock
pub async fn update_collection_clock(
    pool: &SqlitePool,
    collection_id: &str,
    vector_clock: &str,
) -> ServerResult<()> {
    sqlx::query("UPDATE collections SET vector_clock = ?, updated_at = ? WHERE id = ?")
        .bind(vector_clock)
        .bind(Utc::now().to_rfc3339())
        .bind(collection_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete a collection
pub async fn delete_collection(pool: &SqlitePool, collection_id: &str) -> ServerResult<()> {
    sqlx::query("DELETE FROM collections WHERE id = ?")
        .bind(collection_id)
        .execute(pool)
        .await?;
    Ok(())
}

// =============================================================================
// Sync Record Queries
// =============================================================================

/// Find a sync record by collection and credential ID
pub async fn find_sync_record(
    pool: &SqlitePool,
    collection_id: &str,
    credential_id: &str,
) -> ServerResult<Option<DbSyncRecord>> {
    let record = sqlx::query_as::<_, DbSyncRecord>(
        "SELECT * FROM sync_records WHERE collection_id = ? AND credential_id = ?",
    )
    .bind(collection_id)
    .bind(credential_id)
    .fetch_optional(pool)
    .await?;
    Ok(record)
}

/// Find sync records for a collection
#[allow(dead_code)]
pub async fn find_sync_records_by_collection(
    pool: &SqlitePool,
    collection_id: &str,
) -> ServerResult<Vec<DbSyncRecord>> {
    let records =
        sqlx::query_as::<_, DbSyncRecord>("SELECT * FROM sync_records WHERE collection_id = ?")
            .bind(collection_id)
            .fetch_all(pool)
            .await?;
    Ok(records)
}

/// Find sync records updated after a given timestamp
pub async fn find_sync_records_since(
    pool: &SqlitePool,
    user_id: &str,
    since: &str,
    collection_id: Option<&str>,
) -> ServerResult<Vec<DbSyncRecord>> {
    let records = if let Some(coll_id) = collection_id {
        sqlx::query_as::<_, DbSyncRecord>(
            r"
            SELECT sr.* FROM sync_records sr
            JOIN collections c ON sr.collection_id = c.id
            WHERE c.user_id = ? AND sr.updated_at > ? AND sr.collection_id = ?
            ORDER BY sr.updated_at ASC
            ",
        )
        .bind(user_id)
        .bind(since)
        .bind(coll_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, DbSyncRecord>(
            r"
            SELECT sr.* FROM sync_records sr
            JOIN collections c ON sr.collection_id = c.id
            WHERE c.user_id = ? AND sr.updated_at > ?
            ORDER BY sr.updated_at ASC
            ",
        )
        .bind(user_id)
        .bind(since)
        .fetch_all(pool)
        .await?
    };
    Ok(records)
}

/// Find all sync records for a user
pub async fn find_all_sync_records_for_user(
    pool: &SqlitePool,
    user_id: &str,
    collection_id: Option<&str>,
) -> ServerResult<Vec<DbSyncRecord>> {
    let records = if let Some(coll_id) = collection_id {
        sqlx::query_as::<_, DbSyncRecord>(
            r"
            SELECT sr.* FROM sync_records sr
            JOIN collections c ON sr.collection_id = c.id
            WHERE c.user_id = ? AND sr.collection_id = ?
            ORDER BY sr.updated_at ASC
            ",
        )
        .bind(user_id)
        .bind(coll_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, DbSyncRecord>(
            r"
            SELECT sr.* FROM sync_records sr
            JOIN collections c ON sr.collection_id = c.id
            WHERE c.user_id = ?
            ORDER BY sr.updated_at ASC
            ",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?
    };
    Ok(records)
}

/// Upsert a sync record
pub async fn upsert_sync_record(
    pool: &SqlitePool,
    id: &str,
    collection_id: &str,
    credential_id: &str,
    vector_clock: &str,
    encrypted_payload: &[u8],
    tombstone: bool,
) -> ServerResult<()> {
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
    .bind(id)
    .bind(collection_id)
    .bind(credential_id)
    .bind(vector_clock)
    .bind(encrypted_payload)
    .bind(if tombstone { 1 } else { 0 })
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

// =============================================================================
// Conflict Queries
// =============================================================================

/// Find conflicts for a user
pub async fn find_conflicts_by_user(
    pool: &SqlitePool,
    user_id: &str,
) -> ServerResult<Vec<DbConflict>> {
    let conflicts = sqlx::query_as::<_, DbConflict>(
        "SELECT * FROM conflicts WHERE user_id = ? AND resolved = 0 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(conflicts)
}

/// Find a conflict by ID
pub async fn find_conflict_by_id(
    pool: &SqlitePool,
    conflict_id: &str,
) -> ServerResult<Option<DbConflict>> {
    let conflict = sqlx::query_as::<_, DbConflict>("SELECT * FROM conflicts WHERE id = ?")
        .bind(conflict_id)
        .fetch_optional(pool)
        .await?;
    Ok(conflict)
}

/// Create a new conflict
pub async fn create_conflict(pool: &SqlitePool, conflict: &DbConflict) -> ServerResult<()> {
    sqlx::query(
        r"
        INSERT INTO conflicts (id, user_id, collection_id, credential_id, local_payload, local_vector_clock, remote_payload, remote_vector_clock, created_at, resolved)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ",
    )
    .bind(&conflict.id)
    .bind(&conflict.user_id)
    .bind(&conflict.collection_id)
    .bind(&conflict.credential_id)
    .bind(&conflict.local_payload)
    .bind(&conflict.local_vector_clock)
    .bind(&conflict.remote_payload)
    .bind(&conflict.remote_vector_clock)
    .bind(&conflict.created_at)
    .bind(conflict.resolved)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark a conflict as resolved
pub async fn resolve_conflict(pool: &SqlitePool, conflict_id: &str) -> ServerResult<()> {
    sqlx::query("UPDATE conflicts SET resolved = 1 WHERE id = ?")
        .bind(conflict_id)
        .execute(pool)
        .await?;
    Ok(())
}
