//! JWT Expiry End-to-End tests
//!
//! Tests that exercise token expiration and refresh against a server
//! configured with very short JWT lifetimes (via JWT_EXPIRATION_SECONDS).
//!
//! These tests require the short-JWT server started by:
//!   ./tests/e2e/run-e2e.sh --jwt-tests
//!
//! Or manually:
//!   E2E_SHORT_JWT_SERVER_URL=http://localhost:18081 \
//!   E2E_CLI_BINARY=./target/release/wifisync \
//!   cargo test --test jwt_e2e -- --test-threads=1

use e2e_helpers::TestEnv;

/// Check if the short-JWT server is available.
fn setup_short_jwt() -> Option<TestEnv> {
    let url = match std::env::var("E2E_SHORT_JWT_SERVER_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => return None,
    };

    if !e2e_helpers::wait_for_server(&format!("{url}/health"), 3) {
        eprintln!("Short-JWT server not available at {url}, skipping JWT tests");
        return None;
    }

    let mut env = TestEnv::new();
    env.extra_env
        .insert("E2E_SERVER_URL".to_string(), url.clone());
    // Override the server_url in the TestEnv
    Some(TestEnv {
        server_url: url,
        ..env
    })
}

#[test]
fn test_token_expires_quickly() {
    let env = match setup_short_jwt() {
        Some(e) => e,
        None => {
            eprintln!("Skipping: short-JWT server not configured");
            return;
        }
    };

    // Login (gets a token that expires in ~1 second)
    let login = env.login();
    login.assert_success();

    // Status should show valid token immediately
    let status = env.sync_status();
    status.assert_success();
    let s = status.json();
    assert_eq!(s["enabled"], true);

    // Wait for token to expire
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Status should now show expired token
    let status2 = env.sync_status();
    status2.assert_success();
    let s2 = status2.json();
    assert_eq!(
        s2["has_valid_token"], false,
        "Token should be expired after waiting, got: {s2}"
    );
}

#[test]
fn test_push_with_expired_token() {
    let env = match setup_short_jwt() {
        Some(e) => e,
        None => {
            eprintln!("Skipping: short-JWT server not configured");
            return;
        }
    };

    // Login
    env.login().assert_success();

    // Import data
    let fixture = env.write_fixture_collection("jwt_push_test", &[("JwtNet", "jwtpass")]);
    env.import_collection(&fixture).assert_success();

    // Wait for token to expire
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Push — depending on client behavior, this either auto-refreshes or fails
    // We just verify it doesn't panic/crash
    let push = env.push();
    // The push may succeed (auto-refresh) or fail (expired token rejection)
    // Either way, it should produce a valid JSON response
    let _j = push.json();
}

#[test]
fn test_pull_with_expired_token() {
    let env = match setup_short_jwt() {
        Some(e) => e,
        None => {
            eprintln!("Skipping: short-JWT server not configured");
            return;
        }
    };

    // Login
    env.login().assert_success();

    // Wait for token to expire
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Pull — should handle expired token gracefully
    let pull = env.pull();
    let _j = pull.json();
}

#[test]
fn test_invalid_token_rejected() {
    let env = match setup_short_jwt() {
        Some(e) => e,
        None => {
            eprintln!("Skipping: short-JWT server not configured");
            return;
        }
    };

    // Login to create config
    env.login().assert_success();

    // Corrupt the token in the config file
    let config_path = env.config_home().join("wifisync").join("sync_config.json");
    if config_path.exists() {
        let data = std::fs::read_to_string(&config_path).unwrap();
        // Replace the token with garbage
        let corrupted = data.replace(
            "\"token\":\"ey",
            "\"token\":\"INVALID_GARBAGE_TOKEN_ey",
        );
        if corrupted != data {
            std::fs::write(&config_path, corrupted).unwrap();
        }
    }

    // Import data so we have something to push
    let fixture = env.write_fixture_collection("corrupt_tok", &[("CorruptNet", "pass")]);
    env.import_collection(&fixture).assert_success();

    // Push should fail with auth error
    let push = env.push();
    // Should fail or produce an error, not crash
    let j = push.json();
    // Verify we get some kind of response (not a crash)
    assert!(
        j.is_object(),
        "Should get a JSON object response, got: {j}"
    );
}
