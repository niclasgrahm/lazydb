# Configuration

lazydb reads two TOML files from `~/.config/lazydb/`: `config.toml` (application settings) and `profiles.toml` (database connections). If a file does not exist, lazydb uses sensible defaults.

## Application Config

**File**: `~/.config/lazydb/config.toml`

All fields are optional — every field has a sensible default and only needs to be set if you want to customize it.

```toml
# Width of the connections sidebar panel (10-50, default: 25)
sidebar_width = 25

# Enable debug logging (writes to ~/.config/lazydb/debug.log)
debug = false

# Maximum number of recent queries to keep (default: 10)
max_recents = 10

# Override default keybindings
[keybindings]
leader_key = "space"

[keybindings.global]
execute_query = "ctrl+e"
format_query = "ctrl+f"
next_pane = "tab"
prev_pane = "shift+tab"
show_help = "?"

[keybindings.sidebar]
navigate_up = ["k", "up"]
navigate_down = ["j", "down"]
expand = ["l", "right"]
collapse = ["h", "left"]
activate = "enter"
preview = "s"
quit = ["q", "esc"]

[keybindings.results]
scroll_up = ["k", "up"]
scroll_down = ["j", "down"]
scroll_left = ["h", "left"]
scroll_right = ["l", "right"]
next_page = ["n", "pagedown"]
prev_page = ["p", "pageup"]
close = ["c", "esc"]
quit = "q"
```

### Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `sidebar_width` | u16 | 25 | Width of the connections sidebar as a percentage of the total width |
| `debug` | bool | false | Enable debug logging to `~/.config/lazydb/debug.log` |
| `max_recents` | usize | 10 | Maximum number of recent queries to keep |
| `keybindings` | object | (see below) | Keybinding overrides |

### Keybinding Format

Keybindings accept a string for a single key, or an array of strings for alternative keys:

```toml
# Single key
execute_query = "ctrl+e"

# Multiple alternatives — any matching key fires the action
navigate_up = ["k", "up"]
navigate_down = ["j", "down"]

# Modifiers: ctrl, shift, alt (combined with +)
leader_key = "ctrl+l"

# Special keys: tab, enter, esc, up, down, left, right, space, backspace
quit = ["q", "esc"]
```

## Connection Profiles

**File**: `~/.config/lazydb/profiles.toml`

Each connection is defined under a `[connections.<name>]` section. The `name` is used as the display label in the sidebar. Connection type is specified via the `type` field.

### DuckDB

```toml
[connections.mydb]
type = "duckdb"
path = "/path/to/database.duckdb"
```

<!-- TODO: screenshot of DuckDB connection config -->

| Field | Required | Default | Description |
|---|---|---|---|
| `path` | yes | — | Path to the DuckDB file. Use `:memory:` for an in-memory database. |

### PostgreSQL

```toml
[connections.pg]
type = "postgres"
host = "localhost"
port = 5432
user = "postgres"
database = "mydb"
```

<!-- TODO: screenshot of PostgreSQL connection config -->

| Field | Required | Default | Description |
|---|---|---|-|
| `host`    | yes      | —          | PostgreSQL hostname                |
| `port`    | no       | 5432       | PostgreSQL port                    |
| `user`    | yes      | —          | Database username                  |
| `password`| no       | (none)     | Database password                  |
| `database`| yes      | —          | Database name                      |
| `schema`  | no       | `public`   | Default search schema              |

### ClickHouse

```toml
[connections.ch]
type = "clickhouse"
url = "http://localhost:8123"
user = "default"
database = "default"
```

<!-- TODO: screenshot of ClickHouse connection config -->

| Field      | Required | Default                  | Description           |
|---|---|---|-|
| `url`      | no       | `http://localhost:8123`  | ClickHouse HTTP URL     |
| `user`     | no       | `default`                | ClickHouse user         |
| `password` | no       | (none)                   | ClickHouse password     |
| `database` | no       | `default`                | Default database        |

### Snowflake

Snowflake supports three authentication methods: password, OAuth, and browser login.

#### Password authentication

```toml
[connections.sf]
type = "snowflake"
account = "xy12345"
auth = "password"
user = "user@example.com"
password = "your_password"
database = "PROD"
```

#### OAuth authentication

```toml
[connections.sf_oauth]
type = "snowflake"
account = "xy12345"
auth = "oauth"
oauth_token = "your_oauth_token"
database = "PROD"
```

#### Browser authentication (recommended)

```toml
[connections.sf_browser]
type = "snowflake"
account = "xy12345"
auth = "browser"
user = "user@example.com"
database = "PROD"
```

<!-- TODO: screenshot of Snowflake connection config -->

| Field       | Required | Default | Description               |
|---|---|---|-|
| `account`  | yes      | —       | Snowflake account identifier |
| `auth`     | yes      | —       | `password`, `oauth`, or `browser` |
| `user`     | yes      | —       | Snowflake username        |
| `password` | yes*     | —       | Auth type `password` only |
| `oauth_token` | yes*  | —       | Auth type `oauth` only    |
| `database` | yes      | —       | Snowflake database        |
| `warehouse`| no       | —       | Snowflake warehouse       |
| `schema`   | no       | —       | Database schema           |
| `role`     | no       | —       | Snowflake role            |

\* `user` and `database` are always required. One of `password` or `oauth_token` is required depending on auth method.

### Databricks

```toml
[connections.databricks]
type = "databricks"
host = "adb-1234567890123456.7.azuredatabricks.net"
token = "dapi0123456789abcdef"
warehouse_id = "abc123def456"
catalog = "main"
schema = "default"
```

<!-- TODO: screenshot of Databricks connection config -->

| Field        | Required | Default | Description                    |
|---|---|---|-|
| `host`         | yes      | —       | Databricks workspace host |
| `token`        | yes      | —       | Databricks personal access token |
| `warehouse_id` | yes      | —       | Warehouse ID             |
| `catalog`      | no       | —       | Databricks catalog       |
| `schema`       | no       | —       | Database schema          |

### Putting it all together

```toml
# ~/.config/lazydb/profiles.toml

[connections.local]
type = "duckdb"
path = "/tmp/dev.duckdb"

[connections.production_pg]
type = "postgres"
host = "db.example.com"
port = 5432
user = "app_user"
password = "s3cret"
database = "production"
schema = "public"

[connections.analytics]
type = "clickhouse"
url = "http://ch.example.com:8123"
user = "analyst"
database = "analytics"

[connections.sf]
type = "snowflake"
account = "xy12345"
auth = "browser"
user = "user@example.com"
database = "PROD"
warehouse = "ANALYTICS_WH"
schema = "PUBLIC"
```
