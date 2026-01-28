//! List command implementation

use anyhow::Result;
use console::style;
use wifisync_core::adapter::{detect_adapter, NetworkInfo};
use wifisync_core::filter::FilterStats;
use wifisync_core::SecurityType;

pub async fn run(json: bool, syncable: bool) -> Result<()> {
    let adapter = detect_adapter().await?;
    let networks = adapter.list_networks().await?;

    let (display_networks, stats): (Vec<&NetworkInfo>, Option<FilterStats>) = if syncable {
        // Filter to only syncable networks
        let syncable_networks: Vec<&NetworkInfo> = networks
            .iter()
            .filter(|n| n.security_type.is_syncable())
            .collect();

        let stats = FilterStats {
            total: networks.len(),
            passed: syncable_networks.len(),
            ..Default::default()
        };

        (syncable_networks, Some(stats))
    } else {
        (networks.iter().collect(), None)
    };

    if json {
        let output: Vec<_> = display_networks
            .iter()
            .map(|n| {
                serde_json::json!({
                    "ssid": n.ssid,
                    "security_type": format!("{:?}", n.security_type),
                    "hidden": n.hidden,
                    "system_id": n.system_id,
                    "syncable": n.security_type.is_syncable(),
                })
            })
            .collect();

        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        let platform_info = adapter.platform_info();
        println!(
            "{} {} {}",
            style("Platform:").bold(),
            platform_info.name,
            platform_info.version.as_deref().unwrap_or("(unknown version)")
        );
        println!();

        if display_networks.is_empty() {
            println!("{}", style("No networks found.").dim());
        } else {
            println!(
                "{:<30} {:<15} {:<8} {}",
                style("SSID").bold().underlined(),
                style("Security").bold().underlined(),
                style("Hidden").bold().underlined(),
                style("Syncable").bold().underlined()
            );

            for network in &display_networks {
                let security_str = format_security_type(network.security_type);
                let hidden_str = if network.hidden { "yes" } else { "no" };
                let syncable_str = if network.security_type.is_syncable() {
                    style("✓").green().to_string()
                } else {
                    style("✗").red().to_string()
                };

                println!(
                    "{:<30} {:<15} {:<8} {}",
                    network.ssid, security_str, hidden_str, syncable_str
                );
            }
        }

        println!();
        println!(
            "{} {} networks",
            style("Total:").bold(),
            display_networks.len()
        );

        if let Some(stats) = stats {
            if stats.excluded() > 0 {
                println!(
                    "{} {} networks (enterprise/open)",
                    style("Excluded:").dim(),
                    stats.excluded()
                );
            }
        }
    }

    Ok(())
}

fn format_security_type(security: SecurityType) -> String {
    match security {
        SecurityType::Open => "Open".to_string(),
        SecurityType::Wep => "WEP".to_string(),
        SecurityType::WpaPsk => "WPA".to_string(),
        SecurityType::Wpa2Psk => "WPA2".to_string(),
        SecurityType::Wpa3Psk => "WPA3".to_string(),
        SecurityType::WpaWpa2Psk => "WPA/WPA2".to_string(),
        SecurityType::Wpa2Wpa3Psk => "WPA2/WPA3".to_string(),
        SecurityType::WpaEnterprise => "WPA-Ent".to_string(),
        SecurityType::Wpa2Enterprise => "WPA2-Ent".to_string(),
        SecurityType::Wpa3Enterprise => "WPA3-Ent".to_string(),
        SecurityType::Unknown => "Unknown".to_string(),
    }
}
