use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Focus};
use crate::highlight;
use crate::vim;

pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Focus::QueryEditor;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let db_prefix = app
        .connected_db
        .as_ref()
        .map(|name| format!("{} - ", name))
        .unwrap_or_default();

    let title = if focused {
        format!(" {}Query [{}] ", db_prefix, app.vim.mode)
    } else {
        format!(" {}Query ", db_prefix)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let lines = app.editor.lines();
    let (cursor_row, cursor_col) = app.editor.cursor();
    let visible_height = inner.height as usize;

    // Compute scroll offset to keep cursor visible
    let scroll_row = if lines.is_empty() {
        0
    } else if cursor_row >= visible_height {
        cursor_row - visible_height + 1
    } else {
        0
    };

    // Build highlighted lines
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(scroll_row)
        .take(visible_height)
        .map(|line| {
            let spans: Vec<Span> = highlight::highlight_line(line)
                .into_iter()
                .map(|hl| Span::styled(hl.text, hl.style))
                .collect();
            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(visible_lines);
    frame.render_widget(paragraph, inner);

    // Draw visual selection highlight
    if let Some(((sr, sc), (er, ec))) = app.editor.selection_range() {
        let sel_style = Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD);
        for row in sr..=er {
            if row < scroll_row || row >= scroll_row + visible_height {
                continue;
            }
            let line_len = lines.get(row).map(|l| l.len()).unwrap_or(0);
            let col_start = if row == sr { sc } else { 0 };
            let col_end = if row == er { ec } else { line_len };
            let screen_y = inner.y + (row - scroll_row) as u16;
            for col in col_start..col_end {
                let screen_x = inner.x + col as u16;
                if screen_x < inner.x + inner.width {
                    if let Some(cell) = frame.buffer_mut().cell_mut((screen_x, screen_y)) {
                        cell.set_style(sel_style);
                    }
                }
            }
        }
    }

    // Draw cursor if focused
    if focused {
        let cursor_screen_row = cursor_row.saturating_sub(scroll_row);
        let cursor_y = inner.y + cursor_screen_row as u16;
        let cursor_x = inner.x + cursor_col as u16;

        if cursor_x < inner.x + inner.width && cursor_y < inner.y + inner.height {
            let cursor_style = match app.vim.mode {
                vim::Mode::Normal => Style::default()
                    .fg(Color::Reset)
                    .add_modifier(Modifier::REVERSED),
                vim::Mode::Insert => Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::REVERSED),
                vim::Mode::Visual => Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::REVERSED),
                vim::Mode::Operator(_) => Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::REVERSED),
            };

            let cell = frame.buffer_mut().cell_mut((cursor_x, cursor_y));
            if let Some(cell) = cell {
                cell.set_style(cursor_style);
            }
        }
    }

    // Show placeholder if empty and not focused
    if lines.iter().all(|l| l.is_empty()) && !focused {
        let placeholder = "Press Tab to switch here and start typing SQL...";
        let style = Style::default().fg(Color::DarkGray);
        let max_len = inner.width as usize;
        let text: String = placeholder.chars().take(max_len).collect();
        frame.buffer_mut().set_string(inner.x, inner.y, &text, style);
    }
}
