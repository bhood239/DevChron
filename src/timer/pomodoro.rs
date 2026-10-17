use super::mode::PomodoroMode;
use super::session::SessionStats;
use super::state::{FocusCategory, Timer, TimerPhase, TimerState};
use std::time::{Duration, Instant};

/// Events emitted by the timer when a phase completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PhaseEvent {
    /// An intermediate focus session just completed.
    FocusComplete,
    /// A short break just completed.
    ShortBreakComplete,
    /// A long break just completed.
    LongBreakComplete,
    /// A full mode sequence (all focus cycles in the mode) just completed.
    CycleComplete,
}

/// Full-cycle reward state unlocked only when every focus cycle in the active
/// mode sequence completes all the way to `00:00`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleReward {
    /// Name of the completed mode ("Classic", "Session", "Extended").
    pub mode_name: &'static str,
    /// Number of focus cycles completed in the sequence (4, 3, or 2).
    pub cycles_completed: u32,
    /// Total focused minutes earned across the sequence (100, 150, or 150).
    pub total_focus_mins: u64,
    /// Final long break minutes waiting to begin upon acknowledgment (15, 30, or 45).
    pub long_break_mins: u64,
    /// Distinct categories worked on during this completed cycle sequence.
    pub categories_used: Vec<FocusCategory>,
    /// Category active on the final completed focus session.
    pub final_category: FocusCategory,
    /// Cumulative full cycles completed all-time.
    pub all_time_cycles: u32,
    /// Today's total focus seconds (including this completed sequence).
    pub today_focus_secs: u64,
    /// Today's completed focus sessions count.
    pub today_sessions: u32,
    /// Current consecutive active-day streak (from recorded history).
    pub current_streak: u32,
    /// True when today's focus time matches or exceeds the past-year daily peak.
    pub is_new_daily_peak: bool,
    /// Instant when the reward was unlocked (drives the reveal animation).
    pub triggered_at: Instant,
}

impl CycleReward {
    /// Duration of the initial terminal-native reveal animation (1.2 seconds).
    pub const ANIMATION_DURATION: Duration = Duration::from_millis(1200);

    /// Normalized reveal animation progress in `0.0..=1.0`.
    pub fn animation_progress(&self, now: Instant) -> f64 {
        let elapsed = now.saturating_duration_since(self.triggered_at).as_secs_f64();
        (elapsed / Self::ANIMATION_DURATION.as_secs_f64()).clamp(0.0, 1.0)
    }

    /// Milliseconds elapsed since the reward was unlocked.
    pub fn elapsed_ms(&self, now: Instant) -> u64 {
        now.saturating_duration_since(self.triggered_at).as_millis() as u64
    }

    /// Human-readable summary of categories worked on during the completed sequence.
    pub fn categories_summary(&self) -> String {
        if self.categories_used.is_empty() {
            self.final_category.label().to_string()
        } else {
            self.categories_used
                .iter()
                .map(|c| c.label())
                .collect::<Vec<_>>()
                .join(" · ")
        }
    }
}

#[derive(Debug)]
pub struct PomodoroTimer {
    pub current_timer: Timer,
    pub cycle_count: u32,
    pub cycles_before_long_break: u32,
    /// The mode whose durations and cycle count are currently in effect.
    pub mode: PomodoroMode,
    /// Active focus category ("Coding", "Learning", "Reading").
    pub category: FocusCategory,
    pub focus_duration: Duration,
    pub short_break_duration: Duration,
    pub long_break_duration: Duration,
    pub stats: SessionStats,
    /// Categories used in completed focus cycles of the current mode sequence.
    pub cycle_categories: Vec<FocusCategory>,
    /// Active full-cycle reward awaiting acknowledgment before the final long break begins.
    pub reward: Option<CycleReward>,
    /// True for one render frame after a full mode cycle completes.
    pub cycle_just_completed: bool,
    /// True from the moment a phase reaches 00:00 until the user stops the
    /// alarm. While it is set the timer sits at 00:00 and does *not* move on to
    /// the next phase; `dismiss_alarm` / `acknowledge_reward` handles that.
    pub alarm_ringing: bool,
}

impl PomodoroTimer {
    pub fn new(mode: PomodoroMode, cycles_before_long_break: u32) -> Self {
        Self::with_stats(mode, cycles_before_long_break, SessionStats::default())
    }

    pub(crate) fn with_stats(
        mode: PomodoroMode,
        cycles_before_long_break: u32,
        stats: SessionStats,
    ) -> Self {
        let effective_cycles = if mode.name == "Classic" && cycles_before_long_break > 0 {
            cycles_before_long_break
        } else {
            mode.focus_cycles.max(1)
        };
        Self {
            current_timer: Timer::new(TimerPhase::Focus, mode.focus()),
            cycle_count: 0,
            cycles_before_long_break: effective_cycles,
            mode: mode.with_cycles(effective_cycles),
            category: FocusCategory::default(),
            focus_duration: mode.focus(),
            short_break_duration: mode.short_break(),
            long_break_duration: mode.long_break(),
            stats,
            cycle_categories: Vec::new(),
            reward: None,
            cycle_just_completed: false,
            alarm_ringing: false,
        }
    }

