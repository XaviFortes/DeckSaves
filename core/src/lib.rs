use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use tokio::sync::mpsc;
use tokio::fs;
use tokio::time::{Duration, sleep};
use aws_sdk_s3::{Client, primitives::ByteStream};
use aws_config::BehaviorVersion;
use sha2::{Sha256, Digest};
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn, error, debug};

pub mod config;
pub mod sync;
pub mod watcher;
pub mod daemon;
pub mod crypto;
pub mod steam;
pub mod versioning;
pub mod storage;
pub mod versioned_sync;

use crypto::CredentialCrypto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub name: String,
    pub save_paths: Vec<String>,
    pub sync_enabled: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FileInfo {
    pub path: String,
    pub modified_time: SystemTime,
    pub size: u64,
    pub hash: Option<String>,
}

#[derive(Debug, Clone)]
enum SyncAction {
    UploadToS3,
    DownloadFromS3,
    NoAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    #[serde(default)]
    pub aws_access_key_id: Option<String>,
    #[serde(default)]
    pub aws_secret_access_key: Option<String>,
    pub peer_sync_enabled: bool,
    pub websocket_url: Option<String>,
    pub games: HashMap<String, GameConfig>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            s3_bucket: None,
            s3_region: Some("us-east-1".to_string()),
            aws_access_key_id: None,
            aws_secret_access_key: None,
            peer_sync_enabled: false,
            websocket_url: None,
            games: HashMap::new(),
        }
    }
}

impl SyncConfig {
    /// Get decrypted AWS access key
    pub fn get_aws_access_key(&self) -> Result<Option<String>> {
        match &self.aws_access_key_id {
            Some(encrypted) if !encrypted.is_empty() => {
                let crypto = CredentialCrypto::new()?;
                Ok(Some(crypto.decrypt(encrypted)?))
            }
            _ => Ok(None)
        }
    }

    /// Get decrypted AWS secret key
    pub fn get_aws_secret_key(&self) -> Result<Option<String>> {
        match &self.aws_secret_access_key {
            Some(encrypted) if !encrypted.is_empty() => {
                let crypto = CredentialCrypto::new()?;
                Ok(Some(crypto.decrypt(encrypted)?))
            }
            _ => Ok(None)
        }
    }

    /// Set encrypted AWS access key
    pub fn set_aws_access_key(&mut self, key: &str) -> Result<()> {
        println!("DEBUG CRYPTO: set_aws_access_key called with key length: {}", key.len());
        if key.is_empty() {
            println!("DEBUG CRYPTO: Key is empty, setting to None");
            self.aws_access_key_id = None;
        } else {
            println!("DEBUG CRYPTO: Encrypting key...");
            let crypto = CredentialCrypto::new().map_err(|e| {
                println!("DEBUG CRYPTO: Failed to create crypto: {}", e);
                e
            })?;
            let encrypted = crypto.encrypt(key).map_err(|e| {
                println!("DEBUG CRYPTO: Failed to encrypt: {}", e);
                e
            })?;
            println!("DEBUG CRYPTO: Encrypted key length: {}", encrypted.len());
            println!("DEBUG CRYPTO: About to set aws_access_key_id to encrypted value");
            self.aws_access_key_id = Some(encrypted.clone());
            println!("DEBUG CRYPTO: Set aws_access_key_id, is_some: {}, value: {}", 
                     self.aws_access_key_id.is_some(),
                     encrypted.chars().take(10).collect::<String>());
        }
        Ok(())
    }

    /// Set encrypted AWS secret key
    pub fn set_aws_secret_key(&mut self, key: &str) -> Result<()> {
        println!("DEBUG CRYPTO: set_aws_secret_key called with key length: {}", key.len());
        if key.is_empty() {
            println!("DEBUG CRYPTO: Secret key is empty, setting to None");
            self.aws_secret_access_key = None;
        } else {
            println!("DEBUG CRYPTO: Encrypting secret key...");
            let crypto = CredentialCrypto::new().map_err(|e| {
                println!("DEBUG CRYPTO: Failed to create crypto for secret: {}", e);
                e
            })?;
            let encrypted = crypto.encrypt(key).map_err(|e| {
                println!("DEBUG CRYPTO: Failed to encrypt secret: {}", e);
                e
            })?;
            println!("DEBUG CRYPTO: Encrypted secret key length: {}", encrypted.len());
            println!("DEBUG CRYPTO: About to set aws_secret_access_key to encrypted value");
            self.aws_secret_access_key = Some(encrypted.clone());
            println!("DEBUG CRYPTO: Set aws_secret_access_key, is_some: {}, value: {}", 
                     self.aws_secret_access_key.is_some(),
                     encrypted.chars().take(10).collect::<String>());
        }
        Ok(())
    }
}

pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    event_tx: mpsc::Sender<Event>,
}

impl FileWatcher {
    pub fn new() -> Result<(Self, mpsc::Receiver<Event>)> {
        let (tx, rx) = mpsc::channel(100);
        Ok((
            Self {
                watcher: None,
                event_tx: tx,
            },
            rx,
        ))
    }

    pub async fn watch_path(&mut self, path: &str) -> Result<()> {
        let tx = self.event_tx.clone();
        let mut watcher: RecommendedWatcher = Watcher::new(
            move |res: notify::Result<Event>| {
                match res {
                    Ok(event) => {
                        if let Err(e) = tx.blocking_send(event) {
                            error!("Failed to send file event: {}", e);
                        }
                    }
                    Err(e) => error!("Watch error: {}", e),
                }
            },
            notify::Config::default(),
        )?;

        watcher.watch(Path::new(path), RecursiveMode::Recursive)?;
        self.watcher = Some(watcher);
        info!("Started watching path: {}", path);
        Ok(())
    }
}

pub struct GameSaveSync {
    config: SyncConfig,
    s3_client: Option<Client>,
}

impl GameSaveSync {
    pub async fn new(config: SyncConfig) -> Result<Self> {
        debug!("Creating GameSaveSync with bucket: {:?}", config.s3_bucket);
        
        let s3_client = if config.s3_bucket.is_some() {
            debug!("S3 bucket configured, setting up client...");
            
            // Get decrypted credentials
            let access_key = config.get_aws_access_key().context("Failed to get AWS access key")?;
            let secret_key = config.get_aws_secret_key().context("Failed to get AWS secret key")?;
            
            debug!("Access key present: {}", access_key.is_some());
            debug!("Secret key present: {}", secret_key.is_some());
            
            let aws_config = if let (Some(access_key), Some(secret_key)) = (access_key, secret_key) {
                debug!("Using custom credentials for AWS client");
                // Create custom credentials
                use aws_credential_types::Credentials;
                let credentials = Credentials::new(
                    access_key,
                    secret_key,
                    None,
                    None,
                    "decksaves"
                );
                
                aws_config::defaults(BehaviorVersion::latest())
                    .region(aws_config::Region::new(config.s3_region.clone().unwrap_or_else(|| "us-east-1".to_string())))
                    .credentials_provider(credentials)
                    .load()
                    .await
            } else {
                debug!("Using default AWS credentials (IAM role, environment, etc.)");
                // Use default credentials (IAM role, environment, etc.)
                aws_config::defaults(BehaviorVersion::latest())
                    .load()
                    .await
            };
            
            Some(Client::new(&aws_config))
        } else {
            debug!("No S3 bucket configured, skipping S3 client setup");
            None
        };

        Ok(Self { config, s3_client })
    }

    pub async fn sync_game(&self, game_name: &str) -> Result<()> {
        debug!("sync_game starting for: {}", game_name);
        let game_config = self.config.games.get(game_name)
            .context("Game not found in configuration")?;

        if !game_config.sync_enabled {
            info!("Sync disabled for game: {}", game_name);
            return Ok(());
        }

        debug!("Processing {} save paths for game: {}", game_config.save_paths.len(), game_name);
        for save_path in &game_config.save_paths {
            debug!("Syncing save path: {}", save_path);
            self.sync_file(save_path, game_name).await?;
            debug!("Completed syncing save path: {}", save_path);
        }

        debug!("sync_game completed successfully for: {}", game_name);
        Ok(())
    }

    async fn sync_file(&self, file_path: &str, game_name: &str) -> Result<()> {
        debug!("sync_file called for: {} (game: {})", file_path, game_name);
        
        let path = Path::new(file_path);
        
        if !path.exists() {
            warn!("Save path does not exist: {}", file_path);
            // Check if file exists in S3 and download it
            return self.download_missing_file(file_path, game_name).await;
        }

        if path.is_dir() {
            debug!("Path is a directory, syncing all files within: {}", file_path);
            return self.sync_directory(file_path, game_name).await;
        }

        // Handle individual file with bidirectional sync
        self.sync_file_bidirectional(file_path, game_name).await
    }

