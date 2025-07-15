use clap::{Parser, Subcommand};
use anyhow::Result;
use core::{
    config::ConfigManager,
    GameSaveSync, GameConfig,
    watcher::WatcherManager,
    daemon::DaemonService,
};
use std::collections::HashMap;
use tracing::{info, error, warn};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

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
    /// Run as a background daemon service
    Daemon,
    /// Service management commands
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Install the service
    Install {
        /// Install as user service instead of system service
        #[arg(long)]
        user: bool,
    },
    /// Start the service
    Start {
        /// Start user service instead of system service
        #[arg(long)]
        user: bool,
    },
    /// Stop the service
    Stop {
        /// Stop user service instead of system service
        #[arg(long)]
        user: bool,
    },
    /// Uninstall the service
    Uninstall {
        /// Uninstall user service instead of system service
        #[arg(long)]
        user: bool,
    },
    /// Show service status
    Status {
        /// Check user service instead of system service
        #[arg(long)]
        user: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing based on command
    match &args.cmd {
        Command::Daemon => {
            // For daemon mode, try file logging first
            match initialize_daemon_logging() {
                Ok(_) => {
                    info!("Initialized file logging for daemon mode");
                }
                Err(e) => {
                    // Fall back to console logging
                    initialize_cli_logging();
                    warn!("Failed to initialize file logging ({}), using console logging instead", e);
                }
            }
        }
        _ => {
            // For CLI commands, log to console
            initialize_cli_logging();
        }
    }

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
        Command::Daemon => {
            run_daemon().await?;
        }
        Command::Service { action } => {
            handle_service_command(action).await?;
        }
    }

    Ok(())
}

fn initialize_cli_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}

fn try_initialize_daemon_logging() -> Result<()> {
    let log_dir = if cfg!(target_os = "windows") {
        std::env::var("PROGRAMDATA")
            .unwrap_or_else(|_| "C:\\ProgramData".to_string()) + "\\DeckSaves\\logs"
    } else {
        // For development, use a directory in the user's home
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/.local/share/decksaves/logs", home)
    };

    std::fs::create_dir_all(&log_dir)?;

    // Test if we can write to the directory
    let test_file = std::path::Path::new(&log_dir).join("test.log");
    std::fs::write(&test_file, "test")?;
    std::fs::remove_file(&test_file)?;

    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        log_dir,
        "game-sync.log"
    );

    tracing_subscriber::fmt()
        .with_writer(file_appender)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Ok(())
}

fn initialize_daemon_logging() -> Result<()> {
    try_initialize_daemon_logging()
}