    /// True while the full-cycle reward modal is awaiting user acknowledgment.
    pub fn is_reward_pending(&self) -> bool {
        self.reward.is_some()
    }

    /// Start or pause the current phase.
    pub fn toggle_at(&mut self, now: Instant) {
        if self.reward.is_some() {
            return;
        }
        match self.current_timer.state {
            TimerState::Running => self.current_timer.pause(),
            TimerState::Paused | TimerState::Completed => {
                self.current_timer.start_at(now);
                if self.stats.session_start.is_none() {
                    self.stats.start_session();
                }
            }
        }
    }

    pub fn reset(&mut self) {
        if self.reward.is_some() {
            // Resetting while the full-cycle reward is visible clears the reward
            // and resets the sequence back to Focus 1 without duplicating or
            // erasing already-recorded session statistics.
            self.reward = None;
            self.alarm_ringing = false;
            self.cycle_just_completed = false;
            self.cycle_count = 0;
            self.cycle_categories.clear();
            self.current_timer = Timer::new(TimerPhase::Focus, self.focus_duration);
            return;
        }

        if self.current_timer.phase == TimerPhase::Focus
            && self.current_timer.has_progress()
            && !self.alarm_ringing
        {
            let elapsed = self
                .focus_duration
                .saturating_sub(self.current_timer.remaining);
            self.stats.record_abandoned_focus_with_category(
                elapsed,
                self.mode.name,
                self.category.label(),
            );
        }
        self.alarm_ringing = false;
        self.current_timer.reset();
    }

    pub fn skip(&mut self) -> PhaseEvent {
        self.advance_to_next_phase()
    }

    /// Directly choose between Focus, Short Break, and Long Break on the timer page.
    /// Leaves the newly selected phase paused at its full length for the current mode.
    pub fn set_phase(&mut self, phase: TimerPhase) {
        if self.reward.is_some() {
            return;
        }
        if self.current_timer.phase == phase
            && !self.current_timer.has_progress()
            && !self.current_timer.is_running()
            && !self.alarm_ringing
        {
            return;
        }

        if self.current_timer.phase == TimerPhase::Focus
            && self.current_timer.has_progress()
            && !self.alarm_ringing
        {
            let elapsed = self
                .focus_duration
                .saturating_sub(self.current_timer.remaining);
            self.stats.record_abandoned_focus_with_category(
                elapsed,
                self.mode.name,
                self.category.label(),
            );
        }

        self.alarm_ringing = false;
        self.cycle_just_completed = false;
        self.current_timer = Timer::new(phase, self.duration_for(phase));
    }

    /// Toggle to Break time: selects Short Break when currently in Focus or Long Break,
    /// or switches to Long Break when already in Short Break.
    pub fn toggle_break_phase(&mut self) {
        if self.reward.is_some() {
            return;
        }
        let target = match self.current_timer.phase {
            TimerPhase::ShortBreak => TimerPhase::LongBreak,
            TimerPhase::Focus | TimerPhase::LongBreak => TimerPhase::ShortBreak,
        };
        self.set_phase(target);
    }

    /// Cycle the active focus category (`Coding` -> `Learning` -> `Reading` -> `Coding`).
    pub fn cycle_category(&mut self) {
        self.category = self.category.cycle_next();
    }

    #[allow(dead_code)]
    pub fn set_category(&mut self, category: FocusCategory) {
        self.category = category;
    }

    /// Account for the time elapsed up to `now`. Returns a `PhaseEvent` if a
    /// phase just reached 00:00, or `None` if nothing finished. Call it as
    /// often as you like – it is idempotent for a given `now`, and a long gap
    /// (e.g. a suspended laptop) completes at most one phase.
    ///
    /// When the final focus cycle in a mode reaches 00:00, the full-cycle
    /// reward is unlocked immediately (`reward.is_some()`) and the final long
    /// break waits until the reward is acknowledged.
    pub fn tick_at(&mut self, now: Instant) -> Option<PhaseEvent> {
        self.cycle_just_completed = false;

        if self.current_timer.tick_at(now) {
            self.alarm_ringing = true;
            if self.current_timer.phase == TimerPhase::Focus
                && self.cycle_count + 1 >= self.cycles_before_long_break
            {
                self.complete_final_focus_and_unlock_reward(now);
                Some(PhaseEvent::CycleComplete)
            } else {
                Some(self.finished_event())
            }
        } else {
            None
        }
    }

    /// Stop the alarm. For intermediate focus phases and breaks, moves on to
    /// the next phase (paused). When the full-cycle reward is active, silences
    /// the alarm while keeping the reward modal visible until acknowledged via
    /// [`acknowledge_reward`](Self::acknowledge_reward).
    pub fn dismiss_alarm(&mut self) -> Option<PhaseEvent> {
        if !self.alarm_ringing {
            return None;
        }
        if self.reward.is_some() {
            self.alarm_ringing = false;
            Some(PhaseEvent::CycleComplete)
        } else {
            Some(self.advance_to_next_phase())
        }
    }

