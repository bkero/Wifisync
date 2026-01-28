# Change: Add Core Architecture for Wifisync

## Why

Wifisync needs a foundational architecture that enables wifi credential synchronization across platforms while maintaining security and user control. This proposal establishes the core capabilities that all platform implementations will build upon.

## What Changes

- **ADDED** Credential Extraction capability - interface for reading wifi credentials from platform-specific network managers
- **ADDED** Credential Storage capability - portable format for storing and transferring credentials
- **ADDED** Credential Filtering capability - rules for excluding WPA-Enterprise, open networks, and personal networks
- **ADDED** Credential Sharing capability - mechanism for publishing and consuming shared credential collections
- **ADDED** Platform Abstraction capability - adapter pattern for supporting multiple network managers
- **ADDED** Secret Agent capability - D-Bus service providing passwords on-demand to NetworkManager (passwords never stored in NM)
- **ADDED** Profile Management capability - create/remove network profiles in system (without passwords)

## Design Principles

1. **Platform Agnostic Core**: Business logic is platform-independent; only adapters touch platform APIs
2. **Security First**: Credentials are always encrypted at rest and in transit; passwords never stored in system network managers
3. **User Control**: Users explicitly choose what to share; personal networks never leak
4. **Offline First**: Core operations work without network; sharing is the only online feature
5. **Single Source of Truth**: Wifisync's encrypted database is the only place passwords are stored; system network managers query Wifisync on-demand via native plugin mechanisms (e.g., NetworkManager Secret Agent)

## Impact

- Affected specs: All new (credential-extraction, credential-storage, credential-filtering, credential-sharing, platform-abstraction, secret-agent, profile-management)
- Affected code: Creates foundation for entire application
- This is a **greenfield** proposal - no existing code affected

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      Wifisync Core                          │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │  Extraction  │  │   Storage    │  │    Filtering     │   │
│  │   Service    │──│   Service    │──│    Service       │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
│         │                 │                   │              │
│         ▼                 ▼                   ▼              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              Platform Abstraction Layer              │   │
│  └──────────────────────────────────────────────────────┘   │
│         │                                     │              │
│         ▼                                     ▼              │
│  ┌──────────────┐                     ┌──────────────────┐   │
│  │ NetworkMgr   │                     │ Android Adapter  │   │
│  │   Adapter    │                     │   (future)       │   │
│  └──────────────┘                     └──────────────────┘   │
│         ▲                                                    │
│         │ D-Bus GetSecrets()                                │
│  ┌──────┴───────┐                                           │
│  │ Secret Agent │◄─── NetworkManager queries for passwords  │
│  │   Daemon     │                                           │
│  └──────────────┘                                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │  Sharing Service │
                    │  (cloud/p2p)     │
                    └──────────────────┘
```

### Secret Agent Integration (Linux)

```
┌─────────────────────────────────────────────────────────────┐
│                  Connection Activation Flow                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  NetworkManager              Wifisync Agent    Wifisync DB  │
│       │                           │                │        │
│       │ 1. User selects           │                │        │
│       │    "CoffeeShop"           │                │        │
│       │                           │                │        │
│       │ 2. Profile has            │                │        │
│       │    psk-flags=1            │                │        │
│       │    (agent-owned)          │                │        │
│       │                           │                │        │
│       │───GetSecrets()───────────►│                │        │
│       │   "Need PSK"              │                │        │
│       │                           │──lookup()─────►│        │
│       │                           │                │        │
│       │                           │◄──password─────│        │
│       │◄──Return PSK─────────────│                │        │
│       │                           │                │        │
│       │ 3. Complete WPA           │                │        │
│       │    handshake              │                │        │
│       ▼                           │                │        │
│  [Connected]                      │                │        │
└─────────────────────────────────────────────────────────────┘
```

## Data Model

```
WifiCredential {
    id: uuid               # Unique identifier for tracking
    ssid: string           # Network name
    security_type: enum    # WPA2, WPA3, etc.
    password: string       # Pre-shared key (encrypted at rest in Wifisync only)
    hidden: boolean        # Whether network broadcasts SSID
    source_platform: enum  # Where credential was extracted from
    created_at: datetime   # When added to Wifisync
    tags: string[]         # User-defined categories
}

CredentialCollection {
    id: uuid
    name: string           # "Coffee Shops", "Work Networks"
    credentials: WifiCredential[]
    is_shared: boolean     # Whether published for sharing
    owner_id: string       # User who created collection
}

NetworkProfile {
    credential_id: uuid    # Reference to WifiCredential
    system_id: string      # Platform-specific connection ID (e.g., NM UUID)
    platform: enum         # Which platform this profile exists on
    created_at: datetime   # When profile was created in system
    # Note: Profile contains SSID + security type, but NO password
    # Password is provided on-demand by the Secret Agent
}
```

## Open Questions

1. **Sharing Platform**: Should we use a centralized server, P2P, or allow both?
2. **Identity**: How do users identify themselves for sharing? Email, username, public key?
3. **Discovery**: How do users find shared collections? Search, QR codes, links?
4. **Pre-login WiFi**: For networks needed before user login (headless systems), offer option to store password in NM as fallback?
