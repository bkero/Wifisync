//! NetworkManager Secret Agent daemon
//!
//! Implements the `org.freedesktop.NetworkManager.SecretAgent` D-Bus interface
//! to provide wifi passwords on-demand when NetworkManager requests them.
//!
//! # Architecture
//!
//! The agent connects to the system D-Bus, exports the SecretAgent interface at
//! `/org/freedesktop/NetworkManager/SecretAgent`, and registers with
//! NetworkManager's AgentManager. When NM needs a password for a connection
//! with `psk-flags=1` (agent-owned), it calls `GetSecrets()` on the agent.
//!
//! The agent looks up the password from Wifisync's encrypted local storage
//! and returns it to NetworkManager.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::Connection;

use crate::error::Error;
use crate::storage::Storage;
use crate::Result;

/// Agent identifier for NetworkManager registration
const AGENT_IDENTIFIER: &str = "com.wifisync.agent";

/// D-Bus object path where NM expects the SecretAgent interface
const AGENT_OBJECT_PATH: &str = "/org/freedesktop/NetworkManager/SecretAgent";

/// PID file name for tracking daemon state
const PID_FILE: &str = "agent.pid";

/// Type alias for NetworkManager connection settings (a{sa{sv}})
type NMSettings = HashMap<String, HashMap<String, OwnedValue>>;

/// Wifisync Secret Agent for NetworkManager
///
/// Provides wifi passwords on-demand when NetworkManager requests them
/// via the D-Bus Secret Agent API. Passwords are looked up from
/// Wifisync's encrypted local storage.
struct WifisyncSecretAgent {
    storage: Arc<Storage>,
}

impl WifisyncSecretAgent {
    fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// Extract the connection UUID from NM settings
    fn extract_uuid(settings: &NMSettings) -> Option<String> {
        let connection = settings.get("connection")?;
        let uuid_value = connection.get("uuid")?;
        uuid_value
            .downcast_ref::<zbus::zvariant::Str>()
            .ok()
            .map(|s| s.to_string())
    }

    /// Extract the SSID from NM settings (for logging)
    fn extract_ssid(settings: &NMSettings) -> Option<String> {
        let wireless = settings.get("802-11-wireless")?;
        let ssid_value = wireless.get("ssid")?;
        let ssid_bytes: Vec<u8> = ssid_value
            .downcast_ref::<zbus::zvariant::Array>()
            .ok()
            .and_then(|arr| {
                let bytes: Vec<u8> = arr
                    .iter()
                    .filter_map(|v| v.downcast_ref::<u8>().ok())
                    .collect();
                if bytes.is_empty() { None } else { Some(bytes) }
            })?;
        String::from_utf8(ssid_bytes).ok()
    }

