use tauri::{command, State, AppHandle, Emitter};
use decksaves_core::{
    config::ConfigManager, 
    GameSaveSync, 
    GameConfig, 
    SyncConfig,
    watcher::WatcherManager,
    steam::{SteamDetector, SteamGame},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use uuid::Uuid;
use tracing::{info, error, debug};
use chrono;

// Application state
pub struct AppState {
    pub config_manager: ConfigManager,
    pub watcher_manager: Arc<Mutex<WatcherManager>>,
    pub sync_sessions: Arc<Mutex<HashMap<String, String>>>, // game_name -> session_id
    pub sync_history: Arc<Mutex<HashMap<String, String>>>, // game_name -> last_sync_timestamp
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let config_manager = ConfigManager::new().map_err(|e| e.to_string())?;
        let watcher_manager = Arc::new(Mutex::new(WatcherManager::new()));
        let sync_sessions = Arc::new(Mutex::new(HashMap::new()));
        let sync_history = Arc::new(Mutex::new(HashMap::new()));

        Ok(Self {
            config_manager,
            watcher_manager,
            sync_sessions,
            sync_history,
        })
    }
}

// Configuration commands
#[command]
pub async fn get_config(state: State<'_, AppState>) -> Result<SyncConfig, String> {
    state.config_manager.load_config().await.map_err(|e| e.to_string())
}

#[command]
pub async fn save_config(config: SyncConfig, state: State<'_, AppState>) -> Result<String, String> {
    state.config_manager.save_config(&config).await.map_err(|e| e.to_string())?;
    Ok("Configuration saved successfully".to_string())
}

#[command]
pub async fn set_aws_credentials(
    access_key_id: String,
    secret_access_key: String,
    state: State<'_, AppState>
) -> Result<String, String> {
    println!("DEBUG: set_aws_credentials called with access_key_id length: {}, secret_access_key length: {}", 
             access_key_id.len(), secret_access_key.len());
    
    let mut config = state.config_manager.load_config().await.map_err(|e| {
        println!("DEBUG: Failed to load config in set_aws_credentials: {}", e);
        e.to_string()
    })?;
    
    println!("DEBUG: Config loaded, setting credentials...");
    
    // Use the encryption methods
    config.set_aws_access_key(&access_key_id).map_err(|e| {
        println!("DEBUG: Failed to set access key: {}", e);
        e.to_string()
    })?;
    config.set_aws_secret_key(&secret_access_key).map_err(|e| {
        println!("DEBUG: Failed to set secret key: {}", e);
        e.to_string()
    })?;
    
    println!("DEBUG: Credentials set, saving config...");
    
    // Debug what's actually in the config before saving
    println!("DEBUG: Before save - aws_access_key_id is_some: {}, aws_secret_access_key is_some: {}", 
             config.aws_access_key_id.is_some(), 
             config.aws_secret_access_key.is_some());
    if let Some(ref access_key) = config.aws_access_key_id {
        println!("DEBUG: Access key length: {}", access_key.len());
    }
    if let Some(ref secret_key) = config.aws_secret_access_key {
        println!("DEBUG: Secret key length: {}", secret_key.len());
    }
    
    state.config_manager.save_config(&config).await.map_err(|e| {
        println!("DEBUG: Failed to save config: {}", e);
        e.to_string()
    })?;
    
    println!("DEBUG: Config saved successfully");
    Ok("AWS credentials saved securely".to_string())
}

#[command]
pub async fn get_aws_credentials(state: State<'_, AppState>) -> Result<(String, String), String> {
    let config = state.config_manager.load_config().await.map_err(|e| e.to_string())?;
    
    let access_key = config.get_aws_access_key().map_err(|e| e.to_string())?.unwrap_or_default();
    let secret_key = config.get_aws_secret_key().map_err(|e| e.to_string())?.unwrap_or_default();
    
    Ok((access_key, secret_key))
}

#[command]
pub async fn get_config_path(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.config_manager.config_path().display().to_string())
}

// Game management commands
#[command]
pub async fn get_games(state: State<'_, AppState>) -> Result<HashMap<String, GameConfig>, String> {
    let config = state.config_manager.load_config().await.map_err(|e| e.to_string())?;
    Ok(config.games)
}

