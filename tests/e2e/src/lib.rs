//! Shared helpers for E2E tests.
//!
//! Provides CLI invocation, temp directory management, unique user generation,
//! and JSON output parsing for black-box testing of the wifisync binary.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;
use uuid::Uuid;

/// Result of running a CLI command.
#[derive(Debug)]
pub struct CliResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CliResult {
    /// Assert that the command exited with code 0.
    pub fn assert_success(&self) {
        assert_eq!(
            self.exit_code, 0,
            "Expected exit code 0, got {}.\nstdout: {}\nstderr: {}",
            self.exit_code, self.stdout, self.stderr
        );
    }

    /// Assert that the command exited with a non-zero code.
    pub fn assert_failure(&self) {
        assert_ne!(
            self.exit_code, 0,
            "Expected non-zero exit code, got 0.\nstdout: {}\nstderr: {}",
            self.stdout, self.stderr
        );
    }

    /// Parse stdout as JSON.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|e| {
            panic!(
                "Failed to parse stdout as JSON: {e}\nstdout: {}\nstderr: {}",
                self.stdout, self.stderr
            )
        })
    }

    /// Check whether stdout contains a substring.
    pub fn stdout_contains(&self, needle: &str) -> bool {
        self.stdout.contains(needle)
    }

    /// Check whether stderr contains a substring.
    pub fn stderr_contains(&self, needle: &str) -> bool {
        self.stderr.contains(needle)
    }
}

/// An isolated E2E test environment with its own data/config directories
/// and a unique user account.
pub struct TestEnv {
    /// Temporary directory that holds data_dir and config_dir.
    /// Dropped at end of test → automatic cleanup.
    pub temp_dir: TempDir,

    /// Path to the wifisync CLI binary.
    pub cli_binary: PathBuf,

    /// Base URL of the E2E server (e.g. `http://localhost:18080`).
    pub server_url: String,

    /// Unique username for this test.
    pub username: String,

    /// Password for this test user.
    pub password: String,

    /// Extra environment variables to inject into every CLI call.
    pub extra_env: HashMap<String, String>,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl TestEnv {
    /// Create a new isolated test environment.
    ///
    /// Reads `E2E_SERVER_URL` and `E2E_CLI_BINARY` from the process environment
    /// (set by the orchestrator script).
    pub fn new() -> Self {
        let server_url =
            std::env::var("E2E_SERVER_URL").unwrap_or_else(|_| "http://localhost:18080".into());
        let cli_binary = std::env::var("E2E_CLI_BINARY").unwrap_or_else(|_| {
            // Fall back to looking in target/release
            let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            p.push("target/release/wifisync");
            p.to_string_lossy().into()
        });

        let temp_dir = TempDir::new().expect("Failed to create temp dir for E2E test");
        let uid = Uuid::new_v4().to_string();
        let short_uid = &uid[..8];

        Self {
            temp_dir,
            cli_binary: PathBuf::from(cli_binary),
            server_url,
            username: format!("e2e_{short_uid}"),
            password: format!("e2e_pass_{short_uid}"),
            extra_env: HashMap::new(),
        }
    }

    /// Create a second TestEnv sharing the same server and user credentials
    /// but with a separate data directory (simulates a different device).
    pub fn second_device(&self) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir for second device");
        Self {
            temp_dir,
            cli_binary: self.cli_binary.clone(),
            server_url: self.server_url.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            extra_env: self.extra_env.clone(),
        }
    }

    /// XDG_DATA_HOME for this environment (data lives under temp_dir/data).
    pub fn data_home(&self) -> PathBuf {
        self.temp_dir.path().join("data")
    }

    /// XDG_CONFIG_HOME for this environment (config lives under temp_dir/config).
    pub fn config_home(&self) -> PathBuf {
        self.temp_dir.path().join("config")
    }

    /// Run a CLI command with the given arguments.
    ///
    /// Automatically sets `XDG_DATA_HOME` and `XDG_CONFIG_HOME` for isolation,
    /// and adds `--json` for machine-readable output.
    pub fn run(&self, args: &[&str]) -> CliResult {
        self.run_inner(args, None)
    }

    /// Run a CLI command, piping `stdin_data` to its stdin.
    pub fn run_with_stdin(&self, args: &[&str], stdin_data: &str) -> CliResult {
        self.run_inner(args, Some(stdin_data))
    }

    fn run_inner(&self, args: &[&str], stdin_data: Option<&str>) -> CliResult {
        let mut cmd = Command::new(&self.cli_binary);
        cmd.args(args)
            .env("XDG_DATA_HOME", self.data_home())
            .env("XDG_CONFIG_HOME", self.config_home())
            // Suppress interactive prompts
            .env("WIFISYNC_E2E_TEST", "1");

        for (k, v) in &self.extra_env {
            cmd.env(k, v);
        }

        if let Some(data) = stdin_data {
            use std::io::Write;
            use std::process::Stdio;

            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = cmd.spawn().expect("Failed to spawn CLI process");
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(data.as_bytes())
                    .expect("Failed to write to stdin");
            }
            let output = child.wait_with_output().expect("Failed to wait for CLI process");
            return to_cli_result(output);
        }

