use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, warn, debug, error};
use regex::Regex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamGame {
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub library_path: String,
    pub last_updated: Option<u64>,
    pub size_on_disk: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamLibrary {
    pub path: String,
    pub label: String,
    pub mounted: bool,
    pub tool: bool,
}

pub struct SteamDetector {
    steam_path: Option<PathBuf>,
    libraries: Vec<SteamLibrary>,
}

impl SteamDetector {
    pub fn new() -> Result<Self> {
        debug!("Creating new SteamDetector");
        let steam_path = Self::find_steam_installation()?;
        debug!("Found Steam installation at: {:?}", steam_path);
        
        Ok(Self {
            steam_path: Some(steam_path),
            libraries: Vec::new(),
        })
    }

    fn find_steam_installation() -> Result<PathBuf> {
        debug!("Starting Steam installation search");
        
        // Common Steam installation paths on Windows
        let potential_paths = vec![
            PathBuf::from(r"C:\Program Files (x86)\Steam"),
            PathBuf::from(r"C:\Program Files\Steam"),
            PathBuf::from(r"D:\Steam"),
            PathBuf::from(r"E:\Steam"),
        ];

        debug!("Checking potential Steam paths: {:?}", potential_paths);

        // Check registry for Steam path (Windows)
        #[cfg(windows)]
        {
            debug!("Checking Windows registry for Steam path");
            if let Ok(steam_path) = Self::get_steam_path_from_registry() {
                debug!("Found Steam path in registry: {:?}", steam_path);
                if steam_path.exists() {
                    debug!("Registry Steam path exists, using it");
                    return Ok(steam_path);
                } else {
                    debug!("Registry Steam path does not exist: {:?}", steam_path);
                }
            } else {
                debug!("Failed to get Steam path from registry");
            }
        }

        // Check common paths
        for path in potential_paths {
            debug!("Checking path: {:?}", path);
            if path.exists() && path.join("steam.exe").exists() {
                debug!("Found Steam installation at: {:?}", path);
                return Ok(path);
            }
        }

        error!("Steam installation not found in any common locations");
        Err(anyhow::anyhow!("Steam installation not found"))
    }

    #[cfg(windows)]
    fn get_steam_path_from_registry() -> Result<PathBuf> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let steam_key = hklm.open_subkey(r"SOFTWARE\WOW6432Node\Valve\Steam")?;
        let install_path: String = steam_key.get_value("InstallPath")?;
        Ok(PathBuf::from(install_path))
    }

    #[cfg(not(windows))]
    fn get_steam_path_from_registry() -> Result<PathBuf> {
        Err(anyhow::anyhow!("Registry access not available on this platform"))
    }

    pub async fn discover_games(&mut self) -> Result<Vec<SteamGame>> {
        debug!("Starting Steam game discovery");
        
        self.load_library_folders().await?;
        debug!("Found {} Steam libraries to scan", self.libraries.len());
        
        let mut all_games = Vec::new();

        for library in &self.libraries {
            debug!("Scanning library: {}", library.path);
            let games = self.scan_library_for_games(&library.path).await?;
            debug!("Found {} games in library: {}", games.len(), library.path);
            all_games.extend(games);
        }

        info!("Discovered {} Steam games total", all_games.len());
        debug!("All discovered games: {:?}", all_games.iter().map(|g| &g.name).collect::<Vec<_>>());
        Ok(all_games)
    }

    async fn load_library_folders(&mut self) -> Result<()> {
        debug!("Loading Steam library folders");
        
        let steam_path = self.steam_path.as_ref()
            .context("Steam path not found")?;

        let config_path = steam_path.join("config").join("libraryfolders.vdf");
        debug!("Looking for libraryfolders.vdf at: {:?}", config_path);
        
        if !config_path.exists() {
            warn!("Steam libraryfolders.vdf not found at: {:?}", config_path);
            // Add default library
            let default_library = SteamLibrary {
                path: steam_path.join("steamapps").to_string_lossy().to_string(),
                label: "Main".to_string(),
                mounted: true,
                tool: false,
            };
            debug!("Adding default library: {:?}", default_library);
            self.libraries.push(default_library);
            return Ok(());
        }

        debug!("Reading libraryfolders.vdf file");
        let content = fs::read_to_string(&config_path)
            .context("Failed to read libraryfolders.vdf")?;

        debug!("Parsing library folders from VDF content (length: {})", content.len());
        self.libraries = self.parse_library_folders(&content)?;
        debug!("Found {} Steam libraries", self.libraries.len());
        for lib in &self.libraries {
            debug!("Library: {} at path: {}", lib.label, lib.path);
        }
        Ok(())
    }

    fn parse_library_folders(&self, content: &str) -> Result<Vec<SteamLibrary>> {
        let mut libraries = Vec::new();
        
        // Simple VDF parser - Steam's VDF format is key-value pairs
        let path_regex = Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
        let label_regex = Regex::new(r#""label"\s+"([^"]+)""#).unwrap();
        
        for line in content.lines() {
            if let Some(captures) = path_regex.captures(line) {
                if let Some(path_match) = captures.get(1) {
                    let library_path = PathBuf::from(path_match.as_str()).join("steamapps");
                    
                    libraries.push(SteamLibrary {
                        path: library_path.to_string_lossy().to_string(),
                        label: "Steam Library".to_string(),
                        mounted: true,
                        tool: false,
                    });
                }
            }
        }

        // If no libraries found, add default
        if libraries.is_empty() {
            if let Some(steam_path) = &self.steam_path {
                libraries.push(SteamLibrary {
                    path: steam_path.join("steamapps").to_string_lossy().to_string(),
                    label: "Main".to_string(),
                    mounted: true,
                    tool: false,
                });
            }
        }

        Ok(libraries)
    }

    async fn scan_library_for_games(&self, library_path: &str) -> Result<Vec<SteamGame>> {
        let library_dir = Path::new(library_path);
        let common_dir = library_dir.join("common");
        
        if !common_dir.exists() {
            debug!("Common directory not found: {:?}", common_dir);
            return Ok(Vec::new());
        }

        let mut games = Vec::new();
        
        // Read appmanifest files to get game information
        let appmanifest_pattern = library_dir.join("appmanifest_*.acf");
        let manifest_files = glob::glob(&appmanifest_pattern.to_string_lossy())
            .map_err(|e| anyhow::anyhow!("Glob pattern error: {}", e))?;

        for manifest_path in manifest_files {
            match manifest_path {
                Ok(path) => {
                    if let Ok(game) = self.parse_app_manifest(&path, library_path).await {
                        games.push(game);
                    }
                }
                Err(e) => {
                    warn!("Error reading manifest file: {}", e);
                }
            }
        }

        Ok(games)
    }

    async fn parse_app_manifest(&self, manifest_path: &Path, library_path: &str) -> Result<SteamGame> {
        let content = fs::read_to_string(manifest_path)
            .context("Failed to read app manifest")?;

        let appid_regex = Regex::new(r#""appid"\s+"([^"]+)""#).unwrap();
        let name_regex = Regex::new(r#""name"\s+"([^"]+)""#).unwrap();
        let installdir_regex = Regex::new(r#""installdir"\s+"([^"]+)""#).unwrap();
        let lastupdated_regex = Regex::new(r#""LastUpdated"\s+"([^"]+)""#).unwrap();
        let sizeonddisk_regex = Regex::new(r#""SizeOnDisk"\s+"([^"]+)""#).unwrap();

        let app_id = appid_regex.captures(&content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .context("AppID not found in manifest")?;

        let name = name_regex.captures(&content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .context("Name not found in manifest")?;

        let install_dir = installdir_regex.captures(&content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .context("Install directory not found in manifest")?;

        let last_updated = lastupdated_regex.captures(&content)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok());

        let size_on_disk = sizeonddisk_regex.captures(&content)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok());

        Ok(SteamGame {
            app_id,
            name,
            install_dir,
            library_path: library_path.to_string(),
            last_updated,
            size_on_disk,
        })
    }

    pub fn get_common_save_paths(&self, steam_game: &SteamGame) -> Vec<String> {
        let mut save_paths = Vec::new();
        
        // Common save file locations for Steam games
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let documents_dir = dirs::document_dir().unwrap_or_else(|| home_dir.join("Documents"));
        
        // Steam userdata directory (Steam Cloud saves)
        if let Some(steam_path) = &self.steam_path {
            let userdata_pattern = steam_path.join("userdata").join("*").join(&steam_game.app_id);
            save_paths.push(userdata_pattern.to_string_lossy().to_string());
        }

        // Game installation directory
        let install_path = Path::new(&steam_game.library_path)
            .join("common")
            .join(&steam_game.install_dir);
        save_paths.push(install_path.to_string_lossy().to_string());

        // Documents folder variations
        save_paths.push(documents_dir.join("My Games").join(&steam_game.name).to_string_lossy().to_string());
        save_paths.push(documents_dir.join(&steam_game.name).to_string_lossy().to_string());
        
        // AppData locations
        if let Some(appdata) = dirs::data_dir() {
            save_paths.push(appdata.join(&steam_game.name).to_string_lossy().to_string());
        }
        
        if let Some(local_appdata) = dirs::data_local_dir() {
            save_paths.push(local_appdata.join(&steam_game.name).to_string_lossy().to_string());
        }

        save_paths
    }
}

// Known save game patterns for popular games
pub fn get_known_save_patterns() -> HashMap<String, Vec<String>> {
    let mut patterns = HashMap::new();
    
    // Add some common games with known save locations
    patterns.insert("Elden Ring".to_string(), vec![
        r"%APPDATA%\EldenRing".to_string(),
    ]);
    
    patterns.insert("Cyberpunk 2077".to_string(), vec![
        r"%USERPROFILE%\Saved Games\CD Projekt Red\Cyberpunk 2077".to_string(),
    ]);
    
    patterns.insert("The Witcher 3".to_string(), vec![
        r"%USERPROFILE%\Documents\The Witcher 3\gamesaves".to_string(),
    ]);
    
    patterns.insert("Baldur's Gate 3".to_string(), vec![
        r"%LOCALAPPDATA%\Larian Studios\Baldur's Gate 3\PlayerProfiles".to_string(),
    ]);

    patterns.insert("Valheim".to_string(), vec![
        r"%APPDATA%\LocalLow\IronGate\Valheim".to_string(),
    ]);

    patterns
}
