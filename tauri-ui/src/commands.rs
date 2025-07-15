use tauri::command;
use core::{config::ConfigManager, GameSaveSync, GameConfig};
use std::collections::HashMap;

#[command]
pub async fn get_games() -> Result<HashMap<String, GameConfig>, String> {
    let config_manager = ConfigManager::new().map_err(|e| e.to_string())?;
    let config = config_manager.load_config().await.map_err(|e| e.to_string())?;
    Ok(config.games)
}

#[command]
pub async fn sync_game(game_name: String) -> Result<String, String> {
    let config_manager = ConfigManager::new().map_err(|e| e.to_string())?;
    let config = config_manager.load_config().await.map_err(|e| e.to_string())?;
    let sync_handler = GameSaveSync::new(config).await.map_err(|e| e.to_string())?;
    
    sync_handler.sync_game(&game_name).await.map_err(|e| e.to_string())?;
    Ok(format!("Successfully synced {}", game_name))
}

#[command]
pub async fn add_game(name: String, paths: Vec<String>) -> Result<String, String> {
    let config_manager = ConfigManager::new().map_err(|e| e.to_string())?;
    let mut config = config_manager.load_config().await.map_err(|e| e.to_string())?;
    
    let game_config = GameConfig {
        name: name.clone(),
        save_paths: paths,
        sync_enabled: true,
    };
    
    config.games.insert(name.clone(), game_config);
    config_manager.save_config(&config).await.map_err(|e| e.to_string())?;
    
    Ok(format!("Added game: {}", name))
}
