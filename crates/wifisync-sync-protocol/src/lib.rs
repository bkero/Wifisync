//! Shared types for Wifisync sync protocol
//!
//! This crate contains the types used by both the sync client and server:
//! - Vector clocks for conflict detection
//! - Sync change records
//! - API request/response types

mod vector_clock;
mod sync_change;
mod api;
mod error;

pub use vector_clock::{VectorClock, ClockOrdering};
pub use sync_change::{SyncChange, SyncOperation, ChangePayload};
pub use api::*;
pub use error::{SyncError, SyncResult};
