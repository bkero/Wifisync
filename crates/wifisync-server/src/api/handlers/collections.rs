//! Collection management handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use wifisync_sync_protocol::{
    CollectionInfo, CollectionsResponse, CreateCollectionRequest, CreateCollectionResponse,
    VectorClock,
};

use crate::{
    api::auth::AuthenticatedUser,
    db::{models::DbCollection, queries},
    error::{ServerError, ServerResult},
    AppState,
};

/// List collections for the authenticated user
pub async fn list(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> ServerResult<Json<CollectionsResponse>> {
    tracing::info!("Listing collections for user {}", user.user_id);

    let db_collections = queries::find_collections_by_user(&state.db, &user.user_id).await?;

    let collections: Vec<CollectionInfo> = db_collections
        .into_iter()
        .filter_map(|c| {
            let vector_clock = VectorClock::from_json(&c.vector_clock).unwrap_or_default();
            let updated_at = chrono::DateTime::parse_from_rfc3339(&c.updated_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            Some(CollectionInfo {
                id: c.uuid(),
                encrypted_name: c.encrypted_name,
                vector_clock,
                updated_at,
            })
        })
        .collect();

    Ok(Json(CollectionsResponse { collections }))
}

/// Create a new collection (idempotent — re-creating with the same user updates the name)
pub async fn create(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<CreateCollectionRequest>,
) -> ServerResult<Json<CreateCollectionResponse>> {
    tracing::info!("Creating collection for user {}", user.user_id);

    let collection = DbCollection::new(req.id, user.user_id.clone(), req.encrypted_name.clone());

    // Check if a collection with this ID already exists
    if let Some(existing) = queries::find_collection_by_id(&state.db, &collection.id).await? {
        if existing.user_id == user.user_id {
            // Same user — idempotent re-creation: update the encrypted name
            tracing::info!("Collection {} already exists for same user, updating name", collection.id);
            queries::update_collection_name(&state.db, &collection.id, &req.encrypted_name).await?;
            return Ok(Json(CreateCollectionResponse {
                id: collection.uuid(),
            }));
        } else {
            // Different user owns this collection ID
            return Err(ServerError::conflict("Collection already exists"));
        }
    }

    queries::create_collection(&state.db, &collection).await?;

    tracing::info!("Collection created: {}", collection.id);

    Ok(Json(CreateCollectionResponse {
        id: collection.uuid(),
    }))
}

/// Delete a collection
pub async fn delete(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(collection_id): Path<String>,
) -> ServerResult<StatusCode> {
    tracing::info!("Deleting collection {}", collection_id);

    // Find collection and verify ownership
    let collection = queries::find_collection_by_id(&state.db, &collection_id)
        .await?
        .ok_or_else(|| ServerError::not_found("Collection"))?;

    if collection.user_id != user.user_id {
        return Err(ServerError::Validation {
            message: "Collection does not belong to user".to_string(),
        });
    }

    // Delete collection (cascade will delete sync records)
    queries::delete_collection(&state.db, &collection_id).await?;

    tracing::info!("Collection deleted: {}", collection_id);

    Ok(StatusCode::NO_CONTENT)
}
