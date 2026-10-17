use crate::alarm::Alarm;
use crate::config::Config;
use crate::error::Result;
use crate::events::Action;
use crate::hyprland::StatusWriter;
use crate::notification::NotificationManager;
use crate::timer::mode::{build_modes, DEFAULT_MODE};
use crate::timer::pomodoro::PhaseEvent;
use crate::timer::{FocusCategory, PomodoroMode, PomodoroTimer, SystemIdentity, TimerPhase};
use crate::ui::{Theme, UiState};
use std::time::{Duration, Instant};

/// How long the "CYCLE COMPLETE" celebration banner stays visible.
const CELEBRATION_DURATION: Duration = Duration::from_secs(6);

pub struct App {
    pub timer: PomodoroTimer,
    pub theme: Theme,
    pub show_help: bool,
    pub show_analytics: bool,
    pub minimal_mode: bool,
    pub running: bool,
    /// All selectable modes (see `timer::mode::MODES`).
    pub modes: Vec<PomodoroMode>,
    /// Index into `modes` of the active mode.
    pub mode_index: usize,
    /// A requested mode switch waiting for the user's yes/no.
    pub pending_mode: Option<usize>,
    /// Optional category filter on the Analytics page (`None` = All categories).
    pub analytics_filter: Option<FocusCategory>,
    /// Scroll offset for the Analytics recent sessions log.
    pub analytics_scroll: usize,
    /// Offline system username and computer hostname.
    pub system_identity: SystemIdentity,
    /// The celebration banner is shown until this instant.
    celebration_until: Option<Instant>,
    notification_manager: NotificationManager,
    alarm: Alarm,
    status_writer: StatusWriter,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let timer_settings = &config.settings.timer;
        let mut modes = build_modes(
            timer_settings.focus_duration,
            timer_settings.short_break_duration,
            timer_settings.long_break_duration,
        );
        modes[DEFAULT_MODE] =
            modes[DEFAULT_MODE].with_cycles(timer_settings.cycles_before_long_break);
        let timer = PomodoroTimer::new(modes[DEFAULT_MODE], modes[DEFAULT_MODE].focus_cycles);

        let theme = Theme::from_name(&config.settings.ui.theme);
        let notification_manager = NotificationManager::new(config.settings.notifications.enabled);
        let alarm = Alarm::new(config.settings.alarm.enabled);
        let status_writer = StatusWriter::new(config.settings.integrations.hyprland_status_bar)?;
        let system_identity = SystemIdentity::detect();

