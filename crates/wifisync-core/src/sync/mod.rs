//! Sync functionality for Wifisync
//!
//! This module provides client-side sync capabilities:
//! - HTTP client for communicating with sync server
//! - Local sync state management
//! - End-to-end encryption helpers

mod client;
mod encryption;
mod state;

pub use client::SyncClient;
pub use encryption::{generate_salt, SyncEncryption};
pub use state::{ChangeType, PendingChange, SyncConfig, SyncState, SyncStateManager};

// Re-export common types from sync protocol
pub use wifisync_sync_protocol::{
    ApiError, ConflictResolution, LoginRequest, LoginResponse, PullRequest, PullResponse,
    PushChangeResult, PushRequest, PushResponse, SyncChange, SyncConflict, SyncOperation,
    SyncStatus, VectorClock,
};
