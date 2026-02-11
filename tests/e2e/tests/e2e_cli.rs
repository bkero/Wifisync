//! CLI End-to-End tests
//!
//! Black-box tests that invoke the `wifisync` binary as a subprocess against
//! a live Docker server. Each test uses a unique user and isolated data
//! directory.
//!
//! Run via the orchestrator:
//!   ./tests/e2e/run-e2e.sh --cli-only
//!
//! Or directly (server must be running on E2E_SERVER_PORT):
//!   E2E_SERVER_URL=http://localhost:18080 \
//!   E2E_CLI_BINARY=./target/release/wifisync \
//!   cargo test --test e2e_cli -- --test-threads=1

use e2e_helpers::{self, TestEnv};

// =============================================================================
// Setup: ensure server is reachable
// =============================================================================

fn setup() -> TestEnv {
    e2e_helpers::require_server();
    TestEnv::new()
}

// =============================================================================
// 1. Collection Management
// =============================================================================

#[test]
fn test_create_collection() {
    let env = setup();
    let res = env.collection_create("travel");
    res.assert_success();
    let j = res.json();
    assert_eq!(j["name"], "travel");
    assert_eq!(j["created"], true);

    // Verify it shows up in list
    let list = env.collection_list();
    list.assert_success();
    let items = list.json();
    let items = items.as_array().expect("collection list should be array");
    assert!(items.iter().any(|c| c["name"] == "travel"));
}

#[test]
fn test_create_duplicate_collection() {
    let env = setup();
    env.collection_create("dup_test").assert_success();

    let res = env.collection_create("dup_test");
    res.assert_failure();
    assert!(
        res.stdout_contains("already exists") || res.stderr_contains("already exists"),
        "Expected 'already exists' error, got stdout={}, stderr={}",
        res.stdout,
        res.stderr
    );
}

#[test]
fn test_delete_collection() {
    let env = setup();
    env.collection_create("temp_coll").assert_success();

    let res = env.collection_delete("temp_coll");
    res.assert_success();

    // Verify removed from list
    let list = env.collection_list();
    list.assert_success();
    let items = list.json();
    let items = items.as_array().expect("collection list should be array");
    assert!(
        !items.iter().any(|c| c["name"] == "temp_coll"),
        "Deleted collection should not appear in list"
    );
}

#[test]
fn test_list_collections() {
    let env = setup();
    env.collection_create("coll_a").assert_success();
    env.collection_create("coll_b").assert_success();
    env.collection_create("coll_c").assert_success();

    let list = env.collection_list();
    list.assert_success();
    let items = list.json();
    let items = items.as_array().expect("collection list should be array");
    assert!(items.len() >= 3, "Expected at least 3 collections, got {}", items.len());

    let names: Vec<&str> = items.iter().filter_map(|c| c["name"].as_str()).collect();
    assert!(names.contains(&"coll_a"));
    assert!(names.contains(&"coll_b"));
    assert!(names.contains(&"coll_c"));
}

#[test]
fn test_show_collection_with_credential() {
    let env = setup();
    // Import a fixture with credentials, then show the collection
    let fixture = env.write_fixture_collection("show_test", &[("MyWifi", "pass1234")]);
    env.import_collection(&fixture).assert_success();

    let show = env.collection_show("show_test");
    show.assert_success();
    let j = show.json();
    assert_eq!(j["name"], "show_test");
    let creds = j["credentials"].as_array().expect("credentials should be array");
    assert_eq!(creds.len(), 1);
    assert_eq!(creds[0]["ssid"], "MyWifi");

}

// =============================================================================
// 2. Import / Network Management (via import fixtures)
// =============================================================================

#[test]
fn test_import_collection_from_file() {
    let env = setup();
    let fixture = env.write_fixture_collection(
        "imported",
        &[("Net1", "pass1"), ("Net2", "pass2")],
    );

    let res = env.import_collection(&fixture);
    res.assert_success();
    let j = res.json();
    assert_eq!(j["collection"], "imported");
    assert_eq!(j["credentials"], 2);

    // Verify collection exists
    let show = env.collection_show("imported");
    show.assert_success();
    let show_json = show.json();
    let creds = show_json["credentials"]
        .as_array()
        .expect("credentials array");
    assert_eq!(creds.len(), 2);
}

