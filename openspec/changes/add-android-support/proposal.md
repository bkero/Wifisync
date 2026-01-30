# Change: Add Android Platform Support

## Why

Android is designated as P1 priority in `project.md`, and the core architecture (add-core-architecture) was designed with multi-platform extensibility in mind. However, there is currently no detailed Android adapter spec comparable to the NetworkManager adapter. This proposal adds comprehensive specifications for Android support, covering the WifiManager adapter, credential delivery mechanisms, permissions handling, storage locations, and extraction limitations.

## What Changes

- **ADDED** Android WifiManager Adapter - platform-specific adapter using WifiNetworkSuggestion API (API 29+)
- **ADDED** Android Permissions - runtime permission handling for wifi state and location
- **MODIFIED** Platform Detection - expand Android scenario with API level detection
- **MODIFIED** Secret Agent Support - clarify as Linux-only; Android declares no support
- **ADDED** Android Credential Delivery capability - direct password embedding, Keystore integration, suggestion lifecycle
- **MODIFIED** Local Encrypted Storage - add Android storage location scenarios
- **ADDED** Android Credential Extraction Limitations - root vs non-root paths, WifiConfigStore.xml

## Design Principles

1. **API 29+ Only**: Target WifiNetworkSuggestion API exclusively; no legacy WifiConfiguration path
2. **Rust Core via JNI**: Reuse core Rust logic; use cargo-ndk for .so compilation with Kotlin UI layer
3. **No Secret Agent on Android**: Passwords are embedded directly in WifiNetworkSuggestion (unlike Linux)
4. **Hardware-Backed Security**: Use Android Keystore for encryption key storage with biometric integration
5. **Graceful Degradation**: Support both rooted (full extraction) and non-rooted (consumer-only) devices

## Impact

- Affected specs:
  - `platform-abstraction` (MODIFIED: detection, Secret Agent; ADDED: Android adapter, permissions)
  - `android-credential-delivery` (NEW capability)
  - `credential-storage` (MODIFIED: Android locations)
  - `credential-extraction` (ADDED: Android limitations)
- Affected code: New `adapter/android.rs`, JNI bridge crate, Kotlin UI layer
- Resolves open question #3 from `add-core-architecture/design.md:253-256`

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Android Application                       │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────┐   │
│  │                   Kotlin UI Layer                     │   │
│  │         (Activities, ViewModels, Compose UI)          │   │
│  └──────────────────────────────────────────────────────┘   │
│                            │                                 │
│                       JNI Bridge                             │
│                            │                                 │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                  Rust Core (.so)                      │   │
│  │    ┌────────────┐  ┌────────────┐  ┌────────────┐    │   │
│  │    │  Storage   │  │  Filtering │  │   Sharing  │    │   │
│  │    │  Service   │  │  Service   │  │   Service  │    │   │
│  │    └────────────┘  └────────────┘  └────────────┘    │   │
│  └──────────────────────────────────────────────────────┘   │
│                            │                                 │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              Android WifiManager Adapter              │   │
│  │    ┌────────────────────────────────────────────┐    │   │
│  │    │         WifiNetworkSuggestion API          │    │   │
│  │    │              (API 29+)                      │    │   │
│  │    └────────────────────────────────────────────┘    │   │
│  └──────────────────────────────────────────────────────┘   │
│                            │                                 │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                  Android Keystore                     │   │
│  │           (Hardware-backed key storage)               │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Credential Delivery Flow (Android vs Linux)

```
Linux (Secret Agent):                 Android (Direct Embedding):
────────────────────                  ────────────────────────────
Profile created                       Suggestion created
  (no password)                         (WITH password)
        │                                     │
        ▼                                     ▼
NetworkManager                        WifiManager receives
queries agent                         suggestion with PSK
        │                                     │
        ▼                                     ▼
Agent returns PSK                     User approves suggestion
from Wifisync DB                      (system prompt)
        │                                     │
        ▼                                     ▼
Connection completes                  Connection completes
```

## Open Questions

1. **Biometric Unlock UX**: Should database unlock require biometric every time, or allow a timeout period?
2. **Suggestion Limits**: Android limits WifiNetworkSuggestion to ~50 per app. How should we handle users with more networks?
3. **Background Sync**: Should the Android app periodically re-suggest removed networks, or only on user action?