    /// Claim/acknowledge the full-cycle reward and transition into the mode's
    /// final long break (paused at full long-break duration). Returns `None`
    /// if no reward is pending.
    pub fn acknowledge_reward(&mut self) -> Option<CycleReward> {
        let reward = self.reward.take()?;
        self.alarm_ringing = false;
        self.cycle_just_completed = false;
        self.current_timer = Timer::new(TimerPhase::LongBreak, self.long_break_duration);
        Some(reward)
    }

    /// Event describing the phase that is currently in progress / just ended.
    fn finished_event(&self) -> PhaseEvent {
        match self.current_timer.phase {
            TimerPhase::Focus => PhaseEvent::FocusComplete,
            TimerPhase::ShortBreak => PhaseEvent::ShortBreakComplete,
            TimerPhase::LongBreak => PhaseEvent::LongBreakComplete,
        }
    }

    fn complete_final_focus_and_unlock_reward(&mut self, now: Instant) {
        if self.reward.is_some() {
            return;
        }
        let elapsed = self.focus_duration;
        self.stats.record_focus_phase_with_category(
            elapsed,
            self.mode.name,
            self.category.label(),
            true,
        );
        self.stats.record_cycle_complete();
        self.cycle_categories.push(self.category);
        self.cycle_count = self.cycles_before_long_break;
        self.cycle_just_completed = true;

        let mut unique_cats = Vec::new();
        for &cat in &self.cycle_categories {
            if !unique_cats.contains(&cat) {
                unique_cats.push(cat);
            }
        }
        if unique_cats.is_empty() {
            unique_cats.push(self.category);
        }

        let summary = self.stats.analytics_summary(0);
        let today_secs = self.stats.today_focus_time.as_secs();
        let is_new_daily_peak = today_secs > 0 && today_secs >= summary.peak_day_secs;

        self.reward = Some(CycleReward {
            mode_name: self.mode.name,
            cycles_completed: self.cycles_before_long_break,
            total_focus_mins: (self.focus_duration.as_secs() / 60)
                * (self.cycles_before_long_break as u64),
            long_break_mins: self.long_break_duration.as_secs() / 60,
            categories_used: unique_cats,
            final_category: self.category,
            all_time_cycles: self.stats.cycles_completed,
            today_focus_secs: today_secs,
            today_sessions: self.stats.today_sessions,
            current_streak: summary.current_streak,
            is_new_daily_peak,
            triggered_at: now,
        });
    }

    /// Adjust the remaining time of the current phase by `delta_secs` (can be negative).
    /// Clamps to [1, duration].
    pub fn adjust_time(&mut self, delta_secs: i64) {
        if self.reward.is_some() {
            return;
        }
        let remaining = self.current_timer.remaining_secs_ceil() as i64;
        let duration = self.current_timer.duration.as_secs() as i64;
        let new_remaining = (remaining + delta_secs).clamp(1, duration);
        self.current_timer.remaining = Duration::from_secs(new_remaining as u64);
    }

    /// True when switching mode would discard accumulated cycle progression,
    /// interrupt a running or partially completed phase, or dismiss a pending reward.
    pub fn needs_switch_confirmation(&self) -> bool {
        let has_incomplete_cycle_progress =
            self.cycle_count > 0 && self.current_timer.phase != TimerPhase::LongBreak;
        has_incomplete_cycle_progress
            || self.current_timer.is_running()
            || self.current_timer.has_progress()
            || self.reward.is_some()
            || self.alarm_ringing
    }

    /// Switch to `mode`.
    ///
    /// Resets the current mode's unfinished cycle progression (`cycle_count = 0`)
    /// and starts cleanly at Focus cycle 1 of the new mode (paused). Previously
    /// recorded focus sessions in `stats` remain completely intact; if a Focus
    /// phase was abandoned mid-session, it is logged as aborted.
    pub fn set_mode(&mut self, mode: PomodoroMode) {
        if self.current_timer.phase == TimerPhase::Focus
            && self.current_timer.has_progress()
            && !self.alarm_ringing
            && self.reward.is_none()
        {
            let elapsed = self
                .focus_duration
                .saturating_sub(self.current_timer.remaining);
            self.stats.record_abandoned_focus_with_category(
                elapsed,
                self.mode.name,
                self.category.label(),
            );
        }

        self.mode = mode;
        self.cycles_before_long_break = mode.focus_cycles.max(1);
        self.focus_duration = mode.focus();
        self.short_break_duration = mode.short_break();
        self.long_break_duration = mode.long_break();
        self.cycle_count = 0;
        self.cycle_categories.clear();
        self.reward = None;
        self.cycle_just_completed = false;
        self.alarm_ringing = false;
        self.current_timer = Timer::new(TimerPhase::Focus, self.focus_duration);
    }

    pub fn duration_for(&self, phase: TimerPhase) -> Duration {
        match phase {
            TimerPhase::Focus => self.focus_duration,
            TimerPhase::ShortBreak => self.short_break_duration,
            TimerPhase::LongBreak => self.long_break_duration,
        }
    }

