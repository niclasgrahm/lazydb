use postgres::types::Type;
use postgres::Client;

use super::{Database, QueryResult, SchemaNode, Value};

pub struct Postgres {
    client: Client,
    schema: String,
}

impl Postgres {
    pub fn connect(conn_str: &str, schema: &str) -> Result<Self, String> {
        let client = Client::connect(conn_str, postgres::NoTls).map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            schema: schema.to_string(),
        })
    }
}

impl Database for Postgres {
    fn execute_query(&mut self, sql: &str) -> Result<QueryResult, String> {
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
        let table_names = self.query_string_list(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = $1 AND table_type = 'BASE TABLE' \
             ORDER BY table_name",
        )?;
        let view_names = self.query_string_list(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = $1 AND table_type = 'VIEW' \
             ORDER BY table_name",
        )?;

        let tables: Vec<SchemaNode> = table_names
            .into_iter()
            .map(|name| {
                let cols = self.query_columns(&name).unwrap_or_default();
                SchemaNode::group(name, cols)
            })
            .collect();

        let views: Vec<SchemaNode> = view_names
            .into_iter()
            .map(|name| {
                let cols = self.query_columns(&name).unwrap_or_default();
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

        // Wrap in schema node
        Ok(vec![SchemaNode::group(&self.schema, children)])
    }
}

impl Postgres {
    fn query_columns(&mut self, table_name: &str) -> Result<Vec<SchemaNode>, String> {
        let rows = self
            .client
            .query(
                "SELECT column_name, data_type FROM information_schema.columns \
                 WHERE table_schema = $1 AND table_name = $2 \
                 ORDER BY ordinal_position",
                &[&self.schema, &table_name.to_string()],
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

    fn query_string_list(&mut self, sql: &str) -> Result<Vec<String>, String> {
        let rows = self
            .client
            .query(sql, &[&self.schema])
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(|r| r.get::<_, String>(0)).collect())
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