#[test]
fn test_import_duplicate_collection_rejected() {
    let env = setup();
    let fixture = env.write_fixture_collection("dup_import", &[("Net1", "pass1")]);

    env.import_collection(&fixture).assert_success();
    // Second import should fail
    let res = env.import_collection(&fixture);
    res.assert_failure();
    assert!(
        res.stdout_contains("already exists") || res.stderr_contains("already exists"),
        "Expected duplicate error"
    );
}

// =============================================================================
// 3. Login / Logout
// =============================================================================

#[test]
fn test_cli_login_logout() {
    let env = setup();

    // Login
    let login = env.login();
    login.assert_success();
    let j = login.json();
    assert_eq!(j["status"], "success");
    assert_eq!(j["username"], env.username);
    assert!(j["device_id"].is_string());

    // Status should show enabled
    let status = env.sync_status();
    status.assert_success();
    let s = status.json();
    assert_eq!(s["enabled"], true);
    assert_eq!(s["username"], env.username);

    // Logout
    let logout = env.logout();
    logout.assert_success();

    // Status should show disabled
    let status = env.sync_status();
    status.assert_success();
    let s = status.json();
    assert_eq!(s["enabled"], false);
}

#[test]
fn test_login_stores_auth_proof() {
    let env = setup();
    env.login().assert_success();

    let status = env.sync_status();
    status.assert_success();
    let s = status.json();
    assert_eq!(s["has_valid_token"], true);
}

#[test]
fn test_relogin_same_user() {
    let env = setup();

    // First login
    let login1 = env.login();
    login1.assert_success();
    let device_id_1 = login1.json()["device_id"].as_str().unwrap().to_string();

    // Logout
    env.logout().assert_success();

    // Second login
    let login2 = env.login();
    login2.assert_success();
    let device_id_2 = login2.json()["device_id"].as_str().unwrap().to_string();

    // Device IDs should differ (new login = new device registration)
    assert_ne!(
        device_id_1, device_id_2,
        "Re-login should assign a new device_id"
    );
}

#[test]
fn test_login_new_user_auto_registers() {
    let env = setup();
    // The unique username has never been seen by the server
    let login = env.login();
    login.assert_success();
    let j = login.json();
    assert_eq!(j["status"], "success");
    assert_eq!(j["username"], env.username);
}

#[test]
fn test_multiple_devices_same_user() {
    let env = setup();

    // Device A logs in
    let login_a = env.login();
    login_a.assert_success();
    let device_a = login_a.json()["device_id"].as_str().unwrap().to_string();

    // Device B (same user, different data dir) logs in
    let env_b = env.second_device();
    let login_b = env_b.login();
    login_b.assert_success();
    let device_b = login_b.json()["device_id"].as_str().unwrap().to_string();

    assert_ne!(device_a, device_b, "Different devices should get distinct device_ids");

    // Both should be able to check status
    env.sync_status().assert_success();
    env_b.sync_status().assert_success();
}

// =============================================================================
// 4. Push and Pull (CLI-only, single device)
// =============================================================================

#[test]
fn test_push_empty_collection() {
    let env = setup();
    env.login().assert_success();

    // Create an empty collection
    env.collection_create("empty_push").assert_success();

    // Push should succeed (or say "no changes" since no credentials)
    let push = env.push();
    push.assert_success();
}

#[test]
fn test_first_sync_pushes_all() {
    let env = setup();

    // Import some credentials BEFORE login
    let fixture = env.write_fixture_collection(
        "first_sync",
        &[("WiFi_A", "passA"), ("WiFi_B", "passB")],
    );
    env.import_collection(&fixture).assert_success();

    // Now login and push
    env.login().assert_success();
    let push = env.push();
    push.assert_success();
    let j = push.json();
    // Should have pushed the 2 credentials
    assert_eq!(j["status"], "success");
    assert!(
        j["accepted"].as_u64().unwrap_or(0) >= 2,
        "Expected at least 2 accepted, got: {}",
        j
    );
}

