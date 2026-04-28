use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;
use crate::highlight;

pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" SQL Preview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let sql = app.sql_preview();
    let visible_height = inner.height as usize;

    let lines: Vec<Line> = sql
        .lines()
        .take(visible_height)
        .map(|line| {
            let spans: Vec<Span> = highlight::highlight_line(line)
                .into_iter()
                .map(|hl| Span::styled(hl.text, hl.style))
                .collect();
            Line::from(spans)
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}
