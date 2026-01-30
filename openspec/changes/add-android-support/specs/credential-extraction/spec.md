# Credential Extraction - Android Support

Extends credential extraction capability with Android-specific limitations and root-based extraction paths.

## ADDED Requirements

### Requirement: Android Credential Extraction Limitations

The system SHALL clearly communicate Android's credential extraction limitations and provide appropriate functionality based on device root status.

#### Scenario: List Wifisync-managed suggestions without root
- **GIVEN** a non-rooted Android device
- **WHEN** the user requests to list networks
- **THEN** only Wifisync-managed suggestions are returned
- **AND** credentials are retrieved from Wifisync's local encrypted storage
- **AND** system-managed networks are NOT visible

#### Scenario: Extract credentials with root access
- **GIVEN** a rooted Android device
- **WHEN** credential extraction is requested
- **THEN** the system reads /data/misc/apexdata/com.android.wifi/WifiConfigStore.xml
- **AND** parses the XML to extract NetworkList entries
- **AND** retrieves SSID, PreSharedKey, and SecurityType for each network
- **AND** returns credentials with source_platform set to Android

#### Scenario: Extract credentials without root access
- **GIVEN** a non-rooted Android device
- **WHEN** extraction of system networks is requested
- **THEN** an error is returned with code EXTRACTION_REQUIRES_ROOT
- **AND** the error message explains: "Android does not allow apps to read system WiFi credentials. To extract existing networks, you need a rooted device or must import credentials from another source."
- **AND** suggested alternatives are provided (import from file, import from Linux device)

#### Scenario: Root detection and capability reporting
- **GIVEN** the Android adapter initializes
- **WHEN** capabilities are queried
- **THEN** root status is detected using multiple methods
- **AND** extraction_supported is set to true only if root is available
- **AND** the detection result is cached for the session

### Requirement: Root Detection Methods

The system SHALL use multiple methods to reliably detect root access on Android.

#### Scenario: Check for su binary
- **GIVEN** root detection is running
- **WHEN** checking for su binary
- **THEN** the system checks common paths: /system/bin/su, /system/xbin/su, /sbin/su, /data/local/xbin/su
- **AND** if any path exists and is executable, root is potentially available

#### Scenario: Check for root management apps
- **GIVEN** root detection is running
- **WHEN** checking for root management packages
- **THEN** the system checks for: com.topjohnwu.magisk, eu.chainfire.supersu, com.noshufou.android.su
- **AND** if any package is installed, root is potentially available

#### Scenario: Verify root access
- **GIVEN** potential root indicators are found
- **WHEN** verification is requested
- **THEN** the system attempts to execute "su -c id"
- **AND** if the command succeeds and returns uid=0, root is confirmed
- **AND** if the command fails or times out, root is not available

#### Scenario: Handle root denial
- **GIVEN** root management app denies the request
- **WHEN** root verification runs
- **THEN** the denial is detected
- **AND** extraction_supported is set to false
- **AND** the user is informed they denied root access to Wifisync

### Requirement: WifiConfigStore Parsing

The system SHALL parse Android's WifiConfigStore.xml format to extract credentials on rooted devices.

#### Scenario: Parse WPA2-Personal network
- **GIVEN** WifiConfigStore.xml contains a WPA2-PSK network
- **WHEN** parsing the file
- **THEN** the SSID is extracted from the ConfigKey or SSID field
- **AND** the password is extracted from the PreSharedKey field
- **AND** security_type is set to WPA2

#### Scenario: Parse WPA3-Personal network
- **GIVEN** WifiConfigStore.xml contains a WPA3-SAE network
- **WHEN** parsing the file
- **THEN** the SSID is extracted from the ConfigKey or SSID field
- **AND** the password is extracted from the PreSharedKey field
- **AND** security_type is set to WPA3

#### Scenario: Skip enterprise networks
- **GIVEN** WifiConfigStore.xml contains an 802.1X enterprise network
- **WHEN** parsing the file
- **THEN** the network is skipped during extraction
- **AND** a log message indicates the enterprise network was skipped
- **AND** the skip reason is included in extraction statistics

#### Scenario: Handle malformed WifiConfigStore
- **GIVEN** WifiConfigStore.xml is malformed or encrypted
- **WHEN** parsing fails
- **THEN** an error is returned indicating parsing failure
- **AND** the error suggests the file may be from an incompatible Android version
- **AND** partial results (if any) are still returned
