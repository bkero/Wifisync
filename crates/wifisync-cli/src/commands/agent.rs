//! Agent command - manage the Secret Agent daemon
//!
//! The Secret Agent daemon provides wifi passwords to NetworkManager on-demand
//! via the D-Bus Secret Agent API.

use anyhow::{Context, Result};
use console::style;

/// Run the Secret Agent daemon in the foreground
///
/// This is intended to be invoked by systemd as a user service.
/// The daemon blocks until SIGINT or SIGTERM is received.
pub async fn start(json: bool) -> Result<()> {
    let storage = wifisync_core::storage::Storage::new()
        .context("Failed to initialize storage")?;

    if !json {
        eprintln!(
            "{} Starting Wifisync Secret Agent daemon...",
            style("Agent:").cyan().bold()
        );
        eprintln!(
            "  {} Providing wifi passwords to NetworkManager on-demand",
            style("->").dim()
        );
        eprintln!(
            "  {} Press Ctrl+C or send SIGTERM to stop",
            style("->").dim()
        );
    }

    wifisync_core::AgentService::run(storage)
        .await
        .context("Secret Agent daemon failed")?;

    if json {
        let output = serde_json::json!({
            "status": "stopped",
            "message": "Secret Agent daemon stopped"
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        eprintln!(
            "{} Secret Agent daemon stopped",
            style("Agent:").cyan().bold()
        );
    }

    Ok(())
}

/// Show the status of the Secret Agent daemon
pub async fn status(json: bool) -> Result<()> {
    let config = wifisync_core::storage::StorageConfig::default_paths()
        .context("Failed to determine storage paths")?;

    let agent_status = wifisync_core::AgentService::status(&config.data_dir);

    if json {
        let output = match &agent_status {
            Some(s) => serde_json::json!({
                "pid_file_exists": true,
                "pid": s.pid,
                "running": s.running,
            }),
            None => serde_json::json!({
                "pid_file_exists": false,
                "running": false,
            }),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        match &agent_status {
            Some(s) if s.running => {
                println!(
                    "{} {} (PID {})",
                    style("Agent:").cyan().bold(),
                    style("running").green().bold(),
                    s.pid
                );
            }
            Some(s) => {
                println!(
                    "{} {} (stale PID file for PID {})",
                    style("Agent:").cyan().bold(),
                    style("not running").red().bold(),
                    s.pid
                );
                println!(
                    "  {} The agent may have crashed. Start it with: wifisync agent start",
                    style("->").dim()
                );
            }
            None => {
                println!(
                    "{} {}",
                    style("Agent:").cyan().bold(),
                    style("not running").yellow().bold(),
                );
                println!(
                    "  {} Start with: wifisync agent start",
                    style("->").dim()
                );
                println!(
                    "  {} Or enable the systemd service: systemctl --user enable --now wifisync-agent",
                    style("->").dim()
                );
            }
        }
    }

    Ok(())
}
