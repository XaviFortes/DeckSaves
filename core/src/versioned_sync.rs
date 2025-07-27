use anyhow::{Result, Context};
use std::path::Path;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn, error, debug};

use crate::versioning::{VersionManager, VersionConfig, FileVersion, GameVersionManifest};
use crate::storage::{StorageProvider, StorageFactory, StorageConfig};
use crate::progress::{SyncProgress, ProgressCallback, EnhancedSyncSummary, FileOperationResult};

/// Summary of synchronization operations (legacy compatibility)
#[derive(Debug, Clone)]
pub struct SyncSummary {
    pub remote_versions_merged: bool,
    pub manifest_uploaded: bool,
    pub files_downloaded: u32,
    pub files_uploaded: u32,
    pub conflicts_resolved: u32,
}

impl SyncSummary {
    pub fn new() -> Self {
        Self {
            remote_versions_merged: false,
            manifest_uploaded: false,
            files_downloaded: 0,
            files_uploaded: 0,
            conflicts_resolved: 0,
        }
    }

    pub fn from_enhanced(enhanced: &EnhancedSyncSummary) -> Self {
        Self {
            remote_versions_merged: enhanced.remote_versions_merged,
            manifest_uploaded: enhanced.manifest_uploaded,
            files_downloaded: enhanced.files_downloaded,
            files_uploaded: enhanced.files_uploaded,
            conflicts_resolved: enhanced.conflicts_resolved,
        }
    }
}

/// Enhanced sync manager with versioning support
pub struct VersionedSync {
    version_manager: VersionManager,
    storage_provider: Box<dyn StorageProvider>,
    game_name: String,
}

impl VersionedSync {
    /// Create a new versioned sync manager with intelligent manifest merging
    pub async fn new(
        game_name: String,
        storage_config: StorageConfig,
        version_config: VersionConfig,
    ) -> Result<Self> {
        // Create storage provider
        let storage_provider = StorageFactory::create_provider(&storage_config).await?;
        
        // Try to load existing manifest from storage
        let remote_manifest = storage_provider.download_manifest(&game_name).await?;
        
        // Create initial version manager (will create empty if no local data)
        let mut version_manager = VersionManager::load_or_create(
            game_name.clone(),
            version_config,
            None, // Start with no manifest data to get local state
        ).await?;

        // If we have a remote manifest, intelligently merge it with local data
        if let Some(remote_manifest) = remote_manifest {
            info!("Found remote manifest for '{}', merging with local data...", game_name);
            Self::merge_manifests(&mut version_manager, &remote_manifest).await?;
        } else {
            info!("No remote manifest found for '{}', starting fresh", game_name);
        }

        Ok(Self {
            version_manager,
            storage_provider,
            game_name,
        })
    }

