use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;

pub fn draw(app: &App, frame: &mut Frame) {
    if !app.leader_active {
        return;
    }

    let actions = app.leader_actions();
    let mut lines: Vec<Line> = Vec::new();
    for action in &actions {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}  ", action.key),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(action.label),
        ]));
    }

    let content_width: u16 = actions
        .iter()
        .map(|a| 5 + a.label.len() as u16)
        .max()
        .unwrap_or(10)
        + 2;
    let width = content_width.max(12);
    let height = (lines.len() as u16) + 2; // +2 for border
    let area = popup_area_bottom_right(frame.area(), width, height);

    let block = Block::default()
        .title(" Leader ")
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn popup_area_bottom_right(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.right().saturating_sub(w).max(area.left());
    let y = area.bottom().saturating_sub(h).max(area.top());
    Rect::new(x, y, w, h)
}