#[derive(serde::Serialize)]
pub struct GameWithStatus {
    pub id: String,
    pub name: String,
    pub save_paths: Vec<String>,
    pub sync_enabled: bool,
    pub last_sync: Option<String>,
    pub is_watching: bool,
}

#[command]
pub async fn get_games_with_status(state: State<'_, AppState>) -> Result<Vec<GameWithStatus>, String> {
    let config = state.config_manager.load_config().await.map_err(|e| e.to_string())?;
    let watching_games = if let Ok(watcher) = state.watcher_manager.lock() {
        watcher.watched_games()
    } else {
        Vec::new()
    };
    
    let sync_history = if let Ok(history) = state.sync_history.lock() {
        history.clone()
    } else {
        HashMap::new()
    };
    
    let mut games_with_status = Vec::new();
    
    for (game_id, game_config) in config.games.iter() {
        games_with_status.push(GameWithStatus {
            id: game_id.clone(),
            name: game_config.name.clone(),
            save_paths: game_config.save_paths.clone(),
            sync_enabled: game_config.sync_enabled,
            last_sync: sync_history.get(game_id).cloned(),
            is_watching: watching_games.contains(game_id),
        });
    }
    
    Ok(games_with_status)
}

#[command]
pub async fn add_game(
    name: String, 
    display_name: String, 
    paths: Vec<String>, 
    enabled: bool,
    state: State<'_, AppState>
) -> Result<String, String> {
    let mut config = state.config_manager.load_config().await.map_err(|e| e.to_string())?;
    
    let game_config = GameConfig {
        name: display_name,
        save_paths: paths,
        sync_enabled: enabled,
    };
    
    config.games.insert(name.clone(), game_config);
    state.config_manager.save_config(&config).await.map_err(|e| e.to_string())?;
    
    info!("Added game: {}", name);
    Ok(format!("Game '{}' added successfully", name))
}

#[command]
pub async fn update_game(
    name: String,
    game_config: GameConfig,
    state: State<'_, AppState>
) -> Result<String, String> {
    let mut config = state.config_manager.load_config().await.map_err(|e| e.to_string())?;
    
    config.games.insert(name.clone(), game_config);
    state.config_manager.save_config(&config).await.map_err(|e| e.to_string())?;
    
    info!("Updated game: {}", name);
    Ok(format!("Game '{}' updated successfully", name))
}

#[command]
pub async fn remove_game(name: String, state: State<'_, AppState>) -> Result<String, String> {
    let mut config = state.config_manager.load_config().await.map_err(|e| e.to_string())?;
    
    // For now, skip the watcher stop to avoid Send issues - will fix this later
    // TODO: Properly handle watcher stopping without Send issues
    
    config.games.remove(&name);
    state.config_manager.save_config(&config).await.map_err(|e| e.to_string())?;
    
    info!("Removed game: {}", name);
    Ok(format!("Game '{}' removed successfully", name))
}

// Sync commands
#[command]
pub async fn sync_game(game_name: String, state: State<'_, AppState>) -> Result<String, String> {
    info!("sync_game command called for: {}", game_name);
    
    let config = state.config_manager.load_config().await.map_err(|e| {
        error!("Failed to load config: {}", e);
        e.to_string()
    })?;
    
    debug!("Config loaded. S3 bucket: {:?}, Region: {:?}", config.s3_bucket, config.s3_region);
    debug!("Games in config: {:?}", config.games.keys().collect::<Vec<_>>());
    
    let sync_handler = GameSaveSync::new(config).await.map_err(|e| {
        error!("Failed to create GameSaveSync: {}", e);
        e.to_string()
    })?;
    
    info!("Starting sync for game: {}", game_name);
    
    let result = sync_handler.sync_game(&game_name).await.map_err(|e| {
        error!("Sync failed for {}: {}", game_name, e);
        e.to_string()
    });
    
    match result {
        Ok(_) => {
            info!("Sync completed successfully for: {}", game_name);
            
            // Record sync timestamp
            if let Ok(mut history) = state.sync_history.lock() {
                let timestamp = chrono::Utc::now().to_rfc3339();
                history.insert(game_name.clone(), timestamp);
                debug!("Recorded sync timestamp for: {}", game_name);
            }
            
            Ok(format!("Successfully synced {}", game_name))
        }
        Err(e) => {
            error!("Sync failed: {}", e);
            Err(e)
        }
    }
}

