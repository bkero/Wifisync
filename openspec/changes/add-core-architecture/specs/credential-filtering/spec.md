# Credential Filtering

Capability for filtering credentials based on security type, user preferences, and exclusion rules.

## ADDED Requirements

### Requirement: Enterprise Network Exclusion

The system SHALL automatically exclude WPA-Enterprise networks from extraction and sharing.

#### Scenario: Detect WPA2-Enterprise network
- **GIVEN** a saved network using WPA2-Enterprise (802.1X)
- **WHEN** credentials are being extracted
- **THEN** the network is automatically excluded
- **AND** logged as "excluded: enterprise authentication"

#### Scenario: Detect WPA3-Enterprise network
- **GIVEN** a saved network using WPA3-Enterprise
- **WHEN** credentials are being extracted
- **THEN** the network is automatically excluded

#### Scenario: Enterprise exclusion is mandatory
- **GIVEN** a WPA-Enterprise network
- **WHEN** a user attempts to force-include it
- **THEN** the system refuses with explanation
- **AND** no workaround is provided (security requirement)

### Requirement: Open Network Exclusion

The system SHALL automatically exclude networks without passwords.

#### Scenario: Detect open network
- **GIVEN** a saved network with no security (open)
- **WHEN** credentials are being extracted
- **THEN** the network is automatically excluded
- **AND** logged as "excluded: open network"

#### Scenario: Open exclusion rationale
- **GIVEN** an open network being excluded
- **WHEN** the user queries why
- **THEN** explanation states open networks have no credentials to sync

### Requirement: Personal Network Exclusion List

The system SHALL maintain a user-managed list of networks to never share.

#### Scenario: Add network to exclusion list
- **GIVEN** a network the user wants to keep private
- **WHEN** they add its SSID to the exclusion list
- **THEN** the network is excluded from all exports and shares

#### Scenario: Remove network from exclusion list
- **GIVEN** a network on the exclusion list
- **WHEN** the user removes it
- **THEN** the network becomes eligible for export/sharing

#### Scenario: Exclusion list persists
- **GIVEN** networks added to the exclusion list
- **WHEN** the application restarts
- **THEN** the exclusion list is preserved

#### Scenario: Wildcard exclusion patterns
- **GIVEN** multiple home networks named "HomeNetwork", "HomeNetwork-5G"
- **WHEN** the user adds pattern "HomeNetwork*" to exclusions
- **THEN** all matching networks are excluded

### Requirement: Tag-Based Filtering

The system SHALL support filtering credentials by user-assigned tags.

#### Scenario: Filter by single tag
- **GIVEN** credentials tagged with "coffee-shop", "work", "travel"
- **WHEN** the user filters for "coffee-shop"
- **THEN** only credentials with that tag are returned

#### Scenario: Filter by multiple tags (AND)
- **GIVEN** credentials with various tags
- **WHEN** the user filters for "travel" AND "verified"
- **THEN** only credentials with both tags are returned

#### Scenario: Filter by multiple tags (OR)
- **GIVEN** credentials with various tags
- **WHEN** the user filters for "work" OR "office"
- **THEN** credentials with either tag are returned

### Requirement: Filter Pipeline

The system SHALL apply filters in a composable pipeline.

#### Scenario: Chain multiple filters
- **GIVEN** the filtering pipeline
- **WHEN** enterprise filter, open filter, and exclusion list are applied
- **THEN** each filter runs in sequence
- **AND** a credential must pass all filters to be included

#### Scenario: Filter order independence
- **GIVEN** filters A, B, C in the pipeline
- **WHEN** applied in any order
- **THEN** the final result is the same (filters are commutative)

#### Scenario: Filter statistics
- **GIVEN** a filtering operation on 100 credentials
- **WHEN** filtering completes
- **THEN** statistics show how many were excluded by each filter
