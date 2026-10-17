use super::mode::PomodoroMode;
use super::session::SessionStats;
use super::state::{FocusCategory, Timer, TimerPhase, TimerState};
use std::time::{Duration, Instant};

/// Events emitted by the timer when a phase completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PhaseEvent {
    /// A focus session just completed.
    FocusComplete,
    /// A short break just completed.
    ShortBreakComplete,
    /// A long break just completed.
    LongBreakComplete,
    /// A full cycle (N focus sessions + long break) just completed.
    CycleComplete,
}

#[derive(Debug)]
pub struct PomodoroTimer {
    pub current_timer: Timer,
    pub cycle_count: u32,
    pub cycles_before_long_break: u32,
    /// The mode whose durations are currently in effect.
    pub mode: PomodoroMode,
    /// Active focus category ("Coding", "Learning", "Reading").
    pub category: FocusCategory,
    pub focus_duration: Duration,
    pub short_break_duration: Duration,
    pub long_break_duration: Duration,
    pub stats: SessionStats,
    /// True for exactly one render frame after a cycle completes.
    pub cycle_just_completed: bool,
    /// True from the moment a phase reaches 00:00 until the user stops the
    /// alarm. While it is set the timer sits at 00:00 and does *not* move on to
    /// the next phase; `dismiss_alarm` does that.
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
        Self {
            current_timer: Timer::new(TimerPhase::Focus, mode.focus()),
            cycle_count: 0,
            cycles_before_long_break,
            mode,
            category: FocusCategory::default(),
            focus_duration: mode.focus(),
            short_break_duration: mode.short_break(),
            long_break_duration: mode.long_break(),
            stats,
            cycle_just_completed: false,
            alarm_ringing: false,
        }
    }

    /// Start or pause the current phase.
    pub fn toggle_at(&mut self, now: Instant) {
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
    /// Reaching 00:00 (focus session *or* break) starts the alarm and leaves
    /// the timer at 00:00 until [`dismiss_alarm`](Self::dismiss_alarm) is called.
    pub fn tick_at(&mut self, now: Instant) -> Option<PhaseEvent> {
        // Clear the one-frame cycle flag each tick.
        self.cycle_just_completed = false;

        if self.current_timer.tick_at(now) {
            self.alarm_ringing = true;
            Some(self.finished_event())
        } else {
            None
        }
    }

    /// Stop the alarm and move on to the next phase (left paused, waiting for
    /// the user). Returns `None` when no alarm is ringing.
    pub fn dismiss_alarm(&mut self) -> Option<PhaseEvent> {
        if self.alarm_ringing {
            Some(self.advance_to_next_phase())
        } else {
            None
        }
    }

    /// Event describing the phase that is currently in progress / just ended.
    fn finished_event(&self) -> PhaseEvent {
        match self.current_timer.phase {
            TimerPhase::Focus => PhaseEvent::FocusComplete,
            TimerPhase::ShortBreak => PhaseEvent::ShortBreakComplete,
            TimerPhase::LongBreak => PhaseEvent::LongBreakComplete,
        }
    }

    /// Adjust the remaining time of the current phase by `delta_secs` (can be negative).
    /// Clamps to [1, duration].
    pub fn adjust_time(&mut self, delta_secs: i64) {
        let remaining = self.current_timer.remaining_secs_ceil() as i64;
        let duration = self.current_timer.duration.as_secs() as i64;
        let new_remaining = (remaining + delta_secs).clamp(1, duration);
        self.current_timer.remaining = Duration::from_secs(new_remaining as u64);
    }

    /// True when switching mode would throw away work: the phase is running,
    /// or paused part-way through. The UI asks for confirmation in that case.
    pub fn needs_switch_confirmation(&self) -> bool {
        self.current_timer.is_running() || self.current_timer.has_progress()
    }

    /// Switch to `mode`.
    ///
    /// Predictable semantics: the *kind* of phase (focus / short break / long
    /// break) and the position inside the cycle are kept, but the phase is
    /// restarted at the new mode's full length and left **paused**. Nothing
    /// keeps running in the background and the ring shows 0 % of the new
    /// duration. Stats are untouched (an abandoned phase is not recorded).
    pub fn set_mode(&mut self, mode: PomodoroMode) {
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

        self.mode = mode;
        self.focus_duration = mode.focus();
        self.short_break_duration = mode.short_break();
        self.long_break_duration = mode.long_break();

        let phase = self.current_timer.phase;
        self.current_timer = Timer::new(phase, self.duration_for(phase));
        self.cycle_just_completed = false;
        self.alarm_ringing = false;
    }

    pub fn duration_for(&self, phase: TimerPhase) -> Duration {
        match phase {
            TimerPhase::Focus => self.focus_duration,
            TimerPhase::ShortBreak => self.short_break_duration,
            TimerPhase::LongBreak => self.long_break_duration,
        }
    }

    fn advance_to_next_phase(&mut self) -> PhaseEvent {
        let reached_zero = self.alarm_ringing || self.current_timer.remaining.is_zero();
        self.alarm_ringing = false;
        match self.current_timer.phase {
            TimerPhase::Focus => {
                let elapsed = self
                    .focus_duration
                    .saturating_sub(self.current_timer.remaining);
                self.stats.record_focus_phase_with_category(
                    elapsed,
                    self.mode.name,
                    self.category.label(),
                    reached_zero,
                );
                self.cycle_count += 1;

                if self.cycle_count >= self.cycles_before_long_break {
                    self.cycle_count = 0;
                    self.cycle_just_completed = true;
                    self.stats.record_cycle_complete();
                    self.current_timer =
                        Timer::new(TimerPhase::LongBreak, self.long_break_duration);
                    PhaseEvent::FocusComplete
                } else {
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

    pub fn session_info(&self) -> String {
        format!("{}/{}", self.cycle_count + 1, self.cycles_before_long_break)
    }

    /// Seconds of focus work elapsed in the currently active/paused Focus phase
    /// (0 during breaks or when the Focus phase has not started).
    pub fn live_focus_secs(&self) -> u64 {
        if self.current_timer.phase == TimerPhase::Focus {
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
        PomodoroTimer::with_stats(mode, 4, SessionStats::in_memory())
    }

    /// Run the current phase to completion and return the event.
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
            assert!(!t.is_running());
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
    fn default_mode_is_classic_25_5_15() {
        let t = timer(MODES[DEFAULT_MODE]);
        assert_eq!(t.mode.name, "Classic");
        assert_eq!(t.focus_duration, 25 * MIN);
        assert_eq!(t.short_break_duration, 5 * MIN);
        assert_eq!(t.long_break_duration, 15 * MIN);
    }

    #[test]
    fn every_mode_uses_its_own_durations_for_all_three_phases() {
        for mode in MODES {
            let mut t = timer(*mode);
            let mut now = Instant::now();

            // Focus -> short break (cycles 1..3), then the long break after the 4th focus.
            for cycle in 1..=4 {
                assert_eq!(t.current_timer.phase, TimerPhase::Focus);
                assert_eq!(t.current_timer.duration, mode.focus(), "{} focus", mode.name);
                assert_eq!(finish_phase(&mut t, &mut now), PhaseEvent::FocusComplete);

                if cycle < 4 {
                    assert_eq!(t.current_timer.phase, TimerPhase::ShortBreak);
                    assert_eq!(t.current_timer.duration, mode.short_break());
                    assert_eq!(t.current_timer.remaining, mode.short_break());
                    assert_eq!(
                        finish_phase(&mut t, &mut now),
                        PhaseEvent::ShortBreakComplete
                    );
                } else {
                    assert_eq!(t.current_timer.phase, TimerPhase::LongBreak);
                    assert_eq!(t.current_timer.duration, mode.long_break());
                    assert_eq!(t.current_timer.remaining, mode.long_break());
                    assert!(t.cycle_just_completed);
                    assert_eq!(
                        finish_phase(&mut t, &mut now),
                        PhaseEvent::LongBreakComplete
                    );
                }
            }
            // ...and the next cycle starts over with a full focus phase.
            assert_eq!(t.current_timer.phase, TimerPhase::Focus);
            assert_eq!(t.current_timer.remaining, mode.focus());
            assert_eq!(t.cycle_count, 0);
        }
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
    fn running_or_part_way_timer_needs_confirmation() {
        let mut t = timer(MODES[0]);
        let t0 = Instant::now();
        t.toggle_at(t0);
        assert!(t.needs_switch_confirmation(), "running");

        t.tick_at(t0 + Duration::from_secs(5));
        t.toggle_at(t0 + Duration::from_secs(5)); // pause part-way
        assert!(t.needs_switch_confirmation(), "paused with progress");

        t.reset();
        assert!(!t.needs_switch_confirmation(), "reset clears it");
    }

    #[test]
    fn switching_mode_updates_all_three_durations_and_the_current_phase() {
        let mut t = timer(MODES[0]);
        t.set_mode(MODES[1]);
        assert_eq!(t.mode.name, "Session");
        assert_eq!(t.focus_duration, 50 * MIN);
        assert_eq!(t.short_break_duration, 10 * MIN);
        assert_eq!(t.long_break_duration, 30 * MIN);
        assert_eq!(t.current_timer.duration, 50 * MIN);
        assert_eq!(t.current_timer.remaining, 50 * MIN);
        assert_eq!(t.current_timer.progress(), 0.0);

        t.set_mode(MODES[2]);
        assert_eq!(t.current_timer.remaining, 75 * MIN);
        t.set_mode(MODES[0]);
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
    fn switching_during_a_break_keeps_the_phase_type_and_cycle_position() {
        let mut t = timer(MODES[0]);
        let mut now = Instant::now();
        finish_phase(&mut t, &mut now); // -> short break, cycle_count == 1
        assert_eq!(t.current_phase(), TimerPhase::ShortBreak);

        t.set_mode(MODES[2]);
        assert_eq!(t.current_phase(), TimerPhase::ShortBreak);
        assert_eq!(t.current_timer.duration, 15 * MIN);
        assert_eq!(t.cycle_count, 1);

        // Completing the break returns to a focus phase of the *new* mode.
        finish_phase(&mut t, &mut now);
        assert_eq!(t.current_phase(), TimerPhase::Focus);
        assert_eq!(t.current_timer.duration, 75 * MIN);
    }

    #[test]
    fn abandoned_phase_is_not_recorded_in_stats() {
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
