use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Focus};

pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Focus::Recent;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" Recent ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let paragraph = Paragraph::new("").block(block);
    frame.render_widget(paragraph, area);
}
