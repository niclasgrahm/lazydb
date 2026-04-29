use serde_json::{self, json};
use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

use super::{Database, ProgressFn, QueryResult, SchemaNode, Value};

enum AuthMethod {
    Session,
    OAuth,
}

pub struct Snowflake {
    account_url: String,
    token: String,
    auth_method: AuthMethod,
    database: String,
    warehouse: Option<String>,
    schema: String,
    role: Option<String>,
}

impl Snowflake {
    pub fn connect_password(
        account: &str,
        user: &str,
        password: &str,
        database: &str,
        warehouse: Option<&str>,
        schema: Option<&str>,
        role: Option<&str>,
    ) -> Result<Self, String> {
        info!(account, user, database, ?warehouse, ?schema, ?role, "snowflake: connecting with password auth");
        let start = Instant::now();
        let account_url = format!("https://{account}.snowflakecomputing.com");

        // Authenticate via login-request endpoint
        let request_id = uuid_v4();
        let mut login_url = format!(
            "{account_url}/session/v1/login-request?requestId={request_id}&databaseName={database}"
        );
        if let Some(s) = schema {
            login_url.push_str(&format!("&schemaName={s}"));
        } else {
            login_url.push_str("&schemaName=PUBLIC");
        }
        if let Some(wh) = warehouse {
            login_url.push_str(&format!("&warehouse={wh}"));
        }
        if let Some(r) = role {
            login_url.push_str(&format!("&roleName={r}"));
        }

        let login_body = json!({
            "data": {
                "LOGIN_NAME": user,
                "PASSWORD": password,
                "ACCOUNT_NAME": account,
                "CLIENT_APP_ID": "lazydb",
                "CLIENT_APP_VERSION": env!("CARGO_PKG_VERSION"),
            }
        });

        let agent = snowflake_agent();

        debug!("snowflake: sending login request to {login_url}");
        let req_start = Instant::now();
        let mut response = agent
            .post(&login_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send(login_body.to_string().as_bytes())
            .map_err(|e| {
                error!("snowflake: login request failed: {e}");
                format!("Snowflake login request failed: {e}")
            })?;
        debug!(elapsed_ms = req_start.elapsed().as_millis(), status = response.status().as_u16(), "snowflake: login response received");

        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read login response: {e}"))?;
        debug!(body_len = body.len(), "snowflake: login response body read");

        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("Login JSON parse error: {e}"))?;

        if !json["success"].as_bool().unwrap_or(false) {
            let msg = json["message"]
                .as_str()
                .unwrap_or("Unknown login error");
            error!("snowflake: login failed: {msg}");
            return Err(format!("Snowflake login failed: {msg}"));
        }

        let token = json["data"]["token"]
            .as_str()
            .ok_or("No token in login response")?
            .to_string();
        info!(elapsed_ms = start.elapsed().as_millis(), "snowflake: password auth successful");

        let sf = Self {
            account_url,
            token,
            auth_method: AuthMethod::Session,
            database: database.to_string(),
            warehouse: warehouse.map(|s| s.to_string()),
            schema: schema.unwrap_or("PUBLIC").to_string(),
            role: role.map(|s| s.to_string()),
        };

        // Verify connectivity
        debug!("snowflake: verifying connectivity with SELECT 1");
        sf.raw_query("SELECT 1")?;
        info!(total_elapsed_ms = start.elapsed().as_millis(), "snowflake: password connection fully established");
        Ok(sf)
    }

    pub fn connect_oauth(
        account: &str,
        oauth_token: &str,
        database: &str,
        warehouse: Option<&str>,
        schema: Option<&str>,
        role: Option<&str>,
    ) -> Result<Self, String> {
        info!(account, database, ?warehouse, ?schema, ?role, "snowflake: connecting with OAuth");
        let start = Instant::now();
        let sf = Self {
            account_url: format!("https://{account}.snowflakecomputing.com"),
            token: oauth_token.to_string(),
            auth_method: AuthMethod::OAuth,
            database: database.to_string(),
            warehouse: warehouse.map(|s| s.to_string()),
            schema: schema.unwrap_or("PUBLIC").to_string(),
            role: role.map(|s| s.to_string()),
        };

        // Verify connectivity
        debug!("snowflake: verifying OAuth connectivity with SELECT 1");
        sf.raw_query("SELECT 1")?;
        info!(elapsed_ms = start.elapsed().as_millis(), "snowflake: OAuth connection established");
        Ok(sf)
    }