#[command]
pub async fn start_watching_game(
    game_name: String, 
    state: State<'_, AppState>,
    app_handle: AppHandle
) -> Result<String, String> {
    let config = state.config_manager.load_config().await.map_err(|e| e.to_string())?;
    
    let game_config = config.games.get(&game_name)
        .ok_or_else(|| format!("Game '{}' not found", game_name))?;

    if !game_config.sync_enabled {
        return Err(format!("Sync is disabled for game '{}'", game_name));
    }

    let session_id = Uuid::new_v4().to_string();
    
    // Store session
    if let Ok(mut sessions) = state.sync_sessions.lock() {
        sessions.insert(game_name.clone(), session_id.clone());
    }

    // TODO: Start watching - temporarily disabled to avoid Send issues
    // Will implement proper async watcher management later

    // Emit event to frontend
    app_handle.emit("game-watch-started", &game_name).unwrap_or_else(|e| {
        error!("Failed to emit game-watch-started event: {}", e);
    });

    info!("Started watching game: {}", game_name);
    Ok(format!("Started watching {} (session: {})", game_name, session_id))
}

#[command]
pub async fn stop_watching_game(
    game_name: String, 
    state: State<'_, AppState>,
    app_handle: AppHandle
) -> Result<String, String> {
    // Remove session
    if let Ok(mut sessions) = state.sync_sessions.lock() {
        sessions.remove(&game_name);
    }

    // TODO: Stop watching - temporarily disabled to avoid Send issues
    // Will implement proper async watcher management later

    // Emit event to frontend
    app_handle.emit("game-watch-stopped", &game_name).unwrap_or_else(|e| {
        error!("Failed to emit game-watch-stopped event: {}", e);
    });

    info!("Stopped watching game: {}", game_name);
    Ok(format!("Stopped watching {}", game_name))
}

#[command]
pub async fn get_watching_games(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    if let Ok(watcher) = state.watcher_manager.lock() {
        Ok(watcher.watched_games())
    } else {
        Err("Failed to access watcher manager".to_string())
    }
}

// File system commands
#[command]
pub async fn validate_path(path: String) -> Result<bool, String> {
    let path_buf = PathBuf::from(&path);
    Ok(path_buf.exists())
}

// Service management commands
#[command]
pub async fn install_service(user_service: bool) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        use decksaves_core::daemon::linux::install_systemd_service;
        let service_file = install_systemd_service(user_service).map_err(|e| e.to_string())?;
        Ok(format!("Service installed: {}", service_file.display()))
    }
    
    #[cfg(target_os = "macos")]
    {
        use decksaves_core::daemon::macos::install_launchd_service;
        let plist_file = install_launchd_service(user_service).map_err(|e| e.to_string())?;
        Ok(format!("Service installed: {}", plist_file.display()))
    }
    
    #[cfg(target_os = "windows")]
    {
        // Windows service installation would go here
        let _ = user_service; // Acknowledge the parameter until implementation is added
        Ok("Windows service installation not yet implemented".to_string())
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err("Service installation not supported on this platform".to_string())
    }
}

// System information commands
#[command]
pub async fn get_system_info() -> Result<HashMap<String, String>, String> {
    let mut info = HashMap::new();
    
    info.insert("os".to_string(), std::env::consts::OS.to_string());
    info.insert("arch".to_string(), std::env::consts::ARCH.to_string());
    info.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    
    if let Ok(home) = std::env::var("HOME") {
        info.insert("home".to_string(), home);
    }
    
    Ok(info)
}

// Notification commands
#[command]
pub async fn show_notification(title: String, body: String, app_handle: AppHandle) -> Result<(), String> {
    app_handle.emit("notification", serde_json::json!({
        "title": title,
        "body": body,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    })).map_err(|e| e.to_string())?;
    
    Ok(())
}

