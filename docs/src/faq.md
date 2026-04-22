# FAQ & Troubleshooting

## Where are the configuration files?

Both files live in `~/.config/lazydb/`:

- `config.toml` — application settings (sidebar width, keybindings, debug)
- `profiles.toml` — database connection profiles
- `recents.json` — recent query history (auto-managed by lazydb)

If a file does not exist, lazydb uses defaults. You need to create `profiles.toml` at a minimum.

## How do I enable debug logging?

Set `debug = true` in `~/.config/lazydb/config.toml`:

```toml
debug = true
```

Logs are written to `~/.config/lazydb/debug.log`.

## "No connections configured" — how do I fix it?

Create `~/.config/lazydb/profiles.toml` with at least one connection. See [Configuration](configuration.md) for the full format. A minimal example:

```toml
[connections.mydb]
type = "duckdb"
path = "/tmp/example.duckdb"
```

## Connection fails — what to check?

1. **Profile name**: The name after `--conn` or in the sidebar must exactly match the key in `profiles.toml`.
2. **Network**: For remote databases (Postgres, ClickHouse, Snowflake, Databricks), check that the host/port is reachable.
3. **Credentials**: Verify username, password, and database name. Passwords can be omitted from the config for a prompt-less setup by using environment variables or a secrets manager.
4. **Test via CLI**: Run `lazydb conns test <name>` to get a clear error message:
   ```bash
   lazydb conns test my_pg
   # Output: Testing connection 'my_pg' (postgres)...
   #          FAILED: connection refused: tcp connect: Connection refused
   ```

## I can't type in the query editor

You're likely in Normal mode. Press `i` to enter Insert mode. Check the status bar at the bottom of the screen — it shows the current mode (`NORMAL`, `INSERT`, etc.).

## My query returns no results

This could mean one of several things:

- The table is empty
- A `WHERE` clause filtered everything out
- The schema path is wrong (e.g., Postgres defaults to `public` schema but your table is in a custom one — see the `schema` field in the Postgres connection config)

## Can I use multiple Snowflake auth methods?

Yes — define each auth method as a separate connection:

```toml
[connections.sf]
type = "snowflake"
account = "xy12345"
auth = "password"
user = "user@example.com"
password = "s3cret"
database = "PROD"

[connections.sf_oauth]
type = "snowflake"
account = "xy12345"
auth = "oauth"
oauth_token = "your_token"
database = "PROD"

[connections.sf_browser]
type = "snowflake"
account = "xy12345"
auth = "browser"
user = "user@example.com"
database = "PROD"
```

## How do I use docker-compose to start a dev environment?

A `docker-compose.yaml` is included in the repository. It starts a Postgres and a ClickHouse database pre-configured for lazydb:

```bash
# Start dev databases
docker compose up

# Now create ~/.config/lazydb/profiles.toml with:
[connections.pg]
type = "postgres"
host = "localhost"
port = 5432
user = "lazydb"
password = "lazydb"
database = "lazydb"

[connections.ch]
type = "clickhouse"
url = "http://localhost:8123"
user = "lazydb"
password = "lazydb"
database = "lazydb"
```

Stop with `docker compose down`. Data is persisted in Docker named volumes (`pgdata` and `chdata`).

## Where is the schema data for ClickHouse?

The docker-compose ClickHouse image includes sample tables (`products`, `orders`, `order_summary`) automatically via `docker/clickhouse/init.sql`.

The ClickHouse connection type uses a flat schema tree (no schema level — just tables and views directly under the connection root).

## Can I customize the keybindings?

Yes. See [configuration.md](configuration.md) under "Application Config" for the full keybinding format. Any keybinding can be overridden by adding the corresponding path to your `config.toml`.

## How do I customize the sidebar width?

Add to `~/.config/lazydb/config.toml`:

```toml
sidebar_width = 30
```

Value is a percentage (10-50). Default is 25.

## The status bar shows "leader leader key help" — what does that mean?

The status bar always shows `leader key  leader key  ? help` as a reminder:

- Press `leader key` (default: Space) for context-sensitive actions
- Press `?` for the help overlay

This is visible regardless of which panel is focused.
