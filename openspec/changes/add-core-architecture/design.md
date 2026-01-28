# Core Architecture Design

## Context

Wifisync is a new application with no existing codebase. We need to establish a foundation that:
- Supports multiple platforms (Linux, Android, Windows, iOS)
- Keeps platform-specific code isolated
- Enables secure credential handling
- Allows for future sharing mechanisms

## Goals / Non-Goals

### Goals
- Define clean abstractions between core logic and platform code
- Establish data models that work across all platforms
- Choose encryption approach for credentials at rest
- Create extensible filtering system
- Design portable sharing format

### Non-Goals
- Implement all platforms in this phase (only Linux/NetworkManager)
- Build sharing server infrastructure (file-based sharing first)
- Support legacy network managers (wpa_supplicant direct, etc.)

## Decisions

### Decision: Rust as Primary Language
**Choice**: Rust (stable toolchain)
**Rationale**:
- Memory safety without garbage collection
- Excellent cross-platform support via cargo
- Strong D-Bus support (zbus - pure Rust, async)
- Single binary distribution - no runtime dependencies
- Can compile to Android (NDK) and iOS
- Great CLI tooling ecosystem (clap, indicatif)

**Alternatives considered**:
- Python: Easier to write, but runtime dependency, slower, packaging complexity
- Go: Good CLI story, poor D-Bus libraries, no mobile story
- TypeScript/Electron: Heavy runtime, overkill for this use case

### Decision: Adapter Pattern for Platform Abstraction
**Choice**: Trait with platform-specific implementations
**Rationale**:
- Clean separation of concerns
- Easy to add new platforms
- Testable with mock adapters
- Compile-time or runtime adapter selection via feature flags or trait objects

```rust
#[async_trait]
pub trait NetworkAdapter: Send + Sync {
    /// List all known wifi networks (for extraction)
    async fn list_networks(&self) -> Result<Vec<NetworkInfo>>;

    /// Get credentials for a specific network (extraction with secrets)
    async fn get_credentials(&self, ssid: &str) -> Result<WifiCredential>;

    /// Create a network profile WITHOUT password (for Secret Agent pattern)
    /// Profile will have psk-flags=1 so NM queries our Secret Agent for password
    async fn create_profile(&self, credential: &WifiCredential) -> Result<String>;

    /// Delete a network profile from the system
    async fn delete_profile(&self, system_id: &str) -> Result<()>;

    /// Get platform information
    fn platform_info(&self) -> PlatformInfo;

    /// Get the source platform identifier
    fn source_platform(&self) -> SourcePlatform;
}
```

**Note**: The `create_profile` method creates connections with `psk-flags=1` (agent-owned),
meaning the password is NOT stored in NetworkManager. Instead, NM will query our
Secret Agent daemon for the password when the user connects.

### Decision: ChaCha20-Poly1305 for Local Storage
**Choice**: `chacha20poly1305` crate (RustCrypto project)
**Rationale**:
- Modern AEAD cipher (Authenticated Encryption with Associated Data)
- Faster than AES on platforms without hardware AES support
- Simple API via RustCrypto's consistent interface
- Key derived from user password via Argon2 (`argon2` crate)
- Pure Rust, no C dependencies

**Alternatives considered**:
- AES-GCM: Good, but ChaCha20 is faster in software
- age: Great format, but heavier dependency for our use case
- sodiumoxide: Good, but links to C library

### Decision: JSON for Portable Format
**Choice**: JSON via `serde` + `serde_json` with optional encryption layer
**Rationale**:
- Human readable for debugging
- Universal parser support
- Schema versioning is straightforward
- serde's derive macros make serialization trivial
- Can wrap with ChaCha20-Poly1305 for encrypted sharing

**Format structure**:
```json
{
  "version": "1.0",
  "created_at": "2024-01-20T10:00:00Z",
  "created_by": "wifisync/0.1.0",
  "collection": {
    "name": "Coffee Shops",
    "credentials": [...]
  }
}
```

### Decision: Secret Agent Pattern (Linux)
**Choice**: Wifisync acts as a NetworkManager Secret Agent; passwords never stored in NM
**Rationale**:
- Single source of truth: passwords only in Wifisync's encrypted database
- Better security: NetworkManager system files never contain PSKs
- Standard mechanism: D-Bus Secret Agent API is the official way to provide secrets
- Clean separation: profiles in NM contain network config, Wifisync provides authentication

**How It Works**:
1. Wifisync creates network profiles in NM with `psk-flags=1` (agent-owned secret)
2. Wifisync runs a daemon that registers as a Secret Agent on D-Bus
3. When user connects to a network, NM calls `GetSecrets()` on the agent
4. Agent looks up password in Wifisync's encrypted database
5. Agent returns password; NM uses it for WPA handshake
6. Password is never persisted in NM's connection files

