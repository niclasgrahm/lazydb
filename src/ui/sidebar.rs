use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::{App, Focus};

pub fn draw(app: &mut App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Focus::Sidebar;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let has_filter = !app.sidebar_filter.is_empty() || app.sidebar_filtering;

    // Split area: optional filter bar at top, then the list
    let chunks = if has_filter {
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(area)
    } else {
        Layout::vertical([Constraint::Length(0), Constraint::Min(0)]).split(area)
    };

    // Draw filter box when active
    if has_filter {
        let filter_block = Block::default()
            .title(" Filter ")
            .borders(Borders::ALL)
            .border_style(if app.sidebar_filtering {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            });
        let filter_text = Paragraph::new(Line::from(vec![
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.sidebar_filter, Style::default().fg(Color::White)),
            if app.sidebar_filtering {
                Span::styled("▎", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ]))
        .block(filter_block);
        frame.render_widget(filter_text, chunks[0]);
    }

    let block = Block::default()
        .title(" Connections ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let flat = app.filtered_flat_nodes();
    let selected = app.sidebar_state.selected();
    let items: Vec<ListItem> = flat
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let indent = if node.depth > 0 {
                "  ".repeat(node.depth as usize)
            } else {
                String::new()
            };
            let icon = if node.has_children {
                if node.expanded { "▼ " } else { "▶ " }
            } else {
                "  "
            };

            let is_connected = node.depth == 0
                && app
                    .connected_db
                    .as_ref()
                    .is_some_and(|db| db == &node.label);

            let style = match node.depth {
                0 => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                1 => Style::default().fg(Color::Blue),
                _ => Style::default().fg(Color::White),
            };

            let cursor = if selected == Some(i) { "│" } else { " " };

            let mut spans = vec![
                Span::styled(cursor, Style::default().fg(Color::Cyan)),
                Span::raw(indent),
                Span::raw(icon),
                Span::styled(node.label.clone(), style),
            ];
            if is_connected {
                spans.push(Span::styled(" ●", Style::default().fg(Color::Green)));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, chunks[1], &mut app.sidebar_state);
}
