# DeckSaves - Game Save Synchronization Tool

A cross-platform game save synchronization tool with support for cloud storage (S3) and peer-to-peer sync. Built in Rust with a modular architecture supporting CLI and optional Tauri-based GUI.

## Features

- **Real-time File Watching**: Monitor game save files for changes using OS-native file system events
- **Background Daemon**: Run as a system service for continuous monitoring
- **Cloud Sync**: Upload and download saves to/from AWS S3
- **Peer-to-Peer Sync**: WebSocket-based synchronization between devices (optional)
- **Cross-Platform Service Management**: Install, start, stop, and manage as system services on Linux (systemd), macOS (launchd), and Windows (Windows Services)
- **Batched Operations**: Intelligently batches rapid file changes to avoid sync thrashing
- **File Lock Detection**: Prevents syncing files that are currently in use by games
- **CLI Interface**: Full command-line interface with `clap` for argument parsing
- **Configurable**: TOML-based configuration with per-game settings
- **Cross-Platform**: Works on Windows, macOS, and Linux

## Project Structure

```
DeckSaves/
├── Cargo.toml           # Workspace definition
├── core/                # Core library crate
│   ├── src/
│   │   ├── lib.rs       # Main library with sync logic
│   │   ├── config.rs    # Configuration management
│   │   ├── sync.rs      # Peer sync functionality
│   │   └── watcher.rs   # File watching management
│   └── Cargo.toml
├── cli/                 # CLI binary crate
│   ├── src/
│   │   └── main.rs      # Command-line interface
│   └── Cargo.toml
└── tauri-ui/            # Optional Tauri desktop app
    ├── src/
    │   ├── lib.rs       # Tauri library
    │   └── commands.rs  # Tauri commands
    └── Cargo.toml
```

## Installation

### Building from Source

1. Install Rust: https://rustup.rs/
2. Clone this repository
3. Build the project:

```bash
cargo build --release
```

The CLI binary will be available at `target/release/game-sync`.

## Usage

### Initialize Configuration

```bash
game-sync init
```

This creates a default configuration file with an example game entry.

### Configure Games

Add a game to sync:

```bash
game-sync add-game "My Game" --path "/path/to/save/file.dat" --path "/path/to/save/folder/"
```

### List Configured Games

```bash
game-sync list
```

### Manual Sync

Sync a specific game:

```bash
game-sync sync "My Game"
```

### Watch for Changes

Start watching a game for real-time sync:

```bash
game-sync watch "My Game"
```

This will monitor the configured paths and automatically sync when changes are detected.

### Run as Background Daemon

For continuous monitoring, you can run DeckSaves as a background daemon:

```bash
game-sync daemon
```

This will start the daemon which will:
- Monitor all configured games simultaneously
- Automatically sync changes when detected
- Handle graceful shutdown on SIGTERM
- Provide systemd integration on Linux (watchdog, readiness notifications)
- Log to files instead of console

### Service Management

DeckSaves can be installed and managed as a system service on all supported platforms.

#### Install Service

```bash
# Install as system service (requires admin/root)
game-sync service install

# Install as user service
game-sync service install --user
```

#### Start/Stop Service

```bash
# Start the service
game-sync service start [--user]

# Stop the service
game-sync service stop [--user]

# Check service status
game-sync service status [--user]
```

#### Uninstall Service

```bash
# Uninstall service
game-sync service uninstall [--user]
```

#### Platform-Specific Service Details

**Linux (systemd)**:
- System service: `/etc/systemd/system/decksaves.service`
- User service: `~/.config/systemd/user/decksaves.service`
- Supports watchdog and readiness notifications
- Automatic restart on failure
- Logs via systemd journal

**macOS (launchd)**:
- System service: `/Library/LaunchDaemons/com.decksaves.game-sync.plist`
- User service: `~/Library/LaunchAgents/com.decksaves.game-sync.plist`
- Automatic start on boot/login
- Automatic restart on crash

**Windows (Windows Services)**:
- Registered as "DeckSaves" service
- Automatic startup
- Runs in background without user session
- Managed via Services Control Manager

### Configuration

The configuration file is located at:
- **Linux**: `~/.config/game-sync/config.toml`
- **macOS**: `~/Library/Application Support/com.decksaves.game-sync/config.toml`
- **Windows**: `%APPDATA%\\game-sync\\config.toml`

Example configuration:

```toml
s3_bucket = "my-game-saves-bucket"
s3_region = "us-east-1"
peer_sync_enabled = false
websocket_url = "ws://localhost:8080"

[games.skyrim]
name = "The Elder Scrolls V: Skyrim"
save_paths = [
    "C:\\Users\\Username\\Documents\\My Games\\Skyrim\\Saves",
    "C:\\Users\\Username\\Documents\\My Games\\Skyrim\\Plugins.txt"
]
sync_enabled = true

[games.steam-deck-game]
name = "Steam Deck Game"
save_paths = [
    "~/.local/share/Steam/steamapps/compatdata/12345/pfx/drive_c/users/steamuser/Documents/SaveGame"
]
sync_enabled = true
```

