# Profile Management

Capability for managing network profiles in the system network manager. Profiles contain network configuration (SSID, security type, flags) but NOT passwords. Passwords are provided on-demand by the Secret Agent.

## ADDED Requirements

### Requirement: Profile Creation

The system SHALL create network profiles in the system network manager without storing passwords.

#### Scenario: Create profile for credential
- **GIVEN** a credential in Wifisync's database
- **WHEN** the user installs it to the system
- **THEN** a network profile is created with SSID and security settings
- **AND** psk-flags is set to 1 (agent-owned secret)
- **AND** NO password is stored in the system profile
- **AND** the system_id is recorded in Wifisync

#### Scenario: Profile settings
- **GIVEN** a WPA2-PSK credential
- **WHEN** creating the profile
- **THEN** it includes: SSID, key-mgmt=wpa-psk, psk-flags=1
- **AND** does NOT include: psk (password field)

#### Scenario: Hidden network profile
- **GIVEN** a credential marked as hidden
- **WHEN** creating the profile
- **THEN** the hidden flag is set to true
- **AND** NetworkManager will actively scan for this network

#### Scenario: Profile already exists
- **GIVEN** a profile for the same SSID already exists
- **WHEN** the user attempts to create another
- **THEN** the operation fails with a clear message
- **AND** suggests using update or remove first

### Requirement: Profile Tracking

The system SHALL track which profiles have been created in the system.

#### Scenario: Record profile mapping
- **GIVEN** a profile is created successfully
- **WHEN** the system returns the connection UUID
- **THEN** Wifisync stores: credential_id -> system_id mapping
- **AND** records the creation timestamp

#### Scenario: Load tracking on startup
- **GIVEN** profile mappings exist in storage
- **WHEN** Wifisync starts
- **THEN** the mappings are loaded
- **AND** profile operations work correctly

#### Scenario: Query profile status
- **GIVEN** a credential in Wifisync
- **WHEN** the user queries its status
- **THEN** they see: installed (yes/no), system_id, created_at

### Requirement: Profile Deletion

The system SHALL delete profiles from the system when requested.

#### Scenario: Delete profile with credential
- **GIVEN** a credential with an installed profile
- **WHEN** the user deletes the credential from Wifisync
- **THEN** the system profile is also deleted
- **AND** the tracking record is removed

#### Scenario: Delete profile only
- **GIVEN** an installed profile
- **WHEN** the user runs `wifisync uninstall <ssid>`
- **THEN** the system profile is deleted
- **AND** the credential remains in Wifisync database
- **AND** the tracking record is removed

#### Scenario: Delete already-removed profile
- **GIVEN** a profile that was manually deleted from NetworkManager
- **WHEN** Wifisync attempts to delete it
- **THEN** the operation succeeds (idempotent)
- **AND** the tracking record is removed
- **AND** a warning is logged

### Requirement: Bulk Operations

The system SHALL support bulk profile operations.

#### Scenario: Install all credentials
- **GIVEN** multiple credentials without profiles
- **WHEN** the user runs `wifisync install --all`
- **THEN** profiles are created for all credentials
- **AND** a summary shows created count

#### Scenario: Uninstall all profiles
- **GIVEN** multiple installed profiles
- **WHEN** the user runs `wifisync uninstall --all`
- **THEN** all Wifisync-managed profiles are removed
- **AND** credentials remain in database
- **AND** a summary shows removed count

#### Scenario: Confirm destructive operations
- **GIVEN** the user runs uninstall --all
- **WHEN** not using --yes flag
- **THEN** a confirmation prompt lists affected networks
- **AND** the user must confirm before proceeding

### Requirement: Profile Synchronization

The system SHALL detect and handle orphaned profiles.

#### Scenario: Detect orphaned system profile
- **GIVEN** a system profile exists that was created by Wifisync
- **WHEN** the Wifisync database doesn't have a matching credential
- **THEN** `wifisync status` shows it as orphaned
- **AND** offers to remove or adopt it

#### Scenario: Detect stale tracking record
- **GIVEN** a tracking record exists for a profile
- **WHEN** the system profile was manually deleted
- **THEN** `wifisync status` shows the credential as "not installed"
- **AND** the tracking record can be cleaned up

#### Scenario: Sync command
- **GIVEN** orphaned profiles or stale records exist
- **WHEN** the user runs `wifisync sync`
- **THEN** the system is reconciled
- **AND** orphans are optionally removed
- **AND** stale records are cleaned
