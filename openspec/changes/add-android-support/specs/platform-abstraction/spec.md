# Platform Abstraction - Android Support

Extends the platform abstraction capability with Android-specific adapter, permissions, and updated platform detection.

## ADDED Requirements

### Requirement: Android WifiManager Adapter

The system SHALL provide an adapter for Android using the WifiNetworkSuggestion API (API 29+).

#### Scenario: Initialize adapter via JNI
- **GIVEN** the Android application starts
- **WHEN** the Rust core is initialized via JNI
- **THEN** the Android adapter receives the application Context
- **AND** obtains a reference to WifiManager system service

#### Scenario: List Wifisync-managed suggestions
- **GIVEN** Wifisync has previously installed network suggestions
- **WHEN** list_networks() is called
- **THEN** the adapter returns networks from local tracking storage
- **AND** each entry includes SSID, security type, and installation timestamp

#### Scenario: Extract credentials on rooted device
- **GIVEN** the device has root access
- **WHEN** get_credentials() is called
- **THEN** the adapter reads /data/misc/apexdata/com.android.wifi/WifiConfigStore.xml
- **AND** parses all saved networks with their passwords
- **AND** returns credentials with source_platform set to Android

#### Scenario: Extract credentials on non-rooted device
- **GIVEN** the device does not have root access
- **WHEN** get_credentials() is called for system networks
- **THEN** an error is returned indicating extraction requires root
- **AND** the error message suggests importing credentials from another source

#### Scenario: Create network suggestion with passphrase
- **GIVEN** valid WPA2 or WPA3 credentials
- **WHEN** create_profile() is called
- **THEN** a WifiNetworkSuggestion is built using WifiNetworkSuggestion.Builder
- **AND** setWpa2Passphrase() or setWpa3Passphrase() is called with the password
- **AND** the suggestion is added via WifiManager.addNetworkSuggestions()
- **AND** the suggestion is tracked in local storage with a unique ID

#### Scenario: Handle suggestion approval prompt
- **GIVEN** network suggestions have been added
- **WHEN** the user has not yet approved suggestions for this app
- **THEN** Android displays a system notification requesting approval
- **AND** the adapter tracks pending approval state
- **AND** connection attempts wait for user approval

#### Scenario: Delete network suggestion
- **GIVEN** a Wifisync-managed suggestion exists
- **WHEN** delete_profile() is called with the suggestion ID
- **THEN** the suggestion is removed via WifiManager.removeNetworkSuggestions()
- **AND** the tracking entry is removed from local storage

#### Scenario: Report Android capabilities
- **GIVEN** the adapter is initialized
- **WHEN** queried for capabilities
- **THEN** it reports: API level, root status, suggestion count, suggestion limit
- **AND** it reports "secret_agent" as NOT supported

### Requirement: Android Permissions

The system SHALL handle Android runtime permissions required for WiFi operations.

#### Scenario: Request wifi state permissions on initialization
- **GIVEN** the app is starting for the first time
- **WHEN** the adapter initializes
- **THEN** it checks for ACCESS_WIFI_STATE and CHANGE_WIFI_STATE permissions
- **AND** requests them if not granted

#### Scenario: Request location permission for network scanning
- **GIVEN** the user wants to scan for nearby networks
- **WHEN** a scan is requested
- **THEN** the adapter checks for ACCESS_FINE_LOCATION permission
- **AND** requests it with rationale explaining why location is needed for WiFi

#### Scenario: Handle permission denial
- **GIVEN** the user denies a required permission
- **WHEN** a permission-dependent operation is attempted
- **THEN** an error is returned indicating the required permission
- **AND** the error includes instructions for enabling in Settings

#### Scenario: Handle "Don't ask again" state
- **GIVEN** the user has selected "Don't ask again" for a permission
- **WHEN** that permission is needed
- **THEN** the adapter detects the permanently denied state
- **AND** directs the user to app Settings to manually enable the permission

## MODIFIED Requirements

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

#### Scenario: Detect Android API level
- **GIVEN** running on Android
- **WHEN** platform detection runs
- **THEN** the system checks Build.VERSION.SDK_INT
- **AND** if API level is below 29, an error indicates minimum Android 10 is required

#### Scenario: Unsupported platform
- **GIVEN** running on an unsupported platform
- **WHEN** platform detection runs
- **THEN** a clear error indicates the platform is not yet supported
- **AND** links to documentation for contributing an adapter

### Requirement: Secret Agent Support

The system SHALL support platforms that use a Secret Agent pattern for providing passwords. This capability is Linux-specific; other platforms use different credential delivery mechanisms.

#### Scenario: Adapter declares Secret Agent support
- **GIVEN** a platform adapter that uses Secret Agent
- **WHEN** queried for capabilities
- **THEN** it reports "secret_agent" as a supported feature

#### Scenario: Adapter provides Secret Agent interface
- **GIVEN** a platform adapter with Secret Agent support
- **WHEN** the agent daemon starts
- **THEN** it can obtain a SecretAgent trait implementation from the adapter

#### Scenario: Android declares no Secret Agent support
- **GIVEN** the Android WifiManager adapter
- **WHEN** queried for capabilities
- **THEN** it reports "secret_agent" as NOT supported
- **AND** credential delivery uses direct password embedding in WifiNetworkSuggestion