    async fn sync_file_bidirectional(&self, file_path: &str, game_name: &str) -> Result<()> {
        debug!("Starting bidirectional sync for: {}", file_path);
        
        // Check if file is locked
        if self.is_file_locked(file_path).await? {
            warn!("File is locked, skipping sync: {}", file_path);
            return Ok(());
        }

        let local_info = self.get_local_file_info(file_path).await?;
        let cloud_info = self.get_cloud_file_info(file_path, game_name).await;

        let action = self.determine_sync_action(&local_info, &cloud_info);
        
        match action {
            SyncAction::UploadToS3 => {
                debug!("Local file is newer, uploading to S3: {}", file_path);
                let data = fs::read(file_path).await
                    .context("Failed to read local file")?;
                
                if let Some(client) = &self.s3_client {
                    self.upload_to_s3(client, &data, game_name, file_path).await?;
                    info!("Uploaded newer local file to S3: {}", file_path);
                } else {
                    warn!("No S3 client configured, cannot upload: {}", file_path);
                }
            },
            SyncAction::DownloadFromS3 => {
                debug!("Cloud file is newer, downloading from S3: {}", file_path);
                let file_name = Path::new(file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context("Invalid file name")?;
                
                let data = self.download_from_s3(game_name, file_name).await?;
                
                // Create directory if it doesn't exist
                if let Some(parent) = Path::new(file_path).parent() {
                    fs::create_dir_all(parent).await
                        .context("Failed to create directory")?;
                }
                
                fs::write(file_path, &data).await
                    .context("Failed to write downloaded file")?;
                
                info!("Downloaded newer cloud file from S3: {}", file_path);
            },
            SyncAction::NoAction => {
                debug!("Files are in sync, no action needed: {}", file_path);
            }
        }

        Ok(())
    }

    async fn download_missing_file(&self, file_path: &str, game_name: &str) -> Result<()> {
        debug!("Checking if missing local file exists in S3: {}", file_path);
        
        let file_name = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid file name")?;
        
        match self.download_from_s3(game_name, file_name).await {
            Ok(data) => {
                // Create directory if it doesn't exist
                if let Some(parent) = Path::new(file_path).parent() {
                    fs::create_dir_all(parent).await
                        .context("Failed to create directory")?;
                }
                
                fs::write(file_path, &data).await
                    .context("Failed to write downloaded file")?;
                
                info!("Downloaded missing file from S3: {}", file_path);
                Ok(())
            },
            Err(_) => {
                debug!("File does not exist in S3 either: {}", file_path);
                Ok(())
            }
        }
    }

    async fn get_local_file_info(&self, file_path: &str) -> Result<FileInfo> {
        let metadata = fs::metadata(file_path).await
            .context("Failed to get file metadata")?;
        
        let modified_time = metadata.modified()
            .context("Failed to get file modification time")?;
        
        let size = metadata.len();
        
        Ok(FileInfo {
            path: file_path.to_string(),
            modified_time,
            size,
            hash: None, // We'll calculate hash only when needed
        })
    }

    async fn get_cloud_file_info(&self, file_path: &str, game_name: &str) -> Option<FileInfo> {
        let client = self.s3_client.as_ref()?;
        let bucket = self.config.s3_bucket.as_ref()?;
        
        let file_name = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())?;
        
        let key = format!("{}/{}", game_name, file_name);
        
        match client.head_object()
            .bucket(bucket)
            .key(&key)
            .send()
            .await 
        {
            Ok(response) => {
                let modified_time = response.last_modified()
                    .and_then(|dt| {
                        SystemTime::UNIX_EPOCH.checked_add(
                            std::time::Duration::from_secs(dt.secs() as u64)
                        )
                    })
                    .unwrap_or(UNIX_EPOCH);
                
                let size = response.content_length().unwrap_or(0) as u64;
                
                // Get ETag as hash (remove quotes if present)
                let etag_hash = response.e_tag()
                    .map(|etag| etag.trim_matches('"').to_string());
                
                debug!("Found cloud file: {} (size: {}, modified: {:?}, etag: {:?})", 
                       key, size, modified_time, etag_hash);
                
                Some(FileInfo {
                    path: file_path.to_string(),
                    modified_time,
                    size,
                    hash: etag_hash,
                })
            },
            Err(e) => {
                debug!("Cloud file not found or error accessing: {} - {}", key, e);
                None
            }
        }
    }

