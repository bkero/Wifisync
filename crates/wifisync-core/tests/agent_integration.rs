//! Secret Agent integration tests
//!
//! These tests verify the Secret Agent functionality for providing
//! passwords to NetworkManager on demand.

mod common;

use common::{test_credential, test_storage, unique_test_ssid};
use wifisync_core::models::{CredentialCollection, NetworkProfile, SourcePlatform};
use wifisync_core::AgentService;

/// Test that AgentService::status returns None when no PID file exists
#[test]
fn test_agent_status_no_pid_file() {
    let (storage, _tmp) = test_storage();

    let status = AgentService::status(storage.data_dir());
    assert!(status.is_none(), "Should return None when no PID file exists");
}

/// Test that storage can look up credentials by system_id (the lookup path
/// used by the Secret Agent's GetSecrets handler)
#[test]
fn test_credential_lookup_chain() {
    let (storage, _tmp) = test_storage();

    // Create a collection with a credential
    let mut collection = CredentialCollection::new("Test Collection");
    let cred = test_credential("TestNetwork", "secretpassword");
    let cred_id = cred.id;
    collection.add(cred);
    storage.save_collection(&collection).unwrap();

    // Create a network profile mapping (this links system_id to credential_id)
    let system_id = "test-system-uuid-1234";
    let profile = NetworkProfile::new(cred_id, system_id, SourcePlatform::NetworkManager);
    storage.add_profile(profile).unwrap();

    // Now test the lookup chain that GetSecrets uses:
    // 1. Find profile by system_id
    let found_profile = storage
        .find_profile_by_system_id(system_id)
        .unwrap()
        .expect("Should find profile");
    assert_eq!(found_profile.system_id, system_id);

    // 2. Find credential by credential_id
    let found_cred = storage
        .find_credential(found_profile.credential_id)
        .unwrap()
        .expect("Should find credential");
    assert_eq!(found_cred.ssid, "TestNetwork");

    // Verify password (this is what GetSecrets would return)
    use secrecy::ExposeSecret;
    assert_eq!(found_cred.password.expose_secret(), "secretpassword");
}

/// Test that GetSecrets lookup returns None for unknown credentials
#[test]
fn test_credential_lookup_unknown() {
    let (storage, _tmp) = test_storage();

    // No profiles or credentials exist
    let result = storage.find_profile_by_system_id("nonexistent-uuid").unwrap();
    assert!(result.is_none(), "Should return None for unknown system_id");

    // Random credential_id should also return None
    let random_id = uuid::Uuid::new_v4();
    let result = storage.find_credential(random_id).unwrap();
    assert!(result.is_none(), "Should return None for unknown credential_id");
}

/// Test the full lookup chain with multiple collections
#[test]
fn test_credential_lookup_multiple_collections() {
    let (storage, _tmp) = test_storage();

    // Create multiple collections
    let mut col1 = CredentialCollection::new("Home Networks");
    let cred1 = test_credential("HomeWifi", "homepass");
    let cred1_id = cred1.id;
    col1.add(cred1);
    storage.save_collection(&col1).unwrap();

    let mut col2 = CredentialCollection::new("Work Networks");
    let cred2 = test_credential("WorkWifi", "workpass");
    let cred2_id = cred2.id;
    col2.add(cred2);
    storage.save_collection(&col2).unwrap();

    // Create profiles for both
    let profile1 = NetworkProfile::new(cred1_id, "home-uuid", SourcePlatform::NetworkManager);
    let profile2 = NetworkProfile::new(cred2_id, "work-uuid", SourcePlatform::NetworkManager);
    storage.add_profile(profile1).unwrap();
    storage.add_profile(profile2).unwrap();

    // Lookup should find credentials across collections
    let found = storage.find_profile_by_system_id("home-uuid").unwrap().unwrap();
    let cred = storage.find_credential(found.credential_id).unwrap().unwrap();
    assert_eq!(cred.ssid, "HomeWifi");

    let found = storage.find_profile_by_system_id("work-uuid").unwrap().unwrap();
    let cred = storage.find_credential(found.credential_id).unwrap().unwrap();
    assert_eq!(cred.ssid, "WorkWifi");
}

