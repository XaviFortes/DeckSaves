# DeckSaves Versioning Guide

## Overview
The DeckSaves versioning system provides automatic version history and restore capabilities for your game save files, ensuring you never lose progress and can easily recover from any point in time.

## Features

### 🕒 Versioned Sync
- **Auto-Versioning**: Every sync operation creates a timestamped version with hash verification
- **Compression**: Optional compression to save storage space
- **Smart Storage**: Works with both S3 (cloud) and local storage backends
- **Auto-Pinning**: Automatically pins weekly versions to preserve important saves

### 📋 Version History
- **Timeline View**: See all versions of your save files in chronological order
- **File Details**: View file size, timestamp, hash, and descriptions for each version
- **Pin Management**: Pin important versions (before boss fights, achievements, etc.)
- **Cleanup**: Automatically removes old versions while preserving pinned ones

### ↩️ Restore Capabilities
- **One-Click Restore**: Restore any previous version with a single click
- **Safe Recovery**: Backup current version before restoring
- **Conflict Resolution**: Handles file conflicts intelligently

## How to Use

### 1. Versioned Sync
In the Games list, you'll now see two sync options for each game:
- **Sync Now**: Regular sync (legacy)
- **🕒 Versioned Sync**: Sync with automatic versioning

Click "Versioned Sync" to sync your game saves with version tracking enabled.

### 2. View Version History
1. Click the **📋 History** button next to any game
2. Browse through all versions of your save files
3. See details like:
   - When each version was created
   - File size and hash
   - Whether it's pinned
   - Description (if any)

### 3. Restore Previous Versions
1. Open the version history for a game
2. Find the version you want to restore
3. Click the **↩️ Restore** button
4. Confirm the restoration
5. Your save file will be restored to that exact state

### 4. Pin Important Versions
1. In the version history, click the **📍** button next to any version
2. Pinned versions show a **📌 Pinned** badge
3. Pinned versions are protected from automatic cleanup
4. Use this before:
   - Important boss fights
   - Achievement attempts
   - Risky decisions
   - Game updates

### 5. Cleanup Old Versions
- Click **🧹 Cleanup Old** to remove old, unpinned versions
- Automatic cleanup runs based on your configuration:
  - Keeps last 10 versions per file (configurable)
  - Removes versions older than 30 days (configurable)
  - Always preserves pinned versions

## Configuration

The versioning system uses these default settings:
- **Auto-Pin Strategy**: Weekly (pins one version per week)
- **Max Versions**: 10 per file
- **Retention Period**: 30 days
- **Compression**: Enabled
- **Storage**: Uses your existing S3 or local storage configuration

## Storage Locations

### Cloud Storage (S3)
Versions are stored in your configured S3 bucket under:
```
your-bucket/
  versions/
    GameName/
      SaveFile.dat/
        2025-07-25T10-30-00_abc123.dat
        2025-07-25T11-00-00_def456.dat
```

### Local Storage
Versions are stored locally under:
```
~/.decksaves/versions/
  GameName/
    SaveFile.dat/
      2025-07-25T10-30-00_abc123.dat
      2025-07-25T11-00-00_def456.dat
```

## Benefits

### Save Game Safety
- **Never lose saves**: Every sync creates a recoverable version
- **Corruption protection**: If a save gets corrupted, restore a previous version
- **Experiment freely**: Try risky gameplay knowing you can always go back

### Peace of Mind
- **Automatic backups**: No manual intervention required
- **Smart retention**: Keeps important versions, cleans up clutter
- **Easy recovery**: User-friendly interface for browsing and restoring

### Storage Efficiency
- **Compression**: Reduces storage usage for save files
- **Deduplication**: Same content = same hash = one storage entry
- **Smart cleanup**: Removes old versions while preserving important ones

## Troubleshooting

### No Version History Shown
- Make sure you've used "Versioned Sync" at least once
- Check that your storage backend (S3/Local) is configured correctly
- Verify network connectivity for S3 storage

### Restore Failed
- Ensure you have write permissions to the save file location
- Check that the game is not currently running
- Verify the target version still exists in storage

### Storage Issues
- For S3: Check AWS credentials and bucket permissions
- For Local: Verify disk space and write permissions
- Check the application logs for detailed error messages

## Advanced Features

### Multiple Save Paths
- If a game has multiple save file paths, the History button shows versions for the first path
- Future versions will allow selecting which save path to view

### API Integration
The versioning system exposes these Tauri commands for custom integrations:
- `sync_game_with_versioning`
- `get_version_history`
- `restore_version`
- `pin_version`
- `cleanup_old_versions`

## What's Next

Planned improvements include:
- **Save Path Selection**: Choose which save file to view history for
- **Bulk Operations**: Pin/restore multiple versions at once
- **Version Notes**: Add custom descriptions to versions
- **Advanced Filters**: Filter versions by date, size, or tags
- **Sync Scheduling**: Automatic versioned syncing on a schedule