    pub fn connect_browser(
        account: &str,
        user: &str,
        database: &str,
        warehouse: Option<&str>,
        schema: Option<&str>,
        role: Option<&str>,
    ) -> Result<Self, String> {
        info!(account, user, database, ?warehouse, ?schema, ?role, "snowflake: connecting with browser SSO");
        let start = Instant::now();
        let account_url = format!("https://{account}.snowflakecomputing.com");

        // Try cached token first
        if let Some(token) = get_cached_token(account, user, role) {
            debug!("snowflake: found cached SSO token, attempting to reuse");
            let sf = Self {
                account_url: account_url.clone(),
                token,
                auth_method: AuthMethod::Session,
                database: database.to_string(),
                warehouse: warehouse.map(|s| s.to_string()),
                schema: schema.unwrap_or("PUBLIC").to_string(),
                role: role.map(|s| s.to_string()),
            };

            if sf.raw_query("SELECT 1").is_ok() {
                info!("snowflake: reused cached SSO token successfully");
                return Ok(sf);
            } else {
                debug!("snowflake: cached token invalid or expired, initiating full SSO flow");
            }
        }

        // Bind a local listener on a random port for the SSO callback
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("Failed to bind local server for SSO: {e}"))?;
        let local_port = listener
            .local_addr()
            .map_err(|e| format!("Failed to get local port: {e}"))?
            .port();
        debug!(local_port, "snowflake: SSO callback listener bound");

        // Step 1: Request SSO URL from Snowflake
        let auth_url = format!("{account_url}/session/authenticator-request");
        let mut auth_body = json!({
            "data": {
                "LOGIN_NAME": user,
                "ACCOUNT_NAME": account,
                "AUTHENTICATOR": "externalbrowser",
                "BROWSER_MODE_REDIRECT_PORT": local_port.to_string(),
                "CLIENT_APP_ID": "lazydb",
                "CLIENT_APP_VERSION": env!("CARGO_PKG_VERSION"),
            }
        });

        if let Some(role) = role {
            auth_body["data"]["ROLE"] = json!(role);
        }
        if let Some(wh) = warehouse {
            auth_body["data"]["WAREHOUSE"] = json!(wh);
        }

        let agent = snowflake_agent();

        debug!("snowflake: requesting SSO URL from {auth_url}");
        let req_start = Instant::now();
        let mut response = agent
            .post(&auth_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send(auth_body.to_string().as_bytes())
            .map_err(|e| {
                error!("snowflake: authenticator request failed: {e}");
                format!("Snowflake authenticator request failed: {e}")
            })?;
        debug!(elapsed_ms = req_start.elapsed().as_millis(), status = response.status().as_u16(), "snowflake: authenticator response received");

        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read authenticator response: {e}"))?;

        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("Auth JSON parse error: {e}"))?;

        if !json["success"].as_bool().unwrap_or(false) {
            let msg = json["message"]
                .as_str()
                .unwrap_or("Unknown authenticator error");
            error!("snowflake: SSO init failed: {msg}");
            return Err(format!("Snowflake SSO init failed: {msg}"));
        }

        let sso_url = json["data"]["ssoUrl"]
            .as_str()
            .ok_or("No ssoUrl in authenticator response")?;
        let proof_key = json["data"]["proofKey"]
            .as_str()
            .ok_or("No proofKey in authenticator response")?
            .to_string();

        // Step 2: Open the browser to the SSO URL
        info!("snowflake: opening browser for SSO");
        open_browser(sso_url)?;

        // Step 3: Wait for the IdP callback on our local listener
        debug!("snowflake: waiting for SSO callback (120s timeout)");
        listener
            .set_nonblocking(false)
            .map_err(|e| format!("Failed to set listener blocking: {e}"))?;
        let saml_token = accept_sso_callback(&listener)?;
        debug!(elapsed_ms = start.elapsed().as_millis(), "snowflake: SSO callback received, got SAML token");

        // Step 4: Authenticate with the SAML token + proof key
        // Database/schema/warehouse/role go as URL query params per Snowflake protocol
        let request_id = uuid_v4();
        let mut login_url = format!(
            "{account_url}/session/v1/login-request?requestId={request_id}&databaseName={database}"
        );
        if let Some(s) = schema {
            login_url.push_str(&format!("&schemaName={s}"));
        } else {
            login_url.push_str("&schemaName=PUBLIC");
        }
        if let Some(wh) = warehouse {
            login_url.push_str(&format!("&warehouse={wh}"));
        }
        if let Some(r) = role {
            login_url.push_str(&format!("&roleName={r}"));
        }