    fn determine_sync_action(&self, local_info: &FileInfo, cloud_info: &Option<FileInfo>) -> SyncAction {
        match cloud_info {
            None => {
                debug!("No cloud file found, uploading local file");
                SyncAction::UploadToS3
            },
            Some(cloud) => {
                // Calculate time difference in seconds
                let time_diff = if local_info.modified_time > cloud.modified_time {
                    local_info.modified_time.duration_since(cloud.modified_time)
                        .unwrap_or(std::time::Duration::from_secs(0))
                        .as_secs()
                } else {
                    cloud.modified_time.duration_since(local_info.modified_time)
                        .unwrap_or(std::time::Duration::from_secs(0))
                        .as_secs()
                };
                
                // Consider files with timestamp difference <= 2 seconds as "same time" 
                // to account for filesystem timestamp precision differences
                if time_diff <= 2 {
                    // Timestamps are very close, compare sizes
                    if local_info.size != cloud.size {
                        debug!("Similar timestamps but different sizes (local: {}, cloud: {}), files are different - preferring local", 
                               local_info.size, cloud.size);
                        SyncAction::UploadToS3
                    } else {
                        // Same size and similar timestamp - files are almost certainly identical
                        debug!("Files have similar timestamps (diff: {}s) and same size ({} bytes) - skipping sync to avoid unnecessary S3 operations", 
                               time_diff, local_info.size);
                        SyncAction::NoAction
                    }
                } else {
                    // Significant time difference, use the newer file
                    match local_info.modified_time.cmp(&cloud.modified_time) {
                        std::cmp::Ordering::Greater => {
                            debug!("Local file is significantly newer (local: {:?}, cloud: {:?}, diff: {}s)", 
                                   local_info.modified_time, cloud.modified_time, time_diff);
                            SyncAction::UploadToS3
                        },
                        std::cmp::Ordering::Less => {
                            debug!("Cloud file is significantly newer (local: {:?}, cloud: {:?}, diff: {}s)", 
                                   local_info.modified_time, cloud.modified_time, time_diff);
                            SyncAction::DownloadFromS3
                        },
                        std::cmp::Ordering::Equal => {
                            // This shouldn't happen given our time_diff check, but handle it
                            debug!("Timestamps are equal, no sync needed");
                            SyncAction::NoAction
                        }
                    }
                }
            }
        }
    }
    
    #[allow(dead_code)]
    async fn compare_file_hashes(&self, local_path: &str, cloud_hash: &str) -> Result<bool> {
        debug!("Performing hash comparison for: {}", local_path);
        
        let local_data = fs::read(local_path).await
            .context("Failed to read local file for hash comparison")?;
        
        let local_hash = self.calculate_hash(&local_data);
        let cloud_hash_clean = cloud_hash.trim_matches('"');
        
        debug!("Hash comparison - local: {}, cloud: {}", local_hash, cloud_hash_clean);
        
        Ok(local_hash == cloud_hash_clean)
    }

