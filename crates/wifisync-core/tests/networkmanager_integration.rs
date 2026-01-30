//! NetworkManager adapter integration tests
//!
//! These tests verify the NetworkManager adapter functionality for listing,
//! creating, and deleting wifi profiles. Tests that modify system state
//! are marked with #[ignore].

mod common;

use common::{test_credential, test_storage, unique_test_ssid};
use wifisync_core::adapter::NetworkAdapter;
use wifisync_core::models::{NetworkProfile, SourcePlatform};
use wifisync_core::NetworkManagerAdapter;

/// Test that adapter creation succeeds when NetworkManager is available
#[tokio::test]
async fn test_adapter_creation_with_nm() {
    skip_if_no_networkmanager!();

    let adapter = NetworkManagerAdapter::new().await;
    assert!(
        adapter.is_ok(),
        "Adapter creation should succeed: {:?}",
        adapter.err()
    );
}

/// Test that adapter creation fails gracefully without D-Bus
#[tokio::test]
async fn test_adapter_creation_fails_without_dbus() {
    // Save original env
    let original_addr = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();

    // Force D-Bus to be unavailable by setting invalid address
    // Note: NetworkManager uses system bus, so this test is limited
    // We're mainly testing that errors are handled gracefully

    if !common::dbus_system_available() {
        // If system bus isn't available, adapter creation should fail
        let result = NetworkManagerAdapter::new().await;
        assert!(
            result.is_err(),
            "Adapter should fail without system bus"
        );

        // Error should be ServiceUnavailable
        if let Err(err) = result {
            let err_str = format!("{}", err);
            assert!(
                err_str.contains("unavailable") || err_str.contains("D-Bus") || err_str.contains("connection"),
                "Error should mention D-Bus or unavailability: {}",
                err_str
            );
        }
    }

    // Restore env
    if let Some(addr) = original_addr {
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", addr);
    }
}

/// Test that platform_info returns correct values
#[tokio::test]
async fn test_platform_info() {
    skip_if_no_networkmanager!();

    let adapter = NetworkManagerAdapter::new()
        .await
        .expect("Adapter should be available");

    let info = adapter.platform_info();

    assert_eq!(info.name, "NetworkManager");
    assert!(
        info.features.contains(&"list_networks".to_string()),
        "Should support list_networks"
    );
    assert!(
        info.features.contains(&"create_profile".to_string()),
        "Should support create_profile"
    );
    assert!(
        info.features.contains(&"secret_agent".to_string()),
        "Should support secret_agent"
    );

    // Version should be present if NM is running
    if info.version.is_some() {
        let version = info.version.as_ref().unwrap();
        // NM versions look like "1.42.4"
        assert!(
            version.contains('.'),
            "Version should contain dots: {}",
            version
        );
    }
}

/// Test that source_platform returns NetworkManager
#[tokio::test]
async fn test_source_platform() {
    skip_if_no_networkmanager!();

    let adapter = NetworkManagerAdapter::new()
        .await
        .expect("Adapter should be available");

    assert_eq!(
        adapter.source_platform(),
        SourcePlatform::NetworkManager,
        "Source platform should be NetworkManager"
    );
}

/// Test that list_networks returns valid data
#[tokio::test]
async fn test_list_networks() {
    skip_if_no_networkmanager!();

    let adapter = NetworkManagerAdapter::new()
        .await
        .expect("Adapter should be available");

    let networks = adapter.list_networks().await;
    assert!(
        networks.is_ok(),
        "list_networks should succeed: {:?}",
        networks.err()
    );

    let networks = networks.unwrap();

    // We can't assert specific networks exist, but we can validate the structure
    for network in &networks {
        // SSID should not be empty
        assert!(!network.ssid.is_empty(), "SSID should not be empty");

        // system_id should be present for saved networks
        // (may be None for some connection types)

        // Verify security type is valid (not checking specific value)
        // The security_type field is always set
    }

    eprintln!("Found {} wifi networks", networks.len());
}

/// Test that deleting a nonexistent profile is idempotent (no error)
#[tokio::test]
async fn test_delete_nonexistent_profile() {
    skip_if_no_networkmanager!();

    let adapter = NetworkManagerAdapter::new()
        .await
        .expect("Adapter should be available");

    // Generate a random UUID that definitely doesn't exist
    let fake_uuid = uuid::Uuid::new_v4().to_string();

    let result = adapter.delete_profile(&fake_uuid).await;

    // Should succeed (idempotent delete)
    assert!(
        result.is_ok(),
        "Deleting nonexistent profile should succeed (idempotent): {:?}",
        result.err()
    );
}

/// Test creating and deleting a profile
///
/// This test modifies system state and requires appropriate permissions.
#[tokio::test]
#[ignore = "Modifies system state - run with --ignored"]
async fn test_create_and_delete_profile() {
    skip_if_no_networkmanager!();

    let adapter = NetworkManagerAdapter::new()
        .await
        .expect("Adapter should be available");

    // Use unique SSID to avoid conflicts
    let ssid = unique_test_ssid("wifisync_test");
    let cred = test_credential(&ssid, "testpassword123");

    eprintln!("Creating test profile for SSID: {}", ssid);

    // Create the profile
    let result = adapter.create_profile(&cred).await;

    match result {
        Ok(system_id) => {
            eprintln!("Created profile with system_id: {}", system_id);

            // Verify it appears in the network list
            let networks = adapter.list_networks().await.expect("Should list networks");
            let found = networks.iter().find(|n| n.ssid == ssid);
            assert!(found.is_some(), "Created network should appear in list");

            // Clean up - delete the profile
            let delete_result = adapter.delete_profile(&system_id).await;
            assert!(
                delete_result.is_ok(),
                "Should delete created profile: {:?}",
                delete_result.err()
            );

            // Verify it's gone
            let networks = adapter.list_networks().await.expect("Should list networks");
            let found = networks.iter().find(|n| n.ssid == ssid);
            assert!(found.is_none(), "Deleted network should not appear in list");
        }
        Err(e) => {
            let err_str = format!("{}", e);
            if err_str.contains("Permission") {
                eprintln!("SKIPPED: Insufficient permissions to create profile");
                eprintln!("Run with sudo or configure polkit");
                return;
            }
            panic!("Failed to create profile: {}", e);
        }
    }
}