        Ok(Self {
            timer,
            theme,
            show_help: false,
            show_analytics: false,
            minimal_mode: false,
            running: true,
            modes,
            mode_index: DEFAULT_MODE,
            pending_mode: None,
            analytics_filter: None,
            analytics_scroll: 0,
            system_identity,
            celebration_until: None,
            notification_manager,
            alarm,
            status_writer,
        })
    }

    /// Snapshot of everything the renderer needs for the frame at `now`.
    pub fn view(&self, now: Instant) -> UiState<'_> {
        let reward = self.timer.reward.as_ref();
        let reward_anim_progress = reward.map_or(0.0, |r| r.animation_progress(now));
        let reward_elapsed_ms = reward.map_or(0, |r| r.elapsed_ms(now));

        UiState {
            timer: &self.timer,
            theme: &self.theme,
            show_help: self.show_help,
            show_analytics: self.show_analytics,
            minimal_mode: self.minimal_mode,
            celebrating: self.celebration_until.is_some_and(|t| now < t),
            modes: &self.modes,
            mode_index: self.mode_index,
            pending_mode: self.pending_mode,
            reward,
            reward_anim_progress,
            reward_elapsed_ms,
            alarm_ringing: self.timer.alarm_ringing,
            alarm_flash: self.timer.alarm_ringing && self.alarm.flash_on(now),
            system_identity: &self.system_identity,
            analytics_filter: self.analytics_filter,
            analytics_scroll: self.analytics_scroll,
        }
    }

    /// Route a key action. Dialogs and separate pages are modal: while the
    /// help screen, analytics page, mode-switch confirmation, or full-cycle
    /// reward modal is open, keys only affect that view (so a stray key can
    /// never change the timer behind something the user can't see). Quit and
    /// StopAlarm always work.
    pub fn handle_action(&mut self, action: Action) {
        if action == Action::Quit {
            self.quit();
            return;
        }

        // The stop-alarm key works from anywhere (even the help, reward, or analytics screen).
        if action == Action::StopAlarm {
            self.stop_alarm();
            return;
        }

        if self.show_help {
            // "Press any key to return"
            self.show_help = false;
            return;
        }

        // Mode-switch confirmation modal takes priority and blocks unrelated keys
        // so repeated presses cannot stack dialogs or bypass confirmation.
        if self.pending_mode.is_some() {
            match action {
                Action::Confirm => self.confirm_mode_switch(),
                Action::Cancel | Action::Escape => self.cancel_mode_switch(),
                _ => {}
            }
            return;
        }

        // While the dedicated Analytics page is open, keep timer controls isolated
        // and route category-filter (`c` / `Tab`) and scroll (`j`/`k` / `Up`/`Down`)
        // to the Analytics view.
        if self.show_analytics {
            match action {
                Action::ToggleAnalytics | Action::Escape => self.show_analytics = false,
                Action::ToggleHelp => self.toggle_help(),
                Action::CycleCategory | Action::CycleMode => self.cycle_analytics_filter(),
                Action::ScrollUp => {
                    self.analytics_scroll = self.analytics_scroll.saturating_sub(1);
                }
                Action::ScrollDown => {
                    let max_scroll = self.timer.stats.recent_log.len().saturating_sub(1);
                    self.analytics_scroll = (self.analytics_scroll + 1).min(max_scroll);
                }
                _ => {}
            }
            return;
        }

        // While the full-cycle reward modal is visible, require explicit acknowledgment
        // (`Enter` / `Space`) to transition into the final long break. Unrelated timer
        // keys (`S`, `F`, `B`, `L`, `C`, `+`, `-`, `Esc`, `N`) are blocked so a stray
        // press cannot accidentally dismiss the reward or alter the phase.
        if self.timer.is_reward_pending() {
            match action {
                Action::Confirm | Action::TogglePause => self.acknowledge_reward(),
                Action::Reset => self.reset(),
                Action::SelectMode(index) => self.select_mode(index),
                Action::CycleMode => self.cycle_mode(),
                Action::ToggleHelp => self.toggle_help(),
                Action::ToggleAnalytics => self.toggle_analytics(),
                Action::ToggleMinimal => self.toggle_minimal(),
                _ => {}
            }
            return;
        }

        // While the alarm rings on an intermediate phase or break, only the stop key,
        // display/page toggles and quit are active, so a stray key can't skip or
        // restart the finished phase.
        if self.timer.alarm_ringing {
            match action {
                Action::ToggleHelp => self.toggle_help(),
                Action::ToggleAnalytics => self.toggle_analytics(),
                Action::ToggleMinimal => self.toggle_minimal(),
                _ => {}
            }
            return;
        }

        match action {
            Action::Escape => self.quit(),
            Action::TogglePause => self.toggle_pause(),
            Action::Reset => self.reset(),
            Action::Skip => self.skip(),
            Action::SelectPhase(phase) => self.select_phase(phase),
            Action::ToggleBreak => self.toggle_break(),
            Action::CycleCategory => self.cycle_category(),
            Action::ToggleHelp => self.toggle_help(),
            Action::ToggleAnalytics => self.toggle_analytics(),
            Action::AddTime => self.add_time(),
            Action::SubtractTime => self.subtract_time(),
            Action::ToggleMinimal => self.toggle_minimal(),
            Action::SelectMode(index) => self.select_mode(index),
            Action::CycleMode => self.cycle_mode(),
            Action::Quit
            | Action::StopAlarm
            | Action::Confirm
            | Action::Cancel
            | Action::ScrollUp
            | Action::ScrollDown
            | Action::None => {}
        }
    }

    // ── Timer controls ───────────────────────────────────────────────────────

    pub fn toggle_pause(&mut self) {
        let now = self.sync();
        self.timer.toggle_at(now);
        self.update_status();
    }

    pub fn reset(&mut self) {
        self.sync();
        self.alarm.stop();
        self.timer.reset();
        self.update_status();
    }

    pub fn skip(&mut self) {
        let now = self.sync();
        self.alarm.stop();
        let event = self.timer.skip();
        self.handle_phase_event(event, now);
        self.update_status();
    }

    pub fn select_phase(&mut self, phase: TimerPhase) {
        self.sync();
        self.alarm.stop();
        self.timer.set_phase(phase);
        self.update_status();
    }

    pub fn toggle_break(&mut self) {
        self.sync();
        self.alarm.stop();
        self.timer.toggle_break_phase();
        self.update_status();
    }

    pub fn cycle_category(&mut self) {
        self.sync();
        self.timer.cycle_category();
        self.update_status();
    }

    /// Cycle the Analytics category filter: `All` -> `Coding` -> `Learning` -> `Reading` -> `All`.
    pub fn cycle_analytics_filter(&mut self) {
        self.analytics_filter = match self.analytics_filter {
            None => Some(FocusCategory::Coding),
            Some(FocusCategory::Coding) => Some(FocusCategory::Learning),
            Some(FocusCategory::Learning) => Some(FocusCategory::Reading),
            Some(FocusCategory::Reading) => None,
        };
        self.analytics_scroll = 0;
    }

    /// Advance the timer to `now`. Called every frame; the countdown is based
    /// on the wall clock, so the call frequency does not affect accuracy.
    pub fn tick(&mut self, now: Instant) {
        if let Some(event) = self.timer.tick_at(now) {
            self.start_alarm(event, now);
        }
        if self.timer.alarm_ringing {
            self.alarm.pulse(now);
        }
        self.update_status();
    }

    /// Stop the ringing alarm. For intermediate focus phases and breaks, moves
    /// on to the next phase (paused, waiting for the user). When a full-cycle
    /// reward is pending, silences the chime while keeping the reward modal open.
    pub fn stop_alarm(&mut self) {
        if !self.timer.alarm_ringing {
            return;
        }
        let now = self.sync();
        self.alarm.stop();
        if self.timer.dismiss_alarm().is_some()
            && self.timer.cycle_just_completed
            && !self.timer.is_reward_pending()
        {
            self.celebration_until = Some(now + CELEBRATION_DURATION);
        }
        self.update_status();
    }

    /// Acknowledge/claim the full-cycle reward, silence any active alarm chime,
    /// and transition into the mode's final long break (paused).
    pub fn acknowledge_reward(&mut self) {
        if !self.timer.is_reward_pending() {
            return;
        }
        let now = self.sync();
        self.alarm.stop();
        if self.timer.acknowledge_reward().is_some() {
            self.celebration_until = Some(now + CELEBRATION_DURATION);
        }
        self.update_status();
    }

    pub fn add_time(&mut self) {
        self.sync();
        self.timer.adjust_time(5 * 60);
        self.update_status();
    }

    pub fn subtract_time(&mut self) {
        self.sync();
        self.timer.adjust_time(-(5 * 60));
        self.update_status();
    }

    // ── Mode switching ───────────────────────────────────────────────────────

    pub fn select_mode(&mut self, index: usize) {
        if index >= self.modes.len() || index == self.mode_index {
            return;
        }
        self.sync();
        if self.timer.needs_switch_confirmation() {
            self.pending_mode = Some(index);
        } else {
            self.apply_mode(index);
        }
    }

    pub fn cycle_mode(&mut self) {
        if self.modes.len() > 1 {
            self.select_mode((self.mode_index + 1) % self.modes.len());
        }
    }

    pub fn confirm_mode_switch(&mut self) {
        if let Some(index) = self.pending_mode.take() {
            self.sync();
            self.apply_mode(index);
        }
    }

    pub fn cancel_mode_switch(&mut self) {
        self.pending_mode = None;
    }

    fn apply_mode(&mut self, index: usize) {
        self.alarm.stop();
        self.mode_index = index;
        self.pending_mode = None;
        self.timer.set_mode(self.modes[index]);
        self.update_status();
    }

    // ── Display toggles ──────────────────────────────────────────────────────

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn toggle_analytics(&mut self) {
        self.sync();
        self.show_analytics = !self.show_analytics;
    }

    pub fn toggle_minimal(&mut self) {
        self.minimal_mode = !self.minimal_mode;
    }

    pub fn quit(&mut self) {
        self.alarm.stop();
        self.running = false;
    }

    // ── Private ──────────────────────────────────────────────────────────────

    fn sync(&mut self) -> Instant {
        let now = Instant::now();
        self.tick(now);
        now
    }

    fn start_alarm(&mut self, event: PhaseEvent, now: Instant) {
        self.notification_manager.send_phase_complete(event);
        self.pending_mode = None;
        if event == PhaseEvent::CycleComplete {
            self.show_analytics = false;
        }
        self.alarm.start(now);
    }

    fn handle_phase_event(&mut self, event: PhaseEvent, now: Instant) {
        self.notification_manager.send_phase_complete(event);
        if self.timer.cycle_just_completed {
            self.celebration_until = Some(now + CELEBRATION_DURATION);
        }
    }

    fn update_status(&mut self) {
        self.status_writer.update(&self.timer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::Settings;
    use crate::timer::session::SessionStats;

    fn app() -> App {
        let mut settings = Settings::default();
        settings.notifications.enabled = false;
        settings.alarm.enabled = false;
        settings.integrations.hyprland_status_bar = false;
        let mut a = App::new(Config { settings }).expect("app");
        // Ensure tests never touch disk stats
        a.timer.stats = SessionStats::in_memory();
        a
    }

    fn remaining_mins(app: &App) -> u64 {
        app.timer.current_timer.remaining.as_secs() / 60
    }

    /// Complete the currently active phase in `app` via the clock.
    fn finish_active_phase(app: &mut App, now: &mut Instant) {
        app.timer.toggle_at(*now);
        *now += app.timer.current_timer.remaining;
        app.tick(*now);
        assert!(app.timer.alarm_ringing);
        if !app.timer.is_reward_pending() {
            app.handle_action(Action::StopAlarm);
        }
    }

    #[test]
    fn starts_in_classic_with_classic_durations_and_4_cycles() {
        let app = app();
        assert_eq!(app.mode_index, 0);
        assert_eq!(app.timer.mode.name, "Classic");
        assert_eq!(app.timer.cycles_before_long_break, 4);
        assert_eq!(app.timer.category, FocusCategory::Coding);
        assert_eq!(remaining_mins(&app), 25);
        assert!(!app.timer.is_running());
        assert!(app.pending_mode.is_none());
        assert!(!app.timer.is_reward_pending());
        assert!(!app.show_analytics);
        assert!(app.analytics_filter.is_none());
    }

    #[test]
    fn phase_selection_and_category_cycling_work_on_timer_page() {
        let mut app = app();
        assert_eq!(app.timer.current_phase(), TimerPhase::Focus);
        assert_eq!(app.timer.category, FocusCategory::Coding);

        app.handle_action(Action::CycleCategory);
        assert_eq!(app.timer.category, FocusCategory::Learning);
        app.handle_action(Action::CycleCategory);
        assert_eq!(app.timer.category, FocusCategory::Reading);
        app.handle_action(Action::CycleCategory);
        assert_eq!(app.timer.category, FocusCategory::Coding);

        app.handle_action(Action::ToggleBreak);
        assert_eq!(app.timer.current_phase(), TimerPhase::ShortBreak);
        assert_eq!(remaining_mins(&app), 5);

        app.handle_action(Action::ToggleBreak);
        assert_eq!(app.timer.current_phase(), TimerPhase::LongBreak);
        assert_eq!(remaining_mins(&app), 15);

        app.handle_action(Action::SelectPhase(TimerPhase::Focus));
        assert_eq!(app.timer.current_phase(), TimerPhase::Focus);
        assert_eq!(remaining_mins(&app), 25);
    }

    #[test]
    fn analytics_page_opens_and_closes_and_isolates_timer_keys() {
        let mut app = app();
        app.handle_action(Action::ToggleAnalytics);
        assert!(app.show_analytics);

        // Timer keys while on Analytics page do not start or modify the timer
        app.handle_action(Action::TogglePause);
        app.handle_action(Action::SelectMode(2));
        app.handle_action(Action::ToggleBreak);
        assert!(!app.timer.is_running());
        assert_eq!(app.mode_index, 0);
        assert_eq!(app.timer.current_phase(), TimerPhase::Focus);
        assert_eq!(app.timer.category, FocusCategory::Coding);

        // CycleCategory on the Analytics page cycles the analytics_filter (All -> Coding -> Learning -> Reading -> All)
        assert_eq!(app.analytics_filter, None);
        app.handle_action(Action::CycleCategory);
        assert_eq!(app.analytics_filter, Some(FocusCategory::Coding));
        app.handle_action(Action::CycleCategory);
        assert_eq!(app.analytics_filter, Some(FocusCategory::Learning));
        app.handle_action(Action::CycleCategory);
        assert_eq!(app.analytics_filter, Some(FocusCategory::Reading));
        app.handle_action(Action::CycleCategory);
        assert_eq!(app.analytics_filter, None);
        assert_eq!(app.timer.category, FocusCategory::Coding);

        // Pressing ToggleAnalytics again closes the Analytics page
        app.handle_action(Action::ToggleAnalytics);
        assert!(!app.show_analytics);

        // Escape also closes the Analytics page without quitting the app
        app.handle_action(Action::ToggleAnalytics);
        assert!(app.show_analytics);
        app.handle_action(Action::Escape);
        assert!(!app.show_analytics);
        assert!(app.running);
    }

    #[test]
    fn idle_switch_applies_immediately_without_a_prompt() {
        let mut app = app();
        app.handle_action(Action::SelectMode(1));
        assert_eq!(app.mode_index, 1);
        assert_eq!(app.timer.mode.name, "Session");
        assert_eq!(app.timer.cycles_before_long_break, 3);
        assert_eq!(remaining_mins(&app), 50);
        assert!(app.pending_mode.is_none());

        app.handle_action(Action::SelectMode(2));
        assert_eq!(app.timer.mode.name, "Extended");
        assert_eq!(app.timer.cycles_before_long_break, 2);
        assert_eq!(remaining_mins(&app), 75);

        app.handle_action(Action::SelectMode(0));
        assert_eq!(app.timer.cycles_before_long_break, 4);
        assert_eq!(remaining_mins(&app), 25);
    }

    #[test]
    fn tab_cycles_through_all_modes_and_wraps() {
        let mut app = app();
        let seen: Vec<usize> = (0..4)
            .map(|_| {
                app.handle_action(Action::CycleMode);
                app.mode_index
            })
            .collect();
        assert_eq!(seen, vec![1, 2, 0, 1]);
    }

    #[test]
    fn invalid_or_current_mode_selection_is_ignored() {
        let mut app = app();
        app.handle_action(Action::SelectMode(0)); // already active
        app.handle_action(Action::SelectMode(8)); // does not exist
        assert_eq!(app.mode_index, 0);
        assert!(app.pending_mode.is_none());
    }

    #[test]
    fn switching_while_running_asks_first_and_changes_nothing_yet() {
        let mut app = app();
        app.handle_action(Action::TogglePause); // start
        assert!(app.timer.is_running());

        app.handle_action(Action::SelectMode(2));
        assert_eq!(app.pending_mode, Some(2));
        assert_eq!(app.mode_index, 0, "mode not changed before confirmation");
        assert_eq!(app.timer.mode.name, "Classic");
        assert!(app.timer.is_running(), "timer untouched while asking");
    }

    #[test]
    fn switching_with_completed_cycles_requires_confirmation_and_preserves_stats() {
        let mut app = app();
        let mut now = Instant::now();
        finish_active_phase(&mut app, &mut now); // Focus 1 of 4 completed
        assert_eq!(app.timer.cycle_count, 1);
        assert_eq!(app.timer.stats.sessions_completed, 1);
        assert_eq!(app.timer.current_phase(), TimerPhase::ShortBreak);
        assert!(!app.timer.is_running());

        // Attempting to switch to Session mode while 1 of 4 cycles is completed requires confirmation
        app.handle_action(Action::SelectMode(1));
        assert_eq!(app.pending_mode, Some(1));
        assert_eq!(app.mode_index, 0);

        // Repeated mode keys while modal is open cannot bypass or replace the pending confirmation
        app.handle_action(Action::SelectMode(2));
        app.handle_action(Action::CycleMode);
        assert_eq!(app.pending_mode, Some(1));

        // Esc cancels and preserves cycle_count == 1, ShortBreak, and category
        app.handle_action(Action::Escape);
        assert!(app.pending_mode.is_none());
        assert_eq!(app.mode_index, 0);
        assert_eq!(app.timer.cycle_count, 1);
        assert_eq!(app.timer.current_phase(), TimerPhase::ShortBreak);

        // Confirming switches to Session mode once, resets cycle_count to 0, and preserves recorded stats
        app.handle_action(Action::SelectMode(1));
        app.handle_action(Action::Confirm);
        assert!(app.pending_mode.is_none());
        assert_eq!(app.mode_index, 1);
        assert_eq!(app.timer.mode.name, "Session");
        assert_eq!(app.timer.cycles_before_long_break, 3);
        assert_eq!(app.timer.cycle_count, 0);
        assert_eq!(app.timer.current_phase(), TimerPhase::Focus);
        assert_eq!(remaining_mins(&app), 50);
        assert_eq!(app.timer.stats.sessions_completed, 1);
        assert_eq!(app.timer.stats.today_focus_time, Duration::from_secs(25 * 60));
    }

    #[test]
    fn declining_keeps_the_running_session_intact() {
        let mut app = app();
        app.handle_action(Action::TogglePause);
        app.handle_action(Action::SelectMode(1));

        app.handle_action(Action::Cancel);
        assert!(app.pending_mode.is_none());
        assert_eq!(app.mode_index, 0);
        assert!(app.timer.is_running());
        assert_eq!(app.timer.current_timer.duration, Duration::from_secs(25 * 60));

        // Esc cancels too (and does not quit).
        app.handle_action(Action::SelectMode(1));
        app.handle_action(Action::Escape);
        assert!(app.pending_mode.is_none());
        assert!(app.running);
        assert!(app.timer.is_running());
    }

    #[test]
    fn confirming_switches_mode_and_leaves_a_clean_paused_timer() {
        let mut app = app();
        app.handle_action(Action::TogglePause);
        app.handle_action(Action::SelectMode(2));
        app.handle_action(Action::Confirm);

        assert_eq!(app.mode_index, 2);
        assert_eq!(app.timer.mode.name, "Extended");
        assert!(app.pending_mode.is_none());
        assert!(!app.timer.is_running(), "no timer left running");
        assert_eq!(remaining_mins(&app), 75);
        assert_eq!(app.timer.current_timer.progress(), 0.0);
    }

    #[test]
    fn paused_part_way_also_asks_for_confirmation() {
        let mut app = app();
        app.handle_action(Action::TogglePause);
        app.timer.current_timer.remaining -= Duration::from_secs(120);
        app.handle_action(Action::TogglePause); // pause with progress
        app.handle_action(Action::SelectMode(1));
        assert_eq!(app.pending_mode, Some(1));
    }

    #[test]
    fn dialog_is_modal_other_keys_do_nothing() {
        let mut app = app();
        app.handle_action(Action::TogglePause);
        app.handle_action(Action::SelectMode(1));

        for a in [
            Action::TogglePause,
            Action::Reset,
            Action::Skip,
            Action::ToggleBreak,
            Action::CycleCategory,
            Action::AddTime,
            Action::ToggleMinimal,
            Action::ToggleHelp,
            Action::ToggleAnalytics,
            Action::SelectMode(2),
            Action::CycleMode,
            Action::None,
        ] {
            app.handle_action(a);
        }
        assert_eq!(app.pending_mode, Some(1), "target is unchanged");
        assert!(app.timer.is_running(), "timer untouched");
        assert!(!app.show_help);
        assert!(!app.show_analytics);
        assert!(!app.minimal_mode);
    }

    #[test]
    fn full_cycle_reward_modal_blocks_unrelated_keys_and_starts_long_break_on_confirm() {
        let mut app = app();
        app.handle_action(Action::SelectMode(2)); // Extended mode (2 focus cycles)
        let mut now = Instant::now();

        // Cycle 1: Focus 1 (75m) -> Short Break 1 (15m)
        finish_active_phase(&mut app, &mut now);
        assert!(!app.timer.is_reward_pending());
        finish_active_phase(&mut app, &mut now);

        // Cycle 2: Focus 2 (75m) -> unlocks Full-Cycle Reward
        finish_active_phase(&mut app, &mut now);
        assert!(app.timer.is_reward_pending());
        assert_eq!(app.timer.stats.sessions_completed, 2);

        // Unrelated timer keys do not dismiss the reward or alter the phase
        for a in [
            Action::Skip,
            Action::SelectPhase(TimerPhase::ShortBreak),
            Action::ToggleBreak,
            Action::CycleCategory,
            Action::AddTime,
            Action::SubtractTime,
            Action::Cancel,
            Action::Escape,
        ] {
            app.handle_action(a);
        }
        assert!(app.timer.is_reward_pending(), "reward must still be visible");
        assert!(app.running, "Escape on reward modal must not quit the app");

        // Pressing StopAlarm ('A') silences the alarm chime while keeping the reward modal open
        app.handle_action(Action::StopAlarm);
        assert!(!app.timer.alarm_ringing);
        assert!(app.timer.is_reward_pending());

        // Pressing Enter (Confirm) claims the reward and starts the 45-minute final Long Break
        app.handle_action(Action::Confirm);
        assert!(!app.timer.is_reward_pending());
        assert_eq!(app.timer.current_phase(), TimerPhase::LongBreak);
        assert_eq!(remaining_mins(&app), 45);
        assert!(!app.timer.is_running());
        assert_eq!(app.timer.stats.sessions_completed, 2, "stats not duplicated");
    }

    #[test]
    fn quit_works_even_with_a_dialog_open() {
        let mut app = app();
        app.handle_action(Action::TogglePause);
        app.handle_action(Action::SelectMode(1));
        app.handle_action(Action::Quit);
        assert!(!app.running);
    }

    #[test]
    fn escape_quits_only_when_nothing_is_open() {
        let mut app = app();
        app.handle_action(Action::ToggleHelp);
        app.handle_action(Action::Escape); // closes help
        assert!(app.running);
        assert!(!app.show_help);
        app.handle_action(Action::Escape);
        assert!(!app.running);
    }

    #[test]
    fn any_key_closes_help_without_side_effects() {
        let mut app = app();
        app.handle_action(Action::ToggleHelp);
        assert!(app.show_help);
        app.handle_action(Action::TogglePause);
        assert!(!app.show_help);
        assert!(!app.timer.is_running(), "the key only closed help");
    }

    #[test]
    fn reset_clears_the_need_to_confirm() {
        let mut app = app();
        app.handle_action(Action::TogglePause);
        app.handle_action(Action::Reset);
        app.handle_action(Action::SelectMode(1));
        assert!(app.pending_mode.is_none());
        assert_eq!(app.mode_index, 1);
    }

    #[test]
    fn custom_config_durations_define_classic() {
        let mut settings = Settings::default();
        settings.notifications.enabled = false;
        settings.alarm.enabled = false;
        settings.integrations.hyprland_status_bar = false;
        settings.timer.focus_duration = 30;
        settings.timer.short_break_duration = 6;
        settings.timer.long_break_duration = 20;
        let app = App::new(Config { settings }).unwrap();
        assert_eq!(remaining_mins(&app), 30);
        assert_eq!(app.modes[0].summary(), "30/6/20");
        assert_eq!(app.modes[1].summary(), "50/10/30");
    }

    // ── Alarm ────────────────────────────────────────────────────────────────

    fn ring_alarm(app: &mut App) {
        app.handle_action(Action::TogglePause); // start
        app.timer.current_timer.remaining = Duration::from_millis(10);
        app.tick(Instant::now() + Duration::from_secs(1));
        assert!(app.timer.alarm_ringing);
    }

    #[test]
    fn alarm_rings_at_zero_and_the_timer_waits_at_00_00() {
        let mut app = app();
        ring_alarm(&mut app);
        assert_eq!(app.timer.current_timer.format_time(), "00:00");
        assert_eq!(app.timer.current_phase(), crate::timer::TimerPhase::Focus);
        assert!(app.view(Instant::now()).alarm_ringing);
    }

    #[test]
    fn stop_alarm_key_silences_and_moves_to_the_next_phase() {
        let mut app = app();
        ring_alarm(&mut app);
        app.handle_action(Action::StopAlarm);
        assert!(!app.timer.alarm_ringing);
        assert_eq!(app.timer.current_phase(), crate::timer::TimerPhase::ShortBreak);
        assert!(!app.timer.is_running(), "next phase waits for the user");
        assert!(!app.view(Instant::now()).alarm_ringing);
    }

    #[test]
    fn stop_alarm_key_does_nothing_when_no_alarm_rings() {
        let mut app = app();
        app.handle_action(Action::TogglePause);
        app.handle_action(Action::StopAlarm);
        assert!(app.timer.is_running(), "timer untouched");
        assert_eq!(app.timer.current_phase(), crate::timer::TimerPhase::Focus);
    }

    #[test]
    fn while_ringing_only_stop_display_toggles_and_quit_work() {
        let mut app = app();
        ring_alarm(&mut app);
        for a in [
            Action::TogglePause,
            Action::Reset,
            Action::Skip,
            Action::ToggleBreak,
            Action::CycleCategory,
            Action::AddTime,
            Action::SubtractTime,
            Action::SelectMode(1),
            Action::CycleMode,
            Action::Escape,
            Action::Confirm,
        ] {
            app.handle_action(a);
        }
        assert!(app.running);
        assert!(app.timer.alarm_ringing, "still ringing");
        assert_eq!(app.timer.current_timer.format_time(), "00:00");

        app.handle_action(Action::ToggleMinimal);
        assert!(app.minimal_mode);
        assert!(app.timer.alarm_ringing);

        app.handle_action(Action::Quit);
        assert!(!app.running);
    }

    #[test]
    fn one_press_is_enough_and_the_alarm_stays_off() {
        let mut app = app();
        ring_alarm(&mut app);
        app.handle_action(Action::StopAlarm);
        assert!(!app.timer.alarm_ringing);
        app.tick(Instant::now() + Duration::from_secs(30));
        assert!(!app.timer.alarm_ringing);
        assert!(!app.view(Instant::now()).alarm_ringing);
    }

    #[test]
    fn alarm_can_be_stopped_from_the_help_screen() {
        let mut app = app();
        ring_alarm(&mut app);
        app.handle_action(Action::ToggleHelp);
        assert!(app.show_help);
        app.handle_action(Action::StopAlarm);
        assert!(!app.timer.alarm_ringing);
    }

    #[test]
    fn alarm_rings_after_breaks_as_well_as_focus() {
        let mut app = app();
        ring_alarm(&mut app);
        app.handle_action(Action::StopAlarm); // -> short break (paused)
        ring_alarm(&mut app); // break runs out
        assert_eq!(app.timer.current_phase(), crate::timer::TimerPhase::ShortBreak);
        app.handle_action(Action::StopAlarm);
        assert_eq!(app.timer.current_phase(), crate::timer::TimerPhase::Focus);
    }

    #[test]
    fn a_pending_mode_dialog_is_dropped_when_the_alarm_starts() {
        let mut app = app();
        app.handle_action(Action::TogglePause);
        app.handle_action(Action::SelectMode(1));
        assert_eq!(app.pending_mode, Some(1));
        app.timer.current_timer.remaining = Duration::from_millis(10);
        app.tick(Instant::now() + Duration::from_secs(1));
        assert!(app.timer.alarm_ringing);
        assert!(app.pending_mode.is_none());
    }
}