### Daemon Configuration

When running in daemon mode, DeckSaves monitors all games with `sync_enabled = true` simultaneously. The daemon performs periodic health checks and configuration reloads, allowing you to add/remove games without restarting the service.

**Daemon Features:**
- Monitors all enabled games simultaneously
- Automatic configuration reload (checks every 60 seconds)
- Health checks and watchdog support
- Graceful shutdown on SIGTERM
- File logging with rotation (falls back to console if permissions insufficient)
- Platform-specific service integration

**Configuration for Daemon Mode:**

```toml
# ~/.config/game-sync/config.toml (Linux)
# ~/Library/Application Support/com.decksaves.game-sync/config.toml (macOS)

s3_bucket = "my-game-saves"
s3_region = "us-east-1"
peer_sync_enabled = false

[games.elden-ring]
name = "Elden Ring"
save_paths = [
    "~/AppData/Roaming/EldenRing/76561198000000000/ER0000.sl2"
]
sync_enabled = true

[games.cyberpunk]
name = "Cyberpunk 2077"
save_paths = [
    "~/Saved Games/CD Projekt Red/Cyberpunk 2077"
]
sync_enabled = true

[games.steam-deck-game]
name = "Steam Deck Game"
save_paths = [
    "~/.local/share/Steam/steamapps/compatdata/12345/pfx/drive_c/users/steamuser/Documents/SaveGame"
]
sync_enabled = false  # This game won't be monitored by daemon
```

The daemon will automatically start monitoring `elden-ring` and `cyberpunk` but skip `steam-deck-game` since it's disabled.

## AWS S3 Setup

### Security Best Practices

**⚠️ Important: Never put AWS credentials directly in config files!**

### 1. Create IAM User with Minimal Permissions

Create a dedicated IAM user specifically for DeckSaves:

1. Go to AWS IAM Console → Users → Create User
2. Create user named `decksaves-app` (no console access needed)
3. Create an Access Key for this user

### 2. Create IAM Policy

Create a custom policy with minimal permissions:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Effect": "Allow",
            "Action": [
                "s3:GetObject",
                "s3:PutObject",
                "s3:DeleteObject",
                "s3:ListBucket"
            ],
            "Resource": [
                "arn:aws:s3:::your-decksaves-bucket-name",
                "arn:aws:s3:::your-decksaves-bucket-name/*"
            ]
        }
    ]
}
```

### 3. Configure AWS Credentials

**Option A: Environment Variables (Recommended)**
```bash
# Linux/macOS
export AWS_ACCESS_KEY_ID="your-access-key-id"
export AWS_SECRET_ACCESS_KEY="your-secret-access-key"
export AWS_DEFAULT_REGION="us-east-1"