    fn advance_to_next_phase(&mut self) -> PhaseEvent {
        if self.reward.is_some() {
            self.acknowledge_reward();
            return PhaseEvent::CycleComplete;
        }

        let reached_zero = self.alarm_ringing || self.current_timer.remaining.is_zero();
        self.alarm_ringing = false;
        match self.current_timer.phase {
            TimerPhase::Focus => {
                let elapsed = self
                    .focus_duration
                    .saturating_sub(self.current_timer.remaining);
                if reached_zero {
                    self.stats.record_focus_phase_with_category(
                        elapsed,
                        self.mode.name,
                        self.category.label(),
                        true,
                    );
                    self.cycle_categories.push(self.category);
                    self.cycle_count += 1;

                    if self.cycle_count >= self.cycles_before_long_break {
                        self.cycle_just_completed = true;
                        self.stats.record_cycle_complete();
                        self.current_timer =
                            Timer::new(TimerPhase::LongBreak, self.long_break_duration);
                        PhaseEvent::CycleComplete
                    } else {
                        self.current_timer =
                            Timer::new(TimerPhase::ShortBreak, self.short_break_duration);
                        PhaseEvent::FocusComplete
                    }
                } else {
                    // A skipped focus phase must never count as a completed focus cycle
                    // or unlock the full-cycle reward.
                    if elapsed > Duration::ZERO {
                        self.stats.record_focus_phase_with_category(
                            elapsed,
                            self.mode.name,
                            self.category.label(),
                            false,
                        );
                    }
                    self.current_timer =
                        Timer::new(TimerPhase::ShortBreak, self.short_break_duration);
                    PhaseEvent::FocusComplete
                }
            }
            TimerPhase::ShortBreak => {
                self.stats.record_break_phase(
                    "SHORT BRK",
                    self.short_break_duration
                        .saturating_sub(self.current_timer.remaining),
                    self.mode.name,
                );
                self.current_timer = Timer::new(TimerPhase::Focus, self.focus_duration);
                PhaseEvent::ShortBreakComplete
            }
            TimerPhase::LongBreak => {
                self.stats.record_break_phase(
                    "LONG BRK",
                    self.long_break_duration
                        .saturating_sub(self.current_timer.remaining),
                    self.mode.name,
                );
                // Reset cycle progression after the final long break finishes so
                // the next focus session begins at Focus cycle 1.
                self.cycle_count = 0;
                self.cycle_categories.clear();
                self.current_timer = Timer::new(TimerPhase::Focus, self.focus_duration);
                PhaseEvent::LongBreakComplete
            }
        }
    }

    pub fn current_phase(&self) -> TimerPhase {
        self.current_timer.phase
    }

    pub fn is_running(&self) -> bool {
        self.current_timer.is_running()
    }

    /// 1-based focus cycle number for display (`1..=cycles_before_long_break`).
    pub fn display_cycle_number(&self) -> u32 {
        (self.cycle_count + 1).min(self.cycles_before_long_break.max(1))
    }

    /// Number of focus cycles completed in the current unfinished sequence.
    pub fn completed_cycles_in_sequence(&self) -> u32 {
        self.cycle_count.min(self.cycles_before_long_break)
    }

    pub fn session_info(&self) -> String {
        format!(
            "{}/{}",
            self.display_cycle_number(),
            self.cycles_before_long_break
        )
    }

