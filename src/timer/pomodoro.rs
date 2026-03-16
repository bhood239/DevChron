use super::session::SessionStats;
use super::state::{Timer, TimerPhase, TimerState};
use std::time::Duration;

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
    pub focus_duration: Duration,
    pub short_break_duration: Duration,
    pub long_break_duration: Duration,
    pub stats: SessionStats,
    /// True for exactly one render frame after a cycle completes.
    pub cycle_just_completed: bool,
}

impl PomodoroTimer {
    pub fn new(
        focus_mins: u64,
        short_break_mins: u64,
        long_break_mins: u64,
        cycles_before_long_break: u32,
    ) -> Self {
        let focus_duration = Duration::from_secs(focus_mins * 60);
        Self {
            current_timer: Timer::new(TimerPhase::Focus, focus_duration),
            cycle_count: 0,
            cycles_before_long_break,
            focus_duration,
            short_break_duration: Duration::from_secs(short_break_mins * 60),
            long_break_duration: Duration::from_secs(long_break_mins * 60),
            stats: SessionStats::default(),
            cycle_just_completed: false,
        }
    }

    pub fn toggle(&mut self) {
        match self.current_timer.state {
            TimerState::Running => self.current_timer.pause(),
            TimerState::Paused | TimerState::Completed => {
                self.current_timer.start();
                if self.stats.session_start.is_none() {
                    self.stats.start_session();
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.current_timer.reset();
    }

    /// Skip the current phase (no task tag — used for break skips or manual skips).
    #[allow(dead_code)]
    pub fn skip(&mut self) -> PhaseEvent {
        self.advance_phase_with_task(None)
    }

    /// Advance the timer by one second. Returns a `PhaseEvent` if a phase
    /// completed this tick, or `None` if the timer is still running.
    pub fn tick(&mut self) -> Option<PhaseEvent> {
        // Clear the one-frame cycle flag each tick.
        self.cycle_just_completed = false;

        if self.current_timer.tick() {
            Some(self.advance_phase_with_task(None))
        } else {
            None
        }
    }

    /// Complete the current phase, recording an optional task tag for focus sessions.
    pub fn advance_phase_with_task(&mut self, task: Option<String>) -> PhaseEvent {
        match self.current_timer.phase {
            TimerPhase::Focus => {
                let elapsed = self
                    .focus_duration
                    .saturating_sub(self.current_timer.remaining);
                self.stats.complete_focus_session(elapsed, task);
                self.cycle_count += 1;

                if self.cycle_count >= self.cycles_before_long_break {
                    self.cycle_count = 0;
                    self.cycle_just_completed = true;
                    self.current_timer =
                        Timer::new(TimerPhase::LongBreak, self.long_break_duration);
                } else {
                    self.current_timer =
                        Timer::new(TimerPhase::ShortBreak, self.short_break_duration);
                }
                PhaseEvent::FocusComplete
            }
            TimerPhase::ShortBreak => {
                let elapsed = self
                    .short_break_duration
                    .saturating_sub(self.current_timer.remaining);
                self.stats.complete_break_session(elapsed);
                self.current_timer = Timer::new(TimerPhase::Focus, self.focus_duration);
                PhaseEvent::ShortBreakComplete
            }
            TimerPhase::LongBreak => {
                let elapsed = self
                    .long_break_duration
                    .saturating_sub(self.current_timer.remaining);
                self.stats.complete_break_session(elapsed);
                self.current_timer = Timer::new(TimerPhase::Focus, self.focus_duration);
                PhaseEvent::LongBreakComplete
            }
        }
    }

    /// Adjust the remaining time of the current phase by `delta_secs` (can be negative).
    /// Clamps to [1, duration].
    pub fn adjust_time(&mut self, delta_secs: i64) {
        let remaining = self.current_timer.remaining.as_secs() as i64;
        let duration = self.current_timer.duration.as_secs() as i64;
        let new_remaining = (remaining + delta_secs).clamp(1, duration);
        self.current_timer.remaining = Duration::from_secs(new_remaining as u64);
    }

    /// Reset the timer to a new profile's durations, preserving session stats.
    pub fn apply_profile(
        &mut self,
        focus_mins: u64,
        short_break_mins: u64,
        long_break_mins: u64,
        cycles: u32,
    ) {
        self.focus_duration = Duration::from_secs(focus_mins * 60);
        self.short_break_duration = Duration::from_secs(short_break_mins * 60);
        self.long_break_duration = Duration::from_secs(long_break_mins * 60);
        self.cycles_before_long_break = cycles;
        self.cycle_count = 0;
        // Reset the current timer to the new focus duration.
        self.current_timer = Timer::new(TimerPhase::Focus, self.focus_duration);
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
}
