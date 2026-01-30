# Android Credential Delivery

Capability for delivering WiFi credentials to the Android system and managing encrypted local storage using Android-specific security mechanisms.

## ADDED Requirements

### Requirement: Direct Password Delivery

The system SHALL embed passwords directly in WifiNetworkSuggestion objects when creating network suggestions on Android.

#### Scenario: Embed WPA2 password in suggestion
- **GIVEN** a WPA2-Personal credential in Wifisync storage
- **WHEN** installing the network on Android
- **THEN** WifiNetworkSuggestion.Builder.setWpa2Passphrase() is called with the password
- **AND** the password is retrieved from encrypted storage immediately before use
- **AND** the SecretString is zeroized after the JNI call completes

#### Scenario: Embed WPA3 password in suggestion
- **GIVEN** a WPA3-Personal credential in Wifisync storage
- **WHEN** installing the network on Android
- **THEN** WifiNetworkSuggestion.Builder.setWpa3Passphrase() is called with the password
- **AND** the password is retrieved from encrypted storage immediately before use
- **AND** the SecretString is zeroized after the JNI call completes

#### Scenario: Handle suggestion creation failure
- **GIVEN** valid credentials for suggestion creation
- **WHEN** WifiManager.addNetworkSuggestions() returns an error code
- **THEN** the error is mapped to a descriptive message
- **AND** common errors are explained (STATUS_NETWORK_SUGGESTIONS_ERROR_ADD_DUPLICATE, etc.)
- **AND** the failed credential is not marked as installed

### Requirement: Android Keystore Integration

The system SHALL use Android Keystore for storing the database encryption key when available.

#### Scenario: Generate encryption key in Keystore
- **GIVEN** the app is initialized for the first time
- **WHEN** no encryption key exists
- **THEN** an AES-256 key is generated in Android Keystore
- **AND** the key is configured for GCM mode with no padding
- **AND** the key alias is "wifisync_db_key"

#### Scenario: Encrypt credential database with Keystore key
- **GIVEN** an encryption key exists in Keystore
- **WHEN** credentials are saved to local storage
- **THEN** the Keystore key encrypts the ChaCha20-Poly1305 data encryption key
- **AND** the encrypted DEK and ciphertext are stored in app-private storage

#### Scenario: Biometric unlock for database access
- **GIVEN** the database encryption key requires user authentication
- **WHEN** credentials need to be accessed
- **THEN** the BiometricPrompt is displayed
- **AND** upon successful authentication, the Keystore key is unlocked
- **AND** credential operations proceed

#### Scenario: Fallback to password derivation
- **GIVEN** Android Keystore is unavailable (emulator, old device)
- **WHEN** encryption key is needed
- **THEN** the system falls back to Argon2id key derivation from user password
- **AND** the user is prompted to set a database password
- **AND** a warning indicates reduced security compared to Keystore

### Requirement: Network Suggestion Lifecycle

The system SHALL track and manage the lifecycle of installed network suggestions.

#### Scenario: Track installed suggestions in local storage
- **GIVEN** a network suggestion is successfully added
- **WHEN** WifiManager.addNetworkSuggestions() returns success
- **THEN** the suggestion is recorded in local tracking database
- **AND** the record includes: credential_id, SSID, security type, installation timestamp, suggestion hash

#### Scenario: Detect suggestion removal by user
- **GIVEN** suggestions are installed and tracked
- **WHEN** the user removes a suggestion via Android Settings
- **THEN** the next sync detects the missing suggestion
- **AND** the tracking record is updated to reflect removal
- **AND** the credential remains in Wifisync storage (not deleted)

#### Scenario: Re-suggest removed network
- **GIVEN** a user-removed suggestion is detected
- **WHEN** the user requests to reinstall the network
- **THEN** a new suggestion is created from the stored credential
- **AND** the tracking record is updated with the new installation timestamp
- **AND** the user is reminded they may need to approve suggestions again

#### Scenario: Bulk suggestion management
- **GIVEN** multiple credentials are selected for installation
- **WHEN** bulk install is requested
- **THEN** suggestions are batched (max 50 per API call)
- **AND** progress is reported for each batch
- **AND** failures are collected and reported at the end
- **AND** successful suggestions are tracked individually

### Requirement: Password Lifecycle in Memory

The system SHALL minimize password exposure in memory on Android.

#### Scenario: Password retrieved for suggestion creation
- **GIVEN** a credential needs to be installed as a suggestion
- **WHEN** the password is retrieved from encrypted storage
- **THEN** it is held in a SecretString (Rust) that zeroizes on drop
- **AND** the password is passed to JNI as a byte array
- **AND** the Kotlin layer converts to String only for the API call
- **AND** explicit Arrays.fill() is called on byte arrays after use

#### Scenario: Password displayed to user
- **GIVEN** the user requests to view a stored password
- **WHEN** the password is decrypted and displayed
- **THEN** the display uses a secure text field (no clipboard by default)
- **AND** the password is cleared from the view after a timeout (30 seconds)
- **AND** the underlying data is zeroized when the view is dismissed
