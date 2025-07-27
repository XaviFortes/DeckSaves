use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error};

use crate::versioning::{FileVersion, GameVersionManifest};

/// Storage backend identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    S3 { 
        bucket: String, 
        region: String,
        access_key: Option<String>,
        secret_key: Option<String>,
    },
    GoogleDrive { folder_id: String },
    WebDAV { base_url: String, username: String },
    Local { base_path: String },
}

/// Storage operation result with backend-specific metadata
#[derive(Debug, Clone)]
pub struct StorageResult {
    /// Success flag
    pub success: bool,
    /// Backend-specific metadata (e.g., S3 object key, Google Drive file ID)
    pub metadata: HashMap<String, String>,
    /// Optional error message
    pub error: Option<String>,
}

/// Storage abstraction trait for different backends
#[async_trait]
pub trait StorageProvider: Send + Sync {
    /// Upload file data to storage
    async fn upload_file(
        &self,
        game_name: &str,
        file_path: &str,
        version: &FileVersion,
        data: &[u8],
    ) -> Result<StorageResult>;

    /// Download file data from storage
    async fn download_file(
        &self,
        game_name: &str,
        file_path: &str,
        version: &FileVersion,
    ) -> Result<Vec<u8>>;

    /// Upload game version manifest
    async fn upload_manifest(
        &self,
        game_name: &str,
        manifest: &GameVersionManifest,
    ) -> Result<StorageResult>;

    /// Download game version manifest
    async fn download_manifest(&self, game_name: &str) -> Result<Option<GameVersionManifest>>;

    /// Delete a specific file version
    async fn delete_version(
        &self,
        game_name: &str,
        file_path: &str,
        version: &FileVersion,
    ) -> Result<StorageResult>;

    /// List all games in storage
    async fn list_games(&self) -> Result<Vec<String>>;

    /// Check if storage is accessible
    async fn health_check(&self) -> Result<bool>;

    /// Get storage backend information
    fn get_backend_info(&self) -> StorageBackend;
}

/// Storage configuration for different backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub backend: StorageBackend,
    pub connection_timeout_seconds: u64,
    pub retry_attempts: u32,
    pub enable_compression: bool,
    pub encryption_enabled: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Local {
                base_path: "~/.decksaves".to_string(),
            },
            connection_timeout_seconds: 30,
            retry_attempts: 3,
            enable_compression: true,
            encryption_enabled: true,
        }
    }
}

/// Factory for creating storage providers
pub struct StorageFactory;

impl StorageFactory {
    /// Create a storage provider based on config
    pub async fn create_provider(config: &StorageConfig) -> Result<Box<dyn StorageProvider>> {
        match &config.backend {
            StorageBackend::S3 { bucket, region, access_key, secret_key } => {
                let provider = S3StorageProvider::new(
                    bucket.clone(), 
                    region.clone(), 
                    access_key.clone(),
                    secret_key.clone(),
                    config.clone()
                ).await?;
                Ok(Box::new(provider))
            }
            StorageBackend::Local { base_path } => {
                let provider = LocalStorageProvider::new(base_path.clone(), config.clone())?;
                Ok(Box::new(provider))
            }
            // Future implementations
            StorageBackend::GoogleDrive { .. } => {
                Err(anyhow::anyhow!("Google Drive storage not implemented yet"))
            }
            StorageBackend::WebDAV { .. } => {
                Err(anyhow::anyhow!("WebDAV storage not implemented yet"))
            }
        }
    }
}

/// S3 storage provider implementation
pub struct S3StorageProvider {
    bucket: String,
    region: String,
    client: aws_sdk_s3::Client,
    config: StorageConfig,
}

impl S3StorageProvider {
    pub async fn new(
        bucket: String, 
        region: String, 
        access_key: Option<String>,
        secret_key: Option<String>,
        config: StorageConfig
    ) -> Result<Self> {
        // Create AWS config with optional custom credentials
        let aws_config = if let (Some(access_key), Some(secret_key)) = (access_key.clone(), secret_key.clone()) {
            debug!("Using custom AWS credentials for S3 storage provider");
            use aws_credential_types::Credentials;
            let credentials = Credentials::new(
                access_key,
                secret_key,
                None,
                None,
                "decksaves-storage"
            );
            
            aws_config::defaults(aws_config::BehaviorVersion::latest())
                .region(aws_config::Region::new(region.clone()))
                .credentials_provider(credentials)
                .load()
                .await
        } else {
            debug!("Using default AWS credentials for S3 storage provider");
            aws_config::defaults(aws_config::BehaviorVersion::latest())
                .region(aws_config::Region::new(region.clone()))
                .load()
                .await
        };

        let client = aws_sdk_s3::Client::new(&aws_config);

        Ok(Self {
            bucket,
            region,
            client,
            config,
        })
    }

