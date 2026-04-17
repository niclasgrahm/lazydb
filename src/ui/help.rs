use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{App, Focus};
use crate::keybindings::Action;

pub fn draw(app: &App, frame: &mut Frame) {
    if !app.show_help {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Global section
    lines.push(section_header("Global"));
    let g = &app.keys.global;
    lines.push(key_line(&g.execute_query, "Execute query"));
    lines.push(key_line(&g.format_query, "Format query"));
    lines.push(key_line(&g.next_pane, "Next pane"));
    lines.push(key_line(&g.prev_pane, "Previous pane"));
    lines.push(key_line(&g.show_help, "Toggle help"));
    lines.push(Line::raw(""));

    // Context-specific section
    match app.focus {
        Focus::Sidebar => {
            lines.push(section_header("Sidebar"));
            let s = &app.keys.sidebar;
            lines.push(key_line(&s.navigate_up, "Navigate up"));
            lines.push(key_line(&s.navigate_down, "Navigate down"));
            lines.push(key_line(&s.expand, "Expand"));
            lines.push(key_line(&s.collapse, "Collapse"));
            lines.push(key_line(&s.activate, "Connect / toggle"));
            lines.push(key_line(&s.preview, "Preview table/view"));
            lines.push(key_line(&s.quit, "Quit"));
        }
        Focus::QueryEditor => {
            lines.push(section_header("Editor"));
            lines.push(Line::from(vec![
                Span::styled("  Vim keybindings", Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  i/a/o  ", Style::default().fg(Color::Yellow)),
                Span::raw("Enter insert mode"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Esc    ", Style::default().fg(Color::Yellow)),
                Span::raw("Back to normal mode"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  v      ", Style::default().fg(Color::Yellow)),
                Span::raw("Visual mode"),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  d/y/c  ", Style::default().fg(Color::Yellow)),
                Span::raw("Delete/yank/change"),
            ]));
        }
        Focus::Results => {
            lines.push(section_header("Results"));
            let r = &app.keys.results;
            lines.push(key_line(&r.scroll_up, "Scroll up"));
            lines.push(key_line(&r.scroll_down, "Scroll down"));
            lines.push(key_line(&r.scroll_left, "Scroll left"));
            lines.push(key_line(&r.scroll_right, "Scroll right"));
            lines.push(key_line(&r.next_page, "Next page"));
            lines.push(key_line(&r.prev_page, "Previous page"));
            lines.push(key_line(&r.close, "Close results"));
            lines.push(key_line(&r.quit, "Quit"));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("  Press any key to close", Style::default().fg(Color::DarkGray)),
    ]));

    let height = (lines.len() as u16) + 2; // +2 for border
    let area = popup_area(frame.area(), 40, height);

    let block = Block::default()
        .title(" Keybindings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn section_header(name: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("  {name}"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )])
}

fn key_line<'a>(action: &Action, description: &'a str) -> Line<'a> {
    let display = format!("  {:<12}", action.display);
    Line::from(vec![
        Span::styled(display, Style::default().fg(Color::Yellow)),
        Span::raw(description),
    ])
}

fn popup_area(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0])[0]
}