    /// Intelligently merge remote manifest with local data without losing anything
    async fn merge_manifests(
        local_version_manager: &mut VersionManager,
        remote_manifest: &GameVersionManifest,
    ) -> Result<()> {
        info!("Starting intelligent manifest merge...");
        
        let local_manifest = local_version_manager.get_manifest();
        let mut merged_files = local_manifest.files.clone();
        let mut conflicts_resolved = 0;
        let mut remote_versions_added = 0;

        // Process each file in the remote manifest
        for (file_path, remote_file_info) in &remote_manifest.files {
            match merged_files.get_mut(file_path) {
                Some(local_file_info) => {
                    // File exists in both - merge versions
                    info!("Merging versions for file: {}", file_path);
                    
                    // Add any remote versions that don't exist locally
                    for remote_version in &remote_file_info.versions {
                        let version_exists = local_file_info.versions.iter()
                            .any(|v| v.version_id == remote_version.version_id);
                        
                        if !version_exists {
                            info!("Adding remote version {} to local manifest", remote_version.version_id);
                            local_file_info.versions.push(remote_version.clone());
                            remote_versions_added += 1;
                        }
                    }
                    
                    // Sort versions by timestamp
                    local_file_info.versions.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
                    
                    // Resolve current version conflicts intelligently
                    let local_current = local_file_info.current_version.as_ref()
                        .and_then(|id| local_file_info.versions.iter().find(|v| v.version_id == *id));
                    let remote_current = remote_file_info.current_version.as_ref()
                        .and_then(|id| local_file_info.versions.iter().find(|v| v.version_id == *id));
                    
                    match (local_current, remote_current) {
                        (Some(local), Some(remote)) => {
                            // Choose the newer version as current
                            if remote.timestamp > local.timestamp {
                                info!("Remote current version is newer, updating current to: {}", remote.version_id);
                                local_file_info.current_version = Some(remote.version_id.clone());
                                conflicts_resolved += 1;
                            } else {
                                info!("Local current version is newer or equal, keeping local current");
                            }
                        }
                        (None, Some(remote)) => {
                            // No local current, use remote
                            info!("No local current version, using remote: {}", remote.version_id);
                            local_file_info.current_version = Some(remote.version_id.clone());
                        }
                        (Some(_), None) => {
                            // Keep local current (remote has no current)
                            info!("Remote has no current version, keeping local current");
                        }
                        (None, None) => {
                            // Neither has current, set to latest by timestamp
                            if let Some(latest) = local_file_info.versions.last() {
                                info!("No current version on either side, setting to latest: {}", latest.version_id);
                                local_file_info.current_version = Some(latest.version_id.clone());
                            }
                        }
                    }
                }
                None => {
                    // File only exists in remote - add it entirely
                    info!("Adding remote-only file to local manifest: {}", file_path);
                    merged_files.insert(file_path.clone(), remote_file_info.clone());
                    remote_versions_added += remote_file_info.versions.len();
                }
            }
        }

        // Update the local manifest with merged data
        let mut updated_manifest = local_manifest.clone();
        updated_manifest.files = merged_files;
        updated_manifest.last_updated = chrono::Utc::now();

        // Recreate the version manager with the merged manifest
        let manifest_data = serde_json::to_vec(&updated_manifest)?;
        *local_version_manager = VersionManager::load_or_create(
            updated_manifest.game_name.clone(),
            local_version_manager.get_config().clone(),
            Some(manifest_data),
        ).await?;

        info!("Manifest merge completed successfully!");
        info!("- Remote versions added: {}", remote_versions_added);
        info!("- Conflicts resolved: {}", conflicts_resolved);
        info!("- Total files in merged manifest: {}", updated_manifest.files.len());

        Ok(())
    }

    /// Perform a full bidirectional sync between local and remote storage
    pub async fn perform_full_sync(&mut self) -> Result<SyncSummary> {
        let enhanced = self.perform_full_sync_with_progress(None).await?;
        Ok(SyncSummary::from_enhanced(&enhanced))
    }

    /// Perform a full bidirectional sync with progress tracking
    pub async fn perform_full_sync_with_progress(
        &mut self,
        progress_callback: Option<Box<dyn ProgressCallback>>,
    ) -> Result<EnhancedSyncSummary> {
        info!("Starting full bidirectional sync for game: {}", self.game_name);
        
        let mut summary = EnhancedSyncSummary::new();
        let start_time = Instant::now();
        
        // Estimate total steps: download manifest, merge, upload manifest
        let mut progress = SyncProgress::new("Full Sync".to_string(), 3);
        
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        // Step 1: Download and merge remote manifest
        progress.update_step(1, "Downloading remote manifest...".to_string());
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        let step_start = Instant::now();
        if let Some(remote_manifest) = self.storage_provider.download_manifest(&self.game_name).await? {
            info!("Remote manifest found, merging...");
            progress.set_current_file("manifest.json".to_string());
            if let Some(ref callback) = progress_callback {
                callback.on_progress(progress.clone());
            }

            Self::merge_manifests(&mut self.version_manager, &remote_manifest).await?;
            summary.remote_versions_merged = true;
            
            let manifest_bytes = serde_json::to_vec(&remote_manifest)?.len() as u64;
            summary.add_file_operation(
                "manifest.json".to_string(),
                FileOperationResult {
                    operation: "download".to_string(),
                    success: true,
                    bytes: manifest_bytes,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                    error: None,
                },
            );
        } else {
            info!("No remote manifest found");
        }

        // Step 2: Upload our local manifest (now merged) to ensure remote is up to date
        progress.update_step(2, "Uploading local manifest...".to_string());
        progress.set_current_file("manifest.json".to_string());
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        let step_start = Instant::now();
        let local_manifest = self.version_manager.get_manifest();
        let manifest_result = self.storage_provider.upload_manifest(&self.game_name, local_manifest).await?;
        
        if manifest_result.success {
            summary.manifest_uploaded = true;
            let manifest_bytes = serde_json::to_vec(local_manifest)?.len() as u64;
            summary.add_file_operation(
                "manifest.json".to_string(),
                FileOperationResult {
                    operation: "upload".to_string(),
                    success: true,
                    bytes: manifest_bytes,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                    error: None,
                },
            );
        } else {
            summary.add_file_operation(
                "manifest.json".to_string(),
                FileOperationResult {
                    operation: "upload".to_string(),
                    success: false,
                    bytes: 0,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                    error: manifest_result.error,
                },
            );
        }

        // Step 3: Complete
        progress.update_step(3, format!("Sync completed in {:.2}s", start_time.elapsed().as_secs_f32()));
        progress.complete();
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        info!("Full sync completed for game: {}", self.game_name);
        Ok(summary)
    }

