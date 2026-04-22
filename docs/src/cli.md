# CLI Usage

In addition to the TUI, lazydb provides a command-line interface that can be used for scripting, CI pipelines, or quick one-off queries.

<!-- TODO: screenshot of CLI output -->

## Running the TUI

```bash
# Launch with default settings (no file root)
lazydb

# Launch with a file root directory (auto-opens the files panel)
lazydb /path/to/queries/
```

## List Connections

```bash
lazydb conns list
```

Lists all configured connections and their types.

## Test a Connection

```bash
lazydb conns test <name>
```

Tests that a connection profile is valid by attempting to connect. Outputs `OK` with the elapsed time on success, or `FAILED` with the error message on failure.

## Execute a Query

```bash
# Execute an inline query
lazydb query --conn <connection-name> --query "SELECT * FROM users LIMIT 10"

# Execute a query from a SQL file
lazydb query --conn <connection-name> --file /path/to/query.sql

# Limit the number of result rows (default: 1000)
lazydb query --conn mydb --query "SELECT 1" --limit 100

# Output in CSV format (default: Table)
lazydb query --conn mydb --query "SELECT 1" --format csv
```

### `lazydb query` options

| Option | Description |
|---|---|
| `--conn <name>` | Name of the connection from `profiles.toml` (required) |
| `--query <sql>` | SQL query to execute (provide `--query` or `--file`, not both) |
| `--file <path>` | Path to a SQL file to execute (provide `--query` or `--file`, not both) |
| `--limit <n>` | Maximum number of rows to return (0 for unlimited, default: 1000) |
| `--format <table\|csv>` | Output format (default: `table`) |

### Output formatting

The default `table` format renders a unicode-bordered table to stdout:

```
┌────┬──────────┐
│ id │ name     │
├────┼──────────┤
│ 1  │ Alice    │
│ 2  │ Bob      │
└────┴──────────┘
```

Execution statistics (row count and duration) are written to stderr.

The `csv` format outputs raw CSV to stdout (including a header row):

```
id,name
1,Alice
2,Bob
```

## Examples

### Check a connection

```bash
lazydb conns test mydb
# Output: Testing connection 'mydb' (duckdb)...
#          OK (2ms)
```

### Query and pipe to a file

```bash
lazydb query --conn prod --query "SELECT * FROM orders" --limit 0 --format csv > /tmp/orders.csv
```

### Format a query file

Open a file in the TUI (via the File Browser), then press `Ctrl+F` to format it.
