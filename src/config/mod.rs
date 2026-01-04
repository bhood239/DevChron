pub mod settings;

use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;
use crate::error::{Error, Result};
use settings::Settings;

pub struct Config {
    pub settings: Settings,
    config_path: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        
        let settings = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            toml::from_str::<Settings>(&content)?
        } else {
            // Create default config
            let settings = Settings::default();
            Self::save_default_config(&config_path, &settings)?;
            settings
        };
        
        Ok(Self {
            settings,
            config_path,
        })
    }
    
    fn get_config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("", "", "devchron")
            .ok_or_else(|| Error::Config("Could not determine config directory".to_string()))?;
        
        let config_dir = proj_dirs.config_dir();
        fs::create_dir_all(config_dir)?;
        
        Ok(config_dir.join("config.toml"))
    }
    
    fn save_default_config(path: &PathBuf, settings: &Settings) -> Result<()> {
        let toml_string = toml::to_string_pretty(settings)
            .map_err(|e| Error::Config(e.to_string()))?;
        fs::write(path, toml_string)?;
        Ok(())
    }
    
    pub fn reload(&mut self) -> Result<()> {
        if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path)?;
            self.settings = toml::from_str(&content)?;
        }
        Ok(())
    }
}
