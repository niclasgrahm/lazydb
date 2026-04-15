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
        Focus::QueryEditor => "QUERY",
        Focus::Results => "RESULTS",
    };

    let mut spans = vec![
        Span::styled(
            format!(" {focus_label} "),
            Style::default().bg(Color::Cyan).fg(Color::Black).bold(),
        ),
        Span::raw("  "),
        Span::styled("Tab", Style::default().fg(Color::Yellow)),
        Span::raw(" switch pane  "),
        Span::styled("Ctrl+E", Style::default().fg(Color::Yellow)),
        Span::raw(" execute  "),
        Span::styled("q/Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" quit"),
    ];

    if app.results_visible && app.focus == Focus::Results {
        spans.push(Span::raw("  "));
        spans.push(Span::styled("c", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(" close results"));
        spans.push(Span::raw("  "));
        spans.push(Span::styled("Esc", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(" close results"));
    }

    let status = Line::from(spans);
    let bar = Paragraph::new(status).style(Style::default().bg(Color::Black));
    frame.render_widget(bar, area);
}