**Implementation**:
```rust
/// Profile without password - created in NetworkManager
pub struct NetworkProfile {
    pub credential_id: Uuid,      // Reference to WifiCredential in Wifisync DB
    pub system_id: String,        // NetworkManager connection UUID
    pub platform: SourcePlatform,
    pub created_at: DateTime<Utc>,
}

/// D-Bus Secret Agent trait
#[async_trait]
pub trait SecretAgent: Send + Sync {
    /// Called by NetworkManager when secrets are needed
    async fn get_secrets(
        &self,
        connection: &ConnectionSettings,
        connection_path: &str,
        setting_name: &str,
        hints: &[String],
        flags: u32,
    ) -> Result<HashMap<String, HashMap<String, OwnedValue>>>;

    /// Called when connection activation is cancelled
    async fn cancel_get_secrets(&self, connection_path: &str, setting_name: &str) -> Result<()>;

    /// Called when secrets should be saved (we ignore - we're the source of truth)
    async fn save_secrets(&self, connection: &ConnectionSettings, connection_path: &str) -> Result<()>;

    /// Called when secrets should be deleted
    async fn delete_secrets(&self, connection: &ConnectionSettings, connection_path: &str) -> Result<()>;
}
```

**Tracking storage**: Network profile mappings stored in `~/.local/share/wifisync/profiles.json`

**Alternatives considered**:
- Store passwords in NM: Simpler, but passwords scattered across system files
- Connection dispatcher scripts: Hacky, race conditions, poor error handling
- wpa_supplicant direct: Non-standard, only works on some distros

### Decision: Filter Chain Pattern
**Choice**: Composable filter pipeline with AND semantics
**Rationale**:
- Filters are independent and reorderable
- Easy to add new filter types
- Each filter can log its exclusions
- Statistics per filter for transparency

```rust
pub trait CredentialFilter: Send + Sync {
    fn filter(&self, cred: &WifiCredential) -> FilterResult;
    fn name(&self) -> &str;
}

let pipeline = FilterPipeline::new()
    .add(EnterpriseFilter)       // Exclude 802.1X
    .add(OpenNetworkFilter)      // Exclude no-password
    .add(ExclusionListFilter::new(&config.exclusions))
    .add(TagFilter::new(&["work"]));

let (filtered, stats) = pipeline.apply(&credentials);
```

## Risks / Trade-offs

### Risk: D-Bus Permission Issues on Linux
**Mitigation**:
- Clear error messages with remediation steps
- Document polkit rules for reading secrets
- Consider Flatpak portal for sandboxed access

### Risk: Credential Security in Memory
**Mitigation**:
- Use `secrecy` crate for password fields (zeroizes on drop)
- Credentials cleared from memory when no longer needed
- Warn users to lock screen when Wifisync is open

### Risk: Secret Agent Daemon Must Be Running
**Mitigation**:
- Systemd user service for automatic startup
- Clear indication in `wifisync status` when daemon is not running
- Graceful degradation: networks still visible but connection fails with helpful error
- Optional fallback: allow storing password in NM for pre-login networks (headless systems)

### Risk: No Pre-Login WiFi Access
**Description**: If Wifisync daemon isn't running (before user login), networks relying on
Secret Agent can't connect. This affects headless systems that need WiFi to boot.
**Mitigation**:
- Offer optional "pre-login mode" that stores password in NM for specific networks
- Document this limitation clearly
- Target desktop users first where this is rarely an issue

### Trade-off: Daemon Complexity vs Security
**Accepted**: Running a daemon adds complexity, but:
- Passwords never leave our encrypted database
- NetworkManager system files contain no secrets
- Standard D-Bus mechanism (not a hack)
- Same pattern used by GNOME Keyring and KDE Wallet

### Trade-off: Rust Learning Curve
**Accepted**: Rust has a steeper learning curve than Python, but:
- Single binary distribution simplifies user experience
- Memory safety guarantees are valuable for security-sensitive code
- Excellent tooling (cargo, rust-analyzer) improves DX
- Cross-compilation to mobile platforms is more straightforward

## Migration Plan

N/A - Greenfield project.

## Open Questions

1. **Sharing Platform Architecture**: File-based sharing works for v1, but should we plan for:
   - Centralized server (simplest discovery)
   - P2P with DHT (most private)
   - Hybrid with optional server

2. **User Identity**: For sharing, how do users identify?
   - No identity (anonymous sharing via files/links)
   - Email-based (like most apps)
   - Public key (most secure, worst UX)

3. **Android Implementation Path**:
   - Rust core via JNI with Kotlin UI layer
   - Full Kotlin app sharing the JSON data format
   - cargo-ndk for building Android libraries

4. **Pre-Login WiFi Networks**: For headless systems that need WiFi before user login:
   - Offer optional "store password in NM" mode per-network
   - Require manual network selection for these edge cases
   - Document as unsupported for v1
