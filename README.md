# lazydb

A terminal UI database client for the keyboard-driven developer. Inspired by lazygit/lazydocker.

[![CI](https://github.com/niclasgrahm/lazydb/actions/workflows/ci.yml/badge.svg)](https://github.com/niclasgrahm/lazydb/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-2024-orange)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)

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

Install the latest release on Linux or macOS without a Rust toolchain:

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://propell.dev/lazydb/install.sh | sh
```

The installer downloads the matching GitHub Release archive and verifies it
against that release's `SHA256SUMS` file before installing to `~/.local/bin`.

Prebuilt binaries are available from the [latest GitHub release](https://github.com/niclasgrahm/lazydb/releases/latest)
for Linux, macOS (Apple Silicon and Intel), and Windows. Verify a downloaded
archive against the `SHA256SUMS` file included with the release before use.

To build from source:

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
[connections.local]
type = "duckdb"
path = "/path/to/database.db"

[connections.prod]
type = "postgres"
host = "postgres.internal"
port = 5432
user = "postgres"
database = "mydb"

[connections.prod.ssh_tunnel]
host = "bastion.example.com"
user = "deploy"
identity_file = "~/.ssh/id_ed25519"
```

PostgreSQL and ClickHouse profiles can connect through an SSH tunnel. lazydb
uses the system `ssh` client, including its normal SSH-agent, default key, and
host-key verification behavior. `ssh_tunnel.port` defaults to `22`; `identity_file`
and `known_hosts` are optional. `remote_host` and `remote_port` optionally override
the database host and port reached from the bastion. The local forwarding port is
allocated automatically.

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

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](./CONTRIBUTING.md) for the
workflow and local checks. `main` is protected; changes land via pull request
with CI green.

## License

[MIT](./LICENSE)
