# DeckSaves - Game Save Synchronization Tool

A cross-platform game save synchronization tool with support for cloud storage (S3) and peer-to-peer sync. Built in Rust with a modular architecture supporting CLI and optional Tauri-based GUI.

## Features

- **Real-time File Watching**: Monitor game save files for changes using OS-native file system events
- **Cloud Sync**: Upload and download saves to/from AWS S3
- **Peer-to-Peer Sync**: WebSocket-based synchronization between devices (optional)
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

## AWS S3 Setup

1. Create an AWS S3 bucket for your game saves
2. Configure AWS credentials using one of these methods:
   - AWS CLI: `aws configure`
   - Environment variables: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
   - IAM roles (if running on EC2)
   - AWS credentials file

3. Update your configuration file with the bucket name and region

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

- [ ] GUI implementation with Tauri
- [ ] Conflict resolution strategies
- [ ] Incremental/delta sync for large save files
- [ ] Encryption for cloud storage
- [ ] Multiple cloud provider support (Google Drive, Dropbox)
- [ ] Steam Cloud integration
- [ ] Automatic game detection
- [ ] Backup versioning and restoration
