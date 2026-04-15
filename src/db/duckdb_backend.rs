use duckdb::types::ValueRef;
use duckdb::Connection;

use super::{Database, QueryResult, SchemaInfo, Value};

pub struct DuckDb {
    conn: Connection,
}

impl DuckDb {
    pub fn connect(path: &str) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }
}

impl Database for DuckDb {
    fn execute_query(&mut self, sql: &str) -> Result<QueryResult, String> {
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        let mut result_rows = stmt.query([]).map_err(|e| e.to_string())?;

        let column_count = result_rows.as_ref().map(|s| s.column_count()).unwrap_or(0);
        let columns: Vec<String> = (0..column_count)
            .map(|i| {
                result_rows
                    .as_ref()
                    .map(|s| s.column_name(i).map_or("?", |v| v).to_string())
                    .unwrap_or_else(|| "?".to_string())
            })
            .collect();

        let mut rows = Vec::new();
        while let Some(row) = result_rows.next().map_err(|e| e.to_string())? {
            let mut values = Vec::with_capacity(column_count);
            for i in 0..column_count {
                let val = match row.get_ref(i) {
                    Ok(ValueRef::Null) => Value::Null,
                    Ok(ValueRef::Boolean(b)) => Value::Bool(b),
                    Ok(ValueRef::Int(i)) => Value::Int(i as i64),
                    Ok(ValueRef::BigInt(i)) => Value::Int(i),
                    Ok(ValueRef::HugeInt(i)) => Value::Text(i.to_string()),
                    Ok(ValueRef::Float(f)) => Value::Float(f as f64),
                    Ok(ValueRef::Double(f)) => Value::Float(f),
                    Ok(ValueRef::Text(s)) => {
                        Value::Text(String::from_utf8_lossy(s).into_owned())
                    }
                    Ok(_) => Value::Text("<unsupported>".to_string()),
                    Err(e) => Value::Text(format!("<error: {e}>")),
                };
                values.push(val);
            }
            rows.push(values);
        }

        Ok(QueryResult { columns, rows })
    }

    fn schema_info(&mut self) -> Result<SchemaInfo, String> {
        let tables = self.query_string_list(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = 'main' AND table_type = 'BASE TABLE' \
             ORDER BY table_name",
        )?;
        let views = self.query_string_list(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = 'main' AND table_type = 'VIEW' \
             ORDER BY table_name",
        )?;
        Ok(SchemaInfo { tables, views })
    }
}

impl DuckDb {
    fn query_string_list(&self, sql: &str) -> Result<Vec<String>, String> {
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| e.to_string())?);
        }
        Ok(result)
    }
}