    /// Sync a file to storage, creating a new version
    pub async fn sync_file_to_storage<P: AsRef<Path>>(
        &mut self,
        local_file_path: P,
        relative_path: &str,
        description: Option<String>,
    ) -> Result<FileVersion> {
        let _enhanced = self.sync_file_to_storage_with_progress(
            local_file_path,
            relative_path,
            description,
            None,
        ).await?;
        
        // Return the version from the enhanced result
        let manifest = self.version_manager.get_manifest();
        if let Some(file_manifest) = manifest.files.get(relative_path) {
            if let Some(version) = file_manifest.versions.first() {
                return Ok(version.clone());
            }
        }
        
        Err(anyhow::anyhow!("Failed to retrieve created version"))
    }

    /// Sync a file to storage with progress tracking
    pub async fn sync_file_to_storage_with_progress<P: AsRef<Path>>(
        &mut self,
        local_file_path: P,
        relative_path: &str,
        description: Option<String>,
        progress_callback: Option<Box<dyn ProgressCallback>>,
    ) -> Result<EnhancedSyncSummary> {
        let local_path = local_file_path.as_ref();
        let mut summary = EnhancedSyncSummary::new();
        
        debug!("🔄 Starting sync_file_to_storage_with_progress for: {} -> {}", 
               local_path.display(), relative_path);
        
        // Check if file exists and get basic info
        if !local_path.exists() {
            warn!("❌ Local file does not exist: {}", local_path.display());
            return Err(anyhow::anyhow!("Local file does not exist: {}", local_path.display()));
        }
        
        let file_metadata = std::fs::metadata(local_path)?;
        let file_size = file_metadata.len();
        debug!("📁 File info: size={} bytes, modified={:?}", file_size, file_metadata.modified());
        
        // Progress tracking
        let mut progress = SyncProgress::new("File Upload".to_string(), 4);
        progress.set_current_file(relative_path.to_string());
        
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }
        
        // Step 1: Create version entry
        progress.update_step(1, "Creating version entry...".to_string());
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        debug!("📝 Creating version entry for file: {}", relative_path);
        let version = self.version_manager.add_version(
            relative_path,
            local_path,
            HashMap::new(),
            description,
        ).await?;
        
        debug!("✅ Version created: {} (hash: {})", version.version_id, version.hash);

        // Step 2: Read file data
        progress.update_step(2, "Reading file data...".to_string());
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        debug!("📖 Reading file data from: {}", local_path.display());
        let file_data = tokio::fs::read(local_path).await
            .context("Failed to read file for sync")?;
        
        let file_size = file_data.len() as u64;
        debug!("📊 File read successfully: {} bytes", file_size);
        progress.update_bytes(0, file_size);
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        // Step 3: Upload to storage
        progress.update_step(3, "Uploading to storage...".to_string());
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        debug!("☁️  Uploading to storage: game={}, path={}, version={}", 
               self.game_name, relative_path, version.version_id);
        let upload_start = Instant::now();
        let storage_result = self.storage_provider.upload_file(
            &self.game_name,
            relative_path,
            &version,
            &file_data,
        ).await?;