// AWS connection testing commands
#[command]
pub async fn test_aws_connection(
    access_key_id: String, 
    secret_access_key: String, 
    region: String, 
    bucket: String
) -> Result<String, String> {
    use aws_sdk_s3::{Client, config::Region};
    use aws_config::BehaviorVersion;
    use aws_credential_types::Credentials;
    
    // Create credentials from provided keys
    let credentials = Credentials::new(
        access_key_id,
        secret_access_key,
        None,
        None,
        "manual"
    );
    
    // Create config with custom credentials
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(region))
        .credentials_provider(credentials)
        .load()
        .await;
    
    let client = Client::new(&config);
    
    // Test connection by trying to list objects (with limit)
    match client.list_objects_v2()
        .bucket(&bucket)
        .max_keys(1)
        .send()
        .await 
    {
        Ok(_) => Ok("Connection successful! AWS credentials and S3 bucket are working.".to_string()),
        Err(e) => {
            let error_msg = if e.to_string().contains("NoSuchBucket") {
                format!("Bucket '{}' does not exist or is not accessible", bucket)
            } else if e.to_string().contains("InvalidAccessKeyId") {
                "Invalid AWS Access Key ID".to_string()
            } else if e.to_string().contains("SignatureDoesNotMatch") {
                "Invalid AWS Secret Access Key".to_string()
            } else if e.to_string().contains("AccessDenied") {
                "Access denied. Check your AWS permissions for this bucket".to_string()
            } else {
                format!("Connection failed: {}", e.to_string())
            };
            Err(error_msg)
        }
    }
}

