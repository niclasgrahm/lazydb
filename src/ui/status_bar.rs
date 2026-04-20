use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, Focus};

pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let focus_label = match app.focus {
        Focus::Sidebar => "CONNECTIONS",
        Focus::Files => "FILES",
        Focus::QueryEditor => "QUERY",
        Focus::Results => "RESULTS",
        Focus::Recent => "RECENT",
    };

    let mut spans = vec![
        Span::styled(
            format!(" {focus_label} "),
            Style::default().bg(Color::Cyan).fg(Color::Black).bold(),
        ),
    ];

    if let Some(db) = &app.connected_db {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(db.as_str(), Style::default().fg(Color::Green)));
    }

    // Right-align the help hint
    let left_len: usize = spans.iter().map(|s| s.content.len()).sum();
    let hint = format!("{} leader  ? help", app.keys.leader.display);
    let padding = (area.width as usize).saturating_sub(left_len + hint.len());
    if area.width as usize > left_len + hint.len() {
        spans.push(Span::raw(" ".repeat(padding)));
    }
    spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));

    let status = Line::from(spans);
    let bar = Paragraph::new(status).style(Style::default().bg(Color::Black));
    frame.render_widget(bar, area);
}
