use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::eyre::{Context, bail};
use color_eyre::Result;

use crate::config::Profiles;
use crate::db::QueryResult;

const DEFAULT_LIMIT: usize = 1000;

#[derive(Parser)]
#[command(name = "lazydb", about = "Terminal UI database client")]
pub struct Cli {
    /// Root directory for the files pane
    pub path: Option<PathBuf>,

    /// Pre-populate the query editor with this SQL
    #[arg(long)]
    pub query: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage connections
    Conns {
        #[command(subcommand)]
        action: ConnsAction,
    },
    /// Execute a query
    Query {
        /// Connection name from profiles.toml
        #[arg(long)]
        conn: String,
        /// SQL query to execute (provide --query or --file, not both)
        #[arg(long, required_unless_present = "file", conflicts_with = "file")]
        query: Option<String>,
        /// Path to a SQL file to execute
        #[arg(long)]
        file: Option<PathBuf>,
        /// Maximum number of rows to return (0 for unlimited)
        #[arg(long, default_value_t = DEFAULT_LIMIT)]
        limit: usize,
        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Table,
    Csv,
}

#[derive(Subcommand)]
pub enum ConnsAction {
    /// List configured connections
    List,
    /// Test a connection
    Test {
        /// Connection name from profiles.toml
        name: String,
    },
}

pub fn handle(cmd: Command) -> Result<()> {
    match cmd {
        Command::Conns { action } => match action {
            ConnsAction::List => conns_list(),
            ConnsAction::Test { name } => conns_test(&name),
        },
        Command::Query {
            conn,
            query,
            file,
            limit,
            format,
        } => {
            let sql = resolve_query(query, file)?;
            run_query(&conn, &sql, limit, &format)
        }
    }
}

fn conns_list() -> Result<()> {
    let profiles = Profiles::load()?;
    println!("{}", format_conns_list(&profiles));
    Ok(())
}

fn conns_test(name: &str) -> Result<()> {
    let profiles = Profiles::load()?;

    let Some(conn) = profiles.connections.get(name) else {
        eprintln!("Connection '{name}' not found.");
        let available = format_available_names(&profiles);
        if !available.is_empty() {
            eprintln!("Available connections: {available}");
        }
        std::process::exit(1);
    };

    println!("Testing connection '{name}' ({})...", conn.type_name());

    let start = Instant::now();
    match conn.connect() {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("OK ({:.0?})", elapsed);
        }
        Err(e) => {
            println!("FAILED: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn resolve_query(query: Option<String>, file: Option<PathBuf>) -> Result<String> {
    match (query, file) {
        (Some(q), _) => Ok(q),
        (_, Some(path)) => {
            std::fs::read_to_string(&path)
                .wrap_err_with(|| format!("Failed to read SQL file: {}", path.display()))
        }
        _ => bail!("Provide either --query or --file"),
    }
}

fn run_query(conn_name: &str, sql: &str, limit: usize, format: &OutputFormat) -> Result<()> {
    let profiles = Profiles::load()?;

    let Some(conn_cfg) = profiles.connections.get(conn_name) else {
        eprintln!("Connection '{conn_name}' not found.");
        let available = format_available_names(&profiles);
        if !available.is_empty() {
            eprintln!("Available connections: {available}");
        }
        std::process::exit(1);
    };

    let mut db = match conn_cfg.connect() {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Connection failed: {e}");
            std::process::exit(1);
        }
    };

    let start = Instant::now();
    let mut result = match db.execute_query(sql) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Query failed: {e}");
            std::process::exit(1);
        }
    };
    let elapsed = start.elapsed();

    let total_rows = result.rows.len();
    let truncated = limit > 0 && total_rows > limit;
    if truncated {
        result.rows.truncate(limit);
    }

    match format {
        OutputFormat::Table => {
            println!("{}", format_table(&result));
            let row_count = if truncated {
                format!("{limit} of {total_rows} rows (limited)")
            } else {
                format!(
                    "{total_rows} row{}",
                    if total_rows == 1 { "" } else { "s" }
                )
            };
            let duration = if elapsed.as_secs() >= 1 {
                format!("{:.2}s", elapsed.as_secs_f64())
            } else {
                format!("{:.1}ms", elapsed.as_secs_f64() * 1000.0)
            };
            eprintln!("{row_count} in {duration}");
        }
        OutputFormat::Csv => {
            print!("{}", format_csv(&result));
        }
    }

    Ok(())
}

