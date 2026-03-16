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
    let config = Config::load()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config)?;

    let result = run_app(&mut terminal, &mut app).await;

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
        terminal.draw(|f| {
            ui::render(
                f,
                &app.timer,
                &app.theme,
                app.show_help,
                app.minimal_mode,
                app.celebration_ticks,
            );
        })?;

        tokio::select! {
            _ = tick_interval.tick() => {
                app.tick();
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if event::poll(Duration::from_millis(0))? {
                    match event::read()? {
                        Event::Key(key) => {
                            let action = handle_key(key);
                            match action {
                                Action::Quit           => app.quit(),
                                Action::TogglePause    => app.toggle_pause(),
                                Action::Reset          => app.reset(),
                                Action::Skip           => app.skip(),
                                Action::ToggleHelp     => app.toggle_help(),
                                Action::CycleTheme     => app.cycle_theme(),
                                Action::AddTime        => app.add_time(),
                                Action::SubtractTime   => app.subtract_time(),
                                Action::ToggleMinimal  => app.toggle_minimal(),
                                Action::None           => {}
                            }
                        }
                        // Redraw immediately on terminal resize
                        Event::Resize(_, _) => {
                            terminal.autoresize()?;
                        }
                        _ => {}
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
