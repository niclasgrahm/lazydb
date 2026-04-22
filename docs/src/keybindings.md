# Keybindings

All default keybindings are listed below. You can override any of them in `~/.config/lazydb/config.toml`.

<!-- TODO: screenshot showing the help overlay -->

## Global Keybindings

Always available (except when in vim insert mode or loading):

| Key         | Action                   |
|---|---|
| `Ctrl+E`    | Execute query       |
| `Ctrl+F`    | Format query        |
| `Tab`       | Next focus pane     |
| `Shift+Tab` | Previous focus pane |
| `?`         | Toggle help overlay |

## Sidebar Keybindings

When the connections sidebar is focused:

| Key | Action |
|---|---|
| `k` / `Up` | Navigate up |
| `j` / `Down` | Navigate down |
| `l` / `Right` | Expand |
| `h` / `Left` | Collapse |
| `Enter` | Connect / toggle connection |
| `s` | Preview table / view |
| `/` | Start filtering |
| `q` / `Esc` | Quit |

## Results Keybindings

When the results panel is focused:

| Key | Action |
|---|---|
| `k` / `Up` | Scroll up |
| `j` / `Down` | Scroll down |
| `h` / `Left` | Scroll left |
| `l` / `Right` | Scroll right |
| `n` / `PageDown` | Next page |
| `p` / `PageUp` | Previous page |
| `c` / `Esc` | Close results, return to editor |
| `q` | Quit |

## Query Editor Keys

The query editor uses full vim keybindings. Key entry points into each mode:

| Key      | Action                                    |
|---|---|
| `i`        | Enter insert mode (cursor stays) |
| `a`        | Enter insert mode (cursor advances) |
| `o`        | Open new line below, insert mode |
| `A`        | Enter insert mode at end of line |
| `I`        | Enter insert mode at start of line |
| `Esc`      | Return to normal mode            |
| `Ctrl+C`   | Return to normal mode            |

See [Vim Editor](vim-editor.md) for the complete vim keybinding reference.