        let upload_duration = upload_start.elapsed();
        debug!("📤 Storage upload result: success={}, duration={:.2}s", 
               storage_result.success, upload_duration.as_secs_f32());

        if !storage_result.success {
            error!("❌ File upload failed: {:?}", storage_result.error);
            summary.add_file_operation(
                relative_path.to_string(),
                FileOperationResult {
                    operation: "upload".to_string(),
                    success: false,
                    bytes: file_size,
                    duration_ms: upload_duration.as_millis() as u64,
                    error: storage_result.error,
                },
            );
            return Err(anyhow::anyhow!("Failed to upload file to storage"));
        }

        debug!("✅ File upload successful");
        summary.add_file_operation(
            relative_path.to_string(),
            FileOperationResult {
                operation: "upload".to_string(),
                success: true,
                bytes: file_size,
                duration_ms: upload_duration.as_millis() as u64,
                error: None,
            },
        );

        // Step 4: Upload updated manifest
        progress.update_step(4, "Updating manifest...".to_string());
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        debug!("📋 Uploading updated manifest");
        let manifest_start = Instant::now();
        let manifest_result = self.storage_provider.upload_manifest(
            &self.game_name,
            self.version_manager.get_manifest(),
        ).await?;

        let manifest_duration = manifest_start.elapsed();
        debug!("📋 Manifest upload result: success={}, duration={:.2}s", 
               manifest_result.success, manifest_duration.as_secs_f32());

        if manifest_result.success {
            summary.manifest_uploaded = true;
            debug!("✅ Manifest uploaded successfully");
        } else {
            warn!("⚠️  Manifest upload failed: {:?}", manifest_result.error);
        }

        let manifest_bytes = serde_json::to_vec(self.version_manager.get_manifest())?.len() as u64;
        summary.add_file_operation(
            "manifest.json".to_string(),
            FileOperationResult {
                operation: "upload".to_string(),
                success: manifest_result.success,
                bytes: manifest_bytes,
                duration_ms: manifest_duration.as_millis() as u64,
                error: manifest_result.error,
            },
        );

        progress.complete();
        if let Some(ref callback) = progress_callback {
            callback.on_progress(progress.clone());
        }

        info!("🎉 Successfully synced {} to storage (version: {})", 
              relative_path, version.version_id);
        debug!("📊 Final summary: uploaded files={}, total bytes={}", 
               summary.files_uploaded, summary.total_bytes_transferred);

        // Verify upload by trying to list the object
        debug!("🔍 Verifying upload by checking if object exists in S3...");
        if let Err(e) = self.verify_upload(&version.version_id, relative_path).await {
            warn!("⚠️  Upload verification failed: {}", e);
        } else {
            debug!("✅ Upload verification successful");
        }

