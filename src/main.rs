mod cli;
mod error;
mod config;
mod sound;
mod timer;
mod ui;
mod events;
mod notification;
mod hyprland;
mod app;

use std::io;
use std::time::Duration;
use clap::Parser;
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
use cli::{Cli, Commands};
use config::Config;
use events::{handle_key, Action};
use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // ── `devchron status` subcommand ─────────────────────────────────────────
    if let Some(Commands::Status { json }) = cli.command {
        return run_status(json);
    }

    // ── Normal TUI mode ──────────────────────────────────────────────────────
    let mut config = Config::load()?;

    // Apply CLI overrides on top of config file values.
    if let Some(theme) = &cli.theme {
        config.settings.ui.theme = theme.clone();
    }
    if cli.no_notifications {
        config.settings.notifications.enabled = false;
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config)?;

    // Apply --profile override after App is constructed (profiles are resolved there).
    if let Some(profile_name) = &cli.profile {
        let idx = app.profiles.profiles.iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(profile_name));
        if let Some(i) = idx {
            app.switch_profile(i + 1); // switch_profile is 1-based
        }
    }

    let result = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

// ── Status subcommand ─────────────────────────────────────────────────────────

fn run_status(json: bool) -> Result<()> {
    let status_path = dirs::cache_dir()
        .map(|d| d.join("devchron").join("status.json"));

    let path = match status_path {
        Some(p) if p.exists() => p,
        _ => {
            eprintln!("devchron: not running (no status file found)");
            std::process::exit(1);
        }
    };

    let content = std::fs::read_to_string(&path)
        .map_err(|e| crate::error::Error::Io(e))?;

    if json {
        print!("{}", content);
    } else {
        // Parse and pretty-print a one-liner.
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            let phase = val["phase"].as_str().unwrap_or("unknown");
            let time  = val["time_remaining"].as_str().unwrap_or("--:--");
            let session = val["session"].as_str().unwrap_or("?/?");
            let running = val["is_running"].as_bool().unwrap_or(false);
            let state = if running { "▶" } else { "⏸" };
            println!("{} {} · {} · {}", state, phase, time, session);
        } else {
            print!("{}", content);
        }
    }

    Ok(())
}

// ── TUI event loop ────────────────────────────────────────────────────────────

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let mut tick_interval = interval(Duration::from_secs(1));

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
                app.show_history,
                app.auto_start_countdown,
                app.quit_confirm,
                app.quit_confirm_ticks,
                app.sound_enabled,
            );
        })?;

        tokio::select! {
            _ = tick_interval.tick() => {
                app.tick();
            }
            _ = sig1_rx.recv() => {
                app.toggle_pause();
            }
            _ = sig2_rx.recv() => {
                app.skip();
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if event::poll(Duration::from_millis(0))? {
                    match event::read()? {
                        Event::Key(key) => {
                            if app.show_task_input {
                                app.handle_task_input_key(key);
                            } else if app.quit_confirm {
                                // Quit-confirm popup is showing: Q/Esc confirms quit,
                                // anything else (including Ctrl+C) dismisses it.
                                // Ctrl+C always quits immediately regardless.
                                use crossterm::event::{KeyCode, KeyModifiers};
                                let is_ctrl_c = key.code == KeyCode::Char('c')
                                    && key.modifiers.contains(KeyModifiers::CONTROL);
                                let is_quit = matches!(
                                    key.code,
                                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc
                                );
                                if is_ctrl_c || is_quit {
                                    app.quit(); // second press → running = false
                                } else {
                                    app.dismiss_quit_confirm();
                                }
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
                                    Action::ToggleHistory    => app.toggle_history(),
                                    Action::ToggleSound      => app.toggle_sound(),
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