fn format_available_names(profiles: &Profiles) -> String {
    profiles
        .connections
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn format_conns_list(profiles: &Profiles) -> String {
    if profiles.connections.is_empty() {
        return "No connections configured.\nAdd connections in ~/.config/lazydb/profiles.toml"
            .to_string();
    }

    profiles
        .connections
        .iter()
        .map(|(name, conn)| format!("{name} ({type})", type = conn.type_name()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_table(result: &QueryResult) -> String {
    let col_count = result.columns.len();
    if col_count == 0 {
        return String::new();
    }

    let widths: Vec<usize> = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let max_data = result
                .rows
                .iter()
                .map(|r| r.get(i).map(|v| v.to_string().len()).unwrap_or(0))
                .max()
                .unwrap_or(0);
            col.len().max(max_data).max(1)
        })
        .collect();

    let mut out = String::new();

    // Top border: ┌──┬──┐
    out.push('┌');
    for (i, &w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(w + 2));
        out.push(if i + 1 < col_count { '┬' } else { '┐' });
    }
    out.push('\n');

    // Header row
    out.push('│');
    for (i, col) in result.columns.iter().enumerate() {
        out.push_str(&format!(" {:width$} │", col, width = widths[i]));
    }
    out.push('\n');

    // Header separator: ├──┼──┤
    out.push('├');
    for (i, &w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(w + 2));
        out.push(if i + 1 < col_count { '┼' } else { '┤' });
    }
    out.push('\n');

    // Data rows
    for row in &result.rows {
        out.push('│');
        for (i, &w) in widths.iter().enumerate() {
            let val = row.get(i).map(|v| v.to_string()).unwrap_or_default();
            out.push_str(&format!(" {:width$} │", val, width = w));
        }
        out.push('\n');
    }

    // Bottom border: └──┴──┘
    out.push('└');
    for (i, &w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(w + 2));
        out.push(if i + 1 < col_count { '┴' } else { '┘' });
    }

    out
}

