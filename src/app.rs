use crossterm::event::{KeyCode, KeyEvent};

use crate::config::settings::ProfileSettings;
use crate::config::Config;
use crate::error::Result;
use crate::hyprland::StatusWriter;
use crate::notification::NotificationManager;
use crate::timer::pomodoro::PhaseEvent;
use crate::timer::PomodoroTimer;
use crate::ui::Theme;

/// How many render ticks the "CYCLE COMPLETE" celebration banner stays visible.
const CELEBRATION_TICKS: u8 = 6;

/// Maximum length of a task tag string.
const MAX_TASK_LEN: usize = 48;

// ── Profile list ──────────────────────────────────────────────────────────────

/// An ordered, named list of timer profiles available for switching.
pub struct ProfileList {
    pub profiles: Vec<(String, ProfileSettings)>,
    pub active_index: usize,
}

impl ProfileList {
    fn from_config(config: &Config) -> Self {
        let profiles: Vec<(String, ProfileSettings)> = if config.settings.profiles.is_empty() {
            ProfileSettings::defaults()
        } else {
            config
                .settings
                .profiles
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };

        Self {
            profiles,
            active_index: 0,
        }
    }

    pub fn active(&self) -> &(String, ProfileSettings) {
        &self.profiles[self.active_index]
    }

    /// Switch to a 1-based index. Returns true if the index was valid.
    pub fn switch_to(&mut self, one_based: usize) -> bool {
        if one_based == 0 || one_based > self.profiles.len() {
            return false;
        }
        self.active_index = one_based - 1;
        true
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub timer: PomodoroTimer,
    pub theme: Theme,
    pub show_help: bool,
    pub minimal_mode: bool,
    pub running: bool,
    /// Counts down from CELEBRATION_TICKS to 0 after a full cycle completes.
    pub celebration_ticks: u8,

    // ── Task input ────────────────────────────────────────────────────────────
    /// Whether the task-input overlay is currently open.
    pub show_task_input: bool,
    /// Buffer for the text being typed in the task-input overlay.
    pub task_input: Option<String>,
    /// The confirmed task tag for the current focus session.
    pub current_task: Option<String>,

    // ── Profiles ──────────────────────────────────────────────────────────────
    pub profiles: ProfileList,

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
        let profiles = ProfileList::from_config(&config);

        Ok(Self {
            timer,
            theme,
            show_help: false,
            minimal_mode: false,
            running: true,
            celebration_ticks: 0,
            show_task_input: false,
            task_input: None,
            current_task: None,
            profiles,
            notification_manager,
            status_writer,
        })
    }

    // ── Timer controls ────────────────────────────────────────────────────────

    pub fn toggle_pause(&mut self) {
        self.timer.toggle();
        self.update_status();
    }

    pub fn reset(&mut self) {
        self.timer.reset();
        self.update_status();
    }

    pub fn skip(&mut self) {
        let event = self.timer.advance_phase_with_task(self.current_task.take());
        self.handle_phase_event(event);
        self.update_status();
    }

    pub fn tick(&mut self) {
        if self.celebration_ticks > 0 {
            self.celebration_ticks -= 1;
        }

        // When the timer fires naturally, pass the current task tag.
        if self.timer.current_timer.remaining <= std::time::Duration::from_secs(1)
            && self.timer.is_running()
        {
            // Peek: will complete this tick — grab the task now before tick() clears it.
            let task = self.current_task.clone();
            if let Some(event) = {
                // We need to temporarily override the tick to pass the task.
                // Simplest: let tick() fire normally (it calls advance_phase_with_task(None)),
                // then we've already cloned the task above.
                self.timer.tick()
            } {
                // Re-record with the correct task tag if it was a focus session.
                // The stats entry was already written by tick() with None; we need to
                // patch the last entry if there was a task.
                if let Some(tag) = task {
                    if let Some(last) = self.timer.stats.completed_sessions.last_mut() {
                        if last.task.is_none() {
                            last.task = Some(tag);
                        }
                    }
                }
                self.handle_phase_event(event);
                self.current_task = None;
            }
        } else if let Some(event) = self.timer.tick() {
            self.handle_phase_event(event);
        }

        self.update_status();
    }

    // ── UI controls ───────────────────────────────────────────────────────────

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

    // ── Task input ────────────────────────────────────────────────────────────

    /// Open the task-tag input overlay.
    pub fn open_task_input(&mut self) {
        self.show_task_input = true;
        // Pre-fill with the current task if one is set.
        self.task_input = Some(self.current_task.clone().unwrap_or_default());
    }

    /// Handle a keypress while the task-input overlay is open.
    pub fn handle_task_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                // Confirm: trim and store (empty string → clear the tag).
                let tag = self.task_input.take().unwrap_or_default();
                let tag = tag.trim().to_string();
                self.current_task = if tag.is_empty() { None } else { Some(tag) };
                self.show_task_input = false;
            }
            KeyCode::Esc => {
                // Cancel: discard input, keep previous tag.
                self.task_input = None;
                self.show_task_input = false;
            }
            KeyCode::Backspace => {
                if let Some(ref mut buf) = self.task_input {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                let buf = self.task_input.get_or_insert_with(String::new);
                if buf.len() < MAX_TASK_LEN {
                    buf.push(c);
                }
            }
            _ => {}
        }
    }

    // ── Profiles ──────────────────────────────────────────────────────────────

    /// Switch to a 1-based profile index. Resets the timer to the new profile.
    pub fn switch_profile(&mut self, one_based: usize) {
        if self.profiles.switch_to(one_based) {
            let (_, profile) = self.profiles.active();
            let cycles = profile
                .cycles
                .unwrap_or(self.timer.cycles_before_long_break);
            self.timer.apply_profile(
                profile.focus,
                profile.short_break,
                profile.long_break,
                cycles,
            );
            self.current_task = None;
            self.update_status();
        }
    }

    /// The display name of the currently active profile.
    pub fn active_profile_name(&self) -> &str {
        &self.profiles.active().0
    }

    // ── Private ───────────────────────────────────────────────────────────────

    fn handle_phase_event(&mut self, event: PhaseEvent) {
        self.notification_manager.send_phase_complete(event);

        if self.timer.cycle_just_completed {
            self.celebration_ticks = CELEBRATION_TICKS;
        }
    }

    fn update_status(&self) {
        self.status_writer.update(&self.timer);
    }
}
