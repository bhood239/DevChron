use crate::config::Config;
use crate::timer::PomodoroTimer;
use crate::ui::Theme;
use crate::notification::NotificationManager;
use crate::hyprland::StatusWriter;
use crate::error::Result;

pub struct App {
    pub timer: PomodoroTimer,
    pub theme: Theme,
    pub show_help: bool,
    pub running: bool,
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
            running: true,
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
        let old_phase = self.timer.current_phase();
        self.timer.skip();
        self.notification_manager.send_phase_complete(old_phase);
        self.update_status();
    }

    pub fn tick(&mut self) {
        let phase_completed = self.timer.tick();
        if phase_completed {
            // Get the phase that just completed (before it changed)
            let completed_phase = match self.timer.current_phase() {
                crate::timer::TimerPhase::Focus => {
                    // If we're now in Focus, a break just completed
                    if self.timer.cycle_count == 0 {
                        crate::timer::TimerPhase::LongBreak
                    } else {
                        crate::timer::TimerPhase::ShortBreak
                    }
                }
                crate::timer::TimerPhase::ShortBreak | crate::timer::TimerPhase::LongBreak => {
                    // If we're in a break, focus just completed
                    crate::timer::TimerPhase::Focus
                }
            };
            self.notification_manager.send_phase_complete(completed_phase);
        }
        self.update_status();
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    fn update_status(&self) {
        self.status_writer.update(&self.timer);
    }
}
