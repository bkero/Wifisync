# Credential Storage - Android Support

Extends credential storage capability with Android-specific storage locations and export mechanisms.

## MODIFIED Requirements

### Requirement: Local Encrypted Storage

The system SHALL store credentials locally with encryption at rest.

#### Scenario: Save credentials to local storage
- **GIVEN** extracted wifi credentials
- **WHEN** the user saves them to local storage
- **THEN** credentials are encrypted using a user-derived key
- **AND** stored in the platform-appropriate data directory

#### Scenario: Load credentials from local storage
- **GIVEN** previously saved encrypted credentials
- **WHEN** the user requests to load credentials
- **THEN** credentials are decrypted and returned as structured data

#### Scenario: Corrupted storage file
- **GIVEN** a corrupted or tampered storage file
- **WHEN** loading is attempted
- **THEN** an error is returned indicating data corruption
- **AND** the user is advised to restore from backup or re-extract

#### Scenario: Android storage location
- **GIVEN** running on Android
- **WHEN** credentials are saved to local storage
- **THEN** the encrypted database is stored in Context.getFilesDir()
- **AND** the path is app-private (not accessible to other apps)
- **AND** the file is named "wifisync.db.enc"

#### Scenario: Android backup exclusion
- **GIVEN** the credential database exists on Android
- **WHEN** Android backup runs
- **THEN** the database is excluded from cloud backup (android:allowBackup rules)
- **AND** backup exclusion is configured in backup_rules.xml

## ADDED Requirements

### Requirement: Android External Storage Export

The system SHALL support exporting credentials to user-accessible storage on Android using the Storage Access Framework.

#### Scenario: Export via Storage Access Framework
- **GIVEN** the user wants to export credentials to a file
- **WHEN** export is initiated
- **THEN** an Intent with ACTION_CREATE_DOCUMENT is launched
- **AND** the user selects the destination (local storage, Google Drive, etc.)
- **AND** the encrypted export file is written to the selected URI

#### Scenario: Import via Storage Access Framework
- **GIVEN** the user wants to import credentials from a file
- **WHEN** import is initiated
- **THEN** an Intent with ACTION_OPEN_DOCUMENT is launched
- **AND** the user selects the source file
- **AND** the file is read and decrypted from the selected URI

#### Scenario: Share export via Android share sheet
- **GIVEN** an encrypted export file has been created
- **WHEN** the user chooses to share
- **THEN** an Intent with ACTION_SEND is launched
- **AND** the export file is attached with MIME type "application/octet-stream"
- **AND** the user can send via email, messaging apps, or cloud storage
