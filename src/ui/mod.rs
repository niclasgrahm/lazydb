mod editor;
mod help;
mod message;
mod results;
mod sidebar;
mod status_bar;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::app::App;

pub fn draw(app: &mut App, frame: &mut Frame) {
    let outer = if app.results_visible {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
                Constraint::Length(1),
            ])
            .split(frame.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(frame.area())
    };

    let top_area = outer[0];
    let sidebar_pct = app.sidebar_width;
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(sidebar_pct),
            Constraint::Percentage(100 - sidebar_pct),
        ])
        .split(top_area);

    sidebar::draw(app, frame, panes[0]);
    editor::draw(app, frame, panes[1]);

    if app.results_visible {
        app.results_area = outer[1];
        results::draw(app, frame, outer[1]);
        status_bar::draw(app, frame, outer[2]);
    } else {
        status_bar::draw(app, frame, outer[1]);
    }

    message::draw(app, frame);
    help::draw(app, frame);
}
