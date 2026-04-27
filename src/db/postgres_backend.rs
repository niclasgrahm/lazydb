use postgres::types::Type;
use postgres::Client;

use super::{Database, QueryResult, SchemaNode, Value};

pub struct Postgres {
    client: Client,
}

impl Postgres {
    pub fn connect(conn_str: &str) -> Result<Self, String> {
        let client = Client::connect(conn_str, postgres::NoTls).map_err(|e| e.to_string())?;
        Ok(Self { client })
    }
}

impl Database for Postgres {
    fn execute_query(&mut self, sql: &str) -> Result<QueryResult, String> {
        // For non-SELECT statements, use execute() which doesn't expect rows
        let trimmed = sql.trim_start().to_ascii_uppercase();
        let is_row_returning = trimmed.starts_with("SELECT")
            || trimmed.starts_with("WITH")
            || trimmed.starts_with("TABLE ")
            || trimmed.starts_with("VALUES")
            || trimmed.contains("RETURNING");

        if !is_row_returning {
            let affected = self.client.execute(sql, &[]).map_err(|e| e.to_string())?;
            return Ok(QueryResult {
                columns: vec!["result".into()],
                rows: vec![vec![Value::Text(format!("{affected} row(s) affected"))]],
            });
        }

        let rows = self.client.query(sql, &[]).map_err(|e| e.to_string())?;

        if rows.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
            });
        }

        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();

        let mut result_rows = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut values = Vec::with_capacity(columns.len());
            for (i, col) in row.columns().iter().enumerate() {
                let val = extract_value(&row, i, col.type_());
                values.push(val);
            }
            result_rows.push(values);
        }

        Ok(QueryResult {
            columns,
            rows: result_rows,
        })
    }

    fn schema_tree(&mut self) -> Result<Vec<SchemaNode>, String> {
        let schema_names = self.query_all_schemas()?;
        let mut schema_nodes = Vec::new();

        for schema_name in schema_names {
            let table_names = self.query_tables_for_schema(&schema_name)?;
            let view_names = self.query_views_for_schema(&schema_name)?;

            let tables: Vec<SchemaNode> = table_names
                .into_iter()
                .map(|name| {
                    let cols = self.query_columns_for_schema(&schema_name, &name).unwrap_or_default();
                    SchemaNode::group(name, cols)
                })
                .collect();

            let views: Vec<SchemaNode> = view_names
                .into_iter()
                .map(|name| {
                    let cols = self.query_columns_for_schema(&schema_name, &name).unwrap_or_default();
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

            schema_nodes.push(SchemaNode::group(&schema_name, children));
        }

        Ok(schema_nodes)
    }
}

impl Postgres {
    fn query_all_schemas(&mut self) -> Result<Vec<String>, String> {
        let rows = self
            .client
            .query(
                "SELECT schema_name FROM information_schema.schemata \
                 WHERE schema_name NOT IN ('information_schema', 'pg_catalog', 'pg_toast') \
                 AND schema_name NOT LIKE 'pg_%' \
                 ORDER BY schema_name",
                &[],
            )
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(|r| r.get::<_, String>(0)).collect())
    }

    fn query_tables_for_schema(&mut self, schema: &str) -> Result<Vec<String>, String> {
        let rows = self
            .client
            .query(
                "SELECT table_name FROM information_schema.tables \
                 WHERE table_schema = $1 AND table_type = 'BASE TABLE' \
                 ORDER BY table_name",
                &[&schema.to_string()],
            )
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(|r| r.get::<_, String>(0)).collect())
    }

    fn query_views_for_schema(&mut self, schema: &str) -> Result<Vec<String>, String> {
        let rows = self
            .client
            .query(
                "SELECT table_name FROM information_schema.tables \
                 WHERE table_schema = $1 AND table_type = 'VIEW' \
                 ORDER BY table_name",
                &[&schema.to_string()],
            )
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(|r| r.get::<_, String>(0)).collect())
    }

    fn query_columns_for_schema(
        &mut self,
        schema: &str,
        table_name: &str,
    ) -> Result<Vec<SchemaNode>, String> {
        let rows = self
            .client
            .query(
                "SELECT column_name, data_type FROM information_schema.columns \
                 WHERE table_schema = $1 AND table_name = $2 \
                 ORDER BY ordinal_position",
                &[&schema.to_string(), &table_name.to_string()],
            )
            .map_err(|e| e.to_string())?;
        Ok(rows
            .iter()
            .map(|r| {
                let name: String = r.get(0);
                let data_type: String = r.get(1);
                SchemaNode::leaf(format!("{name} ({data_type})"))
            })
            .collect())
    }
}

fn extract_value(row: &postgres::Row, idx: usize, ty: &Type) -> Value {
    match *ty {
        Type::BOOL => match row.try_get::<_, bool>(idx) {
            Ok(v) => Value::Bool(v),
            Err(_) => Value::Null,
        },
        Type::INT2 => match row.try_get::<_, i16>(idx) {
            Ok(v) => Value::Int(v as i64),
            Err(_) => Value::Null,
        },
        Type::INT4 => match row.try_get::<_, i32>(idx) {
            Ok(v) => Value::Int(v as i64),
            Err(_) => Value::Null,
        },
        Type::INT8 => match row.try_get::<_, i64>(idx) {
            Ok(v) => Value::Int(v),
            Err(_) => Value::Null,
        },
        Type::FLOAT4 => match row.try_get::<_, f32>(idx) {
            Ok(v) => Value::Float(v as f64),
            Err(_) => Value::Null,
        },
        Type::FLOAT8 | Type::NUMERIC => match row.try_get::<_, f64>(idx) {
            Ok(v) => Value::Float(v),
            Err(_) => Value::Null,
        },
        _ => match row.try_get::<_, String>(idx) {
            Ok(v) => Value::Text(v),
            Err(_) => Value::Null,
        },
    }
}
