use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
};

use crate::app::App;

const MAX_VISIBLE: u16 = 8;

pub fn draw(app: &App, frame: &mut Frame) {
    let (Some(state), Some(vp)) = (&app.completion, &app.editor_viewport) else {
        return;
    };
    if state.suggestions.is_empty() {
        return;
    }

    let (cursor_row, _) = app.editor.cursor();
    // Anchor popup to the start of the trailing token so it doesn't drift.
    let cursor_col = state.ctx.replace_col_start;

    // Convert buffer coords to screen coords using the recorded viewport.
    let inner = vp.inner;
    let scroll_row = vp.scroll_row;
    if cursor_row < scroll_row {
        return;
    }
    let row_offset = (cursor_row - scroll_row) as u16;
    if row_offset >= inner.height {
        return;
    }
    let screen_y = inner.y + row_offset + 1;
    let screen_x = inner.x + cursor_col as u16;

    let longest = state
        .suggestions
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(10);
    let width = ((longest as u16) + 2).clamp(12, 40);
    let visible_count = (state.suggestions.len() as u16).min(MAX_VISIBLE);
    let height = visible_count + 2;

    // Flip above the cursor row if there isn't enough space below.
    let y = if screen_y + height > inner.bottom() && screen_y > height {
        screen_y.saturating_sub(height + 1)
    } else {
        screen_y
    };
    let x = screen_x.min(inner.right().saturating_sub(width));
    let area = Rect::new(x, y, width, height);

    let items: Vec<ListItem> = state
        .suggestions
        .iter()
        .map(|s| ListItem::new(Line::from(s.as_str())))
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));

    let block = Block::default().borders(Borders::ALL).border_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_widget(Clear, area);
    frame.render_stateful_widget(list, area, &mut list_state);
}