        let login_body = json!({
            "data": {
                "LOGIN_NAME": user,
                "ACCOUNT_NAME": account,
                "AUTHENTICATOR": "externalbrowser",
                "TOKEN": saml_token,
                "PROOF_KEY": proof_key,
                "CLIENT_APP_ID": "lazydb",
                "CLIENT_APP_VERSION": env!("CARGO_PKG_VERSION"),
            }
        });

        debug!("snowflake: sending SSO login request");
        let req_start = Instant::now();
        let mut response = agent
            .post(&login_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send(login_body.to_string().as_bytes())
            .map_err(|e| {
                error!("snowflake: SSO login request failed: {e}");
                format!("Snowflake login request failed: {e}")
            })?;
        debug!(elapsed_ms = req_start.elapsed().as_millis(), status = response.status().as_u16(), "snowflake: SSO login response received");

        let status = response.status();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read login response: {e}"))?;

        if !status.is_success() && body.trim().is_empty() {
            error!(status = status.as_u16(), "snowflake: SSO login failed with empty response");
            return Err(format!("Snowflake SSO login failed with HTTP {}", status.as_u16()));
        }

        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("Login JSON parse error: {e}. Body: {body}"))?;

        if !json["success"].as_bool().unwrap_or(false) {
            let msg = json["message"]
                .as_str()
                .unwrap_or("Unknown login error");
            error!("snowflake: SSO login failed: {msg}");
            return Err(format!("Snowflake SSO login failed: {msg}"));
        }

        let token = json["data"]["token"]
            .as_str()
            .ok_or("No token in SSO login response")?
            .to_string();

        let sf = Self {
            account_url,
            token,
            auth_method: AuthMethod::Session,
            database: database.to_string(),
            warehouse: warehouse.map(|s| s.to_string()),
            schema: schema.unwrap_or("PUBLIC").to_string(),
            role: role.map(|s| s.to_string()),
        };

        debug!("snowflake: verifying SSO connectivity with SELECT 1");
        sf.raw_query("SELECT 1")?;
        info!(total_elapsed_ms = start.elapsed().as_millis(), "snowflake: browser SSO connection fully established");
        
        // Cache the token for future use
        save_cached_token(account, user, role, &sf.token);
        
