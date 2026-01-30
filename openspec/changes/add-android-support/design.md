# Android Support Design

## Context

This design resolves open question #3 from `add-core-architecture/design.md:253-256` regarding the Android implementation path. Android is P1 priority per `project.md`, and the adapter pattern established in core architecture provides the extension point.

**Constraints**:
- Android WiFi APIs are Java/Kotlin only (no native WiFi access)
- WifiConfiguration is deprecated in API 29+; WifiNetworkSuggestion is the replacement
- Android prohibits reading other apps' WiFi credentials (security sandbox)
- Root access required for credential extraction from system storage

**Stakeholders**:
- End users wanting cross-platform credential sync
- Developers maintaining Rust core and Kotlin UI

## Goals / Non-Goals

### Goals
- Define JNI bridge architecture between Rust core and Kotlin
- Specify Android-specific credential delivery mechanism
- Document permission requirements and handling
- Specify storage locations and encryption approach
- Define extraction capabilities for rooted/non-rooted devices

### Non-Goals
- Support API levels below 29 (Android 10)
- Implement legacy WifiConfiguration path
- Build custom WiFi supplicant (use system WifiManager only)
- Support Android TV or Android Automotive (standard Android phones/tablets only)

## Decisions

### Decision: Rust Core via JNI

**Choice**: Compile Rust core to .so via cargo-ndk; expose API through JNI to Kotlin

**Rationale**:
- Reuses all core Rust logic (storage, filtering, encryption, sharing)
- Single codebase for credential management across platforms
- JNI is mature and well-documented
- cargo-ndk simplifies Android library compilation

**Implementation**:
```rust
// jni-bridge/src/lib.rs
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;

#[no_mangle]
pub extern "system" fn Java_com_wifisync_WifisyncCore_listCredentials(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    // Call into Rust core, serialize result as JSON
    let credentials = wifisync_core::storage::list_credentials();
    let json = serde_json::to_string(&credentials).unwrap();
    env.new_string(json).unwrap().into_inner()
}
```

**Alternatives considered**:
- Full Kotlin rewrite: Duplicates core logic, maintenance burden, divergent behavior
- Kotlin Multiplatform: Immature Android-specific API support, still need JNI for Rust
- WebView wrapper: Poor UX, no native WiFi API access

### Decision: API 29+ Only (Android 10+)

**Choice**: Target WifiNetworkSuggestion API exclusively; minimum SDK 29

**Rationale**:
- WifiConfiguration is deprecated and restricted in API 29+
- WifiNetworkSuggestion is the official replacement
- API 29+ covers ~80% of active Android devices (as of 2025)
- Simplifies codebase (no branching for legacy API)
- Better security model (user must approve suggestions)

**Trade-off**: Users on Android 9 or below cannot use Wifisync

**Alternatives considered**:
- Support API 21+: Requires WifiConfiguration path, deprecated APIs, complex branching
- Support API 26+: Still requires deprecated APIs for some features

### Decision: Direct Password Embedding (No Secret Agent)

**Choice**: Embed passwords directly in WifiNetworkSuggestion; no agent pattern

**Rationale**:
- Android has no D-Bus or equivalent agent mechanism
- WifiNetworkSuggestion requires password at creation time
- Passwords provided via `setWpa2Passphrase()` or `setWpa3Passphrase()`
- System handles password after suggestion is accepted

**Implication**: Unlike Linux where passwords stay in Wifisync DB, Android receives passwords at suggestion time. This is an accepted difference due to platform constraints.

**Security mitigations**:
- Use `SecretString` for password handling in Rust (zeroizes on drop)
- Clear Kotlin strings from memory after JNI call returns
- Minimize password lifetime in application memory

### Decision: Android Keystore for Encryption Keys

**Choice**: Store database encryption key in Android Keystore; use ChaCha20-Poly1305 for data encryption

**Rationale**:
- Hardware-backed key storage on devices with secure element
- Keys cannot be extracted from device
- Biometric authentication support built-in
- Consistent with Android security best practices

**Implementation**:
```kotlin
// Generate key in Keystore
val keyGenerator = KeyGenerator.getInstance(
    KeyProperties.KEY_ALGORITHM_AES,
    "AndroidKeyStore"
)
keyGenerator.init(
    KeyGenParameterSpec.Builder("wifisync_db_key", PURPOSE_ENCRYPT or PURPOSE_DECRYPT)
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setUserAuthenticationRequired(true)
        .setUserAuthenticationParameters(300, AUTH_BIOMETRIC_STRONG)
        .build()
)
val key = keyGenerator.generateKey()
```

**Fallback**: If Keystore unavailable (old devices, emulators), fall back to Argon2-derived key from user password (same as Linux)

### Decision: Root Detection for Extraction Capabilities

**Choice**: Detect root access; enable full extraction only on rooted devices

**Rationale**:
- Android sandbox prevents reading system WiFi storage without root
- WifiConfigStore.xml contains all saved networks (rooted access)
- Non-rooted users can only import/consume credentials (not extract)
- Clear UX messaging about extraction limitations

**Root detection methods**:
1. Check for `su` binary in common paths
2. Check for Magisk/SuperSU packages
3. Attempt privileged file access as verification

### Decision: Storage Access Framework for Export

**Choice**: Use SAF for exporting credential files to user-accessible storage

**Rationale**:
- Android scoped storage restrictions (API 29+)
- App-private storage not accessible to other apps or file managers
- SAF allows user to choose export location
- Works with cloud storage providers (Google Drive, etc.)

## Risks / Trade-offs

### Risk: WifiNetworkSuggestion Limitations
**Description**: Suggestions are not guaranteed to connect; user sees system prompt to approve
**Mitigation**:
- Clear UX explaining suggestion vs. profile difference
- Track suggestion state; re-suggest if user removes
- Document that user approval is required

### Risk: JNI Complexity
**Description**: JNI bridge adds complexity; potential for memory leaks, crashes
**Mitigation**:
- Keep JNI interface minimal (JSON in/out)
- Comprehensive error handling with proper exception translation
- Memory management guidelines in code review checklist

### Risk: Extraction Requires Root
**Description**: Most users cannot extract existing credentials from Android
**Mitigation**:
- Clear messaging: "Import credentials from other devices or files"
- Focus UX on consuming shared credentials
- Document root extraction as advanced feature

### Trade-off: Password Delivery Difference
**Accepted**: Android receives passwords at suggestion time (unlike Linux Secret Agent pattern)
- Platform constraint; no alternative mechanism exists
- Security mitigations applied (memory zeroization, Keystore for storage)
- Consistent end-user experience despite implementation difference

### Trade-off: API 29+ Only
**Accepted**: Users on Android 9 and below cannot use Wifisync
- Legacy API path would double maintenance burden
- ~80% device coverage is acceptable for P1 platform
- Can revisit if user demand is significant

## Migration Plan

N/A - New platform support (no existing Android users to migrate)

## Open Questions

1. **Biometric Timeout**: How long should biometric authentication be valid before re-prompt?
   - Option A: Every database access (most secure, worst UX)
   - Option B: 5-minute timeout (balance)
   - Option C: Until app backgrounded (least secure, best UX)

2. **Suggestion Limit Handling**: Android limits ~50 suggestions per app
   - Option A: Warn user when approaching limit
   - Option B: Automatic rotation based on usage frequency
   - Option C: Let user manually prioritize

3. **Offline Credential Access**: Should credentials be viewable when database is locked?
   - Likely no - security over convenience
