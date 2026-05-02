use anyhow::{Context, anyhow};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::{self, File, create_dir};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub server: Option<String>,
}

impl Config {
    pub async fn load() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        Self::load_file().await.unwrap_or_else(|err| {
            info!("Failed to load config, using empty config: {err}");
            Self::default()
        })
    }

    pub async fn save(&self) -> anyhow::Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        self.save_file().await
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn load_file() -> anyhow::Result<Self> {
        let config_file = Self::open_file()
            .await
            .context("Failed to open config file")?;
        let config = fs::read(config_file)
            .await
            .context("Failed to read config file")?;
        if config.is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_slice(&config).context("Failed to parse config file")
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn save_file(&self) -> anyhow::Result<()> {
        let config_file = Self::open_file()
            .await
            .context("Failed to open config file")?;
        let config = serde_json::to_vec(&self).context("Failed to serialize config")?;
        fs::write(&config_file, config)
            .await
            .context("Failed to write config file")?;
        info!("Config saved to {}", config_file.display());
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn open_file() -> anyhow::Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or(anyhow!("No config directory found"))?;
        if !config_dir.exists() {
            create_dir(&config_dir)
                .await
                .context("Failed to create config directory")?;
        } else if !config_dir.is_dir() {
            return Err(anyhow!("Config directory is not a directory"));
        }
        let config_dir = config_dir.join("home-control-app");
        if !config_dir.exists() {
            create_dir(&config_dir)
                .await
                .context("Failed to create app config directory")?;
        } else if !config_dir.is_dir() {
            return Err(anyhow!("App config directory is not a directory"));
        }
        let config_file = config_dir.join("config.json");
        if !config_file.exists() {
            File::create(&config_file)
                .await
                .context("Failed to create app config file")?;
        } else if !config_file.is_file() {
            return Err(anyhow!("Config file is not a file"));
        }
        Ok(config_file)
    }
}
