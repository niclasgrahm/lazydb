mod editor;
mod files;
mod help;
mod leader;
mod loading;
mod message;
mod recent;
mod results;
mod sidebar;
mod sql_preview;
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

    // Build horizontal constraints based on which panes are visible.
    // Left side: sidebar and/or files (stacked vertically if both visible)
    // Center: query editor (always visible)
    // Right side: recent
    let has_left = app.show_sidebar || app.show_files;
    let has_right = app.show_recent;
    let sidebar_pct = app.sidebar_width;

    let mut h_constraints = Vec::new();
    if has_left {
        h_constraints.push(Constraint::Percentage(sidebar_pct));
    }
    h_constraints.push(Constraint::Percentage(
        100 - if has_left { sidebar_pct } else { 0 } - if has_right { sidebar_pct } else { 0 },
    ));
    if has_right {
        h_constraints.push(Constraint::Percentage(sidebar_pct));
    }

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(h_constraints)
        .split(top_area);

    let mut col = 0;

    // Left column: sidebar and/or files stacked vertically
    if has_left {
        let left_area = panes[col];
        col += 1;

        if app.show_sidebar && app.show_files {
            let left_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(left_area);
            sidebar::draw(app, frame, left_split[0]);
            files::draw(app, frame, left_split[1]);
        } else if app.show_sidebar {
            sidebar::draw(app, frame, left_area);
        } else {
            files::draw(app, frame, left_area);
        }
    }

    // Center: query editor, optionally split with SQL preview below
    let center_area = panes[col];
    col += 1;
    if app.show_sql_preview {
        let center_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(center_area);
        editor::draw(app, frame, center_split[0]);
        sql_preview::draw(app, frame, center_split[1]);
    } else {
        editor::draw(app, frame, center_area);
    }

    // Right column: recent
    if has_right {
        recent::draw(app, frame, panes[col]);
    }

    if app.results_visible {
        app.results_area = outer[1];
        results::draw(app, frame, outer[1]);
        status_bar::draw(app, frame, outer[2]);
    } else {
        status_bar::draw(app, frame, outer[1]);
    }

    message::draw(app, frame);
    help::draw(app, frame);
    leader::draw(app, frame);
    loading::draw(app, frame);
}
