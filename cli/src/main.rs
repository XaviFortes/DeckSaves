use clap::{Parser, Subcommand};
use anyhow::Result;
use core::{
    config::ConfigManager,
    GameSaveSync, GameConfig,
    watcher::WatcherManager,
};
use std::collections::HashMap;
use tracing::{info, error, warn};

#[derive(Parser)]
#[command(name = "game-sync")]
#[command(about = "A game save synchronization tool")]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start watching a game's save files for changes
    Watch {
        /// Name of the game to watch
        game: String,
    },
    /// Manually sync a specific game
    Sync {
        /// Name of the game to sync
        game: String,
    },
    /// Add a new game to the configuration
    AddGame {
        /// Name of the game
        name: String,
        /// Path to the save file or directory
        #[arg(short, long)]
        path: Vec<String>,
    },
    /// List all configured games
    List,
    /// Show current configuration
    Config,
    /// Initialize configuration with default values
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let config_manager = ConfigManager::new()?;

    match args.cmd {
        Command::Watch { game } => {
            watch_game(&config_manager, &game).await?;
        }
        Command::Sync { game } => {
            sync_game(&config_manager, &game).await?;
        }
        Command::AddGame { name, path } => {
            add_game(&config_manager, &name, path).await?;
        }
        Command::List => {
            list_games(&config_manager).await?;
        }
        Command::Config => {
            show_config(&config_manager).await?;
        }
        Command::Init => {
            init_config(&config_manager).await?;
        }
    }

    Ok(())
}

async fn watch_game(config_manager: &ConfigManager, game_name: &str) -> Result<()> {
    let config = config_manager.load_config().await?;
    
    let game_config = config.games.get(game_name)
        .ok_or_else(|| anyhow::anyhow!("Game '{}' not found in configuration", game_name))?;

    if !game_config.sync_enabled {
        warn!("Sync is disabled for game: {}", game_name);
        return Ok(());
    }

    info!("Starting to watch game: {}", game_name);
    info!("Watching paths: {:?}", game_config.save_paths);

    let sync_handler = GameSaveSync::new(config.clone()).await?;
    let mut watcher_manager = WatcherManager::new();

    watcher_manager.start_watching_game(
        game_name.to_string(),
        game_config.save_paths.clone(),
        sync_handler,
    ).await?;

    info!("Press Ctrl+C to stop watching...");
    
    // Keep the program running
    tokio::signal::ctrl_c().await?;
    info!("Stopping watcher...");
    
    watcher_manager.stop_all().await;
    Ok(())
}

async fn sync_game(config_manager: &ConfigManager, game_name: &str) -> Result<()> {
    let config = config_manager.load_config().await?;
    let sync_handler = GameSaveSync::new(config).await?;
    
    info!("Syncing game: {}", game_name);
    sync_handler.sync_game(game_name).await?;
    info!("Sync completed for: {}", game_name);
    
    Ok(())
}

async fn add_game(config_manager: &ConfigManager, name: &str, paths: Vec<String>) -> Result<()> {
    let mut config = config_manager.load_config().await?;
    
    let game_config = GameConfig {
        name: name.to_string(),
        save_paths: paths.clone(),
        sync_enabled: true,
    };
    
    config.games.insert(name.to_string(), game_config);
    config_manager.save_config(&config).await?;
    
    info!("Added game '{}' with paths: {:?}", name, paths);
    Ok(())
}

async fn list_games(config_manager: &ConfigManager) -> Result<()> {
    let config = config_manager.load_config().await?;
    
    if config.games.is_empty() {
        info!("No games configured. Use 'add-game' command to add games.");
        return Ok(());
    }
    
    println!("Configured games:");
    for (name, game_config) in &config.games {
        println!("  {}", name);
        println!("    Sync enabled: {}", game_config.sync_enabled);
        println!("    Save paths:");
        for path in &game_config.save_paths {
            println!("      - {}", path);
        }
        println!();
    }
    
    Ok(())
}

async fn show_config(config_manager: &ConfigManager) -> Result<()> {
    let config = config_manager.load_config().await?;
    
    println!("Configuration file: {}", config_manager.config_path().display());
    println!("S3 Bucket: {}", config.s3_bucket.unwrap_or_else(|| "Not configured".to_string()));
    println!("S3 Region: {}", config.s3_region.unwrap_or_else(|| "Not configured".to_string()));
    println!("Peer sync enabled: {}", config.peer_sync_enabled);
    println!("WebSocket URL: {}", config.websocket_url.unwrap_or_else(|| "Not configured".to_string()));
    println!("Number of games: {}", config.games.len());
    
    Ok(())
}

async fn init_config(config_manager: &ConfigManager) -> Result<()> {
    let mut config = core::SyncConfig::default();
    
    // Add some example games
    let mut games = HashMap::new();
    
    // Example: Steam Deck save paths
    games.insert("example-game".to_string(), GameConfig {
        name: "Example Game".to_string(),
        save_paths: vec![
            "~/.local/share/Steam/steamapps/compatdata/12345/pfx/drive_c/users/steamuser/Documents/SaveGame".to_string(),
        ],
        sync_enabled: false,
    });
    
    config.games = games;
    config_manager.save_config(&config).await?;
    
    info!("Initialized configuration at: {}", config_manager.config_path().display());
    info!("Edit the configuration file to add your games and S3 settings.");
    
    Ok(())
}