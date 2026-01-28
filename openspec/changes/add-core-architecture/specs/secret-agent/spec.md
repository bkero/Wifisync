# Secret Agent

Capability for providing wifi passwords to NetworkManager on-demand via the D-Bus Secret Agent API.

## ADDED Requirements

### Requirement: Secret Agent Registration

The system SHALL register as a NetworkManager Secret Agent on the D-Bus session bus.

#### Scenario: Register agent on startup
- **GIVEN** the Wifisync agent daemon is starting
- **WHEN** D-Bus connection is established
- **THEN** the agent registers with org.freedesktop.NetworkManager.AgentManager
- **AND** declares capability to handle 802-11-wireless-security secrets

#### Scenario: Agent identifier
- **GIVEN** the agent is registering
- **WHEN** providing an identifier
- **THEN** it uses "com.wifisync.agent" as the unique identifier

#### Scenario: Handle registration failure
- **GIVEN** another agent with the same identifier is registered
- **WHEN** registration is attempted
- **THEN** a clear error is displayed
- **AND** suggests checking for other Wifisync instances

### Requirement: GetSecrets Handler

The system SHALL respond to NetworkManager GetSecrets() calls with the appropriate password.

#### Scenario: Provide PSK for known network
- **GIVEN** a network profile managed by Wifisync
- **WHEN** NetworkManager calls GetSecrets for 802-11-wireless-security
- **THEN** the agent looks up the credential by system_id
- **AND** returns the decrypted PSK
- **AND** NetworkManager completes the connection

#### Scenario: Unknown network request
- **GIVEN** a network not in Wifisync's database
- **WHEN** NetworkManager calls GetSecrets
- **THEN** the agent returns an error indicating no secrets available
- **AND** NetworkManager prompts the user for password

#### Scenario: Database locked
- **GIVEN** the Wifisync database requires unlocking
- **WHEN** GetSecrets is called
- **THEN** the agent prompts for the database password (if interactive)
- **OR** returns an error indicating secrets unavailable (if non-interactive)

#### Scenario: Connection with hints
- **GIVEN** NetworkManager provides hints (e.g., "psk" for the specific secret needed)
- **WHEN** GetSecrets is called
- **THEN** only the requested secrets are returned
- **AND** unnecessary secrets are not exposed

### Requirement: CancelGetSecrets Handler

The system SHALL handle connection activation cancellation.

#### Scenario: User cancels connection
- **GIVEN** GetSecrets is pending
- **WHEN** CancelGetSecrets is called
- **THEN** any pending lookups are cancelled
- **AND** resources are released

### Requirement: SaveSecrets Handler

The system SHALL handle SaveSecrets calls appropriately.

#### Scenario: NetworkManager sends secrets to save
- **GIVEN** NetworkManager calls SaveSecrets after successful connection
- **WHEN** the agent receives the call
- **THEN** the secrets are ignored (Wifisync is the source of truth)
- **AND** the call returns success
- **AND** no data is written to Wifisync database from this path

### Requirement: DeleteSecrets Handler

The system SHALL handle DeleteSecrets calls appropriately.

#### Scenario: NetworkManager requests secret deletion
- **GIVEN** a profile is being deleted from NetworkManager
- **WHEN** DeleteSecrets is called
- **THEN** the agent removes the profile mapping from Wifisync
- **AND** the credential itself is NOT deleted (may be used elsewhere)

### Requirement: Agent Daemon Lifecycle

The system SHALL run as a persistent daemon for Secret Agent functionality.

#### Scenario: Start daemon manually
- **GIVEN** the user runs `wifisync agent start`
- **WHEN** no daemon is running
- **THEN** the daemon starts in the background
- **AND** registers as a Secret Agent
- **AND** prints the PID

#### Scenario: Start daemon via systemd
- **GIVEN** the user enables the systemd user service
- **WHEN** the user logs in
- **THEN** the daemon starts automatically
- **AND** is ready before network connections are attempted

#### Scenario: Stop daemon
- **GIVEN** the daemon is running
- **WHEN** the user runs `wifisync agent stop`
- **THEN** the daemon unregisters from AgentManager
- **AND** exits cleanly

#### Scenario: Daemon status
- **GIVEN** the user runs `wifisync agent status`
- **WHEN** the daemon is running
- **THEN** it shows: running, PID, registered networks count, uptime

#### Scenario: Daemon health check
- **GIVEN** the main CLI runs
- **WHEN** profiles exist but daemon is not running
- **THEN** a warning suggests starting the daemon
- **AND** notes that connections will fail without it

### Requirement: Logging and Debugging

The system SHALL provide logging for Secret Agent operations.

#### Scenario: Log successful authentication
- **GIVEN** GetSecrets succeeds
- **WHEN** logging is enabled
- **THEN** log includes: timestamp, SSID, success, duration

#### Scenario: Log failed authentication
- **GIVEN** GetSecrets fails
- **WHEN** logging is enabled
- **THEN** log includes: timestamp, SSID, failure reason

#### Scenario: Debug mode
- **GIVEN** the daemon is started with --debug
- **WHEN** D-Bus calls arrive
- **THEN** full call details are logged (excluding password values)
