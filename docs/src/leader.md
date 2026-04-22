# Leader Key

The leader key is the primary way to access context-sensitive actions in lazydb. Press the leader key (default: `Space`) to open a context menu, then press the displayed key to execute the corresponding action.

<!-- TODO: screenshot of the leader key menu -->

## How It Works

The leader key system works like vim's leader key:

1. Press `Space` (or your configured leader key) while not in insert mode
2. A menu appears in the bottom-right corner showing available actions
3. Press the key shown in the menu to execute the action
4. The menu closes after executing, or when you press another key

The menu is context-sensitive — available actions change depending on which panel is currently focused.

## Leader Actions by Pane

When focused on the **Connections sidebar**:

| Key | Action |
|---|---|
| `o` | Connect to database         |
| `d` | Disconnect from database    |
| `s` | Preview table / view        |
| `e` | Execute query               |

When focused on the **Query Editor**:

| Key | Action |
|---|---|
| `e` | Execute query |
| `f` | Format query |

When focused on the **Results** panel:

| Key | Action |
|---|---|
| `c` | Close results |
| `e` | Execute query |

When focused on the **Files** panel:

| Key | Action |
|---|---|
| `e` | Execute query |

When focused on **Recent Queries**:

| Key | Action |
|---|---|
| `d` | Delete recent |
| `e` | Execute query |

## Quick Pane Toggles

The leader key provides quick ways to toggle panel visibility regardless of which panel is focused:

| Key | Panel |
|---|---|
| `1` | Toggle connections sidebar |
| `2` | Toggle file browser      |
| `3` | Toggle recent queries    |

For example, pressing `Space 2` hides the file browser if it's visible (or shows it if hidden).

## Changing the Leader Key

Edit `~/.config/lazydb/config.toml`:

```toml
[keybindings]
leader_key = "\\"
```

Any key or key combination works as the leader key. Common alternatives:

```toml
leader_key = "\\"         # Backslash
leader_key = "ctrl+l"    # Ctrl+L
leader_key = ";"          # Semicolon
```
