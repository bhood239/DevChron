use crate::config::Config;
use crate::error::Result;
use crate::hyprland::StatusWriter;
use crate::notification::NotificationManager;
use crate::timer::pomodoro::PhaseEvent;
use crate::timer::PomodoroTimer;
use crate::ui::Theme;

/// How many render ticks the "CYCLE COMPLETE" celebration banner stays visible.
const CELEBRATION_TICKS: u8 = 6;

pub struct App {
    pub timer: PomodoroTimer,
    pub theme: Theme,
    pub show_help: bool,
    pub minimal_mode: bool,
    pub running: bool,
    /// Counts down from CELEBRATION_TICKS to 0 after a full cycle completes.
    pub celebration_ticks: u8,
    notification_manager: NotificationManager,
    status_writer: StatusWriter,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let timer_settings = &config.settings.timer;
        let timer = PomodoroTimer::new(
            timer_settings.focus_duration,
            timer_settings.short_break_duration,
            timer_settings.long_break_duration,
            timer_settings.cycles_before_long_break,
        );

        let theme = Theme::from_name(&config.settings.ui.theme);
        let notification_manager = NotificationManager::new(config.settings.notifications.enabled);
        let status_writer = StatusWriter::new(config.settings.integrations.hyprland_status_bar)?;

        Ok(Self {
            timer,
            theme,
            show_help: false,
            minimal_mode: false,
            running: true,
            celebration_ticks: 0,
            notification_manager,
            status_writer,
        })
    }

    pub fn toggle_pause(&mut self) {
        self.timer.toggle();
        self.update_status();
    }

    pub fn reset(&mut self) {
        self.timer.reset();
        self.update_status();
    }

    pub fn skip(&mut self) {
        let event = self.timer.skip();
        self.handle_phase_event(event);
        self.update_status();
    }

    pub fn tick(&mut self) {
        // Tick down the celebration banner.
        if self.celebration_ticks > 0 {
            self.celebration_ticks -= 1;
        }

        if let Some(event) = self.timer.tick() {
            self.handle_phase_event(event);
        }
        self.update_status();
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn toggle_minimal(&mut self) {
        self.minimal_mode = !self.minimal_mode;
    }

    pub fn cycle_theme(&mut self) {
        self.theme = self.theme.cycle_next();
    }

    pub fn add_time(&mut self) {
        self.timer.adjust_time(5 * 60);
        self.update_status();
    }

    pub fn subtract_time(&mut self) {
        self.timer.adjust_time(-(5 * 60));
        self.update_status();
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    // ── Private ──────────────────────────────────────────────────────────────

    fn handle_phase_event(&mut self, event: PhaseEvent) {
        // Send desktop notification for the completed phase.
        self.notification_manager.send_phase_complete(event);

        // If a full cycle just completed, trigger the celebration banner.
        if self.timer.cycle_just_completed {
            self.celebration_ticks = CELEBRATION_TICKS;
        }
    }

    fn update_status(&self) {
        self.status_writer.update(&self.timer);
    }
}
