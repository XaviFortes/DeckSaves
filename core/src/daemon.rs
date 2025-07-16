use anyhow::Result;
#[cfg(not(target_os = "windows"))]
use anyhow::Context;
#[cfg(not(target_os = "windows"))]
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, error, warn, debug};

#[cfg(unix)]
use signal_hook::consts::SIGTERM;
#[cfg(unix)]
use signal_hook_tokio::Signals;
#[cfg(unix)]
use futures_util::stream::StreamExt;

use crate::{config::ConfigManager, GameSaveSync, watcher::WatcherManager};

pub struct DaemonService {
    config_manager: ConfigManager,
    watcher_manager: WatcherManager,
    should_stop: bool,
}

impl DaemonService {
    pub fn new() -> Result<Self> {
        let config_manager = ConfigManager::new()?;
        let watcher_manager = WatcherManager::new();

        Ok(Self {
            config_manager,
            watcher_manager,
            should_stop: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting DeckSaves daemon service");

        // Set up signal handling (Unix only)
        #[cfg(unix)]
        let signals = Signals::new(&[SIGTERM])?;
        #[cfg(unix)]
        let mut signals = signals.fuse();

        // Notify systemd that we're ready (Linux only)
        #[cfg(target_os = "linux")]
        self.notify_systemd_ready()?;

        // Start watching all configured games
        self.start_all_watchers().await?;

        // Main service loop
        #[cfg(unix)]
        self.run_unix_service_loop(&mut signals).await?;
        
        #[cfg(windows)]
        self.run_windows_service_loop().await?;

        // Cleanup
        self.shutdown().await?;
        info!("DeckSaves daemon service stopped");
        
        Ok(())
    }

    #[cfg(unix)]
    async fn run_unix_service_loop(&mut self, signals: &mut futures_util::stream::Fuse<Signals>) -> Result<()> {
        loop {
            tokio::select! {
                // Handle shutdown signals
                Some(signal) = signals.next() => {
                    match signal {
                        SIGTERM => {
                            info!("Received SIGTERM, shutting down gracefully");
                            self.should_stop = true;
                            break;
                        }
                        _ => {
                            warn!("Received unexpected signal: {}", signal);
                        }
                    }
                }
                
                // Periodic health check and watchdog notification
                _ = sleep(Duration::from_secs(30)) => {
                    self.health_check().await?;
                    #[cfg(target_os = "linux")]
                    self.notify_systemd_watchdog()?;
                }
                
                // Check for configuration changes
                _ = sleep(Duration::from_secs(60)) => {
                    if let Err(e) = self.reload_configuration().await {
                        error!("Failed to reload configuration: {}", e);
                    }
                }
            }

            if self.should_stop {
                break;
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    async fn run_windows_service_loop(&mut self) -> Result<()> {
        // On Windows, we use a different approach - just run until manually stopped
        // Windows services typically use the service control manager for shutdown signals
        loop {
            tokio::select! {
                // Periodic health check
                _ = sleep(Duration::from_secs(30)) => {
                    self.health_check().await?;
                }
                
                // Check for configuration changes
                _ = sleep(Duration::from_secs(60)) => {
                    if let Err(e) = self.reload_configuration().await {
                        error!("Failed to reload configuration: {}", e);
                    }
                }
            }

            if self.should_stop {
                break;
            }
        }
        Ok(())
    }

    async fn start_all_watchers(&mut self) -> Result<()> {
        let config = self.config_manager.load_config().await?;
        
        for (game_name, game_config) in &config.games {
            if game_config.sync_enabled {
                info!("Starting watcher for game: {}", game_name);
                
                let sync_handler = GameSaveSync::new(config.clone()).await?;
                
                if let Err(e) = self.watcher_manager.start_watching_game(
                    game_name.clone(),
                    game_config.save_paths.clone(),
                    sync_handler,
                ).await {
                    error!("Failed to start watcher for {}: {}", game_name, e);
                }
            }
        }
        
        Ok(())
    }

    async fn health_check(&self) -> Result<()> {
        debug!("Performing health check");
        
        // Check if watchers are still running
        let watched_games = self.watcher_manager.watched_games();
        debug!("Currently watching {} games: {:?}", watched_games.len(), watched_games);
        
        // Verify configuration file is accessible
        if !self.config_manager.config_path().exists() {
            warn!("Configuration file does not exist: {}", self.config_manager.config_path().display());
        }
        
        Ok(())
    }

    async fn reload_configuration(&mut self) -> Result<()> {
        debug!("Checking for configuration changes");
        
        let config = self.config_manager.load_config().await?;
        let currently_watched = self.watcher_manager.watched_games();
        
        // Check for new games to watch
        for (game_name, game_config) in &config.games {
            if game_config.sync_enabled && !currently_watched.contains(game_name) {
                info!("Starting watcher for new game: {}", game_name);
                
                let sync_handler = GameSaveSync::new(config.clone()).await?;
                
                if let Err(e) = self.watcher_manager.start_watching_game(
                    game_name.clone(),
                    game_config.save_paths.clone(),
                    sync_handler,
                ).await {
                    error!("Failed to start watcher for {}: {}", game_name, e);
                }
            }
        }
        
        // Check for games to stop watching
        for watched_game in &currently_watched {
            if let Some(game_config) = config.games.get(watched_game) {
                if !game_config.sync_enabled {
                    info!("Stopping watcher for disabled game: {}", watched_game);
                    self.watcher_manager.stop_watching_game(watched_game).await;
                }
            } else {
                info!("Stopping watcher for removed game: {}", watched_game);
                self.watcher_manager.stop_watching_game(watched_game).await;
            }
        }
        
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down daemon service");
        self.watcher_manager.stop_all().await;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn notify_systemd_ready(&self) -> Result<()> {
        use libsystemd::daemon;
        
        if daemon::booted() {
            daemon::notify(false, &[daemon::NotifyState::Ready])?;
            info!("Notified systemd that service is ready");
        }
        
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn notify_systemd_watchdog(&self) -> Result<()> {
        use libsystemd::daemon;
        
        if daemon::booted() && daemon::watchdog_enabled(false).is_some() {
            daemon::notify(false, &[daemon::NotifyState::Watchdog])?;
            debug!("Sent watchdog notification to systemd");
        }
        
        Ok(())
    }

    pub fn stop(&mut self) {
        info!("Daemon stop requested");
        self.should_stop = true;
    }
}

impl Default for DaemonService {
    fn default() -> Self {
        Self::new().expect("Failed to create daemon service")
    }
}

#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;
    use std::fs;
    use std::path::Path;

    pub fn generate_systemd_service(user_mode: bool) -> String {
        let service_type = if user_mode { "user" } else { "system" };
        let exec_path_buf = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/game-sync"));
        let exec_path = exec_path_buf.display();

        format!(
            r#"[Unit]
Description=DeckSaves Game Save Synchronization Service
After=network.target
Wants=network.target

[Service]
Type=notify
ExecStart={} daemon
Restart=always
RestartSec=10
WatchdogSec=60
StandardOutput=journal
StandardError=journal

{user_section}

[Install]
WantedBy={target}
"#,
            exec_path,
            user_section = if user_mode { "" } else { "User=decksaves\nGroup=decksaves" },
            target = if user_mode { "default.target" } else { "multi-user.target" }
        )
    }

    pub fn install_systemd_service(user_mode: bool) -> Result<PathBuf> {
        let service_content = generate_systemd_service(user_mode);
        
        let service_dir = if user_mode {
            let home = std::env::var("HOME").context("HOME environment variable not set")?;
            PathBuf::from(home).join(".config/systemd/user")
        } else {
            PathBuf::from("/etc/systemd/system")
        };

        fs::create_dir_all(&service_dir)
            .context("Failed to create systemd service directory")?;

        let service_file = service_dir.join("decksaves.service");
        fs::write(&service_file, service_content)
            .context("Failed to write systemd service file")?;

        Ok(service_file)
    }
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use std::fs;

    pub fn generate_launchd_plist(_user_mode: bool) -> String {
        let exec_path_buf = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/game-sync"));
        let exec_path = exec_path_buf.display();

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.decksaves.game-sync</string>
    
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>daemon</string>
    </array>
    
    <key>RunAtLoad</key>
    <true/>
    
    <key>KeepAlive</key>
    <true/>
    
    <key>StandardOutPath</key>
    <string>/tmp/decksaves.log</string>
    
    <key>StandardErrorPath</key>
    <string>/tmp/decksaves-error.log</string>
    
    <key>WorkingDirectory</key>
    <string>/usr/local/bin</string>
</dict>
</plist>
"#,
            exec_path
        )
    }

    pub fn install_launchd_service(user_mode: bool) -> Result<PathBuf> {
        let plist_content = generate_launchd_plist(user_mode);
        
        let service_dir = if user_mode {
            let home = std::env::var("HOME").context("HOME environment variable not set")?;
            PathBuf::from(home).join("Library/LaunchAgents")
        } else {
            PathBuf::from("/Library/LaunchDaemons")
        };

        fs::create_dir_all(&service_dir)
            .context("Failed to create LaunchAgent directory")?;

        let plist_file = service_dir.join("com.decksaves.game-sync.plist");
        fs::write(&plist_file, plist_content)
            .context("Failed to write LaunchAgent plist file")?;

        Ok(plist_file)
    }
}

#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };
    use tokio::sync::mpsc;

    const SERVICE_NAME: &str = "DeckSaves";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    pub struct WindowsService {
        daemon: DaemonService,
        shutdown_rx: mpsc::Receiver<()>,
    }

    impl WindowsService {
        pub fn new(shutdown_rx: mpsc::Receiver<()>) -> Result<Self> {
            Ok(Self {
                daemon: DaemonService::new()?,
                shutdown_rx,
            })
        }

        pub async fn run(mut self) -> Result<()> {
            tokio::select! {
                result = self.daemon.run() => result,
                _ = self.shutdown_rx.recv() => {
                    info!("Windows service shutdown requested");
                    Ok(())
                }
            }
        }
    }

    define_windows_service!(ffi_service_main, service_main);

    pub fn service_main(arguments: Vec<std::ffi::OsString>) {
        if let Err(e) = run_service(arguments) {
            error!("Service failed: {}", e);
        }
    }

    fn run_service(_arguments: Vec<std::ffi::OsString>) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    let _ = shutdown_tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

        // Set service status to running
        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        })?;

        // Create and run the service
        let rt = tokio::runtime::Runtime::new()?;
        let service = WindowsService::new(shutdown_rx)?;
        
        let result = rt.block_on(service.run());

        // Set service status to stopped
        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        })?;

        result
    }

    pub fn run_windows_service() -> Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
        Ok(())
    }
}