    /// Look up a credential password by the connection's UUID or SSID
    fn lookup_password(&self, settings: &NMSettings) -> Option<String> {
        let system_id = Self::extract_uuid(settings)?;
        let ssid = Self::extract_ssid(settings).unwrap_or_else(|| "unknown".to_string());

        tracing::debug!(
            system_id = %system_id,
            ssid = %ssid,
            "Looking up credential for Secret Agent request"
        );

        // First, try to look up by profile (system_id -> credential_id mapping)
        if let Ok(Some(profile)) = self.storage.find_profile_by_system_id(&system_id) {
            if let Ok(Some(cred)) = self.storage.find_credential(profile.credential_id) {
                use secrecy::ExposeSecret;
                tracing::info!(
                    ssid = %ssid,
                    system_id = %system_id,
                    "Found credential via profile mapping, providing password to NetworkManager"
                );
                return Some(cred.password.expose_secret().to_string());
            } else {
                tracing::warn!(
                    system_id = %system_id,
                    credential_id = %profile.credential_id,
                    "Profile found but credential not in any collection"
                );
            }
        }

        // Fallback: look up by SSID directly in collections
        // This handles cases where:
        // - The network was added to a collection but not "installed"
        // - The NM connection was deleted and recreated with a new UUID
        tracing::debug!(
            ssid = %ssid,
            "No profile mapping found, trying SSID lookup in collections"
        );

        match self.storage.find_credential_by_ssid(&ssid) {
            Ok(Some(cred)) => {
                use secrecy::ExposeSecret;
                tracing::info!(
                    ssid = %ssid,
                    system_id = %system_id,
                    "Found credential by SSID, providing password to NetworkManager"
                );
                Some(cred.password.expose_secret().to_string())
            }
            Ok(None) => {
                tracing::debug!(
                    ssid = %ssid,
                    "No credential found for this SSID in any collection"
                );
                None
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to look up credential by SSID");
                None
            }
        }
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.SecretAgent")]
impl WifisyncSecretAgent {
    /// Handle GetSecrets request from NetworkManager
    ///
    /// Called when NM needs passwords for a connection with agent-owned secrets.
    async fn get_secrets(
        &self,
        connection: NMSettings,
        connection_path: ObjectPath<'_>,
        setting_name: &str,
        hints: Vec<String>,
        flags: u32,
    ) -> zbus::fdo::Result<NMSettings> {
        let ssid = Self::extract_ssid(&connection).unwrap_or_else(|| "unknown".to_string());

        tracing::info!(
            path = %connection_path,
            setting = %setting_name,
            ssid = %ssid,
            flags = flags,
            hints = ?hints,
            "GetSecrets called by NetworkManager"
        );

        // Only handle wireless security secrets
        if setting_name != "802-11-wireless-security" {
            tracing::debug!(setting = %setting_name, "Not a wireless security request, declining");
            return Err(zbus::fdo::Error::Failed(
                format!("No secrets for setting: {setting_name}"),
            ));
        }

        // Look up the password from our storage
        let password = self.lookup_password(&connection).ok_or_else(|| {
            tracing::info!(ssid = %ssid, "No secrets available, NM will prompt user");
            zbus::fdo::Error::Failed("No secrets available for this connection".into())
        })?;

        // Build the secrets response: {"802-11-wireless-security": {"psk": "password"}}
        let mut secrets = NMSettings::new();
        let mut wireless_sec = HashMap::new();
        wireless_sec.insert(
            "psk".to_string(),
            Value::from(password.as_str()).try_to_owned().unwrap(),
        );
        secrets.insert("802-11-wireless-security".to_string(), wireless_sec);

        tracing::info!(
            ssid = %ssid,
            path = %connection_path,
            "Successfully provided secrets to NetworkManager"
        );

        Ok(secrets)
    }

    /// Handle CancelGetSecrets from NetworkManager
    ///
    /// Called when a connection attempt is cancelled while GetSecrets was pending.
    async fn cancel_get_secrets(
        &self,
        connection_path: ObjectPath<'_>,
        setting_name: &str,
    ) -> zbus::fdo::Result<()> {
        tracing::debug!(
            path = %connection_path,
            setting = %setting_name,
            "CancelGetSecrets called"
        );
        // Nothing to cancel - our lookups are synchronous and fast
        Ok(())
    }

    /// Handle SaveSecrets from NetworkManager
    ///
    /// Called after a successful connection. We ignore this since Wifisync
    /// is the source of truth for credentials.
    async fn save_secrets(
        &self,
        _connection: NMSettings,
        connection_path: ObjectPath<'_>,
    ) -> zbus::fdo::Result<()> {
        tracing::debug!(
            path = %connection_path,
            "SaveSecrets called (ignored - Wifisync is source of truth)"
        );
        Ok(())
    }

    /// Handle DeleteSecrets from NetworkManager
    ///
    /// Called when a connection profile is deleted from NM. We remove the
    /// profile mapping but keep the credential itself (it may be used elsewhere).
    async fn delete_secrets(
        &self,
        connection: NMSettings,
        connection_path: ObjectPath<'_>,
    ) -> zbus::fdo::Result<()> {
        tracing::debug!(
            path = %connection_path,
            "DeleteSecrets called"
        );

        if let Some(system_id) = Self::extract_uuid(&connection) {
            match self.storage.find_profile_by_system_id(&system_id) {
                Ok(Some(profile)) => {
                    if let Err(e) = self.storage.remove_profile(profile.credential_id) {
                        tracing::warn!(
                            error = %e,
                            system_id = %system_id,
                            "Failed to remove profile mapping"
                        );
                    } else {
                        tracing::info!(
                            system_id = %system_id,
                            credential_id = %profile.credential_id,
                            "Removed profile mapping (credential preserved)"
                        );
                    }
                }
                Ok(None) => {
                    tracing::debug!(
                        system_id = %system_id,
                        "No profile mapping found for deleted connection"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to look up profile for deletion");
                }
            }
        }

        Ok(())
    }
}

/// Secret Agent daemon service
///
/// Manages the lifecycle of the Secret Agent D-Bus daemon.
pub struct AgentService;

impl AgentService {
    /// Run the Secret Agent daemon
    ///
    /// Connects to the system D-Bus, registers as a NetworkManager Secret Agent,
    /// and blocks until a shutdown signal is received (SIGINT or SIGTERM).
    ///
    /// This is designed to be run by systemd as a user service.
    pub async fn run(storage: Storage) -> Result<()> {
        let data_dir = storage.data_dir().to_path_buf();
        let storage = Arc::new(storage);
        let agent = WifisyncSecretAgent::new(Arc::clone(&storage));

        // Connect to system D-Bus
        let connection = Connection::system().await.map_err(|e| {
            Error::service_unavailable(format!("Failed to connect to system D-Bus: {e}"))
        })?;

        // Export the SecretAgent interface at the well-known path
        connection
            .object_server()
            .at(AGENT_OBJECT_PATH, agent)
            .await
            .map_err(|e| Error::internal(format!("Failed to export agent interface: {e}")))?;

        // Register with NetworkManager AgentManager
        let agent_mgr = AgentManagerProxy::new(&connection).await.map_err(|e| {
            Error::service_unavailable(format!(
                "Failed to create AgentManager proxy: {e}. Is NetworkManager running?"
            ))
        })?;

        agent_mgr.register(AGENT_IDENTIFIER).await.map_err(|e| {
            Error::internal(format!(
                "Failed to register with AgentManager: {e}. \
                 Is another Wifisync agent already running?"
            ))
        })?;

        // Write PID file
        write_pid_file(&data_dir)?;

        tracing::info!(
            identifier = AGENT_IDENTIFIER,
            pid = std::process::id(),
            "Wifisync Secret Agent registered with NetworkManager"
        );

        // Wait for shutdown signal (SIGINT or SIGTERM)
        wait_for_shutdown().await;

        // Clean shutdown
        tracing::info!("Shutting down Secret Agent...");

        if let Err(e) = agent_mgr.unregister().await {
            tracing::warn!(error = %e, "Failed to unregister agent (NM may have disconnected)");
        }

        remove_pid_file(&data_dir);
        tracing::info!("Secret Agent stopped");

        Ok(())
    }

    /// Check the status of the Secret Agent daemon
    ///
    /// Returns Some((pid, running)) if a PID file exists, None otherwise.
    pub fn status(data_dir: &Path) -> Option<AgentStatus> {
        let pid = read_pid_file(data_dir)?;
        let running = is_process_running(pid);
        Some(AgentStatus { pid, running })
    }
}

/// Status of the Secret Agent daemon
pub struct AgentStatus {
    /// Process ID from PID file
    pub pid: u32,
    /// Whether the process is actually running
    pub running: bool,
}

// --- PID file management ---

fn write_pid_file(data_dir: &Path) -> Result<()> {
    let pid = std::process::id();
    let path = data_dir.join(PID_FILE);
    std::fs::write(&path, pid.to_string())?;
    Ok(())
}

fn remove_pid_file(data_dir: &Path) {
    let path = data_dir.join(PID_FILE);
    let _ = std::fs::remove_file(&path);
}

fn read_pid_file(data_dir: &Path) -> Option<u32> {
    let path = data_dir.join(PID_FILE);
    std::fs::read_to_string(&path)
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn is_process_running(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

// --- Signal handling ---

async fn wait_for_shutdown() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM");
        }
    }
}

// --- D-Bus proxy for NetworkManager AgentManager ---

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.AgentManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/AgentManager"
)]
trait AgentManager {
    fn register(&self, identifier: &str) -> zbus::Result<()>;

    fn register_with_capabilities(
        &self,
        identifier: &str,
        capabilities: u32,
    ) -> zbus::Result<()>;

    fn unregister(&self) -> zbus::Result<()>;
}
