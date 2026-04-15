use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, MessageLevel};

pub fn draw(app: &App, frame: &mut Frame) {
    let Some(msg) = &app.message else { return };

    let (title, border_color) = match msg.level {
        MessageLevel::Error => (" Error ", Color::Red),
        MessageLevel::Info => (" Info ", Color::Cyan),
    };

    let area = popup_area(frame.area(), 60, 30);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD));

    let hint = Line::from(vec![
        Span::styled("Press ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" to dismiss", Style::default().fg(Color::DarkGray)),
    ]);

    let text = vec![
        Line::from(msg.text.as_str()),
        Line::raw(""),
        hint,
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)])
        .flex(Flex::Center)
        .split(area);
    Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center)
        .split(vertical[0])[0]
}