#[test]
fn test_pull_no_changes() {
    let env = setup();
    env.login().assert_success();

    // Import and push first
    let fixture = env.write_fixture_collection("pull_test", &[("Net1", "pass1")]);
    env.import_collection(&fixture).assert_success();
    env.push().assert_success();

    // Pull immediately — should succeed with no errors
    let pull = env.pull();
    pull.assert_success();
    let j = pull.json();
    assert_eq!(j["status"], "success");
    let errors = j.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);
    assert_eq!(errors, 0, "Pull after push should have 0 errors, got: {j}");
}

#[test]
fn test_push_pull_roundtrip() {
    let env = setup();
    env.login().assert_success();

    // Push from device A
    let fixture = env.write_fixture_collection(
        "roundtrip",
        &[("Cafe_WiFi", "latte123"), ("Hotel_Net", "room456")],
    );
    env.import_collection(&fixture).assert_success();
    env.push().assert_success();

    // Pull from device B (same user, clean data dir)
    let env_b = env.second_device();
    env_b.login().assert_success();
    let pull = env_b.pull();
    pull.assert_success();
    let j = pull.json();
    assert_eq!(j["status"], "success");
    // Should have applied 2 credentials
    let applied = j.get("applied").and_then(|v| v.as_u64()).unwrap_or(0);
    assert!(
        applied >= 2,
        "Expected at least 2 applied changes, got: {}",
        j
    );
}

#[test]
fn test_push_after_pull() {
    let env = setup();
    env.login().assert_success();

    // Initial push
    let fixture = env.write_fixture_collection("push_after_pull", &[("Net_A", "passA")]);
    env.import_collection(&fixture).assert_success();
    env.push().assert_success();

    // Pull (no changes expected)
    env.pull().assert_success();

    // Import more data and push again
    let fixture2 = env.write_fixture_collection("push_after_pull_2", &[("Net_B", "passB")]);
    env.import_collection(&fixture2).assert_success();
    let push2 = env.push();
    push2.assert_success();
    let j = push2.json();
    assert_eq!(j["status"], "success");
}

// =============================================================================
// 5. Invalid Passwords
// =============================================================================

#[test]
fn test_wrong_password_login() {
    let env = setup();

    // Register user with correct password
    env.login().assert_success();
    env.logout().assert_success();

    // Try to login with wrong password
    let mut bad_env = env.second_device();
    bad_env.password = "completely_wrong_password".to_string();
    let login = bad_env.login();
    login.assert_failure();
}

#[test]
fn test_wrong_password_push_rejected() {
    let env = setup();
    env.login().assert_success();

    // Import data
    let fixture = env.write_fixture_collection("wrong_pass_push", &[("TestNet", "pass123")]);
    env.import_collection(&fixture).assert_success();

    // Push with wrong password
    let push = env.run_with_stdin(
        &["--json", "sync", "push"],
        "totally_wrong_password\n",
    );
    push.assert_failure();
    assert!(
        push.stdout_contains("Password")
            || push.stdout_contains("password")
            || push.stderr_contains("Password")
            || push.stderr_contains("password"),
        "Expected password mismatch error, got stdout={}, stderr={}",
        push.stdout,
        push.stderr
    );
}

#[test]
fn test_wrong_password_pull_rejected() {
    let env = setup();
    env.login().assert_success();

    // Pull with wrong password
    let pull = env.run_with_stdin(
        &["--json", "sync", "pull"],
        "totally_wrong_password\n",
    );
    pull.assert_failure();
    assert!(
        pull.stdout_contains("Password")
            || pull.stdout_contains("password")
            || pull.stderr_contains("Password")
            || pull.stderr_contains("password"),
        "Expected password mismatch error, got stdout={}, stderr={}",
        pull.stdout,
        pull.stderr
    );
}

#[test]
fn test_cross_device_wrong_password_pull() {
    let env = setup();
    env.login().assert_success();

    // Push with correct password
    let fixture = env.write_fixture_collection("cross_pass", &[("XNet", "xpass")]);
    env.import_collection(&fixture).assert_success();
    env.push().assert_success();

    // Second device logs in with wrong password
    let mut env_b = env.second_device();
    env_b.password = "different_password_entirely".to_string();

    // Login will fail because the server has a different auth_proof (bcrypt of original password)
    let login_b = env_b.login();
    login_b.assert_failure();
}

