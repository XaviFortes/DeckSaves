use crate::{FileWatcher, GameSaveSync, process_file_events};
use anyhow::Result;
use tokio::task::JoinHandle;
use tracing::{info, error};
use std::collections::HashMap;

pub struct WatcherManager {
    watchers: HashMap<String, JoinHandle<()>>,
}

impl WatcherManager {
    pub fn new() -> Self {
        Self {
            watchers: HashMap::new(),
        }
    }

    pub async fn start_watching_game(
        &mut self,
        game_name: String,
        paths: Vec<String>,
        sync_handler: GameSaveSync,
    ) -> Result<()> {
        // Stop existing watcher if any
        self.stop_watching_game(&game_name).await;

        let (mut file_watcher, event_rx) = FileWatcher::new()?;

        // Start watching all paths for this game
        for path in &paths {
            if let Err(e) = file_watcher.watch_path(path).await {
                error!("Failed to watch path {}: {}", path, e);
            }
        }

        // Spawn the event processing task
        let game_name_clone = game_name.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = process_file_events(event_rx, sync_handler, game_name_clone.clone()).await {
                error!("Error processing file events for {}: {}", game_name_clone, e);
            }
        });

        self.watchers.insert(game_name.clone(), handle);
        info!("Started watching game: {} with paths: {:?}", game_name, paths);

        Ok(())
    }

    pub async fn stop_watching_game(&mut self, game_name: &str) {
        if let Some(handle) = self.watchers.remove(game_name) {
            handle.abort();
            info!("Stopped watching game: {}", game_name);
        }
    }

    pub async fn stop_all(&mut self) {
        for (game_name, handle) in self.watchers.drain() {
            handle.abort();
            info!("Stopped watching game: {}", game_name);
        }
    }

    pub fn is_watching(&self, game_name: &str) -> bool {
        self.watchers.contains_key(game_name)
    }

    pub fn watched_games(&self) -> Vec<String> {
        self.watchers.keys().cloned().collect()
    }
}

impl Default for WatcherManager {
    fn default() -> Self {
        Self::new()
    }
}
