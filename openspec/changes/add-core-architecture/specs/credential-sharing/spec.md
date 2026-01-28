# Credential Sharing

Capability for publishing and consuming shared credential collections.

## ADDED Requirements

### Requirement: Collection Publishing

The system SHALL allow users to publish collections for sharing with others.

#### Scenario: Publish collection to file
- **GIVEN** a collection the user wants to share
- **WHEN** they publish the collection
- **THEN** a shareable file is created
- **AND** it contains encrypted credentials and metadata

#### Scenario: Publish with access control
- **GIVEN** a collection being published
- **WHEN** the user sets a share password
- **THEN** only recipients with the password can import

#### Scenario: Publish excludes personal networks
- **GIVEN** a collection containing some excluded networks
- **WHEN** the collection is published
- **THEN** excluded networks are NOT included in the output
- **AND** the user is notified which networks were excluded

### Requirement: Collection Consumption

The system SHALL allow users to import shared collections from others.

#### Scenario: Import shared collection
- **GIVEN** a shared collection file from another user
- **WHEN** the user imports it
- **THEN** credentials are added to their local storage
- **AND** tagged with the source collection name

#### Scenario: Preview before import
- **GIVEN** a shared collection file
- **WHEN** the user requests a preview
- **THEN** network names (SSIDs) are shown
- **AND** passwords are NOT revealed until import confirmed

#### Scenario: Selective import
- **GIVEN** a shared collection with multiple networks
- **WHEN** the user imports
- **THEN** they can choose which networks to import
- **AND** skip networks they already have or don't want

### Requirement: Sharing Integrity

The system SHALL verify the integrity of shared collections.

#### Scenario: Detect tampered collection
- **GIVEN** a shared collection file that was modified
- **WHEN** import is attempted
- **THEN** integrity check fails
- **AND** user is warned the file may be corrupted or tampered

#### Scenario: Signature verification
- **GIVEN** a collection signed by the publisher
- **WHEN** the recipient imports
- **THEN** the signature is verified
- **AND** publisher identity is displayed if known

### Requirement: Share Metadata

The system SHALL include metadata in shared collections.

#### Scenario: Include collection metadata
- **GIVEN** a collection being shared
- **WHEN** it is published
- **THEN** metadata includes: name, description, credential count, creation date

#### Scenario: Include publisher info
- **GIVEN** a published collection
- **WHEN** optional publisher info is provided
- **THEN** it is included (name, contact) for recipients to see

#### Scenario: Exclude sensitive metadata
- **GIVEN** credentials with local metadata (last connected, priority)
- **WHEN** shared
- **THEN** only portable metadata is included
- **AND** local-only fields are stripped

### Requirement: Share Revocation

The system SHALL support revoking or updating shared collections.

#### Scenario: Version shared collections
- **GIVEN** a published collection
- **WHEN** it is updated and republished
- **THEN** a new version is created with incremented version number

#### Scenario: Notify of updates (future)
- **GIVEN** a user has imported a collection
- **WHEN** a new version is available
- **THEN** they are notified of the update (future: requires sharing platform)
