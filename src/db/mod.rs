pub mod clickhouse_backend;
pub mod duckdb_backend;
pub mod postgres_backend;
pub mod snowflake_backend;

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
#[derive(Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// A node in the schema object tree returned by backends.
/// Each backend builds its own hierarchy (e.g. schema → tables → columns).
#[derive(Clone)]
pub struct SchemaNode {
    pub label: String,
    pub children: Vec<SchemaNode>,
}

impl SchemaNode {
    pub fn leaf(label: impl Into<String>) -> Self {
        Self { label: label.into(), children: vec![] }
    }

    pub fn group(label: impl Into<String>, children: Vec<SchemaNode>) -> Self {
        Self { label: label.into(), children }
    }
}

/// Trait that all database backends implement.
pub trait Database: Send {
    fn execute_query(&mut self, sql: &str) -> Result<QueryResult, String>;
    fn schema_tree(&mut self) -> Result<Vec<SchemaNode>, String>;
}

#[cfg(test)]
pub struct MockDatabase {
    pub query_results: Vec<Result<QueryResult, String>>,
    pub schema: Vec<SchemaNode>,
    call_count: usize,
}

#[cfg(test)]
impl MockDatabase {
    pub fn new() -> Self {
        Self {
            query_results: vec![],
            schema: vec![],
            call_count: 0,
        }
    }

    pub fn with_schema(mut self, schema: Vec<SchemaNode>) -> Self {
        self.schema = schema;
        self
    }

    pub fn with_query_results(mut self, results: Vec<Result<QueryResult, String>>) -> Self {
        self.query_results = results;
        self
    }
}

#[cfg(test)]
impl Database for MockDatabase {
    fn execute_query(&mut self, _sql: &str) -> Result<QueryResult, String> {
        let idx = self.call_count;
        self.call_count += 1;
        self.query_results
            .get(idx)
            .cloned()
            .unwrap_or(Err("no more mock results".into()))
    }

    fn schema_tree(&mut self) -> Result<Vec<SchemaNode>, String> {
        Ok(self.schema.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_display_null() {
        assert_eq!(Value::Null.to_string(), "NULL");
    }

    #[test]
    fn value_display_bool() {
        assert_eq!(Value::Bool(true).to_string(), "true");
        assert_eq!(Value::Bool(false).to_string(), "false");
    }

    #[test]
    fn value_display_int() {
        assert_eq!(Value::Int(42).to_string(), "42");
        assert_eq!(Value::Int(-1).to_string(), "-1");
    }

    #[test]
    fn value_display_float() {
        assert_eq!(Value::Float(3.14).to_string(), "3.14");
    }
}