        let output = cmd.output().expect("Failed to execute CLI binary");
        to_cli_result(output)
    }

    // ── Convenience wrappers ─────────────────────────────────────────────

    /// Login this test user to the E2E server.
    pub fn login(&self) -> CliResult {
        self.run_with_stdin(
            &["--json", "sync", "login", &self.server_url, &self.username],
            &format!("{}\n", self.password),
        )
    }

    /// Logout the current user.
    pub fn logout(&self) -> CliResult {
        self.run(&["--json", "sync", "logout"])
    }

    /// Show sync status.
    pub fn sync_status(&self) -> CliResult {
        self.run(&["--json", "sync", "status"])
    }

    /// Push local changes, providing the password on stdin.
    pub fn push(&self) -> CliResult {
        self.run_with_stdin(
            &["--json", "sync", "push"],
            &format!("{}\n", self.password),
        )
    }

    /// Pull remote changes, providing the password on stdin.
    pub fn pull(&self) -> CliResult {
        self.run_with_stdin(
            &["--json", "sync", "pull"],
            &format!("{}\n", self.password),
        )
    }

    /// Create a collection.
    pub fn collection_create(&self, name: &str) -> CliResult {
        self.run(&["--json", "collection", "create", name])
    }

    /// Create a collection with a description.
    pub fn collection_create_with_desc(&self, name: &str, desc: &str) -> CliResult {
        self.run(&["--json", "collection", "create", name, "-d", desc])
    }

    /// List collections.
    pub fn collection_list(&self) -> CliResult {
        self.run(&["--json", "collection", "list"])
    }

    /// Show a collection.
    pub fn collection_show(&self, name: &str) -> CliResult {
        self.run(&["--json", "collection", "show", name])
    }

    /// Delete a collection (with --yes to skip confirmation).
    pub fn collection_delete(&self, name: &str) -> CliResult {
        self.run(&["--json", "collection", "delete", name, "--yes"])
    }

    /// Import a collection from a file.
    pub fn import_collection(&self, path: &Path) -> CliResult {
        self.run(&["--json", "import", &path.to_string_lossy()])
    }

    /// List sync conflicts.
    pub fn list_conflicts(&self) -> CliResult {
        self.run(&["--json", "sync", "conflicts"])
    }

    /// Resolve a conflict.
    pub fn resolve_conflict(&self, id: &str, strategy: &str) -> CliResult {
        self.run(&["--json", "sync", "resolve", id, strategy])
    }

    // ── Fixture helpers ──────────────────────────────────────────────────

    /// Write a JSON export file that can be imported with `wifisync import`.
    ///
    /// Returns the path to the created file.
    pub fn write_fixture_collection(
        &self,
        name: &str,
        credentials: &[(&str, &str)], // (ssid, password)
    ) -> PathBuf {
        let coll_id = Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339();

        let creds: Vec<Value> = credentials
            .iter()
            .map(|(ssid, pass)| {
                serde_json::json!({
                    "id": Uuid::new_v4().to_string(),
                    "ssid": ssid,
                    "security_type": "wpa2_psk",
                    "password": pass,
                    "hidden": false,
                    "source_platform": "manual",
                    "created_at": &now,
                    "tags": [],
                    "managed": false
                })
            })
            .collect();

        let export = serde_json::json!({
            "version": "1.0",
            "created_by": "wifisync-e2e-test",
            "created_at": &now,
            "collection": {
                "id": coll_id.to_string(),
                "name": name,
                "description": null,
                "credentials": creds,
                "is_shared": false,
                "created_at": &now,
                "updated_at": &now
            }
        });

        let fixture_path = self.temp_dir.path().join(format!("{name}.json"));
        std::fs::write(&fixture_path, serde_json::to_string_pretty(&export).unwrap())
            .expect("Failed to write fixture file");
        fixture_path
    }
}

fn to_cli_result(output: Output) -> CliResult {
    CliResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Wait for a URL to return HTTP 200, with a timeout.
pub fn wait_for_server(url: &str, timeout_secs: u64) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < timeout_secs {
        if reqwest::blocking::get(url)
            .map(|r| r.status().is_success())
            .unwrap_or(false)
        {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    false
}

/// Check whether the E2E server is reachable. Panics with a helpful
/// message if not.
pub fn require_server() {
    let url = std::env::var("E2E_SERVER_URL").unwrap_or_else(|_| "http://localhost:18080".into());
    let health = format!("{url}/health");
    assert!(
        wait_for_server(&health, 5),
        "E2E server not reachable at {url}. Start it with: ./tests/e2e/run-e2e.sh\n\
         Or manually: docker compose -f docker-compose.yml -f tests/e2e/docker-compose.e2e.yml up -d"
    );
}
