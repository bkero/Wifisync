# Wifisync Project Conventions

## Overview

Wifisync enables users to sync saved Wifi credentials between devices and share them with other users across platforms.

## Technology Choices

- **Primary Language**: Rust (memory safety, performance, cross-platform)
- **Data Format**: JSON via serde (portable, human-readable)
- **Configuration**: TOML via toml crate (user-friendly config files)
- **Async Runtime**: tokio (for D-Bus and network operations)

## Platform Support

| Platform | Network Manager | Priority |
|----------|-----------------|----------|
| Linux | NetworkManager (via zbus D-Bus) | P0 - Initial |
| Android | JNI to WifiManager API | P1 |
| Windows | windows-rs WLAN API | P2 |
| iOS | Swift bridge to NEHotspotConfiguration | P3 |

## Code Conventions

- Use `rustfmt` for formatting
- Use `clippy` for linting with pedantic warnings
- Prefer composition over inheritance (traits over inheritance)
- Use trait objects for platform adapters
- Error handling via `thiserror` and `anyhow`
- No `unwrap()` in library code; use proper error propagation

## Security Considerations

- Credentials stored locally are encrypted at rest
- Shared credentials use end-to-end encryption
- Personal network exclusions are respected absolutely
- No plaintext passwords in logs or debug output

## File Locations

- Linux config: `~/.config/wifisync/`
- Linux data: `~/.local/share/wifisync/`
