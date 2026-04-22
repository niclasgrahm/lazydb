# Panels

lazydb has five panels that you can navigate between and toggle visibility. Only the Query Editor is always visible.

<!-- TODO: screenshot of the full UI with all panels visible -->

## Connections Sidebar

The connections sidebar lists your configured database connections. Each connection expands to show its schema (tables, views, etc.) once connected.

### Navigation

| Action | Key |
|---|---|
| Move up / down | `k` / `j` or arrow keys |
| Expand / collapse | `l` / `h` or arrow keys |
| Select (connect / toggle) | `Enter` |
| Filter nodes | `/` to start typing a filter |
| Leave filter mode | `Esc` |

### Connecting to a database

1. Navigate to a connection name (e.g., `mydb (postgres)`)
2. Press `Enter`
3. A loading spinner appears while the connection is established in the background
4. On success, the schema tree populates beneath the connection

### Disconnecting

1. Navigate to a connected database
2. Press `Enter` (same key — it disconnects if already connected)

### Previewing tables and views

Press `s` on any table or view node in the sidebar to automatically generate a `SELECT * FROM <table> LIMIT 10` query in the editor.

<!-- TODO: screenshot of sidebar with expanded tables -->

## File Browser

The file browser lets you browse your local filesystem and open SQL files (or other text files) directly into the query editor.

### Navigation

| Action | Key |
|---|---|
| Move up / down | `k` / `j` or arrow keys |
| Expand / collapse directories | `l` / `h` or arrow keys |
| Open file | `Enter` |

### File types

Only text files are opens into the editor. Supported extensions:

```
sql, txt, csv, json, toml, yaml, yml, md, py, sh, rs, go, js, ts, lua, cfg, ini, xml, html
```

### Viewing the file browser

It is toggled with `leader 2`. If you launch `lazydb` with a path argument (e.g., `lazydb /path/to/queries/`), the files pane is shown by default.

<!-- TODO: screenshot of file browser -->

## Query Editor

The query editor is the central panel of lazydb. You write SQL here and execute it.

### Placeholder

When no files are loaded, the editor shows a placeholder:

> "Press Tab to switch here and start typing SQL..."

### Vim modes

The editor supports full vim keybindings. See [Vim Editor](vim-editor.md) for the complete reference.

The status bar at the bottom shows the current mode:

- `NORMAL` — default mode for navigation and vim commands
- `INSERT` — text entry mode
- `VISUAL` — text selection mode
- `OPERATOR(<key>)` — waiting for the completion key (e.g., after pressing `d`)

<!-- TODO: screenshot of the query editor with vim mode indicator -->

### Formatting SQL

Press `Ctrl+F` (or `leader f`) to format the current query using `sqlformat` with 2-space indentation and uppercase keywords.

### Executing queries

Press `Ctrl+E` from anywhere to execute the query. The results appear in the Results panel.

## Results

The Results panel displays query output in a table format. It appears below the other panels when you execute a `SELECT` query.

### Navigation

| Action | Key |
|---|---|
| Scroll up / down | `k` / `j` or arrow keys |
| Scroll left / right | `h` / `l` or arrow keys |
| Next page | `n` / `pagedown` |
| Previous page | `p` / `pageup` |
| Close & return to editor | `c` / `Esc` |
| Quit | `q` |

### Pagination

Large result sets are paginated automatically (100 rows per page). The status bar shows a `More...` indicator when additional pages are available.

### Column width

Columns auto-size to their widest content. When columns overflow the viewport, use horizontal scrolling.

<!-- TODO: screenshot of the results panel -->

## Recent Queries

The Recent Queries panel shows a history of your last queries, along with execution time and (if successful) the result cached from when the query ran.

### Viewing recent queries

Toggle the panel with `leader 3`. Recent queries appear sorted by most recent, with relative timestamps (e.g., `5m ago`).

### Navigating

| Action | Key |
|---|---|
| Select up / down | `k` / `j` or arrow keys |
| Replay into editor | `Enter` |
| Delete selected entry | `d` |

### Replaying queries

Press `Enter` on a recent query entry to load the query into the editor, along with the cached result and execution time. This lets you review, modify, and re-run previous queries.

### Deleting entries

Press `d` to remove the selected recent entry and update the list.

<!-- TODO: screenshot of the recent queries panel -->