    /// Seconds of focus work elapsed in the currently active/paused Focus phase
    /// (0 during breaks, while a full-cycle reward is pending, or when unstarted).
    pub fn live_focus_secs(&self) -> u64 {
        if self.current_timer.phase == TimerPhase::Focus && self.reward.is_none() {
            self.focus_duration
                .saturating_sub(self.current_timer.remaining)
                .as_secs()
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::mode::{DEFAULT_MODE, MODES};

    const MIN: Duration = Duration::from_secs(60);

    fn timer(mode: PomodoroMode) -> PomodoroTimer {
        // `in_memory` stats: the tests must never touch the real stats file.
        PomodoroTimer::with_stats(mode, mode.focus_cycles, SessionStats::in_memory())
    }

    /// Run an intermediate focus or break phase to completion and dismiss the alarm.
    fn finish_phase(t: &mut PomodoroTimer, now: &mut Instant) -> PhaseEvent {
        t.toggle_at(*now);
        *now += t.current_timer.remaining;
        let rang = t.tick_at(*now).expect("phase should complete");
        assert!(t.alarm_ringing, "alarm must ring at 00:00");
        let advanced = t.dismiss_alarm().expect("alarm was ringing");
        assert_eq!(rang, advanced);
        advanced
    }

    #[test]
    fn new_timer_starts_paused_on_a_full_focus_phase_of_the_given_mode() {
        for mode in MODES {
            let t = timer(*mode);
            assert_eq!(t.current_phase(), TimerPhase::Focus);
            assert_eq!(t.category, FocusCategory::Coding);
            assert_eq!(t.current_timer.remaining, mode.focus());
            assert_eq!(t.current_timer.duration, mode.focus());
            assert_eq!(t.cycles_before_long_break, mode.focus_cycles);
            assert!(!t.is_running());
            assert!(!t.is_reward_pending());
            assert_eq!(t.current_timer.progress(), 0.0);
        }
    }

    #[test]
    fn choosing_phase_switches_between_focus_short_break_and_long_break() {
        let mut t = timer(MODES[0]);
        assert_eq!(t.current_phase(), TimerPhase::Focus);

        t.toggle_break_phase();
        assert_eq!(t.current_phase(), TimerPhase::ShortBreak);
        assert_eq!(t.current_timer.remaining, 5 * MIN);
        assert!(!t.is_running());

        t.toggle_break_phase();
        assert_eq!(t.current_phase(), TimerPhase::LongBreak);
        assert_eq!(t.current_timer.remaining, 15 * MIN);

        t.set_phase(TimerPhase::Focus);
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.current_timer.remaining, 25 * MIN);
    }

    #[test]
    fn completed_focus_records_selected_category_in_stats() {
        let mut t = timer(MODES[0]);
        let mut now = Instant::now();
        t.cycle_category(); // Coding -> Learning
        assert_eq!(t.category, FocusCategory::Learning);
        finish_phase(&mut t, &mut now);
        assert_eq!(
            t.stats.category_focus_secs.get("Learning"),
            Some(&(25 * 60))
        );
        assert_eq!(t.stats.category_sessions.get("Learning"), Some(&1));
        assert_eq!(t.stats.recent_log.last().unwrap().category, "Learning");
    }

    #[test]
    fn default_mode_is_classic_25_5_15_with_4_cycles() {
        let t = timer(MODES[DEFAULT_MODE]);
        assert_eq!(t.mode.name, "Classic");
        assert_eq!(t.focus_duration, 25 * MIN);
        assert_eq!(t.short_break_duration, 5 * MIN);
        assert_eq!(t.long_break_duration, 15 * MIN);
        assert_eq!(t.cycles_before_long_break, 4);
    }

    #[test]
    fn classic_mode_follows_25_5_25_5_25_5_25_reward_15_schedule() {
        let mut t = timer(MODES[0]); // Classic: 4 focus cycles, 100m total
        let mut now = Instant::now();

        for cycle in 1..=3 {
            assert_eq!(t.current_phase(), TimerPhase::Focus);
            assert_eq!(t.current_timer.duration, 25 * MIN);
            assert_eq!(t.cycle_count, cycle - 1);
            assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::FocusComplete);
            assert!(!t.is_reward_pending(), "intermediate cycle {cycle} must not trigger reward");

            assert_eq!(t.current_phase(), TimerPhase::ShortBreak);
            assert_eq!(t.current_timer.duration, 5 * MIN);
            assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::ShortBreakComplete);
        }

