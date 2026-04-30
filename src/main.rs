mod app;
mod cli;
mod config;
mod db;
mod files;
mod highlight;
mod keybindings;
mod recents;
mod schema_cache;
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

    let files_root = Some(match cli.path {
        None => std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        Some(p) if p == std::path::Path::new(".") => {
            std::env::current_dir().unwrap_or(p)
        }
        Some(p) => std::fs::canonicalize(&p).unwrap_or(p),
    });

    let app_config = config::AppConfig::load()?;
    if app_config.debug {
        init_tracing()?;
    }
    let profiles = config::Profiles::load()?;

    let terminal = ratatui::init();
    let result = run(terminal, app_config, profiles, files_root, cli.query);
    ratatui::restore();
    result
}

fn run(
    mut terminal: DefaultTerminal,
    app_config: config::AppConfig,
    profiles: config::Profiles,
    files_root: Option<std::path::PathBuf>,
    initial_query: Option<String>,
) -> Result<()> {
    let mut app = app::App::new(app_config, profiles, files_root, initial_query);

    while app.running {
        terminal.draw(|frame| ui::draw(&mut app, frame))?;
        app.handle_event()?;
    }

    Ok(())
}
