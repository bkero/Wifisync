//! Show command implementation

use anyhow::Result;
use console::style;
use secrecy::ExposeSecret;
use wifisync_core::adapter::detect_adapter;

pub async fn run(ssid: &str, show_password: bool, json: bool) -> Result<()> {
    let adapter = detect_adapter().await?;
    let credential = adapter.get_credentials(ssid).await?;

    if json {
        let mut output = serde_json::json!({
            "ssid": credential.ssid,
            "security_type": format!("{:?}", credential.security_type),
            "hidden": credential.hidden,
            "source_platform": format!("{:?}", credential.source_platform),
            "managed": credential.managed,
            "system_id": credential.system_id,
            "created_at": credential.created_at.to_rfc3339(),
            "tags": credential.tags,
        });

        if show_password {
            output["password"] = serde_json::Value::String(
                credential.password.expose_secret().to_string(),
            );
        }

        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}: {}", style("SSID").bold(), credential.ssid);
        println!(
            "{}: {:?}",
            style("Security").bold(),
            credential.security_type
        );
        println!("{}: {}", style("Hidden").bold(), credential.hidden);
        println!(
            "{}: {}",
            style("Source").bold(),
            credential.source_platform
        );
        println!("{}: {}", style("Managed").bold(), credential.managed);

        if let Some(system_id) = &credential.system_id {
            println!("{}: {}", style("System ID").bold(), system_id);
        }

        if !credential.tags.is_empty() {
            println!("{}: {}", style("Tags").bold(), credential.tags.join(", "));
        }

        if show_password {
            println!();
            println!(
                "{}: {}",
                style("Password").bold().yellow(),
                credential.password.expose_secret()
            );
        } else {
            println!();
            println!(
                "{}",
                style("Use --show-password to reveal the password").dim()
            );
        }
    }

    Ok(())
}
