//! Wifisync Core Library
//!
//! This crate provides the core functionality for Wifisync, including:
//! - Data models for wifi credentials and collections
//! - Platform adapters for different network managers
//! - Profile management (profiles WITHOUT passwords in system)
//! - Secret Agent support for on-demand password delivery
//! - Encrypted storage and sharing

pub mod adapter;
#[cfg(feature = "networkmanager")]
pub mod agent;
pub mod crypto;
pub mod error;
pub mod filter;
pub mod management;
pub mod models;
pub mod storage;

pub use error::{Error, Result};
pub use models::{
    CredentialCollection, NetworkProfile, SecurityType, SourcePlatform, WifiCredential,
};
pub use management::ProfileManager;

#[cfg(feature = "networkmanager")]
pub use adapter::networkmanager::NetworkManagerAdapter;
#[cfg(feature = "networkmanager")]
pub use agent::{AgentService, AgentStatus};

#[cfg(feature = "android")]
pub use adapter::{
    AndroidAdapter, AndroidCapabilities, AndroidJniCallback, SuggestionInfo, SuggestionRequest,
};
