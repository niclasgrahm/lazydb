use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::app::{App, Focus};
use crate::tree::TreeNode;

pub fn draw(app: &mut App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Focus::Sidebar;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" Connections ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let flat = TreeNode::flatten_all(&app.sidebar_items);
    let items: Vec<ListItem> = flat
        .iter()
        .map(|node| {
            let indent = "  ".repeat(node.depth as usize);
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

            let mut spans = vec![
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
        )
        .highlight_symbol("│ ");

    frame.render_stateful_widget(list, area, &mut app.sidebar_state);
}
