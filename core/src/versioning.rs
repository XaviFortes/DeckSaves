use anyhow::{Result, Context};
use chrono::{DateTime, Utc, Datelike};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info};

/// Unique identifier for a file version
pub type VersionId = String;

/// File hash for content verification
pub type FileHash = String;

/// Storage-agnostic file version metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVersion {
    /// Unique version identifier (timestamp + hash)
    pub version_id: VersionId,
    /// Timestamp when this version was created
    pub timestamp: DateTime<Utc>,
    /// Size of the file in bytes
    pub size: u64,
    /// SHA256 hash of the file content
    pub hash: FileHash,
    /// Storage-specific metadata (e.g., S3 object key, Google Drive file ID)
    pub storage_metadata: HashMap<String, String>,
    /// Human-readable description (optional)
    pub description: Option<String>,
    /// Whether this version is marked as important/pinned
    pub is_pinned: bool,
}

/// Manifest containing all versions of a specific file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVersionManifest {
    /// Relative path of the file (game-relative)
    pub file_path: String,
    /// All versions of this file, ordered by timestamp (newest first)
    pub versions: Vec<FileVersion>,
    /// Currently active/latest version
    pub current_version: Option<VersionId>,
    /// Maximum number of versions to keep (None = unlimited)
    pub max_versions: Option<u32>,
}

/// Game-level version manifest containing all files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameVersionManifest {
    /// Game name/identifier
    pub game_name: String,
    /// Manifest format version for future compatibility
    pub manifest_version: u32,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
    /// File manifests keyed by relative file path
    pub files: HashMap<String, FileVersionManifest>,
    /// Game-level metadata
    pub metadata: HashMap<String, String>,
}

/// Version management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionConfig {
    /// Maximum versions to keep per file (default: 10)
    pub max_versions_per_file: u32,
    /// Maximum age for versions in days (default: 30)
    pub max_version_age_days: u32,
    /// Always keep pinned versions regardless of limits
    pub keep_pinned_versions: bool,
    /// Automatically pin versions (e.g., daily, weekly)
    pub auto_pin_strategy: AutoPinStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutoPinStrategy {
    None,
    Daily,
    Weekly,
    Monthly,
    OnMajorChanges, // Pin when file size changes significantly
}

impl Default for VersionConfig {
    fn default() -> Self {
        Self {
            max_versions_per_file: 10,
            max_version_age_days: 30,
            keep_pinned_versions: true,
            auto_pin_strategy: AutoPinStrategy::Weekly,
        }
    }
}

/// Storage-agnostic version manager
pub struct VersionManager {
    config: VersionConfig,
    game_manifest: GameVersionManifest,
}

impl VersionManager {
    /// Create a new version manager for a game
    pub fn new(game_name: String, config: VersionConfig) -> Self {
        let manifest = GameVersionManifest {
            game_name: game_name.clone(),
            manifest_version: 1,
            last_updated: Utc::now(),
            files: HashMap::new(),
            metadata: HashMap::new(),
        };

        Self {
            config,
            game_manifest: manifest,
        }
    }

    /// Load existing manifest from storage or create new one
    pub async fn load_or_create(
        game_name: String,
        config: VersionConfig,
        manifest_data: Option<Vec<u8>>,
    ) -> Result<Self> {
        if let Some(data) = manifest_data {
            let manifest: GameVersionManifest = serde_json::from_slice(&data)
                .context("Failed to parse game version manifest")?;
            
            Ok(Self {
                config,
                game_manifest: manifest,
            })
        } else {
            Ok(Self::new(game_name, config))
        }
    }