#[command]
pub async fn sync_game_with_feedback(
    game_name: String, 
    state: State<'_, AppState>,
    app_handle: AppHandle
) -> Result<String, String> {
    info!("Starting sync for game: {}", game_name);
    
    // Emit sync started event
    debug!("Emitting sync-started event for: {}", game_name);
    let started_payload = serde_json::json!({
        "game_name": game_name,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    match app_handle.emit("sync-started", &started_payload) {
        Ok(_) => debug!("Successfully emitted sync-started event"),
        Err(e) => error!("Failed to emit sync-started event: {}", e)
    }
    
    // Load current config
    let config = match state.config_manager.load_config().await {
        Ok(config) => config,
        Err(e) => {
            let error_msg = format!("Failed to load configuration: {}", e);
            let _ = app_handle.emit("sync-error", serde_json::json!({
                "game_name": game_name,
                "error": error_msg,
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            }));
            return Err(error_msg);
        }
    };
    
    // Check if game exists
    let game_config = match config.games.get(&game_name) {
        Some(game) => game,
        None => {
            let error_msg = format!("Game '{}' not found", game_name);
            let _ = app_handle.emit("sync-error", serde_json::json!({
                "game_name": game_name,
                "error": error_msg,
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            }));
            return Err(error_msg);
        }
    };
    
    // Check if sync is enabled for this game
    if !game_config.sync_enabled {
        let error_msg = format!("Sync is disabled for game '{}'", game_name);
        let _ = app_handle.emit("sync-error", serde_json::json!({
            "game_name": game_name,
            "error": error_msg,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }));
        return Err(error_msg);
    }
    
    // Check AWS configuration
    let has_credentials = match (config.get_aws_access_key(), config.get_aws_secret_key()) {
        (Ok(Some(access_key)), Ok(Some(secret_key))) => {
            debug!("Found encrypted credentials: access_key={}, secret_key={}", 
                   if access_key.is_empty() { "empty" } else { "present" },
                   if secret_key.is_empty() { "empty" } else { "present" });
            !access_key.is_empty() && !secret_key.is_empty()
        },
        (access_result, secret_result) => {
            debug!("Failed to get credentials: access_key={:?}, secret_key={:?}", 
                   access_result, secret_result);
            false
        }
    };
    
    debug!("AWS config check: s3_bucket={:?}, has_credentials={}", config.s3_bucket, has_credentials);
    
    if config.s3_bucket.is_none() || !has_credentials {
        let error_msg = "AWS configuration incomplete. Please set S3 bucket and AWS credentials in settings.".to_string();
        let _ = app_handle.emit("sync-error", serde_json::json!({
            "game_name": game_name,
            "error": error_msg,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }));
        return Err(error_msg);
    }
    
    // Emit sync progress
    debug!("Emitting sync-progress event for: {}", game_name);
    let progress_payload = serde_json::json!({
        "game_name": game_name,
        "status": "Connecting to AWS S3...",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    match app_handle.emit("sync-progress", &progress_payload) {
        Ok(_) => debug!("Successfully emitted sync-progress event"),
        Err(e) => error!("Failed to emit sync-progress event: {}", e)
    }
    
    // Perform the actual sync using existing core functionality
    debug!("About to create GameSaveSync");
    
    // Add a timeout to prevent hanging
    let sync_result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        async {
            let sync_handler = GameSaveSync::new(config.clone()).await?;
            debug!("Successfully created GameSaveSync, starting sync for: {}", game_name);
            sync_handler.sync_game(&game_name).await?;
            debug!("sync_game completed successfully for: {}", game_name);
            Ok::<(), anyhow::Error>(())
        }
    ).await;
    
    match sync_result {
        Ok(Ok(_)) => {
            debug!("Sync completed successfully (with timeout)");
            let result = "Sync completed successfully".to_string();
            
            // Record sync timestamp
            if let Ok(mut history) = state.sync_history.lock() {
                let timestamp = chrono::Utc::now().to_rfc3339();
                history.insert(game_name.clone(), timestamp);
                debug!("Recorded sync timestamp for: {}", game_name);
            }
            
            // Emit sync completed event
            debug!("Emitting sync-completed event for: {}", game_name);
            let event_payload = serde_json::json!({
                "game_name": game_name,
                "result": result,
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            });
            debug!("Event payload: {}", event_payload);
            
            match app_handle.emit("sync-completed", &event_payload) {
                Ok(_) => debug!("Successfully emitted sync-completed event"),
                Err(e) => error!("Failed to emit sync-completed event: {}", e)
            }
            
            debug!("Returning success result for: {}", game_name);
            Ok(result)
        }
        Ok(Err(e)) => {
            debug!("sync_game failed: {}", e);
            let error_msg = format!("Sync failed: {}", e);
            let _ = app_handle.emit("sync-error", serde_json::json!({
                "game_name": game_name,
                "error": error_msg,
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            }));
            Err(error_msg)
        }
        Err(_timeout) => {
            debug!("Sync timed out after 30 seconds");
            let error_msg = "Sync operation timed out after 30 seconds".to_string();
            let _ = app_handle.emit("sync-error", serde_json::json!({
                "game_name": game_name,
                "error": error_msg,
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            }));
            Err(error_msg)
        }
    }
}

#[command]
pub async fn debug_credentials(state: State<'_, AppState>) -> Result<String, String> {
    println!("DEBUG: debug_credentials command called");
    
    let config = state.config_manager.load_config().await.map_err(|e| {
        println!("DEBUG: Failed to load config: {}", e);
        e.to_string()
    })?;
    
    println!("DEBUG: Config loaded successfully");
    
    let access_key_result = config.get_aws_access_key();
    let secret_key_result = config.get_aws_secret_key();
    
    println!("DEBUG: Credential results - access_key: {:?}, secret_key: {:?}", 
             access_key_result.as_ref().map(|r| r.is_some()),
             secret_key_result.as_ref().map(|r| r.is_some()));
    
    let debug_info = serde_json::json!({
        "s3_bucket": config.s3_bucket,
        "s3_region": config.s3_region,
        "access_key_stored": config.aws_access_key_id.is_some(),
        "secret_key_stored": config.aws_secret_access_key.is_some(),
        "access_key_decrypted": access_key_result.as_ref().map(|opt| opt.is_some()).unwrap_or(false),
        "secret_key_decrypted": secret_key_result.as_ref().map(|opt| opt.is_some()).unwrap_or(false),
        "access_key_error": access_key_result.as_ref().err().map(|e| e.to_string()),
        "secret_key_error": secret_key_result.as_ref().err().map(|e| e.to_string()),
    });
    
    let result = debug_info.to_string();
    println!("DEBUG: Returning debug info: {}", result);
    Ok(result)
}

#[command]
pub async fn test_command() -> Result<String, String> {
    println!("TEST: test_command called successfully!");
    Ok("Test command works!".to_string())
}

#[command]
pub async fn set_aws_credentials_and_config(
    access_key_id: String,
    secret_access_key: String,
    config: SyncConfig,
    state: State<'_, AppState>
) -> Result<String, String> {
    println!("DEBUG: set_aws_credentials_and_config called with access_key_id length: {}, secret_access_key length: {}", 
             access_key_id.len(), secret_access_key.len());
    
    let mut final_config = config;
    
    println!("DEBUG: Setting credentials in config...");
    
    // Use the encryption methods
    final_config.set_aws_access_key(&access_key_id).map_err(|e| {
        println!("DEBUG: Failed to set access key: {}", e);
        e.to_string()
    })?;
    final_config.set_aws_secret_key(&secret_access_key).map_err(|e| {
        println!("DEBUG: Failed to set secret key: {}", e);
        e.to_string()
    })?;
    
    println!("DEBUG: Credentials set in config, saving...");
    
    // Debug what's actually in the config before saving
    println!("DEBUG: Before save - aws_access_key_id is_some: {}, aws_secret_access_key is_some: {}", 
             final_config.aws_access_key_id.is_some(), 
             final_config.aws_secret_access_key.is_some());
    if let Some(ref access_key) = final_config.aws_access_key_id {
        println!("DEBUG: Access key length: {}", access_key.len());
    }
    if let Some(ref secret_key) = final_config.aws_secret_access_key {
        println!("DEBUG: Secret key length: {}", secret_key.len());
    }
    
    state.config_manager.save_config(&final_config).await.map_err(|e| {
        println!("DEBUG: Failed to save config: {}", e);
        e.to_string()
    })?;
    
    println!("DEBUG: Config saved successfully");
    Ok("Configuration and AWS credentials saved securely".to_string())
}

#[command]
pub async fn detect_steam_games() -> Result<Vec<SteamGame>, String> {
    info!("Starting Steam game detection");
    debug!("detect_steam_games command called");
    
    let mut detector = SteamDetector::new().map_err(|e| {
        error!("Failed to create Steam detector: {}", e);
        debug!("Steam detector creation failed with error: {:?}", e);
        e.to_string()
    })?;
    
    debug!("Steam detector created successfully");
    
    let games = detector.discover_games().await.map_err(|e| {
        error!("Failed to discover Steam games: {}", e);
        debug!("Steam game discovery failed with error: {:?}", e);
        e.to_string()
    })?;
    
    info!("Found {} Steam games", games.len());
    debug!("Steam games discovered: {:?}", games);
    Ok(games)
}

#[command]
pub async fn test_steam_detection() -> Result<String, String> {
    debug!("Testing Steam detection manually");
    
    match SteamDetector::new() {
        Ok(mut detector) => {
            debug!("Steam detector created successfully");
            match detector.discover_games().await {
                Ok(games) => {
                    debug!("Steam games found: {}", games.len());
                    Ok(format!("Found {} Steam games: {:?}", games.len(), games.iter().take(3).map(|g| &g.name).collect::<Vec<_>>()))
                }
                Err(e) => {
                    debug!("Failed to discover games: {:?}", e);
                    Err(format!("Failed to discover games: {}", e))
                }
            }
        }
        Err(e) => {
            debug!("Failed to create Steam detector: {:?}", e);
            Err(format!("Failed to create Steam detector: {}", e))
        }
    }
}

#[command]
pub async fn get_steam_save_suggestions(steam_game: SteamGame) -> Result<Vec<String>, String> {
    info!("Getting save path suggestions for game: {}", steam_game.name);
    
    let detector = SteamDetector::new().map_err(|e| {
        error!("Failed to create Steam detector: {}", e);
        e.to_string()
    })?;
    
    let save_paths = detector.get_common_save_paths(&steam_game);
    
    // Filter to existing paths
    let existing_paths: Vec<String> = save_paths.into_iter()
        .filter(|path| {
            let path_buf = std::path::PathBuf::from(path);
            let exists = path_buf.exists();
            if exists {
                debug!("Found existing save path: {}", path);
            }
            exists
        })
        .collect();
    
    info!("Found {} existing save paths for {}", existing_paths.len(), steam_game.name);
    Ok(existing_paths)
}

#[command]
pub async fn add_steam_game_to_config(
    steam_game: SteamGame,
    save_paths: Vec<String>,
    state: State<'_, AppState>
) -> Result<String, String> {
    info!("Adding Steam game to config: {}", steam_game.name);
    
    let mut config = state.config_manager.load_config().await.map_err(|e| {
        error!("Failed to load config: {}", e);
        e.to_string()
    })?;
    
    let game_config = GameConfig {
        name: steam_game.name.clone(),
        save_paths: save_paths.clone(),
        sync_enabled: true,
    };
    
    // Use app_id as the key for Steam games
    let game_key = format!("steam_{}", steam_game.app_id);
    config.games.insert(game_key, game_config);
    
    state.config_manager.save_config(&config).await.map_err(|e| {
        error!("Failed to save config: {}", e);
        e.to_string()
    })?;
    
    info!("Successfully added Steam game: {}", steam_game.name);
    Ok(format!("Added {} with {} save paths", steam_game.name, save_paths.len()))
}
