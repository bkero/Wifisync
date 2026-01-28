# Credential Extraction

Capability for reading wifi credentials from platform-specific network managers.

## ADDED Requirements

### Requirement: Network Enumeration

The system SHALL enumerate all saved wifi networks from the local network manager.

#### Scenario: List all saved networks on Linux
- **GIVEN** the user is running on Linux with NetworkManager
- **WHEN** the extraction service requests network list
- **THEN** all saved wifi connections are returned with their SSIDs

#### Scenario: Empty network list
- **GIVEN** no wifi networks are saved on the device
- **WHEN** the extraction service requests network list
- **THEN** an empty list is returned without error

### Requirement: Credential Reading

The system SHALL extract the password and security configuration for each saved network.

#### Scenario: Extract WPA2 credentials
- **GIVEN** a saved WPA2-Personal network exists
- **WHEN** credentials are extracted for that network
- **THEN** the SSID, password, and security type (WPA2) are returned

#### Scenario: Extract WPA3 credentials
- **GIVEN** a saved WPA3-Personal network exists
- **WHEN** credentials are extracted for that network
- **THEN** the SSID, password, and security type (WPA3) are returned

#### Scenario: Handle hidden networks
- **GIVEN** a saved hidden wifi network exists
- **WHEN** credentials are extracted for that network
- **THEN** the hidden flag is set to true in the returned credential

### Requirement: Permission Handling

The system SHALL handle platform-specific permission requirements for reading credentials.

#### Scenario: Insufficient permissions on Linux
- **GIVEN** the user lacks polkit authorization to read secrets
- **WHEN** credential extraction is attempted
- **THEN** a clear error message indicates permission is required
- **AND** instructions for granting permission are provided

#### Scenario: Elevated permissions available
- **GIVEN** the user has proper authorization to read secrets
- **WHEN** credential extraction is attempted
- **THEN** credentials are extracted successfully

### Requirement: Metadata Extraction

The system SHALL extract additional metadata about each network when available.

#### Scenario: Extract last connection time
- **GIVEN** a saved network with connection history
- **WHEN** credentials are extracted
- **THEN** the last connection timestamp is included if available

#### Scenario: Extract network priority
- **GIVEN** a saved network with autoconnect priority set
- **WHEN** credentials are extracted
- **THEN** the priority value is included in the metadata
