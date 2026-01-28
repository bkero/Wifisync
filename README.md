# Wifisync

Sync wifi credentials between devices. Wifisync extracts wifi passwords from
NetworkManager, stores them in an encrypted local database, and provides them
back to NetworkManager on-demand via a Secret Agent daemon.

## How It Works

1. **Extract** wifi credentials from your system's NetworkManager
2. **Store** them locally, encrypted with ChaCha20-Poly1305
3. **Export/Import** credential collections between devices
4. **Install** profiles to NetworkManager without storing passwords in the system
5. **Secret Agent** daemon provides passwords on-demand when connecting

Installed network profiles use `psk-flags=1` (agent-owned secrets), which tells
NetworkManager to ask a registered Secret Agent for the password instead of
storing it in the connection file. This keeps passwords out of
`/etc/NetworkManager/system-connections/`.

## Installation

### From source

```bash
cargo build --release
sudo install -Dm755 target/release/wifisync /usr/bin/wifisync
sudo install -Dm644 packaging/systemd/wifisync-agent.service \
    /usr/lib/systemd/user/wifisync-agent.service
```

### RPM (Fedora)

```bash
./packaging/build-rpm.sh
sudo dnf install dist/wifisync-*.rpm
```

### DEB (Ubuntu/Debian)

```bash
./packaging/build-deb.sh
sudo dpkg -i dist/wifisync_*.deb
```

Both package builds use Docker and produce the package in `dist/`.

## Quick Start

```bash
# List wifi networks on your system
wifisync list

# Create a collection and add networks
wifisync collection create "My Networks"
wifisync collection add "My Networks" "HomeWifi"
wifisync collection add "My Networks" "OfficeWifi"

# Export for transfer to another device
wifisync export "My Networks" -o my-networks.json
wifisync export "My Networks" -o my-networks.json -p   # encrypted

# On the other device: import and install
wifisync import my-networks.json
wifisync install "HomeWifi"
wifisync install "OfficeWifi"

# Start the Secret Agent daemon (required for connections to work)
wifisync agent start

# Or enable it as a systemd user service (starts on login)
systemctl --user enable --now wifisync-agent
```

## Commands

| Command | Description |
|---------|-------------|
| `wifisync list [--syncable]` | List saved wifi networks |
| `wifisync show <ssid> [--show-password]` | Show network details |
| `wifisync collection create <name>` | Create a credential collection |
| `wifisync collection add <name> <ssid>` | Add a network to a collection |
| `wifisync collection remove <name> <ssid>` | Remove a network from a collection |
| `wifisync collection list` | List all collections |
| `wifisync collection delete <name> --yes` | Delete a collection |
| `wifisync export <name> -o <path> [-p]` | Export collection to file |
| `wifisync import <path> [-p <pass>] [--install]` | Import collection from file |
| `wifisync install <ssid>` | Install profile to system (no password stored) |
| `wifisync uninstall <ssid>` | Remove profile from system |
| `wifisync uninstall --all --yes` | Remove all managed profiles |
| `wifisync status` | Show sync status and agent health |
| `wifisync exclude add <pattern>` | Exclude an SSID or glob pattern |
| `wifisync exclude list` | List exclusions |
| `wifisync exclude remove <pattern>` | Remove an exclusion |
| `wifisync agent start` | Start the Secret Agent daemon |
| `wifisync agent status` | Check if the agent is running |

All commands support `--json` for machine-readable output and `-v` for debug logging.

## Secret Agent Daemon

The Secret Agent daemon is required for installed profiles to connect. It
registers with NetworkManager's AgentManager on the system D-Bus and responds
to `GetSecrets()` calls with passwords from Wifisync's encrypted storage.

### systemd user service

```bash
# Enable and start (persists across logins)
systemctl --user enable --now wifisync-agent

# Check status
systemctl --user status wifisync-agent

# View logs
journalctl --user -u wifisync-agent -f
```

### Manual

```bash
# Run in foreground (Ctrl+C to stop)
wifisync agent start

# Check status
wifisync agent status
```

## Architecture

```
wifisync list/show/export/import     wifisync install/uninstall
         |                                    |
         v                                    v
  +----------------+              +-------------------+
  | NetworkManager |              | ProfileManager    |
  | Adapter        |              | (create/delete    |
  | (D-Bus)        |              |  profiles w/o     |
  +----------------+              |  passwords)       |
         |                        +-------------------+
         v                                    |
  +----------------+              +-----------v-------+
  | Credential     |              | NetworkManager    |
  | Storage        |              | (psk-flags=1)     |
  | (encrypted     |              +-------------------+
  |  JSON + Argon2 |                        |
  |  + ChaCha20)   |              +---------v---------+
  +----------------+              | Secret Agent      |
                                  | Daemon (D-Bus)    |
                                  | provides passwords|
                                  | from storage      |
                                  +-------------------+
```

### Crate structure

- **wifisync-core** - Core library: models, storage, crypto, adapters, agent
- **wifisync-cli** - CLI binary with all commands

### Data storage

| File | Location | Purpose |
|------|----------|---------|
| `collections.json` | `~/.local/share/wifisync/` | Encrypted credentials organized in collections |
| `profiles.json` | `~/.local/share/wifisync/` | Tracking records for installed system profiles |
| `exclusions.json` | `~/.config/wifisync/` | SSIDs/patterns to exclude from sync |
| `agent.pid` | `~/.local/share/wifisync/` | PID file for the running agent daemon |

## Filtering

Networks are filtered before sync operations:

- **Enterprise networks** (802.1X) are excluded - they use certificates, not PSK passwords
- **Open networks** (no password) are excluded - nothing to sync
- **Exclusion list** - manually exclude SSIDs or glob patterns (e.g., `Hotel*`)

## Security

- Passwords stored with ChaCha20-Poly1305 (AEAD)
- Key derivation via Argon2id (16 MB, 3 iterations)
- Passwords never stored in NetworkManager connection files
- Agent daemon provides passwords only to NetworkManager via D-Bus
- systemd service hardened with `NoNewPrivileges`, `ProtectSystem=strict`, `PrivateTmp`
- No unsafe code (`#![forbid(unsafe_code)]`)

## Building

Requires Rust 1.75+ and `libdbus-1-dev` (Debian) or `dbus-devel` (Fedora).

```bash
cargo build --release
cargo test --release
```

### Docker package builds

```bash
./packaging/build.sh rpm    # Fedora RPM
./packaging/build.sh deb    # Ubuntu DEB
./packaging/build.sh all    # Both
./packaging/build.sh clean  # Remove build artifacts
```

## License

MIT OR Apache-2.0
