use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use color_eyre::eyre::{Context, Result};
use serde::Deserialize;

fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".config")
        .join("lazydb")
}

// --- Application config (~/.config/lazydb/config.toml) ---

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u16,
}

fn default_sidebar_width() -> u16 {
    25
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sidebar_width: default_sidebar_width(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = config_dir().join("config.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .wrap_err_with(|| format!("Failed to read {}", path.display()))?;
        let config: AppConfig =
            toml::from_str(&content).wrap_err_with(|| format!("Failed to parse {}", path.display()))?;
        Ok(config)
    }
}

// --- Connection profiles (~/.config/lazydb/profiles.toml) ---

#[derive(Debug, Deserialize)]
pub struct Profiles {
    #[serde(default)]
    pub connections: BTreeMap<String, Connection>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Connection {
    #[serde(rename = "duckdb")]
    DuckDb(DuckDbConnection),
}

#[derive(Debug, Deserialize)]
pub struct DuckDbConnection {
    pub path: String,
}

impl Connection {
    pub fn type_name(&self) -> &'static str {
        match self {
            Connection::DuckDb(_) => "duckdb",
        }
    }
}

impl Default for Profiles {
    fn default() -> Self {
        Self {
            connections: BTreeMap::new(),
        }
    }
}

impl Profiles {
    pub fn load() -> Result<Self> {
        let path = config_dir().join("profiles.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .wrap_err_with(|| format!("Failed to read {}", path.display()))?;
        let profiles: Profiles =
            toml::from_str(&content).wrap_err_with(|| format!("Failed to parse {}", path.display()))?;
        Ok(profiles)
    }
}