        Ok(summary)
    }

    /// Download a specific version from storage
    pub async fn download_version<P: AsRef<Path>>(
        &self,
        relative_path: &str,
        version_id: &str,
        local_path: P,
    ) -> Result<()> {
        let version = self.version_manager.get_version(relative_path, version_id)
            .context("Version not found in manifest")?;

        let file_data = self.storage_provider.download_file(
            &self.game_name,
            relative_path,
            version,
        ).await?;

        // Verify hash if available
        let actual_hash = crate::versioning::calculate_hash(&file_data);
        if actual_hash != version.hash {
            return Err(anyhow::anyhow!(
                "File integrity check failed: expected {}, got {}",
                version.hash, actual_hash
            ));
        }

        // Create parent directories if needed
        let local_path = local_path.as_ref();
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(local_path, file_data).await?;
        
        info!("Downloaded version {} of {} to {}", 
              version_id, relative_path, local_path.display());
        Ok(())
    }

    /// Download the latest version of a file
    pub async fn download_latest<P: AsRef<Path>>(
        &self,
        relative_path: &str,
        local_path: P,
    ) -> Result<()> {
        let current_version = self.version_manager.get_current_version(relative_path)
            .context("No current version found for file")?;

        self.download_version(relative_path, &current_version.version_id, local_path).await
    }

    /// Sync from storage - download newer versions if available
    pub async fn sync_from_storage<P: AsRef<Path>>(
        &mut self,
        relative_path: &str,
        local_path: P,
    ) -> Result<bool> {
        // First update our manifest from storage
        if let Some(remote_manifest) = self.storage_provider.download_manifest(&self.game_name).await? {
            // Check if remote has newer versions
            if let Some(remote_file_manifest) = remote_manifest.files.get(relative_path) {
                let local_current = self.version_manager.get_current_version(relative_path);
                let remote_current = remote_file_manifest.current_version.as_ref()
                    .and_then(|id| remote_file_manifest.versions.iter().find(|v| v.version_id == *id));

                match (local_current, remote_current) {
                    (Some(local), Some(remote)) => {
                        if remote.timestamp > local.timestamp {
                            info!("Remote version is newer, downloading...");
                            self.download_version(relative_path, &remote.version_id, local_path).await?;
                            
                            // Update our local manifest with the remote info
                            self.version_manager = VersionManager::load_or_create(
                                self.game_name.clone(),
                                self.version_manager.get_config().clone(),
                                Some(serde_json::to_vec(&remote_manifest)?),
                            ).await?;
                            
                            return Ok(true);
                        }
                    }
                    (None, Some(remote)) => {
                        info!("No local version, downloading from remote...");
                        self.download_version(relative_path, &remote.version_id, local_path).await?;
                        return Ok(true);
                    }
                    _ => {
                        info!("Local version is up to date or newer");
                    }
                }
            }
        }

        Ok(false)
    }

    /// List all versions of a file
    pub fn list_versions(&self, relative_path: &str) -> Option<&Vec<FileVersion>> {
        self.version_manager.get_file_versions(relative_path)
    }

    /// Get all versions for the current game across all files
    pub fn get_all_versions_for_game(&self, game_name: &str) -> Option<Vec<FileVersion>> {
        let manifest = self.version_manager.get_manifest();
        let mut all_versions = Vec::new();
        
        // Iterate through all files in the manifest and collect versions only for the specified game
        for (file_path, file_info) in &manifest.files {
            // Filter to only include files that belong to the requested game
            // Files are stored with format "GameName/filename" 
            if file_path.starts_with(&format!("{}/", game_name)) {
                println!("DEBUG get_all_versions_for_game: found file '{}' with {} versions (matches game '{}')", file_path, file_info.versions.len(), game_name);
                for version in &file_info.versions {
                    // Create a copy of the version with file path information
                    let version_with_path = version.clone();
                    // We can add the file path to the description or store it somehow
                    // For now, the version already has the file_path in the FileInfo structure
                    all_versions.push(version_with_path);
                }
            } else {
                println!("DEBUG get_all_versions_for_game: skipping file '{}' (doesn't match game '{}')", file_path, game_name);
            }
        }
        
        // Sort versions by timestamp (newest first)
        all_versions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        println!("DEBUG get_all_versions_for_game: returning {} total versions for game '{}'", all_versions.len(), game_name);
        Some(all_versions)
    }

    /// Pin a version to prevent automatic cleanup
    pub async fn pin_version(&mut self, relative_path: &str, version_id: &str) -> Result<()> {
        // Update the pin status in the manifest
        self.version_manager.pin_version(relative_path, version_id)?;
        
        // Save the updated manifest
        let manifest = self.version_manager.get_manifest();
        self.storage_provider.upload_manifest(&self.game_name, manifest).await?;
        
        Ok(())
    }

    /// Delete a specific version
    pub async fn delete_version(&mut self, relative_path: &str, version_id: &str) -> Result<()> {
        println!("DEBUG VersionedSync::delete_version: relative_path='{}', version_id='{}'", relative_path, version_id);
        
        // Get the version details before removing it from the manifest
        let version = self.version_manager.get_version(relative_path, version_id)
            .ok_or_else(|| anyhow::anyhow!("Version {} not found for file {}", version_id, relative_path))?
            .clone();
        
        // Remove the version from the manifest
        let version_id_string = version_id.to_string();
        let _removed_version = self.version_manager.remove_version(relative_path, &version_id_string)?;
        
        // Delete the actual file from storage using the StorageProvider interface
        self.storage_provider.delete_version(&self.game_name, relative_path, &version).await?;
        
        // Save the updated manifest - get the manifest reference and upload it
        let manifest = self.version_manager.get_manifest();
        self.storage_provider.upload_manifest(&self.game_name, manifest).await?;
        
        println!("DEBUG VersionedSync::delete_version: successfully deleted version '{}' for '{}'", version_id, relative_path);
        Ok(())
    }

    /// Get version manager for advanced operations
    pub fn get_version_manager(&self) -> &VersionManager {
        &self.version_manager
    }

    /// Get storage provider for advanced operations
    pub fn get_storage_provider(&self) -> &dyn StorageProvider {
        self.storage_provider.as_ref()
    }

    /// Save current manifest to storage
    pub async fn save_manifest(&self) -> Result<()> {
        let result = self.storage_provider.upload_manifest(
            &self.game_name,
            self.version_manager.get_manifest(),
        ).await?;

        if !result.success {
            return Err(anyhow::anyhow!(
                "Failed to save manifest: {}", 
                result.error.unwrap_or_default()
            ));
        }

        Ok(())
    }

    /// Cleanup old versions based on configuration
    pub async fn cleanup_old_versions(&mut self) -> Result<Vec<String>> {
        let mut cleaned_versions = Vec::new();
        
        // Get all files in the manifest
        let manifest = self.version_manager.get_manifest();
        for (file_path, file_manifest) in &manifest.files {
            // Find versions that should be cleaned up
            for version in &file_manifest.versions {
                if !version.is_pinned {
                    // Check if version is old enough to clean up
                    let age = chrono::Utc::now() - version.timestamp;
                    if age.num_days() > 30 { // This should come from config
                        // Delete from storage
                        match self.storage_provider.delete_version(
                            &self.game_name,
                            file_path,
                            version,
                        ).await {
                            Ok(result) => {
                                if result.success {
                                    cleaned_versions.push(format!("{}:{}", file_path, version.version_id));
                                    info!("Cleaned up old version: {} of {}", version.version_id, file_path);
                                } else {
                                    warn!("Failed to delete version from storage: {}", 
                                          result.error.unwrap_or_default());
                                }
                            }
                            Err(e) => {
                                error!("Error deleting version: {}", e);
                            }
                        }
                    }
                }
            }
        }

        // Update manifest after cleanup
        if !cleaned_versions.is_empty() {
            self.save_manifest().await?;
        }

        Ok(cleaned_versions)
    }

    /// Verify that an uploaded file exists in storage
    async fn verify_upload(&self, version_id: &str, relative_path: &str) -> Result<()> {
        debug!("🔍 Verifying upload for version {} of {}", version_id, relative_path);
        
        // Try to get the object metadata to verify it exists
        if let Some(version) = self.version_manager.get_version(relative_path, version_id) {
            debug!("📋 Version found in manifest, checking storage...");
            
            // Try to download a small portion to verify storage accessibility
            match self.storage_provider.download_file(&self.game_name, relative_path, version).await {
                Ok(data) => {
                    debug!("✅ Verification download successful, {} bytes retrieved", data.len());
                    Ok(())
                }
                Err(e) => {
                    warn!("❌ Verification download failed: {}", e);
                    Err(anyhow::anyhow!("Upload verification failed: {}", e))
                }
            }
        } else {
            warn!("❌ Version {} not found in manifest for {}", version_id, relative_path);
            Err(anyhow::anyhow!("Version not found in manifest"))
        }
    }

    /// Sync local manifest with remote manifest for version history display
    pub async fn sync_with_remote_for_history(&mut self) -> Result<()> {
        debug!("📡 Syncing with remote manifest for version history...");
        
        if let Some(remote_manifest) = self.storage_provider.download_manifest(&self.game_name).await? {
            debug!("📥 Remote manifest found, merging with local...");
            
            // Merge the remote manifest with our local one
            Self::merge_manifests(&mut self.version_manager, &remote_manifest).await?;
            
            debug!("✅ Successfully merged remote manifest for version history");
            info!("📊 Remote manifest merged: {} files total", remote_manifest.files.len());
        } else {
            debug!("📭 No remote manifest found");
        }
        
        Ok(())
    }
}
