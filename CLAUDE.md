# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is lazydb

A terminal UI database client built with Rust, inspired by lazygit/lazydocker. Uses ratatui for the TUI, with vim-style keybindings in the query editor. Currently supports DuckDB connection profiles (database connectivity not yet implemented — `db/mod.rs` is a placeholder).

## Commands

```bash
cargo build              # Build
cargo run                # Run the TUI app
cargo test               # Run all tests (~100 tests across all modules)
cargo test app            # Run app state/event handling tests
cargo test config         # Run config deserialization tests
cargo test keybindings    # Run keybinding parsing tests
cargo test highlight      # Run SQL highlighting tests
cargo test tree           # Run tree data structure tests
cargo test vim            # Run vim state machine tests
cargo test db::tests      # Run db value/mock tests
```

Rust edition 2024 — requires nightly or recent stable toolchain.

## Architecture

The app follows a standard TUI pattern: **event loop → state update → render**.

- **`main.rs`** — Loads config/profiles, initializes terminal, runs the event loop
- **`app.rs`** — Central `App` struct holding all state (sidebar tree, editor, vim mode, focus, results). Handles all keyboard events via `handle_event()`. Focus cycles between three panes: `Sidebar`, `QueryEditor`, `Results`
- **`vim.rs`** — Standalone vim emulation state machine wrapping `tui-textarea`. Supports Normal/Insert/Visual/Operator modes with standard motions (hjkl, w/e/b, gg/G) and operators (d/y/c with pending operator pattern)
- **`tree.rs`** — Generic tree data structure for the sidebar. `TreeNode` stores the hierarchy; `FlatNode` is the flattened render representation. Tree operations use flat-index addressing via `walk_mut()`
- **`config/mod.rs`** — Two TOML config files from `~/.config/lazydb/`: `config.toml` (app settings like sidebar width) and `profiles.toml` (database connections, tagged enum by type)
- **`db/mod.rs`** — Database trait (`Database`), value types, and backend implementations (DuckDB, Postgres, ClickHouse, Snowflake). Includes `MockDatabase` for testing (behind `#[cfg(test)]`)
- **`ui/`** — Pure rendering functions, one per pane. Each takes `&App` + `Frame` + `Rect`. No state mutation in render code (except `sidebar.rs` which needs `&mut App` for `ListState`)

## Key design patterns

- **Vim state machine**: `Vim::transition()` takes an `Input` and returns a `Transition` enum (Nop/Mode/Pending). Operator-pending mode (e.g., `d` waiting for motion) uses `Vim::with_pending()` to carry forward the first keystroke.
- **Flat-index tree addressing**: The sidebar tree is stored as nested `TreeNode`s but all selection/toggle operations work on the flat index (what the user sees). `walk_mut()` traverses the visible tree to find the node at a given flat index.
- **Connection profiles**: `profiles.toml` uses serde tagged enums (`#[serde(tag = "type")]`) so new backends only need a new variant in `Connection`.

## Keybindings

- `Tab`/`Shift+Tab` — cycle focus between panes
- `Ctrl+E` — execute query (global, works from any pane)
- `q`/`Esc` — quit (from sidebar or results pane)
- Sidebar: `j/k` or arrows to navigate, `Enter` to connect/expand, `h/l` to collapse/expand
- Editor: full vim keybindings (i/a/o for insert, Esc to normal, v for visual, d/y/c operators)

## Testing & TDD

**TDD is mandatory.** When implementing new features or fixing bugs:
1. Write a failing test first that captures the expected behavior
2. Implement the minimum code to make the test pass
3. Refactor if needed, ensuring all tests still pass

Run `cargo test` frequently. Target specific modules with `cargo test <module>` for faster iteration.

All pure logic must have unit tests. Tests live inline as `#[cfg(test)] mod tests` at the bottom of each source file. A `MockDatabase` is available in `db/mod.rs` (behind `#[cfg(test)]`) for testing app logic that depends on database results.

**What is exempt from unit tests:** UI rendering (`ui/*.rs`), `main.rs` glue code, database backend implementations (require real connections — integration test territory), and filesystem-dependent config loading (test deserialization via `toml::from_str()` instead).

When modifying existing code, add regression tests for the specific behavior being changed before making the change.
