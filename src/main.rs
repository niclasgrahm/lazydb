mod app;
mod config;
mod db;
mod tree;
mod ui;
mod vim;

use color_eyre::Result;
use ratatui::DefaultTerminal;

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    let mut app = app::App::new();

    while app.running {
        terminal.draw(|frame| ui::draw(&mut app, frame))?;
        app.handle_event()?;
    }

    Ok(())
}
