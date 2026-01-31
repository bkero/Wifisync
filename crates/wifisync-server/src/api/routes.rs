//! API route definitions

use axum::{
    routing::{delete, get, post},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use super::handlers;
use crate::AppState;

/// Create the API router with all routes
pub fn create_router(state: AppState) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // Authentication endpoints
        .route("/api/v1/users/register", post(handlers::auth::register))
        .route("/api/v1/auth/login", post(handlers::auth::login))
        .route("/api/v1/auth/refresh", post(handlers::auth::refresh))
        .route("/api/v1/auth/logout", delete(handlers::auth::logout))
        // Sync endpoints
        .route("/api/v1/sync/push", post(handlers::sync::push))
        .route("/api/v1/sync/pull", post(handlers::sync::pull))
        // Conflict endpoints
        .route("/api/v1/sync/conflicts", get(handlers::conflicts::list))
        .route(
            "/api/v1/sync/conflicts/:id/resolve",
            post(handlers::conflicts::resolve),
        )
        // Collection endpoints
        .route("/api/v1/collections", get(handlers::collections::list))
        .route("/api/v1/collections", post(handlers::collections::create))
        .route(
            "/api/v1/collections/:id",
            delete(handlers::collections::delete),
        )
        // Add middleware
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        // Add state
        .with_state(state)
}