/// Test that profile removal doesn't affect the underlying credential
#[test]
fn test_profile_removal_preserves_credential() {
    let (storage, _tmp) = test_storage();

    // Create collection with credential
    let mut collection = CredentialCollection::new("Test");
    let cred = test_credential("Network", "password");
    let cred_id = cred.id;
    collection.add(cred);
    storage.save_collection(&collection).unwrap();

    // Create profile
    let profile = NetworkProfile::new(cred_id, "system-uuid", SourcePlatform::NetworkManager);
    storage.add_profile(profile).unwrap();

    // Remove profile
    let removed = storage.remove_profile(cred_id).unwrap();
    assert!(removed.is_some(), "Profile should be removed");

    // Credential should still exist
    let cred = storage.find_credential(cred_id).unwrap();
    assert!(cred.is_some(), "Credential should still exist after profile removal");
    assert_eq!(cred.unwrap().ssid, "Network");

    // Profile lookup should now fail
    let profile = storage.find_profile_by_system_id("system-uuid").unwrap();
    assert!(profile.is_none(), "Profile lookup should fail after removal");
}

/// Test that we can create storage with unique test SSIDs (for cleanup safety)
#[test]
fn test_unique_ssid_for_integration() {
    let ssid1 = unique_test_ssid("wifisync_test");
    let ssid2 = unique_test_ssid("wifisync_test");

    // Should be unique to avoid interfering with real networks
    assert_ne!(ssid1, ssid2);
    assert!(ssid1.starts_with("wifisync_test_"));

    // Should be short enough for valid SSID
    assert!(ssid1.len() <= 32);
}

/// Test agent registration requires D-Bus (skipped without it)
///
/// This test is marked ignored because it would actually try to register
/// with NetworkManager, which modifies system state.
#[tokio::test]
#[ignore = "Requires NetworkManager running and modifies system state"]
async fn test_agent_registration_with_networkmanager() {
    skip_if_no_networkmanager!();

    let (storage, _tmp) = test_storage();

    // Note: AgentService::run() is blocking and waits for shutdown signals.
    // For a true integration test, we'd need to spawn it in a background task
    // and then send a signal. This is complex enough that it's marked ignored.

    // Just verify we can create the storage that would be passed to run()
    assert!(storage.data_dir().exists());
}

/// Test that multiple profiles can reference different credentials
#[test]
fn test_multiple_profiles_different_credentials() {
    let (storage, _tmp) = test_storage();

    let mut collection = CredentialCollection::new("Mixed");
    let cred1 = test_credential("Net1", "pass1");
    let cred2 = test_credential("Net2", "pass2");
    let cred1_id = cred1.id;
    let cred2_id = cred2.id;
    collection.add(cred1);
    collection.add(cred2);
    storage.save_collection(&collection).unwrap();

    // Create profiles
    storage
        .add_profile(NetworkProfile::new(cred1_id, "uuid1", SourcePlatform::NetworkManager))
        .unwrap();
    storage
        .add_profile(NetworkProfile::new(cred2_id, "uuid2", SourcePlatform::NetworkManager))
        .unwrap();

    // Both lookups should work independently
    let p1 = storage.find_profile_by_system_id("uuid1").unwrap().unwrap();
    let p2 = storage.find_profile_by_system_id("uuid2").unwrap().unwrap();

    assert_eq!(p1.credential_id, cred1_id);
    assert_eq!(p2.credential_id, cred2_id);

    let c1 = storage.find_credential(p1.credential_id).unwrap().unwrap();
    let c2 = storage.find_credential(p2.credential_id).unwrap().unwrap();

    assert_eq!(c1.ssid, "Net1");
    assert_eq!(c2.ssid, "Net2");
}