    async fn sync_directory(&self, dir_path: &str, game_name: &str) -> Result<()> {
        debug!("Syncing directory: {}", dir_path);
        
        // Get all local files
        let mut local_files = Vec::new();
        let mut entries = fs::read_dir(dir_path).await
            .context("Failed to read directory")?;

        while let Some(entry) = entries.next_entry().await
            .context("Failed to read directory entry")? {
            
            let path = entry.path();
            
            if path.is_file() {
                let file_path = path.to_str()
                    .context("Invalid file path")?;
                local_files.push(file_path.to_string());
            } else if path.is_dir() {
                // Optionally handle subdirectories recursively
                debug!("Skipping subdirectory: {:?}", path);
            }
        }

        // Get all cloud files for this game
        let cloud_files = self.list_cloud_files(game_name).await?;
        
        // Create a set of all unique file names (local and cloud)
        let mut all_files = std::collections::HashSet::new();
        
        // Add local file names
        for file_path in &local_files {
            if let Some(file_name) = Path::new(file_path).file_name().and_then(|n| n.to_str()) {
                all_files.insert(file_name.to_string());
            }
        }
        
        // Add cloud file names
        for cloud_file in &cloud_files {
            all_files.insert(cloud_file.clone());
        }
        
        // Sync each unique file
        for file_name in all_files {
            let local_file_path = format!("{}/{}", dir_path.trim_end_matches('/'), file_name);
            
            if Path::new(&local_file_path).exists() {
                // File exists locally, do bidirectional sync
                self.sync_file_bidirectional(&local_file_path, game_name).await?;
            } else {
                // File only exists in cloud, download it
                debug!("File only exists in cloud, downloading: {}", file_name);
                match self.download_from_s3(game_name, &file_name).await {
                    Ok(data) => {
                        fs::write(&local_file_path, &data).await
                            .context("Failed to write downloaded file")?;
                        info!("Downloaded cloud-only file: {}", local_file_path);
                    },
                    Err(e) => {
                        warn!("Failed to download cloud file {}: {}", file_name, e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn list_cloud_files(&self, game_name: &str) -> Result<Vec<String>> {
        let client = self.s3_client.as_ref()
            .context("S3 client not configured")?;
        let bucket = self.config.s3_bucket.as_ref()
            .context("S3 bucket not configured")?;

        let prefix = format!("{}/", game_name);
        
        match client.list_objects_v2()
            .bucket(bucket)
            .prefix(&prefix)
            .send()
            .await 
        {
            Ok(response) => {
                let mut files = Vec::new();
                
                let contents = response.contents();
                for object in contents {
                    if let Some(key) = object.key() {
                        // Remove the prefix to get just the file name
                        if let Some(file_name) = key.strip_prefix(&prefix) {
                            if !file_name.is_empty() && !file_name.ends_with('/') {
                                files.push(file_name.to_string());
                            }
                        }
                    }
                }
                
                debug!("Found {} cloud files for game {}", files.len(), game_name);
                Ok(files)
            },
            Err(e) => {
                debug!("Failed to list cloud files for game {}: {}", game_name, e);
                Ok(Vec::new()) // Return empty list on error
            }
        }
    }

    async fn is_file_locked(&self, file_path: &str) -> Result<bool> {
        // Try to open the file in append mode to check if it's locked
        match fs::OpenOptions::new()
            .append(true)
            .open(file_path)
            .await
        {
            Ok(_) => Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Ok(true),
            Err(e) => Err(e.into()),
        }
    }

    #[allow(dead_code)]
    fn calculate_hash(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    async fn upload_to_s3(
        &self,
        client: &Client,
        data: &[u8],
        game_name: &str,
        file_path: &str,
    ) -> Result<()> {
        let bucket = self.config.s3_bucket.as_ref()
            .context("S3 bucket not configured")?;

        debug!("Uploading to S3 bucket: {}", bucket);

        let file_name = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let key = format!("{}/{}", game_name, file_name);
        debug!("S3 key: {}, data size: {} bytes", key, data.len());

        let result = client
            .put_object()
            .bucket(bucket)
            .key(&key)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await;

        match result {
            Ok(_) => {
                info!("Successfully uploaded {} to S3: {}", file_path, key);
                Ok(())
            }
            Err(e) => {
                error!("Failed to upload {} to S3: {}", file_path, e);
                Err(e.into())
            }
        }
    }

    pub async fn download_from_s3(&self, game_name: &str, file_name: &str) -> Result<Vec<u8>> {
        let client = self.s3_client.as_ref()
            .context("S3 client not configured")?;
        let bucket = self.config.s3_bucket.as_ref()
            .context("S3 bucket not configured")?;

        let key = format!("{}/{}", game_name, file_name);

        debug!("Downloading from S3: bucket={}, key={}", bucket, key);

        let response = client
            .get_object()
            .bucket(bucket)
            .key(&key)
            .send()
            .await
            .context(format!("Failed to download {} from S3", key))?;

        let data = response.body.collect().await?.into_bytes().to_vec();
        debug!("Downloaded {} bytes from S3: {}", data.len(), key);
        Ok(data)
    }
}

pub async fn process_file_events(
    mut event_rx: mpsc::Receiver<Event>,
    sync_handler: GameSaveSync,
    game_name: String,
) -> Result<()> {
    let mut pending_events: HashMap<String, Event> = HashMap::new();
    let mut last_batch_time = std::time::Instant::now();

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                // Only process write events
                if matches!(event.kind, EventKind::Modify(_)) {
                    for path in &event.paths {
                        if let Some(path_str) = path.to_str() {
                            pending_events.insert(path_str.to_string(), event.clone());
                        }
                    }
                    last_batch_time = std::time::Instant::now();
                }
            }
            _ = sleep(Duration::from_millis(500)) => {
                // Process batched events after 500ms of inactivity
                if !pending_events.is_empty() && last_batch_time.elapsed() > Duration::from_millis(500) {
                    for (path, _) in pending_events.drain() {
                        debug!("Processing file change: {}", path);
                        if let Err(e) = sync_handler.sync_file(&path, &game_name).await {
                            error!("Failed to sync file {}: {}", path, e);
                        }
                    }
                }
            }
        }
    }
}
