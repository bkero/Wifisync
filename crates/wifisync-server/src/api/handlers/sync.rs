//! Sync operation handlers

use axum::{extract::State, Json};
use uuid::Uuid;
use wifisync_sync_protocol::{
    ChangePayload, PullRequest, PullResponse, PushChangeResult, PushRequest, PushResponse,
    SyncChange, SyncOperation, VectorClock,
};

use crate::{
    api::auth::AuthenticatedUser,
    db::{models::DbConflict, queries},
    error::{ServerError, ServerResult},
    sync::{ChangeCheckResult, ConflictDetector},
    AppState,
};

/// Push changes to the server
pub async fn push(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<PushRequest>,
) -> ServerResult<Json<PushResponse>> {
    tracing::info!(
        "Push request from device {} with {} changes",
        req.device_id,
        req.changes.len()
    );

    // Verify device belongs to user
    if req.device_id != user.device_id {
        return Err(ServerError::Validation {
            message: "Device ID mismatch".to_string(),
        });
    }

    let mut results = Vec::with_capacity(req.changes.len());
    let mut accepted_count = 0;
    let mut conflict_count = 0;

    for change in req.changes {
        // Verify collection belongs to user
        let collection = queries::find_collection_by_id(&state.db, &change.collection_id.to_string())
            .await?
            .ok_or_else(|| ServerError::not_found("Collection"))?;

        if collection.user_id != user.user_id {
            return Err(ServerError::Validation {
                message: "Collection does not belong to user".to_string(),
            });
        }

        // Check for existing record
        let existing = queries::find_sync_record(
            &state.db,
            &change.collection_id.to_string(),
            &change.credential_id.to_string(),
        )
        .await?;

        let existing_clock = existing.as_ref().and_then(|r| {
            VectorClock::from_json(&r.vector_clock).ok()
        });

        // Check if change can be applied
        match ConflictDetector::check_change(&change.vector_clock, existing_clock.as_ref()) {
            ChangeCheckResult::Accept => {
                // Apply the change
                let clock_json = change.vector_clock.to_json().map_err(|e| {
                    ServerError::internal(format!("Failed to serialize vector clock: {}", e))
                })?;

                let payload_bytes = serde_json::to_vec(&change.payload).map_err(|e| {
                    ServerError::internal(format!("Failed to serialize payload: {}", e))
                })?;

                queries::upsert_sync_record(
                    &state.db,
                    &change.id.to_string(),
                    &change.collection_id.to_string(),
                    &change.credential_id.to_string(),
                    &clock_json,
                    &payload_bytes,
                    change.is_tombstone(),
                )
                .await?;

                // Update collection clock
                let mut collection_clock = VectorClock::from_json(&collection.vector_clock)
                    .unwrap_or_default();
                collection_clock.merge(&change.vector_clock);
                let collection_clock_json = collection_clock.to_json().map_err(|e| {
                    ServerError::internal(format!("Failed to serialize collection clock: {}", e))
                })?;
                queries::update_collection_clock(
                    &state.db,
                    &change.collection_id.to_string(),
                    &collection_clock_json,
                )
                .await?;

                results.push(PushChangeResult::Accepted {
                    change_id: change.id,
                });
                accepted_count += 1;

                tracing::debug!("Accepted change {} for credential {}", change.id, change.credential_id);
            }
            ChangeCheckResult::Outdated => {
                // Change is outdated, treat as accepted (idempotent)
                tracing::debug!("Outdated change {} ignored", change.id);
                results.push(PushChangeResult::Accepted {
                    change_id: change.id,
                });
                accepted_count += 1;
            }
            ChangeCheckResult::Conflict => {
                // Create conflict record
                let existing_record = existing.unwrap();
                let conflict = DbConflict::new(
                    user.user_id.clone(),
                    change.collection_id.to_string(),
                    change.credential_id.to_string(),
                    Some(serde_json::to_vec(&change.payload).unwrap_or_default()),
                    change.vector_clock.to_json().ok(),
                    Some(existing_record.encrypted_payload.clone()),
                    Some(existing_record.vector_clock.clone()),
                );

                queries::create_conflict(&state.db, &conflict).await?;

                results.push(PushChangeResult::Conflict {
                    change_id: change.id,
                    conflict_id: conflict.uuid(),
                });
                conflict_count += 1;

                tracing::warn!(
                    "Conflict detected for credential {} (conflict_id: {})",
                    change.credential_id,
                    conflict.id
                );
            }
        }
    }

    // Update device last sync time
    queries::update_device_last_sync(&state.db, &user.device_id).await?;

    Ok(Json(PushResponse {
        results,
        accepted_count,
        conflict_count,
    }))
}

/// Pull changes from the server
pub async fn pull(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<PullRequest>,
) -> ServerResult<Json<PullResponse>> {
    tracing::info!("Pull request from device {}", req.device_id);

    // Verify device belongs to user
    if req.device_id != user.device_id {
        return Err(ServerError::Validation {
            message: "Device ID mismatch".to_string(),
        });
    }

    // Get changes based on whether we have a "since" clock
    let records = if let Some(_since_clock) = &req.since {
        // For now, we use a simple timestamp-based approach
        // In a full implementation, you'd compare vector clocks more carefully
        let since_time = "1970-01-01T00:00:00Z"; // Placeholder - should extract from clock
        queries::find_sync_records_since(
            &state.db,
            &user.user_id,
            since_time,
            req.collection_id.as_ref().map(|id| id.to_string()).as_deref(),
        )
        .await?
    } else {
        // Pull all records
        queries::find_all_sync_records_for_user(
            &state.db,
            &user.user_id,
            req.collection_id.as_ref().map(|id| id.to_string()).as_deref(),
        )
        .await?
    };

    // Convert records to SyncChange
    let mut changes = Vec::with_capacity(records.len());
    let mut server_clock = VectorClock::new();

    for record in records {
        let vector_clock = VectorClock::from_json(&record.vector_clock).unwrap_or_default();
        server_clock.merge(&vector_clock);

        let payload: ChangePayload = serde_json::from_slice(&record.encrypted_payload)
            .unwrap_or_else(|_| ChangePayload::new(record.encrypted_payload.clone(), Vec::new()));

        let operation = if record.is_tombstone() {
            SyncOperation::Delete
        } else {
            SyncOperation::Update
        };

        let change = SyncChange {
            id: Uuid::parse_str(&record.id).unwrap_or_else(|_| Uuid::new_v4()),
            collection_id: Uuid::parse_str(&record.collection_id).unwrap_or_else(|_| Uuid::new_v4()),
            credential_id: Uuid::parse_str(&record.credential_id).unwrap_or_else(|_| Uuid::new_v4()),
            operation,
            vector_clock,
            payload,
            device_id: "server".to_string(), // Server is source for pulls
            timestamp: chrono::DateTime::parse_from_rfc3339(&record.updated_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
        };

        changes.push(change);
    }

    // Update device last sync time
    queries::update_device_last_sync(&state.db, &user.device_id).await?;

    Ok(Json(PullResponse {
        changes,
        server_clock,
        has_more: false, // Pagination not implemented yet
    }))
}
