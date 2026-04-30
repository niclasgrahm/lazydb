# lazydb

A terminal UI database client for the keyboard-driven developer. Inspired by lazygit/lazydocker.

![Rust](https://img.shields.io/badge/rust-2024-orange)

## Features

- Vim-style query editor with Normal/Insert/Visual/Operator modes
- Sidebar tree for browsing schemas and tables
- SQL syntax highlighting
- [PRQL](https://prql-lang.org/) support
- Recent queries and file browsing

## Supported databases

- DuckDB
- PostgreSQL
- ClickHouse
- Snowflake
- Databricks

## Installation

```bash
cargo install --path .
```

Requires a recent stable Rust toolchain (edition 2024).

## Configuration

Config files live in `~/.config/lazydb/`:

- `config.toml` — app settings (sidebar width, keybindings, etc.)
- `profiles.toml` — database connection profiles

Example `profiles.toml`:

```toml
[[profiles]]
name = "local"
type = "duckdb"
path = "/path/to/database.db"

[[profiles]]
name = "prod"
type = "postgres"
host = "localhost"
port = 5432
user = "postgres"
dbname = "mydb"
```

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle focus between panes |
| `Ctrl+E` | Execute query |
| `q` / `Esc` | Quit (from sidebar or results) |
| `j/k` | Navigate sidebar |
| `Enter` | Connect / expand node |
| `i/a/o` | Enter insert mode in editor |
| `v` | Visual mode |
| `Esc` | Return to normal mode |

## Neovim plugin

A companion Neovim plugin is available in [`nvim-lazydb/`](./nvim-lazydb/).

## License

MIT
