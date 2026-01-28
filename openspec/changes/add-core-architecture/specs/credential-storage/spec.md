# Credential Storage

Capability for storing and managing wifi credentials in a portable, secure format.

## ADDED Requirements

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

### Requirement: Collection Management

The system SHALL organize credentials into named collections.

#### Scenario: Create a new collection
- **GIVEN** the user wants to group credentials
- **WHEN** they create a collection named "Coffee Shops"
- **THEN** an empty collection with that name is created
- **AND** a unique identifier is assigned

#### Scenario: Add credential to collection
- **GIVEN** an existing collection and an extracted credential
- **WHEN** the credential is added to the collection
- **THEN** the credential is stored within that collection

#### Scenario: Remove credential from collection
- **GIVEN** a collection containing multiple credentials
- **WHEN** one credential is removed
- **THEN** only that credential is removed
- **AND** other credentials remain intact

#### Scenario: Delete collection
- **GIVEN** an existing collection
- **WHEN** the collection is deleted
- **THEN** the collection and all its credentials are removed

### Requirement: Portable Export Format

The system SHALL support exporting credentials to a portable JSON format.

#### Scenario: Export collection to JSON
- **GIVEN** a collection with credentials
- **WHEN** the user exports the collection
- **THEN** a JSON file is created containing all credentials
- **AND** the file can be transferred to another device

#### Scenario: Export with encryption
- **GIVEN** credentials being exported
- **WHEN** the user specifies a password for the export
- **THEN** the exported file is encrypted with that password
- **AND** can only be imported with the same password

#### Scenario: Export format versioning
- **GIVEN** an exported credentials file
- **WHEN** the file is created
- **THEN** it includes a format version number
- **AND** the exporting application version

### Requirement: Import from Portable Format

The system SHALL support importing credentials from the portable format.

#### Scenario: Import unencrypted JSON
- **GIVEN** a valid unencrypted credentials JSON file
- **WHEN** the user imports the file
- **THEN** all credentials are added to local storage

#### Scenario: Import encrypted file
- **GIVEN** an encrypted credentials file
- **WHEN** the user provides the correct password
- **THEN** credentials are decrypted and imported

#### Scenario: Import with wrong password
- **GIVEN** an encrypted credentials file
- **WHEN** the user provides an incorrect password
- **THEN** an error indicates the password is wrong
- **AND** no credentials are imported

#### Scenario: Import duplicate handling
- **GIVEN** an imported credential with same SSID as existing
- **WHEN** import is performed
- **THEN** the user is prompted to skip, overwrite, or keep both
