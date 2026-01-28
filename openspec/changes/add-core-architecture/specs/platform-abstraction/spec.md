# Platform Abstraction

Capability for abstracting platform-specific network manager implementations behind a common interface.

## ADDED Requirements

### Requirement: Adapter Interface

The system SHALL define a common interface that all platform adapters implement.

#### Scenario: Adapter implements required methods
- **GIVEN** a new platform adapter
- **WHEN** it is registered
- **THEN** it must implement: list_networks(), get_credentials(), create_profile(), delete_profile()

#### Scenario: Adapter reports capabilities
- **GIVEN** a platform adapter
- **WHEN** queried for capabilities
- **THEN** it reports which optional features it supports (e.g., priority, hidden networks)

#### Scenario: Adapter provides platform info
- **GIVEN** a platform adapter
- **WHEN** queried for info
- **THEN** it returns platform name, version, and any limitations

### Requirement: Platform Detection

The system SHALL automatically detect the current platform and select the appropriate adapter.

#### Scenario: Detect Linux with NetworkManager
- **GIVEN** running on Linux
- **WHEN** NetworkManager is available via D-Bus
- **THEN** the NetworkManager adapter is selected

#### Scenario: Detect Linux without NetworkManager
- **GIVEN** running on Linux
- **WHEN** NetworkManager is not available
- **THEN** an error indicates no supported network manager found
- **AND** suggests installing NetworkManager or lists other options

#### Scenario: Detect Android
- **GIVEN** running on Android
- **WHEN** platform detection runs
- **THEN** the Android WifiManager adapter is selected

#### Scenario: Unsupported platform
- **GIVEN** running on an unsupported platform
- **WHEN** platform detection runs
- **THEN** a clear error indicates the platform is not yet supported
- **AND** links to documentation for contributing an adapter

### Requirement: Adapter Registration

The system SHALL support registering multiple adapters with priority ordering.

#### Scenario: Register adapter
- **GIVEN** a new adapter implementation
- **WHEN** it is registered with the adapter registry
- **THEN** it becomes available for platform detection

#### Scenario: Adapter priority
- **GIVEN** multiple adapters that could handle the current platform
- **WHEN** platform detection runs
- **THEN** the highest priority compatible adapter is selected

#### Scenario: Manual adapter override
- **GIVEN** a user who wants to use a specific adapter
- **WHEN** they specify it via configuration
- **THEN** that adapter is used instead of auto-detection

### Requirement: NetworkManager Adapter

The system SHALL provide an adapter for Linux NetworkManager.

#### Scenario: Connect via D-Bus
- **GIVEN** NetworkManager is running
- **WHEN** the adapter initializes
- **THEN** it connects via D-Bus system bus

#### Scenario: List saved connections
- **GIVEN** a connected adapter
- **WHEN** list_networks() is called
- **THEN** all saved wifi connections are returned

#### Scenario: Read connection secrets
- **GIVEN** proper permissions
- **WHEN** get_credentials() is called for a network
- **THEN** the password is retrieved from NetworkManager

#### Scenario: Create profile without password
- **GIVEN** credentials to install
- **WHEN** create_profile() is called
- **THEN** a new NetworkManager connection is created
- **AND** psk-flags is set to 1 (agent-owned secret)
- **AND** NO password is stored in the connection
- **AND** the system connection UUID is returned for tracking

#### Scenario: Delete managed profile
- **GIVEN** a Wifisync-managed profile exists in NetworkManager
- **WHEN** delete_profile() is called with the system UUID
- **THEN** the connection is removed from NetworkManager
- **AND** success is returned

#### Scenario: Delete non-existent profile
- **GIVEN** a system UUID that no longer exists
- **WHEN** delete_profile() is called
- **THEN** the operation succeeds (idempotent)
- **AND** a warning is logged

### Requirement: Secret Agent Support

The system SHALL support platforms that use a Secret Agent pattern for providing passwords.

#### Scenario: Adapter declares Secret Agent support
- **GIVEN** a platform adapter that uses Secret Agent
- **WHEN** queried for capabilities
- **THEN** it reports "secret_agent" as a supported feature

#### Scenario: Adapter provides Secret Agent interface
- **GIVEN** a platform adapter with Secret Agent support
- **WHEN** the agent daemon starts
- **THEN** it can obtain a SecretAgent trait implementation from the adapter

### Requirement: Adapter Error Handling

The system SHALL handle adapter errors gracefully.

#### Scenario: Network manager not running
- **GIVEN** NetworkManager service is stopped
- **WHEN** adapter operations are attempted
- **THEN** a clear error indicates the service is not running

#### Scenario: D-Bus connection failure
- **GIVEN** D-Bus is unavailable
- **WHEN** adapter initialization is attempted
- **THEN** an error indicates D-Bus connection failed

#### Scenario: Partial operation failure
- **GIVEN** importing multiple credentials
- **WHEN** one import fails
- **THEN** the error is logged
- **AND** remaining imports continue
- **AND** a summary shows successes and failures
