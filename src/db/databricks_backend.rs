use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use serde_json::{self, json};
use tracing::debug;

use super::{Database, ProgressFn, QueryResult, SchemaNode, Value};

pub struct Databricks {
    base_url: String,
    token: String,
    warehouse_id: String,
    catalog: Option<String>,
    schema: Option<String>,
}

fn databricks_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build(),
    )
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
        // Databricks data_array returns most values as strings
        serde_json::Value::String(s) => {
            if let Ok(i) = s.parse::<i64>() {
                Value::Int(i)
            } else if let Ok(f) = s.parse::<f64>() {
                Value::Float(f)
            } else if s == "true" || s == "false" {
                Value::Bool(s == "true")
            } else {
                Value::Text(s.clone())
            }
        }
        other => Value::Text(other.to_string()),
    }
}

impl Databricks {
    pub fn connect(
        host: &str,
        token: &str,
        warehouse_id: &str,
        catalog: Option<&str>,
        schema: Option<&str>,
    ) -> Result<Self, String> {
        let db = Self {
            base_url: format!("https://{}", host.trim_end_matches('/')),
            token: token.to_string(),
            warehouse_id: warehouse_id.to_string(),
            catalog: catalog.map(|s| s.to_string()),
            schema: schema.map(|s| s.to_string()),
        };
        // Verify connectivity
        db.execute_statement("SELECT 1")?;
        Ok(db)
    }

    fn execute_statement(&self, sql: &str) -> Result<serde_json::Value, String> {
        let url = format!("{}/api/2.0/sql/statements", self.base_url);

        let mut body = json!({
            "warehouse_id": self.warehouse_id,
            "statement": sql,
            "wait_timeout": "50s",
            "on_wait_timeout": "CONTINUE",
        });

        if let Some(cat) = &self.catalog {
            body["catalog"] = json!(cat);
        }
        if let Some(sch) = &self.schema {
            body["schema"] = json!(sch);
        }

        let agent = databricks_agent();
        let auth = format!("Bearer {}", self.token);

        let mut response = agent
            .post(&url)
            .header("Authorization", &auth)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send(body.to_string().as_bytes())
            .map_err(|e| format!("Databricks request failed: {e}"))?;

        let status_code = response.status().as_u16();
        let resp_body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read Databricks response: {e}"))?;

        let json: serde_json::Value = serde_json::from_str(&resp_body)
            .map_err(|e| format!("JSON parse error: {e}"))?;

        if status_code >= 400 {
            let msg = json["message"].as_str().unwrap_or(&resp_body);
            return Err(format!("Databricks error: {msg}"));
        }

        let state = json["status"]["state"].as_str().unwrap_or("UNKNOWN");

        match state {
            "SUCCEEDED" => Ok(json),
            "FAILED" => {
                let msg = json["status"]["error"]["message"]
                    .as_str()
                    .unwrap_or("Unknown execution error");
                Err(format!("Databricks query failed: {msg}"))
            }
            "CANCELED" | "CLOSED" => {
                Err(format!("Databricks query was {}", state.to_lowercase()))
            }
            "PENDING" | "RUNNING" => {
                let statement_id = json["statement_id"]
                    .as_str()
                    .ok_or("No statement_id in async response")?;
                self.poll_statement(statement_id)
            }
            other => Err(format!("Unexpected Databricks state: {other}")),
        }
    }

    fn poll_statement(&self, statement_id: &str) -> Result<serde_json::Value, String> {
        let url = format!(
            "{}/api/2.0/sql/statements/{}",
            self.base_url, statement_id
        );
        let agent = databricks_agent();
        let auth = format!("Bearer {}", self.token);

        let backoffs = [500, 1000, 2000, 4000, 8000, 10000, 10000, 10000];
        for (attempt, wait_ms) in backoffs.iter().enumerate() {
            debug!(attempt = attempt + 1, wait_ms, "databricks: polling for results");
            thread::sleep(Duration::from_millis(*wait_ms));

            let mut response = agent
                .get(&url)
                .header("Authorization", &auth)
                .header("Accept", "application/json")
                .call()
                .map_err(|e| format!("Databricks poll failed: {e}"))?;

            let body = response
                .body_mut()
                .read_to_string()
                .map_err(|e| format!("Failed to read poll response: {e}"))?;

            let json: serde_json::Value =
                serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;

            let state = json["status"]["state"].as_str().unwrap_or("UNKNOWN");
            match state {
                "SUCCEEDED" => return Ok(json),
                "PENDING" | "RUNNING" => continue,
                "FAILED" => {
                    let msg = json["status"]["error"]["message"]
                        .as_str()
                        .unwrap_or("Unknown error");
                    return Err(format!("Databricks query failed: {msg}"));
                }
                "CANCELED" | "CLOSED" => {
                    return Err(format!("Databricks query was {}", state.to_lowercase()));
                }
                other => return Err(format!("Unexpected Databricks state: {other}")),
            }
        }

        Err("Databricks query timed out waiting for results".to_string())
    }

