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
    let focused = app.focus == Focus::Files;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = if let Some(ref root) = app.files_root {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.display().to_string());
        format!(" Files — {name} ")
    } else {
        " Files ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let flat = TreeNode::flatten_all(&app.file_tree);
    let selected = app.file_tree_state.selected();

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

            let style = if node.has_children {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            } else if node.label.ends_with(".sql") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };

            let cursor = if selected == Some(i) { "│" } else { " " };

            let spans = vec![
                Span::styled(cursor, Style::default().fg(Color::Cyan)),
                Span::raw(indent),
                Span::raw(icon),
                Span::styled(node.label.clone(), style),
            ];

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

    frame.render_stateful_widget(list, area, &mut app.file_tree_state);
}