        // Focus 4 (final focus cycle in Classic)
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.current_timer.duration, 25 * MIN);
        assert_eq!(t.cycle_count, 3);
        assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::CycleComplete);

        // Reward is unlocked before the final long break starts
        assert!(t.is_reward_pending(), "Classic reward must unlock after 4th focus");
        let reward = t.reward.as_ref().unwrap();
        assert_eq!(reward.mode_name, "Classic");
        assert_eq!(reward.cycles_completed, 4);
        assert_eq!(reward.total_focus_mins, 100);
        assert_eq!(reward.long_break_mins, 15);
        assert_eq!(t.stats.sessions_completed, 4);
        assert_eq!(t.stats.today_focus_time, 100 * MIN);
        assert_eq!(t.stats.cycles_completed, 1);

        // Acknowledging the reward transitions to the 15-minute final long break
        assert!(t.acknowledge_reward().is_some());
        assert!(!t.is_reward_pending());
        assert_eq!(t.current_phase(), TimerPhase::LongBreak);
        assert_eq!(t.current_timer.duration, 15 * MIN);
        assert_eq!(t.current_timer.remaining, 15 * MIN);
        assert!(!t.is_running());

        // Completing the final long break resets cycle progression to Focus 1
        assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::LongBreakComplete);
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.current_timer.remaining, 25 * MIN);
        assert_eq!(t.cycle_count, 0);
        assert_eq!(t.session_info(), "1/4");
        // Stats were not duplicated when acknowledging reward or finishing break
        assert_eq!(t.stats.sessions_completed, 4);
        assert_eq!(t.stats.today_focus_time, 100 * MIN);
    }

    #[test]
    fn session_mode_follows_50_10_50_10_50_reward_30_schedule() {
        let mut t = timer(MODES[1]); // Session: 3 focus cycles, 150m total
        let mut now = Instant::now();
        assert_eq!(t.cycles_before_long_break, 3);

        for cycle in 1..=2 {
            assert_eq!(t.current_phase(), TimerPhase::Focus);
            assert_eq!(t.current_timer.duration, 50 * MIN);
            assert_eq!(t.cycle_count, cycle - 1);
            assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::FocusComplete);
            assert!(!t.is_reward_pending(), "intermediate cycle {cycle} must not trigger reward");

            assert_eq!(t.current_phase(), TimerPhase::ShortBreak);
            assert_eq!(t.current_timer.duration, 10 * MIN);
            assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::ShortBreakComplete);
        }

        // Focus 3 (final focus cycle in Session)
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.current_timer.duration, 50 * MIN);
        assert_eq!(t.cycle_count, 2);
        assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::CycleComplete);

        assert!(t.is_reward_pending(), "Session reward must unlock after 3rd focus");
        let reward = t.reward.as_ref().unwrap();
        assert_eq!(reward.mode_name, "Session");
        assert_eq!(reward.cycles_completed, 3);
        assert_eq!(reward.total_focus_mins, 150);
        assert_eq!(reward.long_break_mins, 30);
        assert_eq!(t.stats.sessions_completed, 3);
        assert_eq!(t.stats.today_focus_time, 150 * MIN);

        assert!(t.acknowledge_reward().is_some());
        assert_eq!(t.current_phase(), TimerPhase::LongBreak);
        assert_eq!(t.current_timer.duration, 30 * MIN);

        assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::LongBreakComplete);
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.cycle_count, 0);
        assert_eq!(t.session_info(), "1/3");
    }

    #[test]
    fn extended_mode_follows_75_15_75_reward_45_schedule() {
        let mut t = timer(MODES[2]); // Extended: 2 focus cycles, 150m total
        let mut now = Instant::now();
        assert_eq!(t.cycles_before_long_break, 2);

        // Focus 1 -> Short Break 1
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.current_timer.duration, 75 * MIN);
        assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::FocusComplete);
        assert!(!t.is_reward_pending(), "cycle 1 must not trigger reward");

        assert_eq!(t.current_phase(), TimerPhase::ShortBreak);
        assert_eq!(t.current_timer.duration, 15 * MIN);
        assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::ShortBreakComplete);

        // Focus 2 (final focus cycle in Extended)
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.current_timer.duration, 75 * MIN);
        assert_eq!(t.cycle_count, 1);
        assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::CycleComplete);

        assert!(t.is_reward_pending(), "Extended reward must unlock after 2nd focus");
        let reward = t.reward.as_ref().unwrap();
        assert_eq!(reward.mode_name, "Extended");
        assert_eq!(reward.cycles_completed, 2);
        assert_eq!(reward.total_focus_mins, 150);
        assert_eq!(reward.long_break_mins, 45);
        assert_eq!(t.stats.sessions_completed, 2);
        assert_eq!(t.stats.today_focus_time, 150 * MIN);

        assert!(t.acknowledge_reward().is_some());
        assert_eq!(t.current_phase(), TimerPhase::LongBreak);
        assert_eq!(t.current_timer.duration, 45 * MIN);

        assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::LongBreakComplete);
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.cycle_count, 0);
        assert_eq!(t.session_info(), "1/2");
    }

    #[test]
    fn skipped_or_reset_focus_phase_never_counts_as_completed_cycle_or_triggers_reward() {
        let mut t = timer(MODES[2]); // Extended (2 cycles)
        let t0 = Instant::now();

        // Start Focus 1, run 20 minutes, then skip
        t.toggle_at(t0);
        t.tick_at(t0 + 20 * MIN);
        t.skip();
        assert_eq!(t.current_phase(), TimerPhase::ShortBreak);
        assert_eq!(t.cycle_count, 0, "skipped focus must not increment cycle_count");
        assert_eq!(t.stats.sessions_completed, 0);
        assert_eq!(t.stats.interrupted_focus_count, 1);
        assert!(!t.is_reward_pending());

        // Skip short break back to Focus 1, run 10 minutes, then reset
        t.skip();
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        t.toggle_at(t0 + 25 * MIN);
        t.tick_at(t0 + 35 * MIN);
        t.reset();
        assert_eq!(t.cycle_count, 0, "reset focus must not increment cycle_count");
        assert_eq!(t.stats.sessions_completed, 0);
        assert_eq!(t.stats.interrupted_focus_count, 2);
        assert!(!t.is_reward_pending());
    }

    #[test]
    fn reward_triggers_once_and_resetting_while_reward_visible_preserves_stats() {
        let mut t = timer(MODES[2]); // Extended (2 cycles)
        let mut now = Instant::now();

        finish_phase(&mut t, &mut now); // Focus 1
        finish_phase(&mut t, &mut now); // Short Break 1

        // Complete Focus 2
        t.toggle_at(now);
        now += 75 * MIN;
        assert_eq!(t.tick_at(now), Some(PhaseEvent::CycleComplete));
        assert!(t.is_reward_pending());
        assert_eq!(t.stats.sessions_completed, 2);
        assert_eq!(t.stats.cycles_completed, 1);

        // Additional ticks while reward is visible never re-trigger or duplicate stats
        assert_eq!(t.tick_at(now + Duration::from_secs(10)), None);
        assert_eq!(t.stats.sessions_completed, 2);
        assert_eq!(t.stats.cycles_completed, 1);

        // Resetting while the reward is visible clears the reward and resets cycle_count to 0
        // while keeping the 2 completed sessions and 1 completed cycle in stats
        t.reset();
        assert!(!t.is_reward_pending());
        assert!(!t.alarm_ringing);
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.cycle_count, 0);
        assert_eq!(t.stats.sessions_completed, 2);
        assert_eq!(t.stats.cycles_completed, 1);
    }

    #[test]
    fn new_phase_starts_paused_with_a_reset_ring() {
        let mut t = timer(MODES[0]);
        let mut now = Instant::now();
        finish_phase(&mut t, &mut now);
        assert!(!t.is_running(), "next phase waits for the user, as before");
        assert_eq!(t.current_timer.progress(), 0.0);
        // Time passing while the next phase is paused changes nothing.
        t.tick_at(now + 10 * MIN);
        assert_eq!(t.current_timer.remaining, MODES[0].short_break());
    }

    #[test]
    fn skip_advances_with_the_modes_durations() {
        let mut t = timer(MODES[1]);
        t.skip();
        assert_eq!(t.current_phase(), TimerPhase::ShortBreak);
        assert_eq!(t.current_timer.duration, 10 * MIN);
    }

    #[test]
    fn start_pause_resume_reset() {
        let mut t = timer(MODES[0]);
        let t0 = Instant::now();

        t.toggle_at(t0); // start
        assert!(t.is_running());
        t.tick_at(t0 + Duration::from_secs(30));
        t.toggle_at(t0 + Duration::from_secs(30)); // pause
        assert!(!t.is_running());
        let frozen = t.current_timer.remaining;
        let frozen_progress = t.current_timer.progress();

        t.tick_at(t0 + 20 * MIN); // time passes while paused
        assert_eq!(t.current_timer.remaining, frozen);
        assert_eq!(t.current_timer.progress(), frozen_progress);

        let resume = t0 + 20 * MIN;
        t.toggle_at(resume); // resume
        t.tick_at(resume + Duration::from_secs(10));
        assert_eq!(t.current_timer.remaining, frozen - Duration::from_secs(10));

        t.reset();
        assert!(!t.is_running());
        assert_eq!(t.current_timer.remaining, 25 * MIN);
        assert_eq!(t.current_timer.progress(), 0.0);
    }

    #[test]
    fn a_long_stall_advances_exactly_one_phase() {
        let mut t = timer(MODES[0]);
        let t0 = Instant::now();
        t.toggle_at(t0);
        let ev = t.tick_at(t0 + Duration::from_secs(12 * 3600));
        assert_eq!(ev, Some(PhaseEvent::FocusComplete));
        // The alarm rings at 00:00; the phase does not advance by itself.
        assert!(t.alarm_ringing);
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.current_timer.format_time(), "00:00");
        assert_eq!(t.tick_at(t0 + Duration::from_secs(12 * 3600 + 1)), None);
        assert!(t.alarm_ringing, "keeps ringing until stopped");

        assert_eq!(t.dismiss_alarm(), Some(PhaseEvent::FocusComplete));
        assert_eq!(t.current_phase(), TimerPhase::ShortBreak);
        assert!(!t.alarm_ringing);
        assert_eq!(t.dismiss_alarm(), None, "nothing left to dismiss");
    }

    #[test]
    fn alarm_rings_at_the_end_of_breaks_too() {
        let mut t = timer(MODES[0]);
        let mut now = Instant::now();
        finish_phase(&mut t, &mut now); // focus -> short break (asserts ringing)
        assert_eq!(t.current_phase(), TimerPhase::ShortBreak);

        t.toggle_at(now);
        now += t.current_timer.remaining;
        assert_eq!(t.tick_at(now), Some(PhaseEvent::ShortBreakComplete));
        assert!(t.alarm_ringing);
        assert_eq!(t.current_phase(), TimerPhase::ShortBreak, "still at 00:00");
        assert_eq!(t.current_timer.format_time(), "00:00");
        assert_eq!(t.dismiss_alarm(), Some(PhaseEvent::ShortBreakComplete));
        assert_eq!(t.current_phase(), TimerPhase::Focus);
    }

    #[test]
    fn reset_skip_and_mode_switch_silence_the_alarm() {
        for action in 0..3 {
            let mut t = timer(MODES[0]);
            let t0 = Instant::now();
            t.toggle_at(t0);
            t.tick_at(t0 + 30 * MIN);
            assert!(t.alarm_ringing);
            match action {
                0 => t.reset(),
                1 => {
                    t.skip();
                }
                _ => t.set_mode(MODES[1]),
            }
            assert!(!t.alarm_ringing);
        }
    }

    #[test]
    fn adjust_time_is_clamped_to_one_second_and_the_phase_length() {
        let mut t = timer(MODES[0]);
        t.adjust_time(-(60 * 60));
        assert_eq!(t.current_timer.remaining, Duration::from_secs(1));
        t.adjust_time(60 * 60);
        assert_eq!(t.current_timer.remaining, 25 * MIN);
    }

    // ── Mode switching ──────────────────────────────────────────────────────

    #[test]
    fn idle_timer_does_not_need_confirmation() {
        let t = timer(MODES[0]);
        assert!(!t.needs_switch_confirmation());
    }

    #[test]
    fn running_part_way_or_completed_cycles_need_confirmation() {
        let mut t = timer(MODES[0]);
        let mut now = Instant::now();
        t.toggle_at(now);
        assert!(t.needs_switch_confirmation(), "running");

        t.tick_at(now + Duration::from_secs(5));
        t.toggle_at(now + Duration::from_secs(5)); // pause part-way
        assert!(t.needs_switch_confirmation(), "paused with progress");

        t.reset();
        assert!(!t.needs_switch_confirmation(), "reset at cycle 0 clears it");

        // Completing 1 focus cycle means cycle_count == 1, which requires confirmation
        finish_phase(&mut t, &mut now);
        assert_eq!(t.cycle_count, 1);
        assert!(
            t.needs_switch_confirmation(),
            "completed cycles in sequence must require confirmation"
        );
    }

    #[test]
    fn switching_mode_updates_durations_cycles_and_resets_sequence_to_focus_one() {
        let mut t = timer(MODES[0]);
        t.set_mode(MODES[1]);
        assert_eq!(t.mode.name, "Session");
        assert_eq!(t.cycles_before_long_break, 3);
        assert_eq!(t.focus_duration, 50 * MIN);
        assert_eq!(t.short_break_duration, 10 * MIN);
        assert_eq!(t.long_break_duration, 30 * MIN);
        assert_eq!(t.current_timer.duration, 50 * MIN);
        assert_eq!(t.current_timer.remaining, 50 * MIN);
        assert_eq!(t.current_timer.progress(), 0.0);

        t.set_mode(MODES[2]);
        assert_eq!(t.cycles_before_long_break, 2);
        assert_eq!(t.current_timer.remaining, 75 * MIN);
        t.set_mode(MODES[0]);
        assert_eq!(t.cycles_before_long_break, 4);
        assert_eq!(t.current_timer.remaining, 25 * MIN);
    }

    #[test]
    fn switching_while_running_leaves_no_background_timer() {
        let mut t = timer(MODES[0]);
        let t0 = Instant::now();
        t.toggle_at(t0);
        t.tick_at(t0 + 10 * MIN);
        assert!(t.current_timer.progress() > 0.3);

        t.set_mode(MODES[2]);
        assert!(!t.is_running(), "switching leaves the timer paused");
        assert_eq!(t.current_timer.progress(), 0.0, "ring must not be stale");
        assert_eq!(t.current_timer.remaining, 75 * MIN);

        // Even an hour later, nothing has moved.
        assert_eq!(t.tick_at(t0 + 70 * MIN), None);
        assert_eq!(t.current_timer.remaining, 75 * MIN);

        // And the timer works normally afterwards.
        let go = t0 + 70 * MIN;
        t.toggle_at(go);
        t.tick_at(go + Duration::from_secs(60));
        assert_eq!(t.current_timer.remaining, 74 * MIN);
    }

    #[test]
    fn switching_after_completed_cycles_resets_sequence_progression_but_preserves_recorded_stats() {
        let mut t = timer(MODES[0]);
        let mut now = Instant::now();
        finish_phase(&mut t, &mut now); // Focus 1 completed (25m recorded)
        assert_eq!(t.cycle_count, 1);
        assert_eq!(t.stats.sessions_completed, 1);
        assert_eq!(t.stats.today_focus_time, 25 * MIN);

        t.set_mode(MODES[1]); // Switch to Session mode (3 cycles)
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.current_timer.duration, 50 * MIN);
        assert_eq!(t.cycle_count, 0, "mode switch resets current mode cycle progression");
        assert_eq!(t.cycles_before_long_break, 3);
        assert_eq!(t.stats.sessions_completed, 1, "recorded focus sessions are preserved");
        assert_eq!(t.stats.today_focus_time, 25 * MIN, "recorded focus time is preserved");
    }

    #[test]
    fn abandoned_phase_is_not_recorded_as_completed_in_stats() {
        let mut t = timer(MODES[0]);
        let t0 = Instant::now();
        t.toggle_at(t0);
        t.tick_at(t0 + 10 * MIN);
        t.set_mode(MODES[1]);
        assert_eq!(t.stats.sessions_completed, 0);
        assert_eq!(t.stats.today_focus_time, Duration::ZERO);
        assert_eq!(t.stats.interrupted_focus_count, 1);
    }

    #[test]
    fn completed_focus_records_its_full_length_in_stats() {
        let mut t = timer(MODES[1]);
        let mut now = Instant::now();
        finish_phase(&mut t, &mut now);
        assert_eq!(t.stats.sessions_completed, 1);
        assert_eq!(t.stats.today_focus_time, 50 * MIN);
        assert_eq!(t.stats.completed_focus_count, 1);
        assert_eq!(t.stats.mode_sessions.get("Session"), Some(&1));
    }
}
