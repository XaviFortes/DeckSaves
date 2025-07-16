use crate::SyncConfig;
use anyhow::{Result, Context};
use directories::ProjectDirs;
use std::path::PathBuf;
use tokio::fs;

pub struct ConfigManager {
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let project_dirs = ProjectDirs::from("com", "decksaves", "game-sync")
            .context("Failed to get project directories")?;
        
        let config_dir = project_dirs.config_dir();
        let config_path = config_dir.join("config.toml");
        
        Ok(Self { config_path })
    }

    pub async fn load_config(&self) -> Result<SyncConfig> {
        println!("DEBUG CONFIG: Loading config from {:?}", self.config_path);
        
        if !self.config_path.exists() {
            println!("DEBUG CONFIG: Config file doesn't exist, creating default");
            let default_config = SyncConfig::default();
            self.save_config(&default_config).await?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(&self.config_path).await
            .context("Failed to read config file")?;
        
        println!("DEBUG CONFIG: Config file content length: {} chars", content.len());
        println!("DEBUG CONFIG: Config file content:\n{}", content);
        
        let config: SyncConfig = toml::from_str(&content)
            .context("Failed to parse config file")?;
        
        println!("DEBUG CONFIG: Parsed config - aws_access_key_id present: {}, aws_secret_access_key present: {}", 
                config.aws_access_key_id.is_some(), 
                config.aws_secret_access_key.is_some());
        
        Ok(config)
    }

    pub async fn save_config(&self, config: &SyncConfig) -> Result<()> {
        println!("DEBUG CONFIG: Saving config to {:?}", self.config_path);
        println!("DEBUG CONFIG: Config before save - aws_access_key_id present: {}, aws_secret_access_key present: {}", 
                config.aws_access_key_id.is_some(), 
                config.aws_secret_access_key.is_some());
        
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).await
                .context("Failed to create config directory")?;
        }

        let content = toml::to_string_pretty(config)
            .context("Failed to serialize config")?;
        
        println!("DEBUG CONFIG: Serialized content length: {} chars", content.len());
        println!("DEBUG CONFIG: Serialized content:\n{}", content);
        
        fs::write(&self.config_path, content).await
            .context("Failed to write config file")?;
        
        println!("DEBUG CONFIG: Config saved successfully");
        Ok(())
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new().expect("Failed to create config manager")
    }
}
