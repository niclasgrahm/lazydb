use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::{App, Focus};

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

    let has_filter = !app.file_filter.is_empty() || app.file_filtering;

    let chunks = if has_filter {
        Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(area)
    } else {
        Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Min(1)])
            .split(area)
    };

    if has_filter {
        let filter_block = Block::default()
            .title(" Filter ")
            .borders(Borders::ALL)
            .border_style(if app.file_filtering {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            });
        let filter_text = Paragraph::new(Line::from(vec![
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled(app.file_filter.clone(), Style::default().fg(Color::White)),
            if app.file_filtering {
                Span::styled("▎", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ]))
        .block(filter_block);
        frame.render_widget(filter_text, chunks[0]);
    }

    let list_area = chunks[if has_filter { 1 } else { 0 }];

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let flat = app.filtered_file_nodes();
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

            let base_style = if node.has_children {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            } else if node.label.ends_with(".sql") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };

            let cursor = if selected == Some(i) { "│" } else { " " };

            let mut spans = vec![
                Span::styled(cursor, Style::default().fg(Color::Cyan)),
                Span::raw(indent),
                Span::raw(icon),
            ];
            spans.extend(highlight_matches(&node.label, &app.file_filter, base_style));

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

    frame.render_stateful_widget(list, list_area, &mut app.file_tree_state);
}

fn highlight_matches(label: &str, filter: &str, base_style: Style) -> Vec<Span<'static>> {
    if filter.is_empty() {
        return vec![Span::styled(label.to_string(), base_style)];
    }
    let lower_label = label.to_lowercase();
    let lower_filter = filter.to_lowercase();
    let match_style = base_style.bg(Color::Rgb(140, 90, 0));
    let mut spans = Vec::new();
    let mut last = 0;
    while let Some(rel) = lower_label[last..].find(&*lower_filter) {
        let start = last + rel;
        let end = start + lower_filter.len();
        if start > last {
            spans.push(Span::styled(label[last..start].to_string(), base_style));
        }
        spans.push(Span::styled(label[start..end].to_string(), match_style));
        last = end;
    }
    if last < label.len() {
        spans.push(Span::styled(label[last..].to_string(), base_style));
    }
    spans
}
