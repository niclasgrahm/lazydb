# Vim Editor

The query editor supports full vim keybindings. When you first open lazydb, the editor starts in Normal mode.

<!-- TODO: screenshot showing the vim mode indicator in the status bar -->

## Modes

lazydb's query editor uses four vim modes:

- **Normal** — Default mode. Use vim movements and operators. No text is inserted.
- **Insert** — Type to insert text. Press `Esc` or `Ctrl+C` to return to Normal mode.
- **Visual** — Select text. Press `y` to yank (copy), `d` to delete, or `c` to delete and enter Insert mode.
- **Operator-pending** — After pressing an operator key (`d`, `y`, or `c`), the editor waits for a motion key to complete the action.

The current mode is displayed in the status bar at the bottom of the screen.

## Moving the Cursor

### Basic Movement

| Key | Action |
|---|---|
| `h` | Left                        |
| `j` | Down                        |
| `k` | Up                          |
| `l` | Right                       |

### Word Movement

| Key | Action |
|---|---|
| `w` | Forward to start of word |
| `e` | Forward to end of word |
| `b` | Backward to start of word |

### Line Movement

| Key | Action |
|---|---|
| `^` | Move to first non-blank char |
| `0` | Move to start of line |
| `$` | Move to end of line |

### Jump

| Key | Action |
|---|---|
| `gg` | Go to top of file |
| `G` | Go to bottom of file |

### Editing Actions

| Key | Action |
|---|---|
| `x` | Delete character under cursor       |
| `D` | Delete from cursor to end of line   |
| `C` | Delete from cursor to end, enter Insert mode |
| `p` | Paste after cursor                  |
| `u` | Undo                                |
| `Ctrl+R` | Redo                          |

## Operators

Operators in normal mode select the current line and wait for a confirmation key:

| Keys | Action |
|---|---|
| `dd` | Delete current line |
| `yy` | Yank (copy) current line |
| `cc` | Change (delete) current line, enter Insert mode |

## Enter Insert Mode

### From Normal mode

| Key | Action |
|---|---|
| `i` | Enter Insert mode (cursor stays) |
| `a` | Enter Insert mode (cursor advances one character) |
| `A` | Enter Insert mode at end of line |
| `I` | Enter Insert mode at first non-blank char |
| `o` | Open a new line below and enter Insert mode |
| `O` | Open a new line above and enter Insert mode |

### From Insert mode

| Key | Action |
|---|---|
| `Esc` | Return to Normal mode |
| `Ctrl+C` | Return to Normal mode |