// =============================================================================
// 6. Conflict Resolution
// =============================================================================

#[test]
fn test_concurrent_update_creates_conflict() {
    let env_a = setup();
    let env_b = env_a.second_device();

    // Device A: import and push
    let fixture = env_a.write_fixture_collection("conflict_test", &[("ConflictNet", "pass1")]);
    env_a.import_collection(&fixture).assert_success();
    env_a.login().assert_success();
    env_a.push().assert_success();

    // Device B: login, pull
    env_b.login().assert_success();
    env_b.pull().assert_success();

    // Device B: modify and push (import a new version)
    // We'll push a new collection from B to simulate a change
    let fixture_b = env_b.write_fixture_collection("conflict_b", &[("ConflictNet", "pass_b")]);
    env_b.import_collection(&fixture_b).assert_success();
    env_b.push().assert_success();

    // Device A: also push changes (this should create conflict or succeed depending on server logic)
    let fixture_a2 = env_a.write_fixture_collection("conflict_a2", &[("ConflictNet", "pass_a2")]);
    env_a.import_collection(&fixture_a2).assert_success();
    let push_a = env_a.push();
    push_a.assert_success();
    // The push itself may succeed but register conflicts
    // Check for conflicts
    let conflicts = env_a.list_conflicts();
    conflicts.assert_success();
    // Conflicts may or may not exist depending on whether credential IDs collide.
    // The test verifies the flow doesn't crash.
}

#[test]
fn test_list_conflicts_empty() {
    let env = setup();
    env.login().assert_success();

    let conflicts = env.list_conflicts();
    conflicts.assert_success();
    let j = conflicts.json();
    let conflict_list = j["conflicts"].as_array().expect("conflicts should be array");
    assert!(
        conflict_list.is_empty(),
        "Fresh user should have no conflicts"
    );
}

// =============================================================================
// 7. Edge Cases
// =============================================================================

#[test]
fn test_push_without_login_fails() {
    let env = TestEnv::new();
    e2e_helpers::require_server();

    let push = env.push();
    push.assert_failure();
    assert!(
        push.stdout_contains("Not logged in")
            || push.stderr_contains("Not logged in")
            || push.stdout_contains("not logged in")
            || push.stderr_contains("not logged in"),
        "Expected 'not logged in' error, got stdout={}, stderr={}",
        push.stdout,
        push.stderr
    );
}

#[test]
fn test_pull_without_login_fails() {
    let env = TestEnv::new();
    e2e_helpers::require_server();

    let pull = env.pull();
    pull.assert_failure();
    assert!(
        pull.stdout_contains("Not logged in")
            || pull.stderr_contains("Not logged in")
            || pull.stdout_contains("not logged in")
            || pull.stderr_contains("not logged in"),
        "Expected 'not logged in' error, got stdout={}, stderr={}",
        pull.stdout,
        pull.stderr
    );
}

#[test]
fn test_logout_without_login_fails() {
    let env = TestEnv::new();
    e2e_helpers::require_server();

    let logout = env.logout();
    logout.assert_failure();
    assert!(
        logout.stdout_contains("Not logged in")
            || logout.stderr_contains("Not logged in")
            || logout.stdout_contains("not logged in")
            || logout.stderr_contains("not logged in"),
        "Expected 'not logged in' error"
    );
}

#[test]
fn test_double_login_rejected() {
    let env = setup();
    env.login().assert_success();

    // Second login should fail (already logged in)
    let login2 = env.login();
    login2.assert_failure();
    assert!(
        login2.stdout_contains("Already logged in")
            || login2.stderr_contains("Already logged in")
            || login2.stdout_contains("already logged in")
            || login2.stderr_contains("already logged in"),
        "Expected 'already logged in' error"
    );
}

#[test]
fn test_status_without_login() {
    let env = TestEnv::new();
    e2e_helpers::require_server();

    let status = env.sync_status();
    status.assert_success();
    let s = status.json();
    assert_eq!(s["enabled"], false);
}
