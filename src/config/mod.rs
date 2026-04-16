use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use color_eyre::eyre::{Context, Result};
use serde::Deserialize;

use crate::keybindings::KeybindingsConfig;

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
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

fn default_sidebar_width() -> u16 {
    25
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sidebar_width: default_sidebar_width(),
            keybindings: KeybindingsConfig::default(),
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
    #[serde(rename = "postgres")]
    Postgres(PostgresConnection),
    #[serde(rename = "clickhouse")]
    ClickHouse(ClickHouseConnection),
}

#[derive(Debug, Deserialize)]
pub struct DuckDbConnection {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct PostgresConnection {
    pub host: String,
    #[serde(default = "default_pg_port")]
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub password: Option<String>,
    pub database: String,
    #[serde(default)]
    pub schema: Option<String>,
}

fn default_pg_port() -> u16 {
    5432
}

impl PostgresConnection {
    pub fn connection_string(&self) -> String {
        let mut s = format!(
            "host={} port={} user={} dbname={}",
            self.host, self.port, self.user, self.database
        );
        if let Some(pw) = &self.password {
            s.push_str(&format!(" password={pw}"));
        }
        s
    }

    pub fn schema_name(&self) -> &str {
        self.schema.as_deref().unwrap_or("public")
    }
}

#[derive(Debug, Deserialize)]
pub struct ClickHouseConnection {
    #[serde(default = "default_clickhouse_url")]
    pub url: String,
    #[serde(default = "default_clickhouse_user")]
    pub user: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "default_clickhouse_database")]
    pub database: String,
}

fn default_clickhouse_url() -> String {
    "http://localhost:8123".to_string()
}

fn default_clickhouse_user() -> String {
    "default".to_string()
}

fn default_clickhouse_database() -> String {
    "default".to_string()
}

impl Connection {
    pub fn type_name(&self) -> &'static str {
        match self {
            Connection::DuckDb(_) => "duckdb",
            Connection::Postgres(_) => "postgres",
            Connection::ClickHouse(_) => "clickhouse",
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
