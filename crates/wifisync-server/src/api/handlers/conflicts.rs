//! Conflict resolution handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;
use wifisync_sync_protocol::{
    ChangePayload, ConflictResolution, ConflictsResponse, ResolveConflictRequest, SyncChange,
    SyncConflict, SyncOperation, VectorClock,
};

use crate::{
    api::auth::AuthenticatedUser,
    db::queries,
    error::{ServerError, ServerResult},
    sync::ConflictDetector,
    AppState,
};

/// List pending conflicts for the authenticated user
pub async fn list(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> ServerResult<Json<ConflictsResponse>> {
    tracing::info!("Listing conflicts for user {}", user.user_id);

    let db_conflicts = queries::find_conflicts_by_user(&state.db, &user.user_id).await?;

    let conflicts: Vec<SyncConflict> = db_conflicts
        .into_iter()
        .filter_map(|c| {
            let local_clock = c
                .local_vector_clock
                .as_ref()
                .and_then(|s| VectorClock::from_json(s).ok())
                .unwrap_or_default();
            let remote_clock = c
                .remote_vector_clock
                .as_ref()
                .and_then(|s| VectorClock::from_json(s).ok())
                .unwrap_or_default();

            let local_payload: ChangePayload = c
                .local_payload
                .as_ref()
                .and_then(|p| serde_json::from_slice(p).ok())
                .unwrap_or_else(ChangePayload::empty);
            let remote_payload: ChangePayload = c
                .remote_payload
                .as_ref()
                .and_then(|p| serde_json::from_slice(p).ok())
                .unwrap_or_else(ChangePayload::empty);

            let collection_id = Uuid::parse_str(&c.collection_id).ok()?;
            let credential_id = Uuid::parse_str(&c.credential_id).ok()?;

            let local_change = SyncChange {
                id: Uuid::new_v4(),
                collection_id,
                credential_id,
                operation: SyncOperation::Update,
                vector_clock: local_clock,
                payload: local_payload,
                device_id: "local".to_string(),
                timestamp: chrono::DateTime::parse_from_rfc3339(&c.created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
            };

            let remote_change = SyncChange {
                id: Uuid::new_v4(),
                collection_id,
                credential_id,
                operation: SyncOperation::Update,
                vector_clock: remote_clock,
                payload: remote_payload,
                device_id: "remote".to_string(),
                timestamp: chrono::DateTime::parse_from_rfc3339(&c.created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
            };

            Some(SyncConflict {
                id: c.uuid(),
                collection_id,
                credential_id,
                local_change,
                remote_change,
                created_at: chrono::DateTime::parse_from_rfc3339(&c.created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
            })
        })
        .collect();

    Ok(Json(ConflictsResponse { conflicts }))
}

/// Resolve a conflict
pub async fn resolve(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(conflict_id): Path<String>,
    Json(req): Json<ResolveConflictRequest>,
) -> ServerResult<StatusCode> {
    tracing::info!(
        "Resolving conflict {} with {:?}",
        conflict_id,
        req.resolution
    );

    // Find the conflict
    let conflict = queries::find_conflict_by_id(&state.db, &conflict_id)
        .await?
        .ok_or_else(|| ServerError::not_found("Conflict"))?;

    // Verify conflict belongs to user
    if conflict.user_id != user.user_id {
        return Err(ServerError::Validation {
            message: "Conflict does not belong to user".to_string(),
        });
    }

    // Get or create the merged vector clock
    let local_clock = conflict
        .local_vector_clock
        .as_ref()
        .and_then(|s| VectorClock::from_json(s).ok())
        .unwrap_or_default();
    let remote_clock = conflict
        .remote_vector_clock
        .as_ref()
        .and_then(|s| VectorClock::from_json(s).ok())
        .unwrap_or_default();

    let mut merged_clock = ConflictDetector::merge_clocks(&local_clock, &remote_clock);
    // Increment for this resolution
    merged_clock.increment(&user.device_id);

    match req.resolution {
        ConflictResolution::KeepLocal => {
            // Use local payload
            if let Some(local_payload) = &conflict.local_payload {
                let clock_json = merged_clock.to_json().map_err(|e| {
                    ServerError::internal(format!("Failed to serialize clock: {}", e))
                })?;

                queries::upsert_sync_record(
                    &state.db,
                    &Uuid::new_v4().to_string(),
                    &conflict.collection_id,
                    &conflict.credential_id,
                    &clock_json,
                    local_payload,
                    false,
                )
                .await?;
            }
        }
        ConflictResolution::KeepRemote => {
            // Use remote payload
            if let Some(remote_payload) = &conflict.remote_payload {
                let clock_json = merged_clock.to_json().map_err(|e| {
                    ServerError::internal(format!("Failed to serialize clock: {}", e))
                })?;

                queries::upsert_sync_record(
                    &state.db,
                    &Uuid::new_v4().to_string(),
                    &conflict.collection_id,
                    &conflict.credential_id,
                    &clock_json,
                    remote_payload,
                    false,
                )
                .await?;
            }
        }
        ConflictResolution::KeepBoth => {
            // Create two records with different credential IDs
            // The local one keeps original ID, remote gets a new ID
            let clock_json = merged_clock.to_json().map_err(|e| {
                ServerError::internal(format!("Failed to serialize clock: {}", e))
            })?;

            if let Some(local_payload) = &conflict.local_payload {
                queries::upsert_sync_record(
                    &state.db,
                    &Uuid::new_v4().to_string(),
                    &conflict.collection_id,
                    &conflict.credential_id,
                    &clock_json,
                    local_payload,
                    false,
                )
                .await?;
            }

            if let Some(remote_payload) = &conflict.remote_payload {
                // Create with a new credential ID
                let new_credential_id = Uuid::new_v4();
                queries::upsert_sync_record(
                    &state.db,
                    &Uuid::new_v4().to_string(),
                    &conflict.collection_id,
                    &new_credential_id.to_string(),
                    &clock_json,
                    remote_payload,
                    false,
                )
                .await?;
            }
        }
    }

    // If a merged payload was provided (for custom merge), use it instead
    if let Some(merged_payload) = req.merged_payload {
        let clock_json = merged_clock.to_json().map_err(|e| {
            ServerError::internal(format!("Failed to serialize clock: {}", e))
        })?;
        let payload_bytes = serde_json::to_vec(&merged_payload).map_err(|e| {
            ServerError::internal(format!("Failed to serialize payload: {}", e))
        })?;

        queries::upsert_sync_record(
            &state.db,
            &Uuid::new_v4().to_string(),
            &conflict.collection_id,
            &conflict.credential_id,
            &clock_json,
            &payload_bytes,
            false,
        )
        .await?;
    }

    // Mark conflict as resolved
    queries::resolve_conflict(&state.db, &conflict_id).await?;

    tracing::info!("Conflict {} resolved", conflict_id);

    Ok(StatusCode::NO_CONTENT)
}
