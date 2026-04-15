use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
};

use crate::app::{App, Focus};
use crate::vim;

pub fn draw(app: &mut App, frame: &mut Frame, area: Rect) {
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

    app.editor.set_block(block);
    app.editor.set_style(Style::default().fg(Color::White));

    if focused {
        let cursor_color = match app.vim.mode {
            vim::Mode::Normal => Color::Reset,
            vim::Mode::Insert => Color::LightBlue,
            vim::Mode::Visual => Color::LightYellow,
            vim::Mode::Operator(_) => Color::LightGreen,
        };
        app.editor.set_cursor_style(
            Style::default()
                .fg(cursor_color)
                .add_modifier(Modifier::REVERSED),
        );
    } else {
        app.editor.set_cursor_style(Style::default());
    }

    frame.render_widget(&app.editor, area);
}
