pub mod clickhouse_backend;
pub mod duckdb_backend;
pub mod postgres_backend;

use std::fmt;

/// A single value returned from a query.
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Text(s) => write!(f, "{s}"),
        }
    }
}

/// Tabular result from a query.
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// Schema object name lists.
pub struct SchemaInfo {
    pub tables: Vec<String>,
    pub views: Vec<String>,
}

/// Trait that all database backends implement.
pub trait Database {
    fn execute_query(&mut self, sql: &str) -> Result<QueryResult, String>;
    fn schema_info(&mut self) -> Result<SchemaInfo, String>;
}