async fn run_daemon() -> Result<()> {
    info!("Starting DeckSaves daemon");

    #[cfg(target_os = "windows")]
    {
        // Check if we're running as a Windows service
        if std::env::args().any(|arg| arg == "--service") {
            return core::daemon::windows::run_windows_service();
        }
    }

    // Run as regular daemon
    match DaemonService::new() {
        Ok(mut daemon) => {
            daemon.run().await?;
        }
        Err(e) => {
            error!("Failed to create daemon service: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

async fn handle_service_command(action: ServiceAction) -> Result<()> {
    match action {
        ServiceAction::Install { user } => {
            install_service(user).await?;
        }
        ServiceAction::Start { user } => {
            start_service(user).await?;
        }
        ServiceAction::Stop { user } => {
            stop_service(user).await?;
        }
        ServiceAction::Uninstall { user } => {
            uninstall_service(user).await?;
        }
        ServiceAction::Status { user } => {
            show_service_status(user).await?;
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
async fn install_service(user: bool) -> Result<()> {
    use core::daemon::linux::install_systemd_service;
    use std::process::Command;

    let service_file = install_systemd_service(user)?;
    info!("Installed systemd service file: {}", service_file.display());

    // Reload systemd
    let reload_cmd = if user {
        Command::new("systemctl")
            .args(&["--user", "daemon-reload"])
            .output()
    } else {
        Command::new("sudo")
            .args(&["systemctl", "daemon-reload"])
            .output()
    };

    match reload_cmd {
        Ok(output) if output.status.success() => {
            info!("Systemd daemon reloaded successfully");
        }
        Ok(output) => {
            error!("Failed to reload systemd daemon: {}", String::from_utf8_lossy(&output.stderr));
        }
        Err(e) => {
            error!("Failed to execute systemctl: {}", e);
        }
    }

    // Enable the service
    let enable_cmd = if user {
        Command::new("systemctl")
            .args(&["--user", "enable", "decksaves.service"])
            .output()
    } else {
        Command::new("sudo")
            .args(&["systemctl", "enable", "decksaves.service"])
            .output()
    };

    match enable_cmd {
        Ok(output) if output.status.success() => {
            info!("DeckSaves service enabled successfully");
        }
        Ok(output) => {
            error!("Failed to enable service: {}", String::from_utf8_lossy(&output.stderr));
        }
        Err(e) => {
            error!("Failed to execute systemctl enable: {}", e);
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
async fn install_service(user: bool) -> Result<()> {
    use core::daemon::macos::install_launchd_service;
    use std::process::Command;

    let plist_file = install_launchd_service(user)?;
    info!("Installed LaunchAgent plist: {}", plist_file.display());

    // Load the service
    let load_cmd = Command::new("launchctl")
        .args(&["load", plist_file.to_str().unwrap()])
        .output();

    match load_cmd {
        Ok(output) if output.status.success() => {
            info!("LaunchAgent loaded successfully");
        }
        Ok(output) => {
            error!("Failed to load LaunchAgent: {}", String::from_utf8_lossy(&output.stderr));
        }
        Err(e) => {
            error!("Failed to execute launchctl: {}", e);
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
async fn install_service(_user: bool) -> Result<()> {
    use service_manager::*;

    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)?;

    let service_info = ServiceInfo {
        name: "DeckSaves".into(),
        display_name: "DeckSaves Game Save Synchronization".into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AUTOMATIC,
        error_control: ServiceErrorControl::NORMAL,
        executable_path: std::env::current_exe()?,
        launch_arguments: vec!["daemon".into(), "--service".into()],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };

    let service = manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
    info!("Windows service installed successfully");

    Ok(())
}

async fn start_service(_user: bool) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let cmd = if _user {
            Command::new("systemctl")
                .args(&["--user", "start", "decksaves.service"])
                .output()
        } else {
            Command::new("sudo")
                .args(&["systemctl", "start", "decksaves.service"])
                .output()
        };

        match cmd {
            Ok(output) if output.status.success() => {
                info!("DeckSaves service started successfully");
            }
            Ok(output) => {
                error!("Failed to start service: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(e) => {
                error!("Failed to execute systemctl: {}", e);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let cmd = Command::new("launchctl")
            .args(&["start", "com.decksaves.game-sync"])
            .output();

        match cmd {
            Ok(output) if output.status.success() => {
                info!("DeckSaves service started successfully");
            }
            Ok(output) => {
                error!("Failed to start service: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(e) => {
                error!("Failed to execute launchctl: {}", e);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use service_manager::*;
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service("DeckSaves", ServiceAccess::START)?;
        service.start::<&str>(&[])?;
        info!("DeckSaves service started successfully");
    }

    Ok(())
}

async fn stop_service(_user: bool) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let cmd = if user {
            Command::new("systemctl")
                .args(&["--user", "stop", "decksaves.service"])
                .output()
        } else {
            Command::new("sudo")
                .args(&["systemctl", "stop", "decksaves.service"])
                .output()
        };

        match cmd {
            Ok(output) if output.status.success() => {
                info!("DeckSaves service stopped successfully");
            }
            Ok(output) => {
                error!("Failed to stop service: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(e) => {
                error!("Failed to execute systemctl: {}", e);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let cmd = Command::new("launchctl")
            .args(&["stop", "com.decksaves.game-sync"])
            .output();

        match cmd {
            Ok(output) if output.status.success() => {
                info!("DeckSaves service stopped successfully");
            }
            Ok(output) => {
                error!("Failed to stop service: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(e) => {
                error!("Failed to execute launchctl: {}", e);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use service_manager::*;
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service("DeckSaves", ServiceAccess::STOP)?;
        service.stop()?;
        info!("DeckSaves service stopped successfully");
    }

    Ok(())
}

async fn uninstall_service(user: bool) -> Result<()> {
    // First stop the service
    let _ = stop_service(user).await;

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        
        // Disable the service
        let disable_cmd = if user {
            Command::new("systemctl")
                .args(&["--user", "disable", "decksaves.service"])
                .output()
        } else {
            Command::new("sudo")
                .args(&["systemctl", "disable", "decksaves.service"])
                .output()
        };

        let _ = disable_cmd; // Ignore errors for disable

        // Remove service file
        let service_file = if user {
            let home = std::env::var("HOME")?;
            std::path::PathBuf::from(home).join(".config/systemd/user/decksaves.service")
        } else {
            std::path::PathBuf::from("/etc/systemd/system/decksaves.service")
        };

        if service_file.exists() {
            if user {
                std::fs::remove_file(&service_file)?;
            } else {
                Command::new("sudo")
                    .args(&["rm", service_file.to_str().unwrap()])
                    .output()?;
            }
            info!("Removed service file: {}", service_file.display());
        }

        // Reload systemd
        let reload_cmd = if user {
            Command::new("systemctl")
                .args(&["--user", "daemon-reload"])
                .output()
        } else {
            Command::new("sudo")
                .args(&["systemctl", "daemon-reload"])
                .output()
        };

        let _ = reload_cmd; // Ignore errors
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        
        // Unload the service
        let unload_cmd = Command::new("launchctl")
            .args(&["unload", "com.decksaves.game-sync"])
            .output();

        let _ = unload_cmd; // Ignore errors

        // Remove plist file
        let plist_file = if user {
            let home = std::env::var("HOME")?;
            std::path::PathBuf::from(home).join("Library/LaunchAgents/com.decksaves.game-sync.plist")
        } else {
            std::path::PathBuf::from("/Library/LaunchDaemons/com.decksaves.game-sync.plist")
        };

        if plist_file.exists() {
            std::fs::remove_file(&plist_file)?;
            info!("Removed plist file: {}", plist_file.display());
        }
    }

    #[cfg(target_os = "windows")]
    {
        use service_manager::*;
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service("DeckSaves", ServiceAccess::DELETE)?;
        service.delete()?;
        info!("Windows service uninstalled successfully");
    }

    Ok(())
}

async fn show_service_status(_user: bool) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let cmd = if user {
            Command::new("systemctl")
                .args(&["--user", "status", "decksaves.service"])
                .output()
        } else {
            Command::new("systemctl")
                .args(&["status", "decksaves.service"])
                .output()
        };

        match cmd {
            Ok(output) => {
                println!("{}", String::from_utf8_lossy(&output.stdout));
                if !output.stderr.is_empty() {
                    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                error!("Failed to get service status: {}", e);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let cmd = Command::new("launchctl")
            .args(&["list", "com.decksaves.game-sync"])
            .output();

        match cmd {
            Ok(output) => {
                println!("{}", String::from_utf8_lossy(&output.stdout));
                if !output.stderr.is_empty() {
                    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                error!("Failed to get service status: {}", e);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use service_manager::*;
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service("DeckSaves", ServiceAccess::QUERY_STATUS)?;
        let status = service.query_status()?;
        println!("Service Status: {:?}", status.current_state);
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