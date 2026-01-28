//! Exclude command implementation

use anyhow::Result;
use console::style;
use wifisync_core::storage::Storage;

pub async fn list(json: bool) -> Result<()> {
    let storage = Storage::new()?;
    let exclusions = storage.load_exclusions()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&exclusions)?);
    } else {
        if exclusions.is_empty() {
            println!("{}", style("No exclusions configured.").dim());
            println!();
            println!("Use {} to add exclusions", style("wifisync exclude add <pattern>").cyan());
        } else {
            println!("{}", style("Exclusion List:").bold().underlined());
            for exclusion in &exclusions {
                let is_pattern = exclusion.contains('*') || exclusion.contains('?');
                if is_pattern {
                    println!("  {} {}", style("•").dim(), style(exclusion).yellow());
                } else {
                    println!("  {} {}", style("•").dim(), exclusion);
                }
            }
            println!();
            println!("{} {} exclusions", style("Total:").bold(), exclusions.len());
        }
    }

    Ok(())
}

pub async fn add(pattern: &str, json: bool) -> Result<()> {
    let storage = Storage::new()?;
    let added = storage.add_exclusion(pattern)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "pattern": pattern,
                "added": added
            })
        );
    } else if added {
        println!(
            "{} Added '{}' to exclusion list",
            style("✓").green(),
            pattern
        );
    } else {
        println!(
            "{} '{}' is already in the exclusion list",
            style("!").yellow(),
            pattern
        );
    }

    Ok(())
}

pub async fn remove(pattern: &str, json: bool) -> Result<()> {
    let storage = Storage::new()?;
    let removed = storage.remove_exclusion(pattern)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "pattern": pattern,
                "removed": removed
            })
        );
    } else if removed {
        println!(
            "{} Removed '{}' from exclusion list",
            style("✓").green(),
            pattern
        );
    } else {
        println!(
            "{} '{}' was not in the exclusion list",
            style("!").yellow(),
            pattern
        );
    }

    Ok(())
}
