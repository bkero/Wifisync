# Implementation Tasks

## 1. Spec Finalization
- [ ] 1.1 Resolve open question: biometric timeout period (every access, 5-min, until backgrounded)
- [ ] 1.2 Resolve open question: suggestion limit handling strategy (warn, rotate, or manual prioritize)
- [ ] 1.3 Resolve open question: background re-suggestion behavior (automatic vs user-initiated)
- [ ] 1.4 Review specs with stakeholders and incorporate feedback
- [ ] 1.5 Run `openspec validate add-android-support --strict` and fix any issues

## 2. Build System Setup
- [ ] 2.1 Add cargo-ndk to development dependencies
- [ ] 2.2 Configure Cargo.toml for Android targets (aarch64-linux-android, armv7-linux-androideabi, x86_64-linux-android)
- [ ] 2.3 Create jni-bridge crate in workspace with JNI dependencies
- [ ] 2.4 Set up Android project with Gradle (app module, Kotlin DSL)
- [ ] 2.5 Configure Gradle to invoke cargo-ndk during build
- [ ] 2.6 Set up CI for Android builds (GitHub Actions with NDK)
- [ ] 2.7 Configure minSdk=29, targetSdk=34 in build.gradle.kts

## 3. Android Adapter Implementation
- [ ] 3.1 Create adapter/android.rs with AndroidAdapter struct
- [ ] 3.2 Implement JNI initialization receiving Android Context
- [ ] 3.3 Implement list_networks() returning tracked suggestions from local storage
- [ ] 3.4 Implement create_profile() using WifiNetworkSuggestion.Builder
- [ ] 3.5 Implement delete_profile() using WifiManager.removeNetworkSuggestions()
- [ ] 3.6 Implement get_credentials() with root detection branching
- [ ] 3.7 Implement platform_info() returning API level, root status, suggestion stats
- [ ] 3.8 Implement source_platform() returning SourcePlatform::Android
- [ ] 3.9 Add adapter to registry with Android platform detection

## 4. Credential Delivery Implementation
- [ ] 4.1 Implement WifiNetworkSuggestion builder wrapper in Kotlin
- [ ] 4.2 Implement setWpa2Passphrase/setWpa3Passphrase based on security type
- [ ] 4.3 Implement suggestion tracking storage (Room database or JSON file)
- [ ] 4.4 Implement suggestion state sync (detect user removals)
- [ ] 4.5 Implement batch suggestion installation with progress reporting
- [ ] 4.6 Implement suggestion approval status tracking
- [ ] 4.7 Handle all WifiManager error codes with descriptive messages

## 5. Storage and Security
- [ ] 5.1 Implement Android Keystore key generation (AES-256-GCM)
- [ ] 5.2 Implement BiometricPrompt integration for key access
- [ ] 5.3 Implement Keystore key wrapping for ChaCha20-Poly1305 DEK
- [ ] 5.4 Implement Argon2 fallback for devices without Keystore
- [ ] 5.5 Configure storage location (Context.getFilesDir())
- [ ] 5.6 Configure backup exclusion in backup_rules.xml
- [ ] 5.7 Implement SAF export (ACTION_CREATE_DOCUMENT)
- [ ] 5.8 Implement SAF import (ACTION_OPEN_DOCUMENT)
- [ ] 5.9 Implement share sheet export (ACTION_SEND)

## 6. Permissions Handling
- [ ] 6.1 Declare permissions in AndroidManifest.xml (ACCESS_WIFI_STATE, CHANGE_WIFI_STATE, ACCESS_FINE_LOCATION)
- [ ] 6.2 Implement runtime permission request flow
- [ ] 6.3 Implement permission rationale dialogs
- [ ] 6.4 Implement "Don't ask again" detection and Settings redirect
- [ ] 6.5 Implement graceful degradation when permissions denied

## 7. Root Detection and Extraction
- [ ] 7.1 Implement su binary path checks
- [ ] 7.2 Implement root management app detection (Magisk, SuperSU)
- [ ] 7.3 Implement root verification via su command execution
- [ ] 7.4 Implement WifiConfigStore.xml parsing
- [ ] 7.5 Handle WPA2, WPA3, and skip enterprise networks during parsing
- [ ] 7.6 Implement extraction error handling with user-friendly messages
- [ ] 7.7 Cache root detection result for session

## 8. Testing
- [ ] 8.1 Create mock JNI environment for unit testing Rust code
- [ ] 8.2 Unit tests for Android adapter methods
- [ ] 8.3 Unit tests for WifiConfigStore.xml parsing
- [ ] 8.4 Unit tests for suggestion tracking storage
- [ ] 8.5 Instrumented tests for Keystore integration
- [ ] 8.6 Instrumented tests for BiometricPrompt flow
- [ ] 8.7 Instrumented tests for SAF export/import
- [ ] 8.8 Integration tests on emulator (API 29, 30, 33, 34)
- [ ] 8.9 Integration tests on physical rooted device (if available)
- [ ] 8.10 End-to-end test: import credentials, install suggestions, verify connectivity

## 9. Documentation
- [ ] 9.1 Document Android build setup in README (NDK, cargo-ndk)
- [ ] 9.2 Document minimum requirements (Android 10+, permissions)
- [ ] 9.3 Document extraction limitations (root required for system networks)
- [ ] 9.4 Document suggestion approval UX flow
- [ ] 9.5 Update project.md with Android-specific conventions
- [ ] 9.6 Create troubleshooting guide for common Android issues
- [ ] 9.7 Document Keystore vs password-based encryption trade-offs
