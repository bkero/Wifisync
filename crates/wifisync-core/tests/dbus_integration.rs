//! D-Bus connectivity integration tests
//!
//! These tests verify basic D-Bus connectivity without requiring NetworkManager.
//! Tests will be skipped when D-Bus is unavailable.

mod common;

use zbus::Connection;

/// Test that session bus connection succeeds when available
#[tokio::test]
async fn test_session_bus_connection() {
    skip_if_no_dbus_session!();

    let result = Connection::session().await;
    assert!(
        result.is_ok(),
        "Session bus connection should succeed: {:?}",
        result.err()
    );

    let conn = result.unwrap();
    // Verify we can get bus names (basic connectivity check)
    let unique_name = conn.unique_name();
    assert!(
        unique_name.is_some(),
        "Should have a unique name on the bus"
    );
}

/// Test that system bus connection can be attempted
///
/// Note: May require appropriate permissions on some systems
#[tokio::test]
async fn test_system_bus_connection() {
    skip_if_no_dbus_system!();

    let result = Connection::system().await;

    // On most systems this should succeed, but may fail without permissions
    match result {
        Ok(conn) => {
            let unique_name = conn.unique_name();
            assert!(
                unique_name.is_some(),
                "Should have a unique name on the bus"
            );
        }
        Err(e) => {
            // Connection failure is acceptable in restricted environments
            eprintln!("System bus connection failed (may be expected): {}", e);
        }
    }
}

/// Test graceful failure when D-Bus is unavailable
///
/// This test verifies that zbus returns a proper error when connection fails.
/// We use a definitely-invalid socket path to test error handling.
#[tokio::test]
async fn test_dbus_graceful_failure() {
    // Instead of modifying the global environment (which affects parallel tests),
    // we test by attempting to connect to a known-invalid address using zbus's
    // builder API.

    use zbus::connection::Builder;

    // Try to connect to a non-existent socket
    let result = Builder::address("unix:path=/nonexistent/socket/that/does/not/exist")
        .expect("Should parse address")
        .build()
        .await;

    // Should fail gracefully (not panic)
    assert!(
        result.is_err(),
        "Connection to invalid socket should fail"
    );

    // Verify the error is reasonable
    let err = result.unwrap_err();
    let err_str = format!("{}", err);
    // Should mention something about the connection failing
    assert!(
        err_str.contains("No such file") || err_str.contains("Connection") || err_str.contains("Input"),
        "Error should indicate connection failure: {}",
        err_str
    );
}

/// Test that we can introspect the session bus
#[tokio::test]
async fn test_session_bus_introspection() {
    skip_if_no_dbus_session!();

    let conn = Connection::session()
        .await
        .expect("Session bus should be available");

    // Try to get the bus name - a basic D-Bus operation
    let proxy = zbus::fdo::DBusProxy::new(&conn)
        .await
        .expect("Should create DBus proxy");

    // ListNames is a standard D-Bus method
    let names = proxy.list_names().await;
    assert!(
        names.is_ok(),
        "Should be able to list bus names: {:?}",
        names.err()
    );

    let names = names.unwrap();
    // Should at least have org.freedesktop.DBus
    assert!(
        names
            .iter()
            .any(|n| n.as_str() == "org.freedesktop.DBus"),
        "Should find org.freedesktop.DBus in the list"
    );
}