        Ok(sf)
    }

    fn raw_query(&self, sql: &str) -> Result<serde_json::Value, String> {
        let truncated_sql = if sql.len() > 200 { &sql[..200] } else { sql };
        debug!(sql = truncated_sql, "snowflake: executing raw query");
        let start = Instant::now();
        let result = match self.auth_method {
            AuthMethod::Session => self.raw_query_v1(sql),
            AuthMethod::OAuth => self.raw_query_v2(sql),
        };
        match &result {
            Ok(_) => debug!(elapsed_ms = start.elapsed().as_millis(), "snowflake: raw query succeeded"),
            Err(e) => error!(elapsed_ms = start.elapsed().as_millis(), error = %e, "snowflake: raw query failed"),
        }
        result
    }

    /// Session-token auth: use the v1 query endpoint used by all official connectors.
    fn raw_query_v1(&self, sql: &str) -> Result<serde_json::Value, String> {
        let request_id = uuid_v4();
        let url = format!(
            "{}/queries/v1/query-request?requestId={}",
            self.account_url, request_id
        );

        let body = json!({
            "sqlText": sql,
            "asyncExec": false,
            "sequenceId": 1,
        });

        let agent = snowflake_agent();

        debug!(request_id, "snowflake v1: sending query request");
        let req_start = Instant::now();
        let auth = format!("Snowflake Token=\"{}\"", self.token);
        let mut response = agent
            .post(&url)
            .header("Authorization", &auth)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send(body.to_string().as_bytes())
            .map_err(|e| format!("Snowflake request failed: {e}"))?;
        debug!(elapsed_ms = req_start.elapsed().as_millis(), status = response.status().as_u16(), "snowflake v1: response received");

        let resp_body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read Snowflake response: {e}"))?;
        debug!(body_len = resp_body.len(), "snowflake v1: response body read");

        if resp_body.is_empty() {
            error!(status = response.status().as_u16(), "snowflake v1: empty response body");
            return Err(format!(
                "Snowflake returned empty response (HTTP {})",
                response.status().as_u16()
            ));
        }

        let json: serde_json::Value = serde_json::from_str(&resp_body)
            .map_err(|e| format!("JSON parse error: {e} — raw response: {}", &resp_body[..resp_body.len().min(500)]))?;

        if !json["success"].as_bool().unwrap_or(false) {
            let msg = json["message"]
                .as_str()
                .unwrap_or(&resp_body);
            error!(msg, "snowflake v1: query returned error");
            return Err(format!("Snowflake error: {msg}"));
        }

        // Normalize v1 response to match v2 shape for uniform handling:
        // v1: { data: { rowtype: [...], rowset: [[...], ...] } }
        // v2: { resultSetMetaData: { rowType: [...] }, data: [[...], ...] }
        let normalized = json!({
            "resultSetMetaData": {
                "rowType": json["data"]["rowtype"]
            },
            "data": json["data"]["rowset"]
        });

        Ok(normalized)
    }

    /// OAuth auth: use the SQL API v2.
    fn raw_query_v2(&self, sql: &str) -> Result<serde_json::Value, String> {
        let url = format!("{}/api/v2/statements", self.account_url);

        let mut body = json!({
            "statement": sql,
            "database": self.database,
            "schema": self.schema,
        });

        if let Some(wh) = &self.warehouse {
            body["warehouse"] = json!(wh);
        }
        if let Some(role) = &self.role {
            body["role"] = json!(role);
        }

        let agent = snowflake_agent();

        debug!("snowflake v2: sending query request");
        let req_start = Instant::now();
        let auth = format!("Bearer {}", self.token);
        let mut response = agent
            .post(&url)
            .header("Authorization", &auth)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("X-Snowflake-Authorization-Token-Type", "OAUTH")
            .send(body.to_string().as_bytes())
            .map_err(|e| format!("Snowflake request failed: {e}"))?;
        debug!(elapsed_ms = req_start.elapsed().as_millis(), status = response.status().as_u16(), "snowflake v2: response received");

        let resp_body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read Snowflake response: {e}"))?;
        debug!(body_len = resp_body.len(), "snowflake v2: response body read");

        let json: serde_json::Value = serde_json::from_str(&resp_body)
            .map_err(|e| format!("JSON parse error: {e}"))?;

        let status_code = response.status().as_u16();
        if status_code == 202 {
            let statement_handle = json["statementHandle"]
                .as_str()
                .ok_or("No statementHandle in async response")?;
            info!(statement_handle, "snowflake v2: query running async, polling for results");
            return self.poll_for_results(statement_handle);
        }

        if status_code >= 400 {
            let msg = json["message"]
                .as_str()
                .unwrap_or(&resp_body);
            error!(status_code, msg, "snowflake v2: query returned error");
            return Err(format!("Snowflake error: {msg}"));
        }

        Ok(json)
    }

    fn poll_for_results(&self, statement_handle: &str) -> Result<serde_json::Value, String> {
        let url = format!(
            "{}/api/v2/statements/{}",
            self.account_url, statement_handle
        );

        let agent = snowflake_agent();
        let poll_start = Instant::now();

        let auth = format!("Bearer {}", self.token);
        let backoffs = [500, 1000, 2000, 4000, 8000, 10000, 10000, 10000];
        for (attempt, wait_ms) in backoffs.iter().enumerate() {
            debug!(attempt = attempt + 1, wait_ms, "snowflake: polling for async results");
            thread::sleep(Duration::from_millis(*wait_ms));

            let mut response = agent
                .get(&url)
                .header("Authorization", &auth)
                .header("Accept", "application/json")
                .call()
                .map_err(|e| format!("Snowflake poll failed: {e}"))?;

            let body = response
                .body_mut()
                .read_to_string()
                .map_err(|e| format!("Failed to read poll response: {e}"))?;

            let json: serde_json::Value =
                serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;

            let status = response.status().as_u16();
            if status == 202 {
                debug!(elapsed_ms = poll_start.elapsed().as_millis(), "snowflake: still waiting (202)");
                continue;
            }
            if status >= 400 {
                let msg = json["message"].as_str().unwrap_or(&body);
                error!(status, elapsed_ms = poll_start.elapsed().as_millis(), msg, "snowflake: poll returned error");
                return Err(format!("Snowflake error: {msg}"));
            }
            info!(elapsed_ms = poll_start.elapsed().as_millis(), attempts = attempt + 1, "snowflake: async query completed");
            return Ok(json);
        }

        error!(elapsed_ms = poll_start.elapsed().as_millis(), "snowflake: query timed out after all poll attempts");
        Err("Snowflake query timed out waiting for results".to_string())
    }

    fn query_string_list(&self, sql: &str) -> Result<Vec<String>, String> {
        self.query_string_column(sql, 0)
    }

    fn query_string_column(&self, sql: &str, col_idx: usize) -> Result<Vec<String>, String> {
        debug!(sql, col_idx, "snowflake: query_string_column");
        let json = self.raw_query(sql)?;
        let rows = json["data"]
            .as_array()
            .ok_or("Expected 'data' array in response")?;

        let mut result = Vec::new();
        for row in rows {
            if let Some(arr) = row.as_array() {
                if let Some(val) = arr.get(col_idx).and_then(|v| v.as_str()) {
                    result.push(val.to_string());
                }
            }
        }
        Ok(result)
    }

    fn list_accessible_databases(&self) -> Result<Vec<String>, String> {
        debug!("snowflake: listing accessible databases via SHOW DATABASES");
        // SHOW DATABASES result: created_on(0), name(1), is_default(2), ...
        self.query_string_column("SHOW DATABASES", 1)
    }

    fn introspect_database(&self, db: &str) -> Result<Option<SchemaNode>, String> {
        debug!(database = db, "snowflake: introspecting database");

        let schemas = self.query_string_list(&format!(
            "SELECT SCHEMA_NAME FROM {db}.INFORMATION_SCHEMA.SCHEMATA \
             WHERE CATALOG_NAME = '{db}' \
             AND SCHEMA_NAME != 'INFORMATION_SCHEMA' \
             ORDER BY SCHEMA_NAME"
        ))?;

        if schemas.is_empty() {
            return Ok(None);
        }

        let schema_list = schemas
            .iter()
            .map(|s| format!("'{s}'"))
            .collect::<Vec<_>>()
            .join(",");

        let tables_rows = self.raw_query_rows(&format!(
            "SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE \
             FROM {db}.INFORMATION_SCHEMA.TABLES \
             WHERE TABLE_SCHEMA IN ({schema_list}) \
             AND TABLE_TYPE IN ('BASE TABLE', 'VIEW') \
             ORDER BY TABLE_SCHEMA, TABLE_TYPE, TABLE_NAME"
        ))?;

        let columns_rows = self.raw_query_rows(&format!(
            "SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, DATA_TYPE \
             FROM {db}.INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA IN ({schema_list}) \
             ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION"
        ))?;

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

        let mut table_map: HashMap<String, (Vec<SchemaNode>, Vec<SchemaNode>)> = HashMap::new();
        for row in &tables_rows {
            if row.len() >= 3 {
                let schema = &row[0];
                let table_name = &row[1];
                let table_type = &row[2];
                let cols = column_map
                    .remove(&(schema.clone(), table_name.clone()))
                    .unwrap_or_default();
                let node = SchemaNode::group(table_name.clone(), cols);
                let entry = table_map.entry(schema.clone()).or_default();
                if table_type == "BASE TABLE" {
                    entry.0.push(node);
                } else {
                    entry.1.push(node);
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

        if schema_nodes.is_empty() {
            return Ok(None);
        }

        Ok(Some(SchemaNode::group(db.to_string(), schema_nodes)))
    }

    fn raw_query_rows(&self, sql: &str) -> Result<Vec<Vec<String>>, String> {
        debug!(sql, "snowflake: raw_query_rows");
        let json = self.raw_query(sql)?;
        let rows = json["data"]
            .as_array()
            .ok_or("Expected 'data' array in response")?;

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

impl Database for Snowflake {
    fn execute_query(&mut self, sql: &str) -> Result<QueryResult, String> {
        let truncated = if sql.len() > 200 { &sql[..200] } else { sql };
        info!(sql = truncated, "snowflake: execute_query");
        let start = Instant::now();
        let json = self.raw_query(sql)?;

        let columns: Vec<String> = json["resultSetMetaData"]["rowType"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|col| col["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let rows: Vec<Vec<Value>> = json["data"]
            .as_array()
            .map(|data| {
                data.iter()
                    .filter_map(|row| row.as_array())
                    .map(|arr| arr.iter().map(json_to_value).collect())
                    .collect()
            })
            .unwrap_or_default();

        info!(elapsed_ms = start.elapsed().as_millis(), columns = columns.len(), rows = rows.len(), "snowflake: execute_query complete");
        Ok(QueryResult { columns, rows })
    }

    fn schema_tree(&mut self, progress: &ProgressFn) -> Result<Vec<SchemaNode>, String> {
        info!("snowflake: fetching schema tree for all accessible databases");
        let start = Instant::now();

        progress("listing accessible databases…");
        let databases = self.list_accessible_databases()?;
        debug!(db_count = databases.len(), ?databases, "snowflake: found accessible databases");

        let total = databases.len();
        let mut db_nodes = Vec::new();
        for (i, db) in databases.iter().enumerate() {
            progress(&format!("fetching schema ({}/{total}): {db}", i + 1));
            match self.introspect_database(db) {
                Ok(Some(node)) => db_nodes.push(node),
                Ok(None) => debug!(database = db, "snowflake: skipping empty database"),
                Err(e) => debug!(database = db, error = %e, "snowflake: failed to introspect database, skipping"),
            }
        }

        info!(total_databases = db_nodes.len(), elapsed_ms = start.elapsed().as_millis(), "snowflake: schema tree complete");
        Ok(db_nodes)
    }
}

fn snowflake_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build(),
    )
}

fn open_browser(url: &str) -> Result<(), String> {
    let result = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", url]).status()
    } else {
        std::process::Command::new("xdg-open").arg(url).status()
    };

    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("Browser exited with status: {status}")),
        Err(e) => Err(format!("Failed to open browser: {e}")),
    }
}

fn accept_sso_callback(listener: &TcpListener) -> Result<String, String> {
    use std::time::Instant;

    // Poll with timeout instead of blocking forever
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Listener error: {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(120);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(conn) => break conn,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err("SSO callback timed out (120s) — browser may not have redirected back".to_string());
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("Failed to accept SSO callback: {e}")),
        }
    };

    // Set a read timeout on the accepted stream
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));

    let mut buf = vec![0u8; 8192];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("Failed to read SSO callback: {e}"))?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Extract the token from the GET request query string
    // The IdP redirects to: GET /?token=<SAML_TOKEN> HTTP/1.1
    let token = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1)) // URI part
        .and_then(|uri| uri.split('?').nth(1)) // query string
        .and_then(|qs| {
            qs.split('&').find_map(|param| {
                let (key, val) = param.split_once('=')?;
                if key == "token" { Some(val.to_string()) } else { None }
            })
        })
        .ok_or_else(|| format!("No token found in SSO callback: {}", request.lines().next().unwrap_or("")))?;

    // Send a success response to the browser
    let html = "<html><body><h3>Authentication successful.</h3>\
        <p>You can close this window and return to lazydb.</p></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.shutdown(std::net::Shutdown::Both);

    Ok(token)
}