    /// Add a new version of a file
    pub async fn add_version<P1: AsRef<str>, P2: AsRef<Path>>(
        &mut self,
        file_path: P1,
        local_path: P2,
        storage_metadata: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<FileVersion> {
        let file_path = file_path.as_ref().to_string();
        let local_path = local_path.as_ref();

        // Read file and calculate metadata
        let file_data = fs::read(local_path).await
            .context("Failed to read file for versioning")?;
        
        let size = file_data.len() as u64;
        let hash = calculate_hash(&file_data);
        let timestamp = Utc::now();
        
        // Generate unique version ID
        let version_id = generate_version_id(&timestamp, &hash);

        // Check if we should auto-pin this version
        let is_pinned = self.should_auto_pin(&file_path, size);

        let version = FileVersion {
            version_id: version_id.clone(),
            timestamp,
            size,
            hash,
            storage_metadata,
            description,
            is_pinned,
        };

        // Add to manifest
        let file_manifest = self.game_manifest.files
            .entry(file_path.clone())
            .or_insert_with(|| FileVersionManifest {
                file_path: file_path.clone(),
                versions: Vec::new(),
                current_version: None,
                max_versions: Some(self.config.max_versions_per_file),
            });

        // Insert version in chronological order (newest first)
        file_manifest.versions.insert(0, version.clone());
        file_manifest.current_version = Some(version_id.clone());

        // Clean up old versions
        self.cleanup_old_versions(&file_path).await?;

        // Update manifest timestamp
        self.game_manifest.last_updated = Utc::now();

        info!("Added version {} for file {}", version_id, file_path);
        Ok(version)
    }

    /// Get the version configuration
    pub fn get_config(&self) -> &VersionConfig {
        &self.config
    }

    /// Get all versions of a file
    pub fn get_file_versions(&self, file_path: &str) -> Option<&Vec<FileVersion>> {
        self.game_manifest.files.get(file_path)
            .map(|manifest| &manifest.versions)
    }

    /// Get a specific version of a file
    pub fn get_version(&self, file_path: &str, version_id: &str) -> Option<&FileVersion> {
        self.game_manifest.files.get(file_path)?
            .versions.iter()
            .find(|v| v.version_id == version_id)
    }

    /// Get the current/latest version of a file
    pub fn get_current_version(&self, file_path: &str) -> Option<&FileVersion> {
        let manifest = self.game_manifest.files.get(file_path)?;
        let current_id = manifest.current_version.as_ref()?;
        self.get_version(file_path, current_id)
    }

    /// Pin or unpin a specific version to prevent cleanup
    pub fn pin_version(&mut self, file_path: &str, version_id: &str) -> Result<()> {
        let manifest = self.game_manifest.files.get_mut(file_path)
            .context("File not found in manifest")?;
        
        let version = manifest.versions.iter_mut()
            .find(|v| v.version_id == version_id)
            .context("Version not found")?;
        
        // Toggle the pin status
        version.is_pinned = !version.is_pinned;
        
        if version.is_pinned {
            info!("Pinned version {} for file {}", version_id, file_path);
        } else {
            info!("Unpinned version {} for file {}", version_id, file_path);
        }
        
        Ok(())
    }

    /// Remove a specific version (if not pinned)
    pub fn remove_version(&mut self, file_path: &str, version_id: &VersionId) -> Result<FileVersion> {
        let manifest = self.game_manifest.files.get_mut(file_path)
            .context("File not found in manifest")?;
        
        let version_index = manifest.versions.iter()
            .position(|v| v.version_id == *version_id)
            .context("Version not found")?;
        
        let version = &manifest.versions[version_index];
        if version.is_pinned {
            return Err(anyhow::anyhow!("Cannot remove pinned version"));
        }

        let removed_version = manifest.versions.remove(version_index);
        
        // Update current version if we removed it
        if manifest.current_version.as_ref() == Some(version_id) {
            manifest.current_version = manifest.versions.first().map(|v| v.version_id.clone());
        }

        info!("Removed version {} for file {}", version_id, file_path);
        Ok(removed_version)
    }

    /// Serialize manifest for storage
    pub fn serialize_manifest(&self) -> Result<Vec<u8>> {
        serde_json::to_vec_pretty(&self.game_manifest)
            .context("Failed to serialize game manifest")
    }

    /// Get manifest for inspection
    pub fn get_manifest(&self) -> &GameVersionManifest {
        &self.game_manifest
    }

    /// Clean up old versions based on config
    async fn cleanup_old_versions(&mut self, file_path: &str) -> Result<()> {
        let manifest = self.game_manifest.files.get_mut(file_path)
            .context("File manifest not found")?;

        let now = Utc::now();
        let max_age = chrono::Duration::days(self.config.max_version_age_days as i64);
        let cutoff_date = now - max_age;

        // Separate pinned and unpinned versions
        let mut pinned_versions = Vec::new();
        let mut unpinned_versions = Vec::new();

        for version in manifest.versions.drain(..) {
            if version.is_pinned || (self.config.keep_pinned_versions && version.is_pinned) {
                pinned_versions.push(version);
            } else if version.timestamp > cutoff_date {
                unpinned_versions.push(version);
            } else {
                debug!("Removing old version {} (age: {} days)", 
                       version.version_id, 
                       (now - version.timestamp).num_days());
            }
        }

        // Keep only the most recent unpinned versions
        unpinned_versions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        unpinned_versions.truncate(self.config.max_versions_per_file as usize);

        // Combine and sort all kept versions
        manifest.versions = pinned_versions;
        manifest.versions.extend(unpinned_versions);
        manifest.versions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(())
    }

    /// Check if a version should be automatically pinned
    fn should_auto_pin(&self, file_path: &str, new_size: u64) -> bool {
        match self.config.auto_pin_strategy {
            AutoPinStrategy::None => false,
            AutoPinStrategy::OnMajorChanges => {
                // Pin if file size changed by more than 20%
                if let Some(current) = self.get_current_version(file_path) {
                    let size_diff = (new_size as f64 - current.size as f64).abs();
                    let size_change_ratio = size_diff / current.size as f64;
                    size_change_ratio > 0.2
                } else {
                    true // Pin first version
                }
            },
            AutoPinStrategy::Daily => {
                // Pin if no version today
                let now = Utc::now();
                !self.has_version_today(file_path, now)
            },
            AutoPinStrategy::Weekly => {
                // Pin if no version this week
                let now = Utc::now();
                !self.has_version_this_week(file_path, now)
            },
            AutoPinStrategy::Monthly => {
                // Pin if no version this month
                let now = Utc::now();
                !self.has_version_this_month(file_path, now)
            },
        }
    }

    fn has_version_today(&self, file_path: &str, now: DateTime<Utc>) -> bool {
        self.get_file_versions(file_path)
            .map(|versions| {
                versions.iter().any(|v| {
                    v.timestamp.date_naive() == now.date_naive()
                })
            })
            .unwrap_or(false)
    }

    fn has_version_this_week(&self, file_path: &str, now: DateTime<Utc>) -> bool {
        self.get_file_versions(file_path)
            .map(|versions| {
                let week_start = now.date_naive() - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
                versions.iter().any(|v| {
                    v.timestamp.date_naive() >= week_start
                })
            })
            .unwrap_or(false)
    }

    fn has_version_this_month(&self, file_path: &str, now: DateTime<Utc>) -> bool {
        self.get_file_versions(file_path)
            .map(|versions| {
                versions.iter().any(|v| {
                    v.timestamp.year() == now.year() && v.timestamp.month() == now.month()
                })
            })
            .unwrap_or(false)
    }
}

/// Calculate SHA256 hash of file content
pub fn calculate_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Generate unique version ID from timestamp and hash
pub fn generate_version_id(timestamp: &DateTime<Utc>, hash: &str) -> String {
    format!("{}_{}", timestamp.format("%Y%m%d_%H%M%S_%f"), &hash[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_version_manager_basic() {
        let config = VersionConfig::default();
        let mut vm = VersionManager::new("test_game".to_string(), config);

        // Create a temporary file
        let temp_dir = std::env::temp_dir();
        let temp_file_path = temp_dir.join("test_save.dat");
        tokio::fs::write(&temp_file_path, b"test content").await.unwrap();

        // Add version
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("storage_key".to_string(), "test/save.dat".to_string());

        let version = vm.add_version(
            "save.dat",
            &temp_file_path,
            metadata,
            Some("Test version".to_string()),
        ).await.unwrap();

        assert_eq!(vm.get_file_versions("save.dat").unwrap().len(), 1);
        assert_eq!(vm.get_current_version("save.dat").unwrap().version_id, version.version_id);

        // Clean up
        let _ = tokio::fs::remove_file(&temp_file_path).await;
    }

    #[test]
    fn test_hash_calculation() {
        let data = b"test content";
        let hash = calculate_hash(data);
        assert_eq!(hash.len(), 64); // SHA256 produces 64 character hex string
    }

    #[test]
    fn test_version_id_generation() {
        let timestamp = Utc::now();
        let hash = "abcdef1234567890";
        let version_id = generate_version_id(&timestamp, hash);
        assert!(version_id.contains("_abcdef12"));
    }
}
