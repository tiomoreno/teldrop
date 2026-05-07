use anyhow::{Context, Result};
use dialoguer::Input;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_id: i32,
    pub api_hash: String,
}

impl Config {
    pub fn dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("teldrop");
        fs::create_dir_all(&dir).context("Failed to create config directory")?;
        Ok(dir)
    }

    pub fn path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("config.toml"))
    }

    pub fn session_path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("session.session"))
    }

    /// Load config from env vars, config file, or interactively prompt the user.
    pub fn load_or_prompt() -> Result<Config> {
        // Environment variables take priority
        if let (Ok(id), Ok(hash)) = (std::env::var("TG_API_ID"), std::env::var("TG_API_HASH")) {
            return Ok(Config {
                api_id: id.parse().context("TG_API_ID must be a number")?,
                api_hash: hash,
            });
        }

        let path = Self::path()?;

        if path.exists() {
            let content = fs::read_to_string(&path).context("Failed to read config file")?;
            return toml::from_str(&content).context("Failed to parse config file");
        }

        // First run: prompt the user
        println!("No credentials found. Get your API credentials at https://my.telegram.org");
        println!("(Or set TG_API_ID and TG_API_HASH environment variables.)");
        println!();

        let api_id: i32 = Input::new()
            .with_prompt("API ID")
            .interact_text()
            .context("Failed to read API ID")?;

        let api_hash: String = Input::new()
            .with_prompt("API Hash")
            .interact_text()
            .context("Failed to read API Hash")?;

        let config = Config { api_id, api_hash };

        let content = toml::to_string_pretty(&config).context("Failed to serialize config")?;
        fs::write(&path, content).context("Failed to write config file")?;
        println!("Credentials saved to {}", path.display());
        println!();

        Ok(config)
    }
}
