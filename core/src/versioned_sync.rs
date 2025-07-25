use anyhow::{Result, Context};
use std::path::Path;
use std::collections::HashMap;
use tracing::{info, warn, error};

use crate::versioning::{VersionManager, VersionConfig, FileVersion};
use crate::storage::{StorageProvider, StorageFactory, StorageConfig};

/// Enhanced sync manager with versioning support
pub struct VersionedSync {
    version_manager: VersionManager,
    storage_provider: Box<dyn StorageProvider>,
    game_name: String,
}

impl VersionedSync {
    /// Create a new versioned sync manager
    pub async fn new(
        game_name: String,
        storage_config: StorageConfig,
        version_config: VersionConfig,
    ) -> Result<Self> {
        // Create storage provider
        let storage_provider = StorageFactory::create_provider(&storage_config).await?;
        
        // Try to load existing manifest from storage
        let existing_manifest = storage_provider.download_manifest(&game_name).await?;
        let manifest_data = existing_manifest.as_ref().map(|m| {
            serde_json::to_vec(m).unwrap_or_default()
        });

        // Create version manager
        let version_manager = VersionManager::load_or_create(
            game_name.clone(),
            version_config,
            manifest_data,
        ).await?;

        Ok(Self {
            version_manager,
            storage_provider,
            game_name,
        })
    }

    /// Sync a file to storage, creating a new version
    pub async fn sync_file_to_storage<P: AsRef<Path>>(
        &mut self,
        local_file_path: P,
        relative_path: &str,
        description: Option<String>,
    ) -> Result<FileVersion> {
        let local_path = local_file_path.as_ref();
        
        // First create a version entry
        let version = self.version_manager.add_version(
            relative_path,
            local_path,
            HashMap::new(), // Will be filled by storage provider
            description,
        ).await?;

        // Read file data
        let file_data = tokio::fs::read(local_path).await
            .context("Failed to read file for sync")?;

        // Upload to storage
        let storage_result = self.storage_provider.upload_file(
            &self.game_name,
            relative_path,
            &version,
            &file_data,
        ).await?;

        if !storage_result.success {
            return Err(anyhow::anyhow!(
                "Failed to upload file to storage: {}", 
                storage_result.error.unwrap_or_default()
            ));
        }

        // Update version with storage metadata
        let manifest = self.version_manager.get_manifest();
        if let Some(file_manifest) = manifest.files.get(relative_path) {
            if let Some(updated_version) = file_manifest.versions.first() {
                // Storage metadata is already in the version from upload_file
                info!("Successfully synced {} to storage (version: {})", 
                      relative_path, updated_version.version_id);
            }
        }

        // Upload updated manifest
        let manifest_result = self.storage_provider.upload_manifest(
            &self.game_name,
            self.version_manager.get_manifest(),
        ).await?;

        if !manifest_result.success {
            warn!("Failed to upload manifest: {}", 
                  manifest_result.error.unwrap_or_default());
        }

        Ok(version)
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
    pub fn pin_version(&mut self, relative_path: &str, version_id: &str) -> Result<()> {
        self.version_manager.pin_version(relative_path, version_id)
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
}
