use serde_json;

use super::{Database, ProgressFn, QueryResult, SchemaNode, Value};

pub struct ClickHouse {
    base_url: String,
    database: String,
    user: String,
    password: Option<String>,
}

impl ClickHouse {
    pub fn connect(
        url: &str,
        database: &str,
        user: &str,
        password: Option<&str>,
    ) -> Result<Self, String> {
        let ch = Self {
            base_url: url.trim_end_matches('/').to_string(),
            database: database.to_string(),
            user: user.to_string(),
            password: password.map(|s| s.to_string()),
        };
        // Verify connectivity with a simple query
        ch.raw_query("SELECT 1")?;
        Ok(ch)
    }

    fn raw_query(&self, sql: &str) -> Result<String, String> {
        let full_sql = format!("{sql} FORMAT JSONCompact");
        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .http_status_as_error(false)
                .build(),
        );
        let mut req = agent
            .post(&self.base_url)
            .query("database", &self.database)
            .query("user", &self.user);

        if let Some(pw) = &self.password {
            req = req.query("password", pw);
        }

        let mut response = req
            .send(full_sql.as_bytes())
            .map_err(|e| format!("ClickHouse request failed: {e}"))?;

        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read ClickHouse response: {e}"))?;

        if status >= 400 {
            // ClickHouse returns the error message in the response body
            let msg = body.trim().to_string();
            return Err(msg);
        }

        Ok(body)
    }

    fn query_string_list(&self, sql: &str) -> Result<Vec<String>, String> {
        let body = self.raw_query(sql)?;
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;

        let rows = json["data"]
            .as_array()
            .ok_or("Expected 'data' array in response")?;

        let mut result = Vec::new();
        for row in rows {
            if let Some(arr) = row.as_array() {
                if let Some(val) = arr.first().and_then(|v| v.as_str()) {
                    result.push(val.to_string());
                }
            }
        }
        Ok(result)
    }

    fn query_columns(&self, table_name: &str) -> Result<Vec<SchemaNode>, String> {
        let sql = format!(
            "SELECT name, type FROM system.columns \
             WHERE database = '{}' AND table = '{}' \
             ORDER BY position",
            self.database, table_name
        );
        let body = self.raw_query(&sql)?;
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;
        let rows = json["data"]
            .as_array()
            .ok_or("Expected 'data' array in response")?;
        let mut result = Vec::new();
        for row in rows {
            if let Some(arr) = row.as_array() {
                let name = arr.first().and_then(|v| v.as_str()).unwrap_or("?");
                let data_type = arr.get(1).and_then(|v| v.as_str()).unwrap_or("?");
                result.push(SchemaNode::leaf(format!("{name} ({data_type})")));
            }
        }
        Ok(result)
    }
}

impl Database for ClickHouse {
    fn execute_query(&mut self, sql: &str) -> Result<QueryResult, String> {
        let body = self.raw_query(sql)?;
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;

        let meta = json["meta"]
            .as_array()
            .ok_or("Expected 'meta' array in response")?;

        let columns: Vec<String> = meta
            .iter()
            .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
            .collect();

        let data = json["data"]
            .as_array()
            .ok_or("Expected 'data' array in response")?;

        let rows: Vec<Vec<Value>> = data
            .iter()
            .filter_map(|row| row.as_array())
            .map(|arr| arr.iter().map(json_to_value).collect())
            .collect();

        Ok(QueryResult { columns, rows })
    }

    fn schema_tree(&mut self, progress: &ProgressFn) -> Result<Vec<SchemaNode>, String> {
        progress("fetching tables…");
        let table_names = self.query_string_list(&format!(
            "SELECT name FROM system.tables \
             WHERE database = '{}' AND engine != 'View' \
             ORDER BY name",
            self.database
        ))?;
        progress("fetching views…");
        let view_names = self.query_string_list(&format!(
            "SELECT name FROM system.tables \
             WHERE database = '{}' AND engine = 'View' \
             ORDER BY name",
            self.database
        ))?;

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

        Ok(children)
    }
}

fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => Value::Text(s.clone()),
        other => Value::Text(other.to_string()),
    }
}
