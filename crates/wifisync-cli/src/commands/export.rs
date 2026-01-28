//! Export command implementation
//!
//! Exports a credential collection to a file, optionally encrypted.

use std::path::Path;

use anyhow::{Context, Result};
use console::style;
use wifisync_core::storage::Storage;

pub async fn run(name: &str, output: &Path, encrypt: bool, json: bool) -> Result<()> {
    let storage = Storage::new().context("Failed to initialize storage")?;

    let collection = storage.load_collection(name)
        .context(format!("Collection '{}' not found", name))?;

    if collection.is_empty() {
        anyhow::bail!("Collection '{}' is empty, nothing to export", name);
    }

    let password = if encrypt {
        // Read password from terminal
        let pass = rpassword_prompt("Encryption password: ")?;
        let confirm = rpassword_prompt("Confirm password: ")?;
        if pass != confirm {
            anyhow::bail!("Passwords do not match");
        }
        Some(pass)
    } else {
        None
    };

    storage.export_collection(&collection, output, password.as_deref())
        .context("Failed to export collection")?;

    let actual_path = if encrypt {
        output.with_extension("json.enc")
    } else {
        output.to_path_buf()
    };

    if json {
        let output_json = serde_json::json!({
            "collection": name,
            "credentials": collection.credentials.len(),
            "encrypted": encrypt,
            "path": actual_path.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&output_json)?);
    } else {
        println!(
            "{} Exported '{}' ({} credentials) to {}",
            style("OK").green().bold(),
            name,
            collection.credentials.len(),
            actual_path.display()
        );
        if encrypt {
            println!(
                "  {} File is encrypted with ChaCha20-Poly1305",
                style("->").dim()
            );
        } else {
            println!(
                "  {} File is NOT encrypted. Passwords are in plaintext!",
                style("Warning:").yellow()
            );
        }
    }

    Ok(())
}

/// Prompt for a password without echoing
fn rpassword_prompt(prompt: &str) -> Result<String> {
    eprint!("{}", prompt);
    let mut password = String::new();
    std::io::stdin().read_line(&mut password)?;
    password = password.trim_end().to_string();
    if password.is_empty() {
        anyhow::bail!("Password cannot be empty");
    }
    Ok(password)
}
