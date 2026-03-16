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

/// How many seconds the quit-confirm popup stays visible before auto-dismissing.
const QUIT_CONFIRM_TIMEOUT_SECS: u8 = 3;

/// Maximum length of a task tag string.
const MAX_TASK_LEN: usize = 48;

/// How many seconds to wait before auto-starting the next phase.
const AUTO_START_DELAY_SECS: u64 = 3;

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

    // ── Overlays ──────────────────────────────────────────────────────────────
    /// Whether the session history overlay is open.
    pub show_history: bool,
    /// Whether the task-input overlay is currently open.
    pub show_task_input: bool,
    /// Whether the quit-confirm popup is showing (first Q/Esc press).
    pub quit_confirm: bool,
    /// Countdown ticks until the quit-confirm popup auto-dismisses.
    pub quit_confirm_ticks: u8,
    /// Buffer for the text being typed in the task-input overlay.
    pub task_input: Option<String>,
    /// The confirmed task tag for the current focus session.
    pub current_task: Option<String>,

    // ── Profiles ──────────────────────────────────────────────────────────────
    pub profiles: ProfileList,

    // ── Behaviour ─────────────────────────────────────────────────────────────
    auto_start_breaks: bool,
    auto_start_focus: bool,
    /// Countdown (in ticks) before auto-starting the next phase. 0 = not pending.
    pub auto_start_countdown: u64,

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
        let auto_start_breaks = config.settings.behaviour.auto_start_breaks;
        let auto_start_focus = config.settings.behaviour.auto_start_focus;

        Ok(Self {
            timer,
            theme,
            show_help: false,
            minimal_mode: false,
            running: true,
            celebration_ticks: 0,
            show_history: false,
            show_task_input: false,
            quit_confirm: false,
            quit_confirm_ticks: 0,
            task_input: None,
            current_task: None,
            profiles,
            auto_start_breaks,
            auto_start_focus,
            auto_start_countdown: 0,
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

        // Auto-dismiss the quit-confirm popup after timeout.
        if self.quit_confirm_ticks > 0 {
            self.quit_confirm_ticks -= 1;
            if self.quit_confirm_ticks == 0 {
                self.quit_confirm = false;
            }
        }

        // Auto-start countdown: tick down and fire when it hits zero.
        if self.auto_start_countdown > 0 {
            self.auto_start_countdown -= 1;
            if self.auto_start_countdown == 0 {
                self.timer.toggle(); // start the new phase
            }
        }

        // When the timer fires naturally, pass the current task tag.
        if self.timer.current_timer.remaining <= std::time::Duration::from_secs(1)
            && self.timer.is_running()
        {
            let task = self.current_task.clone();
            if let Some(event) = self.timer.tick() {
                // Patch the task tag onto the last completed session entry.
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

    pub fn toggle_history(&mut self) {
        self.show_history = !self.show_history;
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

    /// First press: show confirm popup. Second press within timeout: actually quit.
    pub fn quit(&mut self) {
        if self.quit_confirm {
            self.running = false;
        } else {
            self.quit_confirm = true;
            self.quit_confirm_ticks = QUIT_CONFIRM_TIMEOUT_SECS;
        }
    }

    /// Dismiss the quit-confirm popup without quitting.
    pub fn dismiss_quit_confirm(&mut self) {
        self.quit_confirm = false;
        self.quit_confirm_ticks = 0;
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
        use crate::timer::TimerPhase;

        self.notification_manager.send_phase_complete(event);

        if self.timer.cycle_just_completed {
            self.celebration_ticks = CELEBRATION_TICKS;
        }

        // Arm auto-start if configured for the phase we just entered.
        let just_entered_break = matches!(
            self.timer.current_phase(),
            TimerPhase::ShortBreak | TimerPhase::LongBreak
        );
        let just_entered_focus = self.timer.current_phase() == TimerPhase::Focus;

        if (just_entered_break && self.auto_start_breaks)
            || (just_entered_focus && self.auto_start_focus)
        {
            self.auto_start_countdown = AUTO_START_DELAY_SECS;
        }
    }

    fn update_status(&self) {
        self.status_writer.update(&self.timer);
    }
}
