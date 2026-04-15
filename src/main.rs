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

    let app_config = config::AppConfig::load()?;
    let profiles = config::Profiles::load()?;

    let terminal = ratatui::init();
    let result = run(terminal, app_config, profiles);
    ratatui::restore();
    result
}

fn run(
    mut terminal: DefaultTerminal,
    app_config: config::AppConfig,
    profiles: config::Profiles,
) -> Result<()> {
    let mut app = app::App::new(app_config, profiles);

    while app.running {
        terminal.draw(|frame| ui::draw(&mut app, frame))?;
        app.handle_event()?;
    }

    Ok(())
}
