use duckdb::types::ValueRef;
use duckdb::Connection;

use super::{Database, QueryResult, SchemaNode, Value};

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

    fn schema_tree(&mut self) -> Result<Vec<SchemaNode>, String> {
        let schemas = self.query_string_list(
            "SELECT schema_name FROM information_schema.schemata \
             WHERE catalog_name = current_database() \
             AND schema_name NOT IN ('information_schema', 'pg_catalog') \
             ORDER BY schema_name",
        )?;

        let mut nodes = Vec::new();
        for schema in schemas {
            let table_names = self.query_string_list(&format!(
                "SELECT table_name FROM information_schema.tables \
                 WHERE table_schema = '{schema}' AND table_type = 'BASE TABLE' \
                 ORDER BY table_name"
            ))?;
            let view_names = self.query_string_list(&format!(
                "SELECT table_name FROM information_schema.tables \
                 WHERE table_schema = '{schema}' AND table_type = 'VIEW' \
                 ORDER BY table_name"
            ))?;

            let tables: Vec<SchemaNode> = table_names
                .into_iter()
                .map(|name| {
                    let cols = self.query_columns(&schema, &name).unwrap_or_default();
                    SchemaNode::group(name, cols)
                })
                .collect();

            let views: Vec<SchemaNode> = view_names
                .into_iter()
                .map(|name| {
                    let cols = self.query_columns(&schema, &name).unwrap_or_default();
                    SchemaNode::group(name, cols)
                })
                .collect();

            let mut children = Vec::new();
            if !tables.is_empty() {
                children.push(SchemaNode::group("Tables", tables));
            }
            if !views.is_empty() {
                children.push(SchemaNode::group("Views", views));
            }

            if !children.is_empty() {
                nodes.push(SchemaNode::group(schema, children));
            }
        }

        Ok(nodes)
    }
}

impl DuckDb {
    fn query_columns(&self, schema: &str, table_name: &str) -> Result<Vec<SchemaNode>, String> {
        let sql = format!(
            "SELECT column_name, data_type FROM information_schema.columns \
             WHERE table_schema = '{schema}' AND table_name = '{table_name}' \
             ORDER BY ordinal_position"
        );
        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for row in rows {
            let (name, data_type) = row.map_err(|e| e.to_string())?;
            result.push(SchemaNode::leaf(format!("{name} ({data_type})")));
        }
        Ok(result)
    }

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
