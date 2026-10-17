mod error;
mod config;
mod timer;
mod ui;
mod events;
mod notification;
mod hyprland;
mod alarm;
mod app;

use std::io;
use std::time::{Duration, Instant};
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

use app::App;
use config::Config;
use events::handle_key;
use error::Result;

/// Longest the UI waits for input before redrawing. Purely a repaint cadence:
/// the countdown itself is computed from the wall clock, so this value has no
/// effect on timer accuracy.
const FRAME_INTERVAL: Duration = Duration::from_millis(50);

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config)?;

    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

/// Single event loop: advance the timer to "now", draw if anything visible
/// changed, then wait for input for at most one frame. There is exactly one
/// loop and no background timers or tasks, so nothing can run twice or keep
/// running after a mode switch.
fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    // What the last frame showed. The ring moves far less than one dot per
    // second, so repainting when the displayed second changes keeps it exact
    // while idling at ~0 % CPU instead of repainting 20 times a second.
    // While the full-cycle reward reveal animation is active (first 1.2s),
    // `reward_step` ticks every 50ms for a smooth 20 FPS reveal, then slows to
    // a gentle 250ms shimmer.
    let mut last_drawn: Option<(u64, bool, Option<bool>, Option<u64>)> = None;
    let mut dirty = true;

    loop {
        let now = Instant::now();
        app.tick(now);

        let view = app.view(now);
        let reward_step = view.reward.map(|_| {
            if view.reward_elapsed_ms <= 1200 {
                view.reward_elapsed_ms / 50
            } else {
                24 + (view.reward_elapsed_ms / 250)
            }
        });
        let shown = (
            app.timer.current_timer.remaining_secs_ceil(),
            view.celebrating,
            // Repaint when the alarm flash toggles (twice a second).
            view.alarm_ringing.then_some(view.alarm_flash),
            reward_step,
        );
        if dirty || last_drawn != Some(shown) {
            terminal.draw(|f| ui::render(f, &app.view(now)))?;
            last_drawn = Some(shown);
            dirty = false;
        }

        if event::poll(FRAME_INTERVAL)? {
            // Handle everything that is already queued before redrawing.
            loop {
                match event::read()? {
                    Event::Key(key) => app.handle_action(handle_key(key)),
                    // Redraw immediately on terminal resize
                    Event::Resize(_, _) => {
                        terminal.autoresize()?;
                    }
                    _ => {}
                }
                dirty = true;
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        if !app.running {
            break;
        }
    }

    Ok(())
}