fn format_csv(result: &QueryResult) -> String {
    let mut out = String::new();

    // Header
    out.push_str(
        &result
            .columns
            .iter()
            .map(|c| csv_escape(c))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');

    // Rows
    for row in &result.rows {
        out.push_str(
            &row.iter()
                .map(|v| csv_escape(&v.to_string()))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }

    out
}

fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ClickHouseConnection, Connection, DuckDbConnection, PostgresConnection, Profiles,
        SnowflakeAuth, SnowflakeConnection,
    };
    use crate::db::Value;
    use std::collections::BTreeMap;

    fn make_result(columns: Vec<&str>, rows: Vec<Vec<Value>>) -> QueryResult {
        QueryResult {
            columns: columns.into_iter().map(String::from).collect(),
            rows,
        }
    }

    // --- conns list tests ---

    #[test]
    fn format_empty_profiles() {
        let profiles = Profiles {
            connections: BTreeMap::new(),
        };
        let output = format_conns_list(&profiles);
        assert!(output.contains("No connections configured."));
        assert!(output.contains("profiles.toml"));
    }

    #[test]
    fn format_single_duckdb() {
        let mut connections = BTreeMap::new();
        connections.insert(
            "mydb".to_string(),
            Connection::DuckDb(DuckDbConnection {
                path: "/tmp/test.duckdb".to_string(),
                cache_schema: false,
            }),
        );
        let profiles = Profiles { connections };
        let output = format_conns_list(&profiles);
        assert_eq!(output, "mydb (duckdb)");
    }

    #[test]
    fn format_multiple_connections_sorted_by_name() {
        let mut connections = BTreeMap::new();
        connections.insert(
            "warehouse".to_string(),
            Connection::Snowflake(SnowflakeConnection {
                account: "xy123".into(),
                auth: SnowflakeAuth::Browser {
                    user: "u".into(),
                },
                database: "PROD".into(),
                warehouse: None,
                schema: None,
                role: None,
                cache_schema: false,
            }),
        );
        connections.insert(
            "analytics".to_string(),
            Connection::Postgres(PostgresConnection {
                host: "db.example.com".into(),
                port: 5432,
                user: "admin".into(),
                password: None,
                database: "analytics".into(),
                schema: None,
                cache_schema: false,
            }),
        );
        connections.insert(
            "local".to_string(),
            Connection::DuckDb(DuckDbConnection {
                path: "/tmp/test.duckdb".into(),
                cache_schema: false,
            }),
        );
        connections.insert(
            "clicks".to_string(),
            Connection::ClickHouse(ClickHouseConnection {
                url: "http://localhost:8123".into(),
                user: "default".into(),
                password: None,
                database: "default".into(),
                cache_schema: false,
            }),
        );

        let profiles = Profiles { connections };
        let output = format_conns_list(&profiles);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "analytics (postgres)");
        assert_eq!(lines[1], "clicks (clickhouse)");
        assert_eq!(lines[2], "local (duckdb)");
        assert_eq!(lines[3], "warehouse (snowflake)");
    }

    #[test]
    fn format_available_names_empty() {
        let profiles = Profiles {
            connections: BTreeMap::new(),
        };
        assert_eq!(format_available_names(&profiles), "");
    }

    #[test]
    fn format_available_names_multiple() {
        let mut connections = BTreeMap::new();
        connections.insert(
            "beta".to_string(),
            Connection::DuckDb(DuckDbConnection { path: "x".into(), cache_schema: false }),
        );
        connections.insert(
            "alpha".to_string(),
            Connection::DuckDb(DuckDbConnection { path: "y".into(), cache_schema: false }),
        );
        let profiles = Profiles { connections };
        assert_eq!(format_available_names(&profiles), "alpha, beta");
    }

    #[test]
    fn conns_test_duckdb_memory_succeeds() {
        let mut connections = BTreeMap::new();
        connections.insert(
            "memdb".to_string(),
            Connection::DuckDb(DuckDbConnection {
                path: ":memory:".to_string(),
                cache_schema: false,
            }),
        );
        let profiles = Profiles { connections };
        let conn = profiles.connections.get("memdb").unwrap();
        assert!(conn.connect().is_ok());
    }

    // --- format_table tests ---

    #[test]
    fn table_empty_columns() {
        let result = make_result(vec![], vec![]);
        assert_eq!(format_table(&result), "");
    }

    #[test]
    fn table_header_only() {
        let result = make_result(vec!["id", "name"], vec![]);
        let table = format_table(&result);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 4); // top border, header, separator, bottom border
        assert_eq!(lines[0], "┌────┬──────┐");
        assert_eq!(lines[1], "│ id │ name │");
        assert_eq!(lines[2], "├────┼──────┤");
        assert_eq!(lines[3], "└────┴──────┘");
    }

    #[test]
    fn table_with_data() {
        let result = make_result(
            vec!["id", "name"],
            vec![
                vec![Value::Int(1), Value::Text("Alice".into())],
                vec![Value::Int(2), Value::Text("Bob".into())],
            ],
        );
        let table = format_table(&result);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0], "┌────┬───────┐");
        assert_eq!(lines[1], "│ id │ name  │");
        assert_eq!(lines[2], "├────┼───────┤");
        assert_eq!(lines[3], "│ 1  │ Alice │");
        assert_eq!(lines[4], "│ 2  │ Bob   │");
        assert_eq!(lines[5], "└────┴───────┘");
    }

    #[test]
    fn table_column_width_adapts_to_data() {
        let result = make_result(
            vec!["x"],
            vec![vec![Value::Text("longvalue".into())]],
        );
        let table = format_table(&result);
        let lines: Vec<&str> = table.lines().collect();
        // Column should be 9 wide (len of "longvalue"), not 1 (len of "x")
        assert_eq!(lines[1], "│ x         │");
        assert_eq!(lines[3], "│ longvalue │");
    }

    #[test]
    fn table_null_values() {
        let result = make_result(
            vec!["val"],
            vec![vec![Value::Null]],
        );
        let table = format_table(&result);
        assert!(table.contains("NULL"));
    }

    // --- format_csv tests ---

    #[test]
    fn csv_basic() {
        let result = make_result(
            vec!["id", "name"],
            vec![
                vec![Value::Int(1), Value::Text("Alice".into())],
                vec![Value::Int(2), Value::Text("Bob".into())],
            ],
        );
        let csv = format_csv(&result);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "id,name");
        assert_eq!(lines[1], "1,Alice");
        assert_eq!(lines[2], "2,Bob");
    }

    #[test]
    fn csv_escapes_commas() {
        let result = make_result(
            vec!["val"],
            vec![vec![Value::Text("a,b".into())]],
        );
        let csv = format_csv(&result);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[1], "\"a,b\"");
    }

    #[test]
    fn csv_escapes_quotes() {
        let result = make_result(
            vec!["val"],
            vec![vec![Value::Text("say \"hi\"".into())]],
        );
        let csv = format_csv(&result);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[1], "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn csv_null_value() {
        let result = make_result(
            vec!["val"],
            vec![vec![Value::Null]],
        );
        let csv = format_csv(&result);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[1], "NULL");
    }

    // --- limit truncation test ---

    #[test]
    fn limit_truncates_rows() {
        let mut result = make_result(
            vec!["n"],
            (0..10).map(|i| vec![Value::Int(i)]).collect(),
        );
        assert_eq!(result.rows.len(), 10);
        let limit = 3;
        result.rows.truncate(limit);
        assert_eq!(result.rows.len(), 3);
        // Verify we kept the first 3
        for (i, row) in result.rows.iter().enumerate() {
            match &row[0] {
                Value::Int(v) => assert_eq!(*v, i as i64),
                _ => panic!("expected Int"),
            }
        }
    }

    // --- resolve_query tests ---

    #[test]
    fn resolve_query_inline() {
        let sql = resolve_query(Some("SELECT 1".into()), None).unwrap();
        assert_eq!(sql, "SELECT 1");
    }

    #[test]
    fn resolve_query_from_file() {
        let dir = std::env::temp_dir().join("lazydb_test_resolve");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.sql");
        std::fs::write(&path, "SELECT * FROM foo").unwrap();
        let sql = resolve_query(None, Some(path)).unwrap();
        assert_eq!(sql, "SELECT * FROM foo");
    }

    #[test]
    fn resolve_query_missing_file() {
        let result = resolve_query(None, Some(PathBuf::from("/nonexistent/query.sql")));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_query_neither() {
        let result = resolve_query(None, None);
        assert!(result.is_err());
    }

    // --- csv_escape tests ---

    #[test]
    fn csv_escape_plain() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn csv_escape_newline() {
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }
}