    fn query_string_list(&self, sql: &str) -> Result<Vec<String>, String> {
        let json = self.execute_statement(sql)?;
        let rows = json["result"]["data_array"]
            .as_array()
            .ok_or("Expected 'result.data_array' in response")?;
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

    fn raw_query_rows(&self, sql: &str) -> Result<Vec<Vec<String>>, String> {
        let json = self.execute_statement(sql)?;
        let rows = json["result"]["data_array"]
            .as_array()
            .ok_or("Expected 'result.data_array' in response")?;
        let mut result = Vec::new();
        for row in rows {
            if let Some(arr) = row.as_array() {
                let string_row: Vec<String> = arr
                    .iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect();
                result.push(string_row);
            }
        }
        Ok(result)
    }
}

impl Database for Databricks {
    fn execute_query(&mut self, sql: &str) -> Result<QueryResult, String> {
        let json = self.execute_statement(sql)?;

        let columns: Vec<String> = json["manifest"]["schema"]["columns"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|col| col["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let rows: Vec<Vec<Value>> = json["result"]["data_array"]
            .as_array()
            .map(|data| {
                data.iter()
                    .filter_map(|row| row.as_array())
                    .map(|arr| arr.iter().map(json_to_value).collect())
                    .collect()
            })
            .unwrap_or_default();

        Ok(QueryResult { columns, rows })
    }

    fn schema_tree(&mut self, progress: &ProgressFn) -> Result<Vec<SchemaNode>, String> {
        progress("listing catalogs…");
        let catalogs = if let Some(cat) = &self.catalog {
            vec![cat.clone()]
        } else {
            self.query_string_list("SHOW CATALOGS")?
        };

        let total = catalogs.len();
        let mut catalog_nodes = Vec::new();
        for (i, catalog) in catalogs.iter().enumerate() {
            progress(&format!("fetching catalog ({}/{total}): {catalog}", i + 1));
            let schemas = self.query_string_list(&format!(
                "SELECT schema_name FROM {catalog}.information_schema.schemata \
                 WHERE schema_name != 'information_schema' \
                 ORDER BY schema_name"
            ))?;

            if schemas.is_empty() {
                continue;
            }

            let schema_list = schemas
                .iter()
                .map(|s| format!("'{s}'"))
                .collect::<Vec<_>>()
                .join(",");

            let tables_rows = self.raw_query_rows(&format!(
                "SELECT table_schema, table_name, table_type \
                 FROM {catalog}.information_schema.tables \
                 WHERE table_schema IN ({schema_list}) \
                 ORDER BY table_schema, table_type, table_name"
            ))?;

            let columns_rows = self.raw_query_rows(&format!(
                "SELECT table_schema, table_name, column_name, data_type \
                 FROM {catalog}.information_schema.columns \
                 WHERE table_schema IN ({schema_list}) \
                 ORDER BY table_schema, table_name, ordinal_position"
            ))?;

            // Build column_map: (schema, table) -> Vec<SchemaNode>
            let mut column_map: HashMap<(String, String), Vec<SchemaNode>> = HashMap::new();
            for row in &columns_rows {
                if row.len() >= 4 {
                    let key = (row[0].clone(), row[1].clone());
                    column_map
                        .entry(key)
                        .or_default()
                        .push(SchemaNode::leaf(format!("{} ({})", row[2], row[3])));
                }
            }

            // Build table_map: schema -> (tables, views)
            let mut table_map: HashMap<String, (Vec<SchemaNode>, Vec<SchemaNode>)> = HashMap::new();
            for row in &tables_rows {
                if row.len() >= 3 {
                    let cols = column_map
                        .remove(&(row[0].clone(), row[1].clone()))
                        .unwrap_or_default();
                    let node = SchemaNode::group(row[1].clone(), cols);
                    let entry = table_map.entry(row[0].clone()).or_default();
                    // Databricks Unity Catalog uses TABLE/MANAGED/EXTERNAL for tables, VIEW for views
                    match row[2].as_str() {
                        "VIEW" => entry.1.push(node),
                        _ => entry.0.push(node),
                    }
                }
            }

            let mut schema_nodes = Vec::new();
            for schema_name in &schemas {
                let (tables, views) = table_map.remove(schema_name).unwrap_or_default();
                let mut children = Vec::new();
                if !tables.is_empty() {
                    children.push(SchemaNode::group("Tables", tables));
                }
                if !views.is_empty() {
                    children.push(SchemaNode::group("Views", views));
                }
                if !children.is_empty() {
                    schema_nodes.push(SchemaNode::group(schema_name.clone(), children));
                }
            }

            if !schema_nodes.is_empty() {
                catalog_nodes.push(SchemaNode::group(catalog.clone(), schema_nodes));
            }
        }

        Ok(catalog_nodes)
    }
}