# Windows
set AWS_ACCESS_KEY_ID=your-access-key-id
set AWS_SECRET_ACCESS_KEY=your-secret-access-key
set AWS_DEFAULT_REGION=us-east-1
```

**Option B: AWS Credentials File**
Create `~/.aws/credentials` (Linux/macOS) or `%USERPROFILE%\.aws\credentials` (Windows):
```ini
[default]
aws_access_key_id = your-access-key-id
aws_secret_access_key = your-secret-access-key
region = us-east-1
```

### 4. Test Your Setup

Test with debug logging to see detailed information:

```bash
# Enable debug logging to see what's happening
RUST_LOG=debug ./target/release/game-sync sync "your-game-name"
```

Common issues and solutions:

- **"Is a directory" error**: Make sure `save_paths` points to files, not directories
- **"No credentials" error**: Set up AWS credentials using one of the methods above
- **"Access denied" error**: Check your IAM policy permissions
- **"Bucket not found" error**: Make sure the bucket name in config matches your actual S3 bucket

### 5. S3 Bucket Security

- Create a **private** S3 bucket (block all public access)
- Enable versioning for backup protection
- Consider enabling server-side encryption
- Use bucket policies to restrict access to your IAM user only

1. Create an AWS S3 bucket for your game saves
2. Configure AWS credentials using one of these methods:
   - Environment variables: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
   - AWS credentials file: `~/.aws/credentials`
   - AWS CLI: `aws configure`
   - IAM roles (if running on EC2)
3. Update your configuration file with the bucket name and region (never put credentials in the config file!)

## AWS Credential Encryption

**🔒 DeckSaves implements secure, machine-specific encryption for AWS credentials.**

### How It Works

When you enter AWS credentials through the GUI or CLI, DeckSaves automatically encrypts them using **AES-256-GCM** encryption before storing them in the configuration file. The credentials are never stored in plaintext.

### Encryption Details

**Algorithm**: AES-256-GCM (Galois/Counter Mode)
- **Key Size**: 256-bit (32 bytes)
- **Authentication**: Built-in authenticated encryption (prevents tampering)
- **Nonce**: 96-bit random nonce generated for each encryption operation
- **Output**: Base64-encoded (nonce + ciphertext)

**Key Derivation**: Machine-specific key derived from:
- System hostname (`HOSTNAME` or `COMPUTERNAME` environment variable)
- Username (`USER` or `USERNAME` environment variable)  
- Static salt: `"decksaves_crypto_v1"`
- Hash function: SHA-256

```rust
// Simplified key derivation process
let key_material = SHA256("decksaves_crypto_v1" + hostname + username)
let encryption_key = key_material[0..32] // First 32 bytes as AES-256 key
```

### Security Properties

**✅ Machine-Specific**: Credentials encrypted on one machine cannot be decrypted on another
**✅ User-Specific**: Different users on the same machine get different encryption keys
**✅ Authenticated**: Tampering with encrypted credentials is detected and prevents decryption
**✅ Non-Deterministic**: Same credential encrypted twice produces different ciphertext (due to random nonce)
**✅ No Hardcoded Keys**: No master keys or secrets embedded in the application

### Example Configuration

In your `config.toml`, encrypted credentials look like this:
```toml
s3_bucket = "my-game-saves"
s3_region = "us-east-1"
aws_access_key_id = "2FALL2z3/6VT7/mB08PZVc4gHmHE8OHfcw6jTqcMff46pNyL4NL3jPKiCiCqt6lL"
aws_secret_access_key = "QiU5MuUNK11MqDn9fbZTP1ckqZxVG0Fa+l4atSlq/TCRsuZ3v0jIyYodF2EvjWjijVrW6DzIahak4Xe09mY084F+BTI="
```

### Important Security Notes

1. **Backup Considerations**: Encrypted credentials are tied to your machine and user account. If you restore the config file to a different machine or user, you'll need to re-enter the credentials.

2. **Migration**: When moving to a new machine, you cannot simply copy the config file - the encrypted credentials won't work. Use the GUI or CLI to re-enter credentials on the new machine.

3. **Not Network Security**: This encryption protects credentials at rest on your local machine. It doesn't encrypt data in transit to AWS (that's handled by HTTPS/TLS).

4. **Environment Variables Still Work**: You can still use AWS environment variables or AWS credential files instead of storing encrypted credentials in the config.

### Implementation

The encryption is handled by the `CredentialCrypto` struct in `core/src/crypto.rs`:

```rust
// Encrypt credentials when saving
config.set_aws_access_key("AKIA...")?;  // Automatically encrypts
config.set_aws_secret_key("secret")?;   // Automatically encrypts

// Decrypt credentials when using
let access_key = config.get_aws_access_key()?;  // Automatically decrypts
let secret_key = config.get_aws_secret_key()?;  // Automatically decrypts
```

## Development

### Core Library Features

- **File System Watching**: Uses the `notify` crate for cross-platform file system events
- **Async I/O**: Built on Tokio for non-blocking file operations and networking
- **S3 Integration**: Uses the official AWS SDK for Rust
- **WebSocket Support**: tokio-tungstenite for peer-to-peer synchronization
- **Configuration Management**: TOML-based configuration with the `directories` crate for OS-appropriate paths

### Building Components

Build individual components:

```bash
# Core library only
cargo build -p core

# CLI only  
cargo build -p cli

# Tauri UI (when ready)
cargo build -p tauri-ui
```

### Running Tests

```bash
cargo test --workspace
```

## Architecture

### Core Library (`core/`)

The core library handles:
- File system monitoring with batching and debouncing
- S3 upload/download operations
- File integrity checking with SHA-256 hashing
- Configuration management
- Peer synchronization via WebSockets

### CLI (`cli/`)

The CLI provides a user-friendly interface to:
- Configure games and sync settings
- Manually trigger sync operations
- Start file watching for real-time sync
- View sync status and configuration

### Tauri UI (`tauri-ui/`)

Optional desktop GUI built with Tauri that provides:
- Visual game configuration
- Real-time sync status
- Graphical file browser for selecting save paths
- System tray integration

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Roadmap

- [x] Daemon mode and background service
- [x] Cross-platform service management (systemd, launchd, Windows Services)
- [x] Signal handling and graceful shutdown
- [x] File logging with rotation
- [x] Health checks and watchdog support
- [x] GUI implementation with Tauri
- [x] Encryption for AWS credentials (AES-256-GCM)
- [ ] Conflict resolution strategies
- [ ] Incremental/delta sync for large save files
- [ ] End-to-end encryption for cloud storage (encrypt save files themselves)
- [ ] Multiple cloud provider support (Google Drive, Dropbox)
- [ ] Steam Cloud integration
- [ ] Automatic game detection
- [ ] Backup versioning and restoration
- [ ] Web-based configuration interface
- [ ] Real-time sync status dashboard