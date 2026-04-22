# Getting Started

## What is lazydb?

lazydb is a terminal UI database client inspired by lazygit and lazydocker. It provides a vim-style query editor, connection profiles, and an intuitive sidebar for browsing database schemas — all from your terminal.

<!-- TODO: screenshot of the full UI -->

## Prerequisites

- A terminal emulator with true color support (e.g., iTerm2, kitty, alacritty, or any modern terminal with VT100 support)
- Cargo (to build from source) or the `lazydb` binary on your system

## Installation

### From source

```bash
cargo install --path .
```

### Build and run from source

```bash
git clone https://github.com/your-org/lazydb.git
cd lazydb
cargo run
```

### Using Docker

A `docker-compose.yaml` is provided for quickly spinning up development databases. See the [FAQ & Troubleshooting](faq.md) section for details.

## First Launch

When you start lazydb for the first time, you will see:

- **A blank connections sidebar** — no database profiles are configured yet.
- **A query editor** with a placeholder message.
- **A status bar** at the bottom showing the current pane.

## Quick Example: Connect DuckDB

DuckDB is the easiest way to try lazydb. It requires no server setup.

### 1. Create a connection profile

Create `~/.config/lazydb/profiles.toml` with:

```toml
[connections.mydb]
type = "duckdb"
path = "/tmp/example.duckdb"
```

### 2. Launch lazydb

```bash
lazydb
```

Press `Enter` on the `mydb (duckdb)` entry in the connections sidebar. If the file does not exist, DuckDB creates it automatically.

### 3. Execute a query

Tab to the query editor, type a SQL query (e.g., `SELECT 1`), then press `Ctrl+E` to run it.

<!-- TODO: screenshot of the results pane -->

That's it — you're connected. See [Configuration](configuration.md) for connecting to other database types.

## Navigation Overview

lazydb uses three main panels:

| Panel        | Purpose                            |
|--------------|------------------------------------|
| Sidebar      | Browse databases and file trees    |
| Query Editor | Write and edit SQL queries (vim)   |
| Results      | View query output (table format)   |

Use `Tab` and `Shift+Tab` to cycle focus between panels. Press `Ctrl+E` at any time to execute the query.

See [Panels](panels.md) for a full description of each panel.

See [Keybindings](keybindings.md) for the complete keybinding reference.
