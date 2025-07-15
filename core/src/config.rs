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
        if !self.config_path.exists() {
            let default_config = SyncConfig::default();
            self.save_config(&default_config).await?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(&self.config_path).await
            .context("Failed to read config file")?;
        
        let config: SyncConfig = toml::from_str(&content)
            .context("Failed to parse config file")?;
        
        Ok(config)
    }

    pub async fn save_config(&self, config: &SyncConfig) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).await
                .context("Failed to create config directory")?;
        }

        let content = toml::to_string_pretty(config)
            .context("Failed to serialize config")?;
        
        fs::write(&self.config_path, content).await
            .context("Failed to write config file")?;
        
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
