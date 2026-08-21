use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph},
    Frame,
};

use crate::app::App;
use crate::highlight;
use crate::ui::theme;

pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" SQL Preview ")
        .title_style(theme::title(false))
        .padding(Padding::top(1))
        .style(theme::surface(theme::WORKSPACE));

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
