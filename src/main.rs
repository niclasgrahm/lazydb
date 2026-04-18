mod app;
mod cli;
mod config;
mod db;
mod highlight;
mod keybindings;
mod tree;
mod ui;
mod vim;

use std::fs;

use clap::Parser;
use color_eyre::Result;
use ratatui::DefaultTerminal;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

fn init_tracing() -> Result<()> {
    let log_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("lazydb");
    fs::create_dir_all(&log_dir)?;
    let log_file = fs::File::create(log_dir.join("debug.log"))?;

    tracing_subscriber::registry()
        .with(EnvFilter::new("lazydb=debug"))
        .with(
            fmt::layer()
                .with_writer(log_file)
                .with_ansi(false)
                .with_timer(fmt::time::uptime()),
        )
        .init();

    tracing::info!("debug mode enabled — logging to ~/.config/lazydb/debug.log");
    Ok(())
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::Cli::parse();

    if let Some(cmd) = cli.command {
        return cli::handle(cmd);
    }

    let app_config = config::AppConfig::load()?;
    if app_config.debug {
        init_tracing()?;
    }
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
