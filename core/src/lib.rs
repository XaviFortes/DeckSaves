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
use tracing::{info, warn, error, debug};

pub mod config;
pub mod sync;
pub mod watcher;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub name: String,
    pub save_paths: Vec<String>,
    pub sync_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    pub peer_sync_enabled: bool,
    pub websocket_url: Option<String>,
    pub games: HashMap<String, GameConfig>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            s3_bucket: None,
            s3_region: Some("us-east-1".to_string()),
            peer_sync_enabled: false,
            websocket_url: None,
            games: HashMap::new(),
        }
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
        let s3_client = if config.s3_bucket.is_some() {
            let aws_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
            Some(Client::new(&aws_config))
        } else {
            None
        };

        Ok(Self { config, s3_client })
    }

    pub async fn sync_game(&self, game_name: &str) -> Result<()> {
        let game_config = self.config.games.get(game_name)
            .context("Game not found in configuration")?;

        if !game_config.sync_enabled {
            info!("Sync disabled for game: {}", game_name);
            return Ok(());
        }

        for save_path in &game_config.save_paths {
            self.sync_file(save_path, game_name).await?;
        }

        Ok(())
    }

    async fn sync_file(&self, file_path: &str, game_name: &str) -> Result<()> {
        if !Path::new(file_path).exists() {
            warn!("Save file does not exist: {}", file_path);
            return Ok(());
        }

        // Check if file is locked
        if self.is_file_locked(file_path).await? {
            warn!("File is locked, skipping sync: {}", file_path);
            return Ok(());
        }

        let data = fs::read(file_path).await
            .context("Failed to read save file")?;

        let hash = self.calculate_hash(&data);
        debug!("File hash for {}: {}", file_path, hash);

        if let Some(client) = &self.s3_client {
            self.upload_to_s3(client, &data, game_name, file_path).await?;
        }

        Ok(())
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

        let file_name = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let key = format!("{}/{}", game_name, file_name);

        client
            .put_object()
            .bucket(bucket)
            .key(&key)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await
            .context("Failed to upload to S3")?;

        info!("Uploaded {} to S3: {}", file_path, key);
        Ok(())
    }

    pub async fn download_from_s3(&self, game_name: &str, file_name: &str) -> Result<Vec<u8>> {
        let client = self.s3_client.as_ref()
            .context("S3 client not configured")?;
        let bucket = self.config.s3_bucket.as_ref()
            .context("S3 bucket not configured")?;

        let key = format!("{}/{}", game_name, file_name);

        let response = client
            .get_object()
            .bucket(bucket)
            .key(&key)
            .send()
            .await
            .context("Failed to download from S3")?;

        let data = response.body.collect().await?.into_bytes().to_vec();
        info!("Downloaded {} from S3", key);
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
