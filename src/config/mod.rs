use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use color_eyre::eyre::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::db::clickhouse_backend::ClickHouse;
use crate::db::databricks_backend::Databricks;
use crate::db::duckdb_backend::DuckDb;
use crate::db::postgres_backend::Postgres;
use crate::db::snowflake_backend::Snowflake;
use crate::keybindings::KeybindingsConfig;

pub(crate) fn config_dir() -> PathBuf {
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
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_max_recents")]
    pub max_recents: usize,
}

fn default_sidebar_width() -> u16 {
    25
}

fn default_max_recents() -> usize {
    10
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sidebar_width: default_sidebar_width(),
            keybindings: KeybindingsConfig::default(),
            debug: false,
            max_recents: default_max_recents(),
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
        let config: AppConfig = toml::from_str(&content)
            .wrap_err_with(|| format!("Failed to parse {}", path.display()))?;
        Ok(config)
    }
}

// --- Connection profiles (~/.config/lazydb/profiles.toml) ---

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Profiles {
    #[serde(default)]
    pub connections: BTreeMap<String, Connection>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Connection {
    #[serde(rename = "duckdb")]
    DuckDb(DuckDbConnection),
    #[serde(rename = "postgres")]
    Postgres(PostgresConnection),
    #[serde(rename = "clickhouse")]
    ClickHouse(ClickHouseConnection),
    #[serde(rename = "snowflake")]
    Snowflake(SnowflakeConnection),
    #[serde(rename = "databricks")]
    Databricks(DatabricksConnection),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DuckDbConnection {
    pub path: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cache_schema: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostgresConnection {
    pub host: String,
    #[serde(default = "default_pg_port")]
    pub port: u16,
    pub user: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    pub database: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cache_schema: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
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

    #[allow(dead_code)] // Part of the connection config API; covered by tests.
    pub fn schema_name(&self) -> &str {
        self.schema.as_deref().unwrap_or("public")
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClickHouseConnection {
    #[serde(default = "default_clickhouse_url")]
    pub url: String,
    #[serde(default = "default_clickhouse_user")]
    pub user: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default = "default_clickhouse_database")]
    pub database: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cache_schema: bool,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "auth")]
pub enum SnowflakeAuth {
    #[serde(rename = "password")]
    Password { user: String, password: String },
    #[serde(rename = "oauth")]
    OAuth { oauth_token: String },
    #[serde(rename = "browser")]
    Browser { user: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SnowflakeConnection {
    pub account: String,
    #[serde(flatten)]
    pub auth: SnowflakeAuth,
    pub database: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warehouse: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cache_schema: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabricksConnection {
    pub host: String,
    pub token: String,
    pub warehouse_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cache_schema: bool,
}

impl Connection {
    pub fn type_name(&self) -> &'static str {
        match self {
            Connection::DuckDb(_) => "duckdb",
            Connection::Postgres(_) => "postgres",
            Connection::ClickHouse(_) => "clickhouse",
            Connection::Snowflake(_) => "snowflake",
            Connection::Databricks(_) => "databricks",
        }
    }

    pub fn cache_schema(&self) -> bool {
        match self {
            Connection::DuckDb(c) => c.cache_schema,
            Connection::Postgres(c) => c.cache_schema,
            Connection::ClickHouse(c) => c.cache_schema,
            Connection::Snowflake(c) => c.cache_schema,
            Connection::Databricks(c) => c.cache_schema,
        }
    }

    pub fn connect(&self) -> Result<Box<dyn Database>, String> {
        match self {
            Connection::DuckDb(cfg) => {
                DuckDb::connect(&cfg.path).map(|db| Box::new(db) as Box<dyn Database>)
            }
            Connection::Postgres(cfg) => Postgres::connect(&cfg.connection_string())
                .map(|db| Box::new(db) as Box<dyn Database>),
            Connection::ClickHouse(cfg) => {
                ClickHouse::connect(&cfg.url, &cfg.database, &cfg.user, cfg.password.as_deref())
                    .map(|db| Box::new(db) as Box<dyn Database>)
            }
            Connection::Snowflake(cfg) => match &cfg.auth {
                SnowflakeAuth::Password { user, password } => Snowflake::connect_password(
                    &cfg.account,
                    user,
                    password,
                    &cfg.database,
                    cfg.warehouse.as_deref(),
                    cfg.schema.as_deref(),
                    cfg.role.as_deref(),
                )
                .map(|db| Box::new(db) as Box<dyn Database>),
                SnowflakeAuth::OAuth { oauth_token } => Snowflake::connect_oauth(
                    &cfg.account,
                    oauth_token,
                    &cfg.database,
                    cfg.warehouse.as_deref(),
                    cfg.schema.as_deref(),
                    cfg.role.as_deref(),
                )
                .map(|db| Box::new(db) as Box<dyn Database>),
                SnowflakeAuth::Browser { user } => Snowflake::connect_browser(
                    &cfg.account,
                    user,
                    &cfg.database,
                    cfg.warehouse.as_deref(),
                    cfg.schema.as_deref(),
                    cfg.role.as_deref(),
                )
                .map(|db| Box::new(db) as Box<dyn Database>),
            },
            Connection::Databricks(cfg) => Databricks::connect(
                &cfg.host,
                &cfg.token,
                &cfg.warehouse_id,
                cfg.catalog.as_deref(),
                cfg.schema.as_deref(),
            )
            .map(|db| Box::new(db) as Box<dyn Database>),
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
        let profiles: Profiles = toml::from_str(&content)
            .wrap_err_with(|| format!("Failed to parse {}", path.display()))?;
        Ok(profiles)
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = config_dir().join("profiles.toml");
        self.save_to(&path)?;
        Ok(path)
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .wrap_err_with(|| format!("Failed to create {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self).wrap_err("Failed to serialize profiles")?;
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, &content).wrap_err_with(|| format!("Failed to write {}", tmp.display()))?;
        fs::rename(&tmp, path).wrap_err_with(|| {
            format!("Failed to rename {} -> {}", tmp.display(), path.display())
        })?;
        Ok(())
    }
}

pub fn validate_new_name(profiles: &Profiles, name: &str) -> std::result::Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty".into());
    }
    if trimmed != name {
        return Err("Name cannot have leading or trailing whitespace".into());
    }
    if profiles.connections.contains_key(name) {
        return Err(format!("Connection '{name}' already exists"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duckdb_profile() {
        let toml = r#"
            [connections.mydb]
            type = "duckdb"
            path = "/tmp/test.duckdb"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        assert!(matches!(
            profiles.connections.get("mydb"),
            Some(Connection::DuckDb(DuckDbConnection { path, .. })) if path == "/tmp/test.duckdb"
        ));
    }

    #[test]
    fn parse_postgres_full() {
        let toml = r#"
            [connections.pg]
            type = "postgres"
            host = "db.example.com"
            port = 5433
            user = "admin"
            password = "secret"
            database = "analytics"
            schema = "reporting"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        let conn = profiles.connections.get("pg").unwrap();
        match conn {
            Connection::Postgres(pg) => {
                assert_eq!(pg.host, "db.example.com");
                assert_eq!(pg.port, 5433);
                assert_eq!(pg.user, "admin");
                assert_eq!(pg.password.as_deref(), Some("secret"));
                assert_eq!(pg.database, "analytics");
                assert_eq!(pg.schema.as_deref(), Some("reporting"));
            }
            _ => panic!("expected Postgres"),
        }
    }

    #[test]
    fn parse_postgres_defaults() {
        let toml = r#"
            [connections.pg]
            type = "postgres"
            host = "localhost"
            user = "test"
            database = "testdb"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        match profiles.connections.get("pg").unwrap() {
            Connection::Postgres(pg) => {
                assert_eq!(pg.port, 5432);
                assert_eq!(pg.password, None);
                assert_eq!(pg.schema, None);
            }
            _ => panic!("expected Postgres"),
        }
    }

    #[test]
    fn connection_string_with_password() {
        let pg = PostgresConnection {
            host: "localhost".into(),
            port: 5432,
            user: "admin".into(),
            password: Some("secret".into()),
            database: "mydb".into(),
            schema: None,
            cache_schema: false,
        };
        let s = pg.connection_string();
        assert!(s.contains("host=localhost"));
        assert!(s.contains("port=5432"));
        assert!(s.contains("user=admin"));
        assert!(s.contains("dbname=mydb"));
        assert!(s.contains("password=secret"));
    }

    #[test]
    fn connection_string_without_password() {
        let pg = PostgresConnection {
            host: "localhost".into(),
            port: 5432,
            user: "admin".into(),
            password: None,
            database: "mydb".into(),
            schema: None,
            cache_schema: false,
        };
        let s = pg.connection_string();
        assert!(!s.contains("password"));
    }

    #[test]
    fn schema_name_default() {
        let pg = PostgresConnection {
            host: "localhost".into(),
            port: 5432,
            user: "test".into(),
            password: None,
            database: "db".into(),
            schema: None,
            cache_schema: false,
        };
        assert_eq!(pg.schema_name(), "public");
    }

    #[test]
    fn schema_name_custom() {
        let pg = PostgresConnection {
            host: "localhost".into(),
            port: 5432,
            user: "test".into(),
            password: None,
            database: "db".into(),
            schema: Some("reporting".into()),
            cache_schema: false,
        };
        assert_eq!(pg.schema_name(), "reporting");
    }

    #[test]
    fn parse_clickhouse_defaults() {
        let toml = r#"
            [connections.ch]
            type = "clickhouse"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        match profiles.connections.get("ch").unwrap() {
            Connection::ClickHouse(ch) => {
                assert_eq!(ch.url, "http://localhost:8123");
                assert_eq!(ch.user, "default");
                assert_eq!(ch.database, "default");
                assert_eq!(ch.password, None);
            }
            _ => panic!("expected ClickHouse"),
        }
    }

    #[test]
    fn parse_snowflake_password_auth() {
        let toml = r#"
            [connections.sf]
            type = "snowflake"
            account = "xy12345"
            auth = "password"
            user = "user@example.com"
            password = "pw"
            database = "PROD"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        match profiles.connections.get("sf").unwrap() {
            Connection::Snowflake(sf) => {
                assert_eq!(sf.account, "xy12345");
                assert_eq!(sf.database, "PROD");
                assert!(
                    matches!(&sf.auth, SnowflakeAuth::Password { user, .. } if user == "user@example.com")
                );
            }
            _ => panic!("expected Snowflake"),
        }
    }

    #[test]
    fn parse_snowflake_oauth_auth() {
        let toml = r#"
            [connections.sf]
            type = "snowflake"
            account = "xy12345"
            auth = "oauth"
            oauth_token = "tok123"
            database = "PROD"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        match profiles.connections.get("sf").unwrap() {
            Connection::Snowflake(sf) => {
                assert!(
                    matches!(&sf.auth, SnowflakeAuth::OAuth { oauth_token } if oauth_token == "tok123")
                );
            }
            _ => panic!("expected Snowflake"),
        }
    }

    #[test]
    fn parse_snowflake_browser_auth() {
        let toml = r#"
            [connections.sf]
            type = "snowflake"
            account = "xy12345"
            auth = "browser"
            user = "user@example.com"
            database = "PROD"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        match profiles.connections.get("sf").unwrap() {
            Connection::Snowflake(sf) => {
                assert!(
                    matches!(&sf.auth, SnowflakeAuth::Browser { user } if user == "user@example.com")
                );
            }
            _ => panic!("expected Snowflake"),
        }
    }

    #[test]
    fn type_name_variants() {
        let duckdb = Connection::DuckDb(DuckDbConnection {
            path: "x".into(),
            cache_schema: false,
        });
        assert_eq!(duckdb.type_name(), "duckdb");

        let pg = Connection::Postgres(PostgresConnection {
            host: "h".into(),
            port: 5432,
            user: "u".into(),
            password: None,
            database: "d".into(),
            schema: None,
            cache_schema: false,
        });
        assert_eq!(pg.type_name(), "postgres");

        let ch = Connection::ClickHouse(ClickHouseConnection {
            url: "u".into(),
            user: "u".into(),
            password: None,
            database: "d".into(),
            cache_schema: false,
        });
        assert_eq!(ch.type_name(), "clickhouse");

        let sf = Connection::Snowflake(SnowflakeConnection {
            account: "a".into(),
            auth: SnowflakeAuth::Browser { user: "u".into() },
            database: "d".into(),
            warehouse: None,
            schema: None,
            role: None,
            cache_schema: false,
        });
        assert_eq!(sf.type_name(), "snowflake");

        let db = Connection::Databricks(DatabricksConnection {
            host: "h".into(),
            token: "t".into(),
            warehouse_id: "w".into(),
            catalog: None,
            schema: None,
            cache_schema: false,
        });
        assert_eq!(db.type_name(), "databricks");
    }

    #[test]
    fn parse_databricks_profile() {
        let toml = r#"
            [connections.db]
            type = "databricks"
            host = "adb-1234567890123456.7.azuredatabricks.net"
            token = "dapi0123456789abcdef"
            warehouse_id = "abc123def456"
            catalog = "main"
            schema = "default"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        match profiles.connections.get("db").unwrap() {
            Connection::Databricks(db) => {
                assert_eq!(db.host, "adb-1234567890123456.7.azuredatabricks.net");
                assert_eq!(db.token, "dapi0123456789abcdef");
                assert_eq!(db.warehouse_id, "abc123def456");
                assert_eq!(db.catalog.as_deref(), Some("main"));
                assert_eq!(db.schema.as_deref(), Some("default"));
            }
            _ => panic!("expected Databricks"),
        }
    }

    #[test]
    fn parse_databricks_minimal() {
        let toml = r#"
            [connections.db]
            type = "databricks"
            host = "workspace.azuredatabricks.net"
            token = "dapi_token"
            warehouse_id = "wh123"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        match profiles.connections.get("db").unwrap() {
            Connection::Databricks(db) => {
                assert_eq!(db.host, "workspace.azuredatabricks.net");
                assert_eq!(db.catalog, None);
                assert_eq!(db.schema, None);
            }
            _ => panic!("expected Databricks"),
        }
    }

    #[test]
    fn config_default_max_recents() {
        let config: AppConfig = toml::from_str("").unwrap();
        assert_eq!(config.max_recents, 10);
    }

    #[test]
    fn config_custom_max_recents() {
        let config: AppConfig = toml::from_str("max_recents = 25").unwrap();
        assert_eq!(config.max_recents, 25);
    }

    #[test]
    fn cache_schema_defaults_to_false() {
        let toml = r#"
            [connections.pg]
            type = "postgres"
            host = "h"
            user = "u"
            database = "d"
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        let conn = profiles.connections.get("pg").unwrap();
        assert!(!conn.cache_schema());
    }

    #[test]
    fn cache_schema_parses_when_set() {
        let toml = r#"
            [connections.sf]
            type = "snowflake"
            account = "x"
            auth = "browser"
            user = "u"
            database = "d"
            cache_schema = true
        "#;
        let profiles: Profiles = toml::from_str(toml).unwrap();
        let conn = profiles.connections.get("sf").unwrap();
        assert!(conn.cache_schema());
    }

    #[test]
    fn cache_schema_on_each_variant() {
        let cases = [
            (
                r#"type = "duckdb"
                path = "/tmp/x.db"
                cache_schema = true"#,
                true,
            ),
            (
                r#"type = "clickhouse"
                cache_schema = true"#,
                true,
            ),
            (
                r#"type = "databricks"
                host = "h"
                token = "t"
                warehouse_id = "w"
                cache_schema = true"#,
                true,
            ),
        ];
        for (body, expected) in cases {
            let toml = format!("[connections.c]\n{body}");
            let profiles: Profiles = toml::from_str(&toml).expect(&toml);
            assert_eq!(
                profiles.connections.get("c").unwrap().cache_schema(),
                expected,
                "for body: {body}"
            );
        }
    }

    // --- save / round-trip tests ---

    fn round_trip(conn: Connection) -> Connection {
        let mut profiles = Profiles::default();
        profiles.connections.insert("c".to_string(), conn);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profiles.toml");
        profiles.save_to(&path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let parsed: Profiles = toml::from_str(&content).unwrap();
        parsed.connections.into_iter().next().unwrap().1
    }

    #[test]
    fn save_round_trip_duckdb() {
        let c = round_trip(Connection::DuckDb(DuckDbConnection {
            path: "/tmp/x.db".into(),
            cache_schema: true,
        }));
        match c {
            Connection::DuckDb(d) => {
                assert_eq!(d.path, "/tmp/x.db");
                assert!(d.cache_schema);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn save_round_trip_postgres_with_optionals() {
        let c = round_trip(Connection::Postgres(PostgresConnection {
            host: "h".into(),
            port: 5433,
            user: "u".into(),
            password: Some("pw".into()),
            database: "d".into(),
            schema: Some("s".into()),
            cache_schema: false,
        }));
        match c {
            Connection::Postgres(p) => {
                assert_eq!(p.port, 5433);
                assert_eq!(p.password.as_deref(), Some("pw"));
                assert_eq!(p.schema.as_deref(), Some("s"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn save_omits_none_options() {
        let mut profiles = Profiles::default();
        profiles.connections.insert(
            "c".into(),
            Connection::Postgres(PostgresConnection {
                host: "h".into(),
                port: 5432,
                user: "u".into(),
                password: None,
                database: "d".into(),
                schema: None,
                cache_schema: false,
            }),
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profiles.toml");
        profiles.save_to(&path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("password"));
        assert!(!content.contains("schema"));
        assert!(!content.contains("cache_schema"));
    }

    #[test]
    fn save_round_trip_snowflake_password() {
        let c = round_trip(Connection::Snowflake(SnowflakeConnection {
            account: "xy".into(),
            auth: SnowflakeAuth::Password {
                user: "u".into(),
                password: "pw".into(),
            },
            database: "D".into(),
            warehouse: Some("W".into()),
            schema: None,
            role: None,
            cache_schema: false,
        }));
        match c {
            Connection::Snowflake(s) => {
                assert!(matches!(s.auth, SnowflakeAuth::Password { user, password }
                    if user == "u" && password == "pw"));
                assert_eq!(s.warehouse.as_deref(), Some("W"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn save_round_trip_snowflake_browser() {
        let c = round_trip(Connection::Snowflake(SnowflakeConnection {
            account: "xy".into(),
            auth: SnowflakeAuth::Browser {
                user: "u@e.com".into(),
            },
            database: "D".into(),
            warehouse: None,
            schema: None,
            role: None,
            cache_schema: false,
        }));
        match c {
            Connection::Snowflake(s) => {
                assert!(matches!(s.auth, SnowflakeAuth::Browser { user } if user == "u@e.com"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn save_round_trip_clickhouse_and_databricks() {
        let c = round_trip(Connection::ClickHouse(ClickHouseConnection {
            url: "http://x:8123".into(),
            user: "default".into(),
            password: Some("p".into()),
            database: "d".into(),
            cache_schema: true,
        }));
        match c {
            Connection::ClickHouse(ch) => {
                assert_eq!(ch.url, "http://x:8123");
                assert_eq!(ch.password.as_deref(), Some("p"));
                assert!(ch.cache_schema);
            }
            _ => panic!("wrong variant"),
        }

        let c = round_trip(Connection::Databricks(DatabricksConnection {
            host: "h".into(),
            token: "t".into(),
            warehouse_id: "w".into(),
            catalog: Some("c".into()),
            schema: None,
            cache_schema: false,
        }));
        match c {
            Connection::Databricks(d) => {
                assert_eq!(d.host, "h");
                assert_eq!(d.catalog.as_deref(), Some("c"));
                assert_eq!(d.schema, None);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn save_atomically_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profiles.toml");
        fs::write(&path, "garbage").unwrap();

        let mut profiles = Profiles::default();
        profiles.connections.insert(
            "c".into(),
            Connection::DuckDb(DuckDbConnection {
                path: "x".into(),
                cache_schema: false,
            }),
        );
        profiles.save_to(&path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("duckdb"));
        assert!(!content.contains("garbage"));
    }

    // --- validate_new_name tests ---

    #[test]
    fn validate_new_name_accepts_fresh() {
        let profiles = Profiles::default();
        assert!(validate_new_name(&profiles, "fresh").is_ok());
    }

    #[test]
    fn validate_new_name_rejects_empty() {
        let profiles = Profiles::default();
        assert!(validate_new_name(&profiles, "").is_err());
        assert!(validate_new_name(&profiles, "   ").is_err());
    }

    #[test]
    fn validate_new_name_rejects_whitespace_padding() {
        let profiles = Profiles::default();
        assert!(validate_new_name(&profiles, " foo").is_err());
        assert!(validate_new_name(&profiles, "foo ").is_err());
    }

    #[test]
    fn validate_new_name_rejects_duplicate() {
        let mut profiles = Profiles::default();
        profiles.connections.insert(
            "dup".into(),
            Connection::DuckDb(DuckDbConnection {
                path: "x".into(),
                cache_schema: false,
            }),
        );
        let err = validate_new_name(&profiles, "dup").unwrap_err();
        assert!(err.contains("dup"));
    }

    #[test]
    fn malformed_toml_missing_required_field() {
        let toml = r#"
            [connections.pg]
            type = "postgres"
            host = "localhost"
        "#;
        // Missing user and database fields
        let result: Result<Profiles, _> = toml::from_str(toml);
        assert!(result.is_err());
    }
}