/// Generate a random UUID v4 string without external crate.
fn uuid_v4() -> String {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // Use nanos + thread id for basic uniqueness; not cryptographic but sufficient for requestId
    let hash = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let bytes = hash.to_le_bytes();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u16::from_le_bytes([bytes[4], bytes[5]]),
        u16::from_le_bytes([bytes[6], bytes[7]]) & 0x0FFF,
        (u16::from_le_bytes([bytes[8], bytes[9]]) & 0x3FFF) | 0x8000,
        u64::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15], 0, 0]),
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
        // Snowflake REST API returns most values as strings
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

fn cache_file_path() -> std::path::PathBuf {
    crate::config::config_dir().join("sf_tokens.json")
}

fn get_cached_token(account: &str, user: &str, role: Option<&str>) -> Option<String> {
    let path = cache_file_path();
    let contents = std::fs::read_to_string(&path).ok()?;
    let cache: std::collections::HashMap<String, String> = serde_json::from_str(&contents).ok()?;
    let key = format!("{}:{}:{}", account, user, role.unwrap_or(""));
    cache.get(&key).cloned()
}

fn save_cached_token(account: &str, user: &str, role: Option<&str>, token: &str) {
    let path = cache_file_path();
    let mut cache: std::collections::HashMap<String, String> = if let Ok(contents) = std::fs::read_to_string(&path) {
        serde_json::from_str(&contents).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };
    
    let key = format!("{}:{}:{}", account, user, role.unwrap_or(""));
    cache.insert(key, token.to_string());
    
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = std::fs::write(path, json);
    }
}
