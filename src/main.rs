mod error;
mod config;
mod timer;
mod ui;
mod events;
mod notification;
mod hyprland;
mod app;

use std::io;
use std::time::Duration;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use tokio::time::interval;

use app::App;
use config::Config;
use events::{handle_key, Action};
use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::load()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new(config)?;

    // Run app
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let mut tick_interval = interval(Duration::from_secs(1));

    loop {
        // Render UI
        terminal.draw(|f| {
            ui::render(f, &app.timer, &app.theme, app.show_help);
        })?;

        // Handle events
        tokio::select! {
            _ = tick_interval.tick() => {
                app.tick();
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if event::poll(Duration::from_millis(0))? {
                    if let Event::Key(key) = event::read()? {
                        let action = handle_key(key);
                        match action {
                            Action::Quit => app.quit(),
                            Action::TogglePause => app.toggle_pause(),
                            Action::Reset => app.reset(),
                            Action::Skip => app.skip(),
                            Action::ToggleHelp => app.toggle_help(),
                            Action::None => {}
                        }
                    }
                }
            }
        }

        if !app.running {
            break;
        }
    }

    Ok(())
}
