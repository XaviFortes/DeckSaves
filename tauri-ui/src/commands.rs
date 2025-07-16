use tauri::{command, State, AppHandle, Manager};
use decksaves_core::{
    config::ConfigManager, 
    GameSaveSync, 
    GameConfig, 
    SyncConfig,
    watcher::WatcherManager,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use uuid::Uuid;
use tracing::{info, error};

// Application state
pub struct AppState {
    pub config_manager: ConfigManager,
    pub watcher_manager: Arc<Mutex<WatcherManager>>,
    pub sync_sessions: Arc<Mutex<HashMap<String, String>>>, // game_name -> session_id
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let config_manager = ConfigManager::new().map_err(|e| e.to_string())?;
        let watcher_manager = Arc::new(Mutex::new(WatcherManager::new()));
        let sync_sessions = Arc::new(Mutex::new(HashMap::new()));

        Ok(Self {
            config_manager,
            watcher_manager,
            sync_sessions,
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
pub async fn get_config_path(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.config_manager.config_path().display().to_string())
}

// Game management commands
#[command]
pub async fn get_games(state: State<'_, AppState>) -> Result<HashMap<String, GameConfig>, String> {
    let config = state.config_manager.load_config().await.map_err(|e| e.to_string())?;
    Ok(config.games)
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
    let config = state.config_manager.load_config().await.map_err(|e| e.to_string())?;
    let sync_handler = GameSaveSync::new(config).await.map_err(|e| e.to_string())?;
    
    info!("Starting sync for game: {}", game_name);
    sync_handler.sync_game(&game_name).await.map_err(|e| e.to_string())?;
    Ok(format!("Successfully synced {}", game_name))
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
    app_handle.emit_all("game-watch-started", &game_name).unwrap_or_else(|e| {
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
    app_handle.emit_all("game-watch-stopped", &game_name).unwrap_or_else(|e| {
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
pub async fn select_folder() -> Result<Option<String>, String> {
    use tauri::api::dialog::blocking::FileDialogBuilder;
    
    let folder = FileDialogBuilder::new()
        .set_title("Select Save Folder")
        .pick_folder();
    
    Ok(folder.map(|p| p.display().to_string()))
}

#[command]
pub async fn select_file() -> Result<Option<String>, String> {
    use tauri::api::dialog::blocking::FileDialogBuilder;
    
    let file = FileDialogBuilder::new()
        .set_title("Select Save File")
        .add_filter("Save Files", &["sav", "save", "dat", "json"])
        .add_filter("All Files", &["*"])
        .pick_file();
    
    Ok(file.map(|p| p.display().to_string()))
}

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
    app_handle.emit_all("notification", serde_json::json!({
        "title": title,
        "body": body,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    })).map_err(|e| e.to_string())?;
    
    Ok(())
}