    fn get_object_key(&self, game_name: &str, file_path: &str, version_id: &str) -> String {
        format!("games/{}/files/{}/versions/{}", game_name, file_path, version_id)
    }

    fn get_manifest_key(&self, game_name: &str) -> String {
        format!("games/{}/manifest.json", game_name)
    }
}

#[async_trait]
impl StorageProvider for S3StorageProvider {
    async fn upload_file(
        &self,
        game_name: &str,
        file_path: &str,
        version: &FileVersion,
        data: &[u8],
    ) -> Result<StorageResult> {
        let key = self.get_object_key(game_name, file_path, &version.version_id);
        
        debug!("🔧 S3 Upload Details:");
        debug!("   Bucket: {}", self.bucket);
        debug!("   Region: {}", self.region);
        debug!("   Key: {}", key);
        debug!("   Data size: {} bytes", data.len());
        debug!("   Version ID: {}", version.version_id);
        
        let mut put_request = self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(aws_sdk_s3::primitives::ByteStream::from(data.to_vec()));

        // Add metadata
        put_request = put_request
            .metadata("file-path", file_path)
            .metadata("version-id", &version.version_id)
            .metadata("file-hash", &version.hash)
            .metadata("timestamp", version.timestamp.to_rfc3339());

        debug!("📤 Sending S3 PutObject request...");
        let result = put_request.send().await;

        match result {
            Ok(output) => {
                debug!("✅ S3 PutObject successful!");
                debug!("   ETag: {:?}", output.e_tag());
                debug!("   Server Side Encryption: {:?}", output.server_side_encryption());
                debug!("   Version ID (S3): {:?}", output.version_id());
                
                let mut metadata = HashMap::new();
                metadata.insert("s3_bucket".to_string(), self.bucket.clone());
                metadata.insert("s3_key".to_string(), key);
                metadata.insert("s3_region".to_string(), self.region.clone());

                Ok(StorageResult {
                    success: true,
                    metadata,
                    error: None,
                })
            }
            Err(e) => {
                error!("❌ S3 PutObject failed: {}", e);
                debug!("   Error details: {:?}", e);
                Ok(StorageResult {
                    success: false,
                    metadata: HashMap::new(),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    async fn download_file(
        &self,
        game_name: &str,
        file_path: &str,
        version: &FileVersion,
    ) -> Result<Vec<u8>> {
        let key = self.get_object_key(game_name, file_path, &version.version_id);
        
        let result = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await?;

        let data = result.body.collect().await?.into_bytes().to_vec();
        Ok(data)
    }

    async fn upload_manifest(
        &self,
        game_name: &str,
        manifest: &GameVersionManifest,
    ) -> Result<StorageResult> {
        let key = self.get_manifest_key(game_name);
        let manifest_data = serde_json::to_vec_pretty(manifest)?;

        let result = self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(aws_sdk_s3::primitives::ByteStream::from(manifest_data))
            .content_type("application/json")
            .metadata("manifest-version", &manifest.manifest_version.to_string())
            .send()
            .await;

        match result {
            Ok(_) => {
                let mut metadata = HashMap::new();
                metadata.insert("s3_bucket".to_string(), self.bucket.clone());
                metadata.insert("s3_key".to_string(), key);

                Ok(StorageResult {
                    success: true,
                    metadata,
                    error: None,
                })
            }
            Err(e) => Ok(StorageResult {
                success: false,
                metadata: HashMap::new(),
                error: Some(e.to_string()),
            }),
        }
    }

    async fn download_manifest(&self, game_name: &str) -> Result<Option<GameVersionManifest>> {
        let key = self.get_manifest_key(game_name);
        
        match self.client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(result) => {
                let data = result.body.collect().await?.into_bytes();
                let manifest: GameVersionManifest = serde_json::from_slice(&data)?;
                Ok(Some(manifest))
            }
            Err(_) => {
                // Manifest doesn't exist yet
                Ok(None)
            }
        }
    }

    async fn delete_version(
        &self,
        game_name: &str,
        file_path: &str,
        version: &FileVersion,
    ) -> Result<StorageResult> {
        let key = self.get_object_key(game_name, file_path, &version.version_id);
        
        match self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(_) => Ok(StorageResult {
                success: true,
                metadata: HashMap::new(),
                error: None,
            }),
            Err(e) => Ok(StorageResult {
                success: false,
                metadata: HashMap::new(),
                error: Some(e.to_string()),
            }),
        }
    }

    async fn list_games(&self) -> Result<Vec<String>> {
        let mut games = Vec::new();
        let prefix = "games/";

        let result = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
            .delimiter("/")
            .send()
            .await?;

        if let Some(common_prefixes) = result.common_prefixes {
            for prefix_obj in common_prefixes {
                if let Some(prefix_str) = prefix_obj.prefix {
                    if let Some(game_name) = prefix_str.strip_prefix("games/").and_then(|s| s.strip_suffix("/")) {
                        games.push(game_name.to_string());
                    }
                }
            }
        }

        Ok(games)
    }

    async fn health_check(&self) -> Result<bool> {
        // Try to list objects in the bucket
        match self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .max_keys(1)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    fn get_backend_info(&self) -> StorageBackend {
        StorageBackend::S3 {
            bucket: self.bucket.clone(),
            region: self.region.clone(),
            access_key: None, // Don't expose credentials in backend info
            secret_key: None,
        }
    }
}

/// Local filesystem storage provider
pub struct LocalStorageProvider {
    base_path: std::path::PathBuf,
    config: StorageConfig,
}

impl LocalStorageProvider {
    pub fn new(base_path: String, config: StorageConfig) -> Result<Self> {
        let path = shellexpand::tilde(&base_path).into_owned();
        let base_path = std::path::PathBuf::from(path);
        
        // Create base directory if it doesn't exist
        std::fs::create_dir_all(&base_path)?;

        Ok(Self {
            base_path,
            config,
        })
    }

    fn get_file_path(&self, game_name: &str, file_path: &str, version_id: &str) -> std::path::PathBuf {
        self.base_path
            .join("games")
            .join(game_name)
            .join("files")
            .join(file_path)
            .join("versions")
            .join(version_id)
    }

    fn get_manifest_path(&self, game_name: &str) -> std::path::PathBuf {
        self.base_path
            .join("games")
            .join(game_name)
            .join("manifest.json")
    }
}

#[async_trait]
impl StorageProvider for LocalStorageProvider {
    async fn upload_file(
        &self,
        game_name: &str,
        file_path: &str,
        version: &FileVersion,
        data: &[u8],
    ) -> Result<StorageResult> {
        let file_path_buf = self.get_file_path(game_name, file_path, &version.version_id);
        
        if let Some(parent) = file_path_buf.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&file_path_buf, data).await?;

        let mut metadata = HashMap::new();
        metadata.insert("local_path".to_string(), file_path_buf.to_string_lossy().to_string());

        Ok(StorageResult {
            success: true,
            metadata,
            error: None,
        })
    }

    async fn download_file(
        &self,
        game_name: &str,
        file_path: &str,
        version: &FileVersion,
    ) -> Result<Vec<u8>> {
        let file_path = self.get_file_path(game_name, file_path, &version.version_id);
        let data = tokio::fs::read(file_path).await?;
        Ok(data)
    }

    async fn upload_manifest(
        &self,
        game_name: &str,
        manifest: &GameVersionManifest,
    ) -> Result<StorageResult> {
        let manifest_path = self.get_manifest_path(game_name);
        
        if let Some(parent) = manifest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let manifest_data = serde_json::to_vec_pretty(manifest)?;
        tokio::fs::write(&manifest_path, manifest_data).await?;

        let mut metadata = HashMap::new();
        metadata.insert("local_path".to_string(), manifest_path.to_string_lossy().to_string());

        Ok(StorageResult {
            success: true,
            metadata,
            error: None,
        })
    }

    async fn download_manifest(&self, game_name: &str) -> Result<Option<GameVersionManifest>> {
        let manifest_path = self.get_manifest_path(game_name);
        
        if !manifest_path.exists() {
            return Ok(None);
        }

        let data = tokio::fs::read(manifest_path).await?;
        let manifest: GameVersionManifest = serde_json::from_slice(&data)?;
        Ok(Some(manifest))
    }

    async fn delete_version(
        &self,
        game_name: &str,
        file_path: &str,
        version: &FileVersion,
    ) -> Result<StorageResult> {
        let file_path = self.get_file_path(game_name, file_path, &version.version_id);
        
        match tokio::fs::remove_file(file_path).await {
            Ok(_) => Ok(StorageResult {
                success: true,
                metadata: HashMap::new(),
                error: None,
            }),
            Err(e) => Ok(StorageResult {
                success: false,
                metadata: HashMap::new(),
                error: Some(e.to_string()),
            }),
        }
    }

    async fn list_games(&self) -> Result<Vec<String>> {
        let games_dir = self.base_path.join("games");
        let mut games = Vec::new();

        if !games_dir.exists() {
            return Ok(games);
        }

        let mut entries = tokio::fs::read_dir(games_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    games.push(name.to_string());
                }
            }
        }

        Ok(games)
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.base_path.exists() && self.base_path.is_dir())
    }

    fn get_backend_info(&self) -> StorageBackend {
        StorageBackend::Local {
            base_path: self.base_path.to_string_lossy().to_string(),
        }
    }
}