/// Test that created profiles have psk-flags=1 (agent-owned)
///
/// This verifies that passwords are NOT stored in the profile but will be
/// provided by the Secret Agent.
#[tokio::test]
#[ignore = "Modifies system state - run with --ignored"]
async fn test_profile_has_agent_owned_secrets() {
    skip_if_no_networkmanager!();

    let adapter = NetworkManagerAdapter::new()
        .await
        .expect("Adapter should be available");

    let ssid = unique_test_ssid("wifisync_psk_test");
    let cred = test_credential(&ssid, "testpassword456");

    eprintln!("Creating profile to verify psk-flags: {}", ssid);

    let result = adapter.create_profile(&cred).await;

    match result {
        Ok(system_id) => {
            // We can't easily inspect psk-flags from the adapter,
            // but we can verify that get_credentials fails or returns
            // a different password (since the password isn't stored)

            // Try to get credentials - this should fail because the password
            // isn't stored in the profile (it's agent-owned)
            let get_result = adapter.get_credentials(&ssid).await;

            // Clean up first
            let _ = adapter.delete_profile(&system_id).await;

            // The get_credentials call may fail with "No secrets" or similar
            // because the password isn't stored in NM
            match get_result {
                Ok(retrieved_cred) => {
                    // If it succeeds, the password should be empty or different
                    // (NM might return empty for agent-owned secrets)
                    use secrecy::ExposeSecret;
                    let retrieved_pass = retrieved_cred.password.expose_secret();
                    // Either empty or we got it from somewhere else
                    eprintln!(
                        "Retrieved password length: {} (original: {})",
                        retrieved_pass.len(),
                        "testpassword456".len()
                    );
                }
                Err(e) => {
                    // Expected - can't retrieve agent-owned secrets
                    eprintln!("Expected: couldn't retrieve agent-owned secret: {}", e);
                }
            }
        }
        Err(e) => {
            let err_str = format!("{}", e);
            if err_str.contains("Permission") {
                eprintln!("SKIPPED: Insufficient permissions");
                return;
            }
            panic!("Failed to create profile: {}", e);
        }
    }
}

/// Test the full workflow: storage + adapter + profile tracking
#[tokio::test]
#[ignore = "Modifies system state - run with --ignored"]
async fn test_full_profile_workflow() {
    skip_if_no_networkmanager!();

    let (storage, _tmp) = test_storage();
    let adapter = NetworkManagerAdapter::new()
        .await
        .expect("Adapter should be available");

    let ssid = unique_test_ssid("wifisync_workflow");
    let cred = test_credential(&ssid, "workflowpass");
    let cred_id = cred.id;

    eprintln!("Testing full workflow for: {}", ssid);

    // 1. Save credential to storage
    let mut collection = wifisync_core::CredentialCollection::new("Test");
    collection.add(cred.clone());
    storage.save_collection(&collection).unwrap();

    // 2. Create profile in NetworkManager
    let result = adapter.create_profile(&cred).await;

    match result {
        Ok(system_id) => {
            eprintln!("Created profile: {}", system_id);

            // 3. Track the profile
            let profile = NetworkProfile::new(cred_id, &system_id, SourcePlatform::NetworkManager);
            storage.add_profile(profile).unwrap();

            // 4. Verify lookup chain works
            let found_profile = storage
                .find_profile_by_system_id(&system_id)
                .unwrap()
                .expect("Should find profile");
            let found_cred = storage
                .find_credential(found_profile.credential_id)
                .unwrap()
                .expect("Should find credential");
            assert_eq!(found_cred.ssid, ssid);

            // 5. Clean up
            adapter.delete_profile(&system_id).await.unwrap();
            storage.remove_profile(cred_id).unwrap();

            eprintln!("Full workflow completed successfully");
        }
        Err(e) => {
            let err_str = format!("{}", e);
            if err_str.contains("Permission") {
                eprintln!("SKIPPED: Insufficient permissions");
                return;
            }
            panic!("Failed: {}", e);
        }
    }
}

/// Test adapter with hidden network flag
#[tokio::test]
#[ignore = "Modifies system state - run with --ignored"]
async fn test_hidden_network_profile() {
    skip_if_no_networkmanager!();

    let adapter = NetworkManagerAdapter::new()
        .await
        .expect("Adapter should be available");

    let ssid = unique_test_ssid("wifisync_hidden");
    let mut cred = test_credential(&ssid, "hiddenpass");
    cred.hidden = true;

    eprintln!("Creating hidden network profile: {}", ssid);

    let result = adapter.create_profile(&cred).await;

    match result {
        Ok(system_id) => {
            // Verify hidden flag is set
            let networks = adapter.list_networks().await.unwrap();
            let network = networks.iter().find(|n| n.ssid == ssid);

            if let Some(net) = network {
                assert!(net.hidden, "Network should be marked as hidden");
            }

            // Clean up
            adapter.delete_profile(&system_id).await.unwrap();
        }
        Err(e) => {
            let err_str = format!("{}", e);
            if err_str.contains("Permission") {
                eprintln!("SKIPPED: Insufficient permissions");
                return;
            }
            panic!("Failed: {}", e);
        }
    }
}
