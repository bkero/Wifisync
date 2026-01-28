# Implementation Tasks

## 1. Project Setup
- [ ] 1.1 Initialize Cargo workspace with lib and CLI crates
- [ ] 1.2 Configure rustfmt.toml and clippy.toml
- [ ] 1.3 Set up CI with cargo fmt, clippy, and tests
- [ ] 1.4 Add core dependencies: serde, tokio, thiserror, anyhow

## 2. Core Data Models
- [ ] 2.1 Define WifiCredential struct with serde derives
- [ ] 2.2 Define CredentialCollection struct
- [ ] 2.3 Define SecurityType enum (WPA2, WPA3, WPA2Enterprise, Open, etc.)
- [ ] 2.4 Define SourcePlatform enum (NetworkManager, Android, Windows, iOS)
- [ ] 2.5 Use `secrecy::SecretString` for password fields
- [ ] 2.6 Add unit tests for serialization roundtrip

## 3. Platform Abstraction Layer
- [ ] 3.1 Create NetworkAdapter trait with async_trait
- [ ] 3.2 Define trait methods: list_networks(), get_credentials(), import_credentials()
- [ ] 3.3 Create AdapterRegistry for runtime adapter selection
- [ ] 3.4 Implement platform detection via cfg attributes and runtime checks
- [ ] 3.5 Add mock adapter for testing

## 4. NetworkManager Adapter (Linux)
- [ ] 4.1 Add zbus dependency for D-Bus communication
- [ ] 4.2 Generate or write NetworkManager D-Bus proxy interfaces
- [ ] 4.3 Implement connection enumeration via Settings interface
- [ ] 4.4 Implement credential extraction (SSID, password, security type)
- [ ] 4.5 Implement profile creation (AddConnection2 with psk-flags=1, NO password stored)
- [ ] 4.6 Implement profile deletion (Delete method on connection object)
- [ ] 4.7 Handle polkit authentication and permission errors
- [ ] 4.8 Add integration tests (require running NetworkManager)

## 5. Credential Storage
- [ ] 5.1 Add chacha20poly1305 and argon2 dependencies
- [ ] 5.2 Design storage directory structure (~/.local/share/wifisync/)
- [ ] 5.3 Implement key derivation from password via Argon2id
- [ ] 5.4 Implement encrypted file read/write with ChaCha20-Poly1305
- [ ] 5.5 Implement collection CRUD operations
- [ ] 5.6 Add export to portable JSON format (unencrypted and encrypted)
- [ ] 5.7 Add import from portable JSON format
- [ ] 5.8 Add unit tests for encryption roundtrip

## 6. Credential Filtering
- [ ] 6.1 Define CredentialFilter trait
- [ ] 6.2 Implement EnterpriseFilter (detect 802.1X)
- [ ] 6.3 Implement OpenNetworkFilter (detect no-password)
- [ ] 6.4 Implement ExclusionListFilter (user-managed SSID list)
- [ ] 6.5 Implement TagFilter for collection filtering
- [ ] 6.6 Implement FilterPipeline with statistics collection
- [ ] 6.7 Add unit tests for each filter type

## 7. Profile Management (System Profiles without Passwords)
- [ ] 7.1 Define NetworkProfile struct with credential_id, system_id, platform, created_at
- [ ] 7.2 Implement profiles.json storage for tracking installed profiles
- [ ] 7.3 Implement install() - create profile in system (no password) and track mapping
- [ ] 7.4 Implement uninstall() - remove profile from system and tracking
- [ ] 7.5 Implement uninstall_all() with confirmation and report
- [ ] 7.6 Add orphan detection (system profiles without Wifisync records)
- [ ] 7.7 Add sync status command to show profile state

## 7b. Secret Agent Daemon (Linux)
- [ ] 7b.1 Define SecretAgent trait with D-Bus method signatures
- [ ] 7b.2 Implement D-Bus service at org.freedesktop.NetworkManager.SecretAgent
- [ ] 7b.3 Implement GetSecrets() - lookup password from encrypted database
- [ ] 7b.4 Implement CancelGetSecrets() - cancel pending lookups
- [ ] 7b.5 Implement SaveSecrets() - no-op (we're the source of truth)
- [ ] 7b.6 Implement DeleteSecrets() - remove profile mapping
- [ ] 7b.7 Register with org.freedesktop.NetworkManager.AgentManager
- [ ] 7b.8 Create systemd user service file for agent daemon
- [ ] 7b.9 Implement `wifisync agent start/stop/status` commands
- [ ] 7b.10 Add health check to main CLI (warn if daemon not running)

## 8. Credential Sharing (Basic)
- [ ] 8.1 Define ShareableCollection format with version field
- [ ] 8.2 Implement collection export to encrypted file
- [ ] 8.3 Implement collection import from encrypted file
- [ ] 8.4 Add HMAC integrity verification
- [ ] 8.5 Add import preview (show SSIDs without revealing passwords)

## 9. CLI Interface
- [ ] 9.1 Add clap dependency with derive feature
- [ ] 9.2 Create main entry point and subcommand structure
- [ ] 9.3 Implement `list` command (show local networks)
- [ ] 9.4 Implement `export` command (create shareable file)
- [ ] 9.5 Implement `import` command (import from file to database)
- [ ] 9.6 Implement `exclude` command (manage exclusion list)
- [ ] 9.7 Implement `install` command (create profile in system, no password)
- [ ] 9.8 Implement `uninstall` command (remove profile from system)
- [ ] 9.9 Implement `status` command (show profile state and daemon status)
- [ ] 9.10 Implement `agent` command (start/stop/status for Secret Agent daemon)
- [ ] 9.11 Add colored output with indicatif progress bars
- [ ] 9.12 Add --json flag for machine-readable output

## 10. Testing & Documentation
- [ ] 10.1 Unit tests for all core modules
- [ ] 10.2 Unit tests for profile management lifecycle
- [ ] 10.3 Integration tests for NetworkManager adapter
- [ ] 10.4 Integration tests for Secret Agent D-Bus interface
- [ ] 10.5 End-to-end test for install profile + agent GetSecrets cycle
- [ ] 10.6 End-to-end test for export/import cycle
- [ ] 10.7 Add rustdoc documentation for public API
- [ ] 10.8 Create README with installation and usage instructions
- [ ] 10.9 Document systemd service setup for Secret Agent
