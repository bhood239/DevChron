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

    // SIGUSR1 → toggle pause/resume (Waybar on-click)
    // SIGUSR2 → skip to next phase
    // We use channels so the select! branches are always present (just never fire on non-Unix).
    let (sig1_tx, mut sig1_rx) = tokio::sync::mpsc::channel::<()>(1);
    let (sig2_tx, mut sig2_rx) = tokio::sync::mpsc::channel::<()>(1);

    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigusr1 = signal(SignalKind::user_defined1())
            .expect("failed to register SIGUSR1 handler");
        let mut sigusr2 = signal(SignalKind::user_defined2())
            .expect("failed to register SIGUSR2 handler");

        let tx1 = sig1_tx.clone();
        tokio::spawn(async move {
            loop {
                sigusr1.recv().await;
                let _ = tx1.send(()).await;
            }
        });

        let tx2 = sig2_tx.clone();
        tokio::spawn(async move {
            loop {
                sigusr2.recv().await;
                let _ = tx2.send(()).await;
            }
        });
    }

    // Suppress unused-variable warnings on non-Unix.
    let _ = sig1_tx;
    let _ = sig2_tx;

    loop {
        terminal.draw(|f| {
            ui::render(
                f,
                &app.timer,
                &app.theme,
                app.show_help,
                app.minimal_mode,
                app.celebration_ticks,
                app.task_input.as_deref(),
                app.show_task_input,
                app.active_profile_name(),
                app.current_task.as_deref(),
            );
        })?;

        tokio::select! {
            _ = tick_interval.tick() => {
                app.tick();
            }

            _ = sig1_rx.recv() => {
                // SIGUSR1: toggle pause (Waybar on-click)
                app.toggle_pause();
            }

            _ = sig2_rx.recv() => {
                // SIGUSR2: skip phase
                app.skip();
            }

            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if event::poll(Duration::from_millis(0))? {
                    match event::read()? {
                        Event::Key(key) => {
                            if app.show_task_input {
                                app.handle_task_input_key(key);
                            } else {
                                let action = handle_key(key);
                                match action {
                                    Action::Quit             => app.quit(),
                                    Action::TogglePause      => app.toggle_pause(),
                                    Action::Reset            => app.reset(),
                                    Action::Skip             => app.skip(),
                                    Action::ToggleHelp       => app.toggle_help(),
                                    Action::CycleTheme       => app.cycle_theme(),
                                    Action::AddTime          => app.add_time(),
                                    Action::SubtractTime     => app.subtract_time(),
                                    Action::ToggleMinimal    => app.toggle_minimal(),
                                    Action::OpenTaskInput    => app.open_task_input(),
                                    Action::SwitchProfile(i) => app.switch_profile(i),
                                    Action::None             => {}
                                }
                            }
                        }
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
