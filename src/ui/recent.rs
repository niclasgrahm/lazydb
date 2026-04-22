use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::app::{App, Focus};
use crate::recents::format_relative_time;

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

    if app.recents.entries.is_empty() {
        let empty = List::new(vec![ListItem::new(Span::styled(
            "No recent queries",
            Style::default().fg(Color::DarkGray),
        ))])
        .block(block);
        frame.render_widget(empty, area);
        return;
    }

    let inner_width = area.width.saturating_sub(4) as usize; // borders + highlight symbol
    let items: Vec<ListItem> = app
        .recents
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let query_preview: String = entry
                .query
                .chars()
                .take(inner_width)
                .map(|c| if c == '\n' { ' ' } else { c })
                .collect();
            let time = format_relative_time(entry.timestamp);
            let conn = entry.connection.as_deref().unwrap_or("?");

            let query_style = if i == app.recents_selected && focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if i == app.recents_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let error_marker = if entry.error.is_some() { " ✗" } else { "" };
            let meta_style = Style::default().fg(Color::DarkGray);

            ListItem::new(vec![
                Line::from(Span::styled(query_preview, query_style)),
                Line::from(vec![
                    Span::styled(format!("{conn}"), meta_style),
                    Span::styled(format!(" · {time}{error_marker}"), meta_style),
                ]),
            ])
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.recents_selected));

    let list = List::new(items)
        .block(block)
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut state);
}
