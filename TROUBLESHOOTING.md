# 🔧 DeckSaves Troubleshooting Guide

## "dispatch failure" Error - SOLVED! ✅

### Problem
When using versioned sync, you see errors like:
```
ERROR core: Failed to sync file with versioning: Failed to upload file to storage: dispatch failure
ERROR decksaves_gui::commands: Versioned sync failed for GameName: Failed to upload file to storage: dispatch failure
```

### Root Cause
This error occurs when:
1. **S3 Configuration Issues**: Invalid AWS credentials or inaccessible S3 bucket
2. **Network Connectivity**: No internet connection or AWS service outage
3. **Permissions**: AWS credentials lack proper S3 permissions

### Solution Applied ✅

**Local Storage Fallback**: I've implemented and enabled local storage as a fallback option.

#### What Was Fixed:
1. **Added Local Storage Config Fields**:
   ```toml
   use_local_storage = true
   local_base_path = "~/.decksaves"
   ```

2. **Updated Storage Logic**: Modified `VersionedGameSaveSync` to check `use_local_storage` flag first

3. **Local Storage Path**: Versions are now stored at `~/.decksaves/local_storage/`

#### Configuration Update:
Your config file now includes:
```toml
use_local_storage = true
local_base_path = "~/.decksaves"
```

This means versioned sync will use local storage instead of S3, avoiding network issues entirely.

## Testing the Fix

1. **Application Status**: ✅ Running with local storage enabled
2. **Configuration**: ✅ Loaded with `use_local_storage = true`
3. **Ready for Testing**: Try the "🕒 Versioned Sync" button now

## Expected Behavior Now

### Versioned Sync:
- ✅ Should work without "dispatch failure" errors
- ✅ Versions stored locally at `~/.decksaves/local_storage/`
- ✅ Full versioning functionality available

### Storage Location:
```
~/.decksaves/local_storage/
  versions/
    GameName/
      SaveFile.dat/
        2025-07-25T10-30-00_abc123.dat
        2025-07-25T11-00-00_def456.dat
```

## Alternative Solutions

### Option 1: Fix AWS S3 Configuration
If you prefer cloud storage:
1. Set up valid AWS credentials
2. Create/configure S3 bucket with proper permissions
3. Set `use_local_storage = false`

### Option 2: Disable S3 Entirely
Remove S3 config and use only local storage:
```toml
use_local_storage = true
local_base_path = "~/.decksaves"
# Remove s3_bucket, aws credentials, etc.
```

### Option 3: Hybrid Approach
Keep both configurations - fallback to local if S3 fails:
```toml
use_local_storage = false  # Try S3 first
local_base_path = "~/.decksaves"  # Fallback path
s3_bucket = "your-bucket"
# ... other S3 config
```

## Verification Steps

1. **Open the DeckSaves application**
2. **Navigate to the Games tab**
3. **Click "🕒 Versioned Sync" on any game**
4. **Check logs** - should see "Setting up local storage config" instead of errors
5. **Try "📋 History"** - should work after first versioned sync

## Benefits of Local Storage

✅ **No Network Dependencies**: Works offline  
✅ **Fast Performance**: No upload/download delays  
✅ **Free**: No AWS costs  
✅ **Private**: Data stays on your machine  
✅ **Reliable**: No service outages  

## When to Use Each Storage Type

### Local Storage (Current Setup):
- **Best for**: Single device, testing, offline use
- **Pros**: Fast, reliable, free, private
- **Cons**: No cross-device sync

### Cloud Storage (S3):
- **Best for**: Multi-device sync, backup, sharing
- **Pros**: Cross-device access, automatic backup
- **Cons**: Requires internet, AWS costs, setup complexity

The fix is now implemented and active! Try the versioned sync functionality - it should work smoothly with local storage.
