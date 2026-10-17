use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    Running,
    Paused,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerPhase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl TimerPhase {
    pub const ALL: [TimerPhase; 3] = [
        TimerPhase::Focus,
        TimerPhase::ShortBreak,
        TimerPhase::LongBreak,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TimerPhase::Focus => "Focus",
            TimerPhase::ShortBreak => "Short break",
            TimerPhase::LongBreak => "Long break",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            TimerPhase::Focus => "Focus",
            TimerPhase::ShortBreak => "Short",
            TimerPhase::LongBreak => "Long",
        }
    }

    pub fn key_hint(self) -> &'static str {
        match self {
            TimerPhase::Focus => "f",
            TimerPhase::ShortBreak => "b",
            TimerPhase::LongBreak => "l",
        }
    }
}

/// Category of focus work selected on the Timer page and tracked in Analytics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub enum FocusCategory {
    #[default]
    Coding,
    Learning,
    Reading,
}

impl FocusCategory {
    pub const ALL: [FocusCategory; 3] = [
        FocusCategory::Coding,
        FocusCategory::Learning,
        FocusCategory::Reading,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FocusCategory::Coding => "Coding",
            FocusCategory::Learning => "Learning",
            FocusCategory::Reading => "Reading",
        }
    }

    pub fn upper_label(self) -> &'static str {
        match self {
            FocusCategory::Coding => "CODING",
            FocusCategory::Learning => "LEARNING",
            FocusCategory::Reading => "READING",
        }
    }

    pub fn cycle_next(self) -> Self {
        match self {
            FocusCategory::Coding => FocusCategory::Learning,
            FocusCategory::Learning => FocusCategory::Reading,
            FocusCategory::Reading => FocusCategory::Coding,
        }
    }

    #[allow(dead_code)]
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_lowercase().as_str() {
            "learning" | "learn" => FocusCategory::Learning,
            "reading" | "read" => FocusCategory::Reading,
            _ => FocusCategory::Coding,
        }
    }
}

/// A single countdown (one focus session or one break).
///
/// Time is measured against the wall clock (`Instant`) rather than by counting
/// UI ticks, so the countdown cannot drift when a frame is late or the loop
/// stalls, and the progress ring always matches the real remaining time.
/// Every method that needs "now" takes it as a parameter, which keeps the
/// logic deterministic and unit-testable.
#[derive(Debug, Clone)]
pub struct Timer {
    pub phase: TimerPhase,
    pub state: TimerState,
    pub duration: Duration,
    pub remaining: Duration,
    /// Instant up to which `remaining` has already been updated.
    /// `Some` only while `state == Running`.
    last_tick: Option<Instant>,
}

impl Timer {
    pub fn new(phase: TimerPhase, duration: Duration) -> Self {
        // A zero-length phase would make the progress maths meaningless.
        let duration = duration.max(Duration::from_secs(1));
        Self {
            phase,
            state: TimerState::Paused,
            duration,
            remaining: duration,
            last_tick: None,
        }
    }

    pub fn start_at(&mut self, now: Instant) {
        if self.state != TimerState::Running {
            self.state = TimerState::Running;
            self.last_tick = Some(now);
        }
    }

    pub fn pause(&mut self) {
        if self.state == TimerState::Running {
            self.state = TimerState::Paused;
        }
        self.last_tick = None;
    }

    pub fn reset(&mut self) {
        self.remaining = self.duration;
        self.state = TimerState::Paused;
        self.last_tick = None;
    }

    /// Consume the time that has passed since the previous call.
    /// Returns `true` exactly once: on the call that finishes the countdown.
    pub fn tick_at(&mut self, now: Instant) -> bool {
        if self.state != TimerState::Running {
            return false;
        }

        let last = self.last_tick.unwrap_or(now);
        let elapsed = now.saturating_duration_since(last);
        self.last_tick = Some(now);

        if elapsed >= self.remaining {
            self.remaining = Duration::ZERO;
            self.state = TimerState::Completed;
            self.last_tick = None;
            true
        } else {
            self.remaining -= elapsed;
            false
        }
    }

    pub fn is_running(&self) -> bool {
        self.state == TimerState::Running
    }

    /// True once any time of this phase has been consumed.
    pub fn has_progress(&self) -> bool {
        self.remaining < self.duration
    }

    /// Fraction of the phase that has elapsed, in `0.0..=1.0`.
    /// This is the single source of truth for the progress ring.
    pub fn progress(&self) -> f64 {
        let total = self.duration.as_secs_f64();
        if total <= 0.0 {
            return 1.0;
        }
        (1.0 - self.remaining.as_secs_f64() / total).clamp(0.0, 1.0)
    }

    /// Whole-percent progress (rounded down), used by the status-bar JSON.
    pub fn percentage_complete(&self) -> u16 {
        (self.progress() * 100.0).floor() as u16
    }

    /// Remaining whole seconds, rounded *up* – the usual countdown convention
    /// (a fresh 25:00 timer reads 25:00 until a full second has passed and
    /// 00:00 appears only when the phase is actually over).
    pub fn remaining_secs_ceil(&self) -> u64 {
        let ms = self.remaining.as_millis();
        ((ms + 999) / 1000) as u64
    }

    pub fn format_time(&self) -> String {
        let total_secs = self.remaining_secs_ceil();
        format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    #[test]
    fn focus_categories_cycle_through_coding_learning_reading() {
        let mut cat = FocusCategory::default();
        assert_eq!(cat, FocusCategory::Coding);
        assert_eq!(cat.label(), "Coding");
        cat = cat.cycle_next();
        assert_eq!(cat, FocusCategory::Learning);
        assert_eq!(cat.label(), "Learning");
        cat = cat.cycle_next();
        assert_eq!(cat, FocusCategory::Reading);
        assert_eq!(cat.label(), "Reading");
        cat = cat.cycle_next();
        assert_eq!(cat, FocusCategory::Coding);
    }

    #[test]
    fn counts_down_against_the_clock() {
        let t0 = Instant::now();
        let mut t = Timer::new(TimerPhase::Focus, 25 * MIN);
        t.start_at(t0);
        assert!(!t.tick_at(t0 + Duration::from_secs(10)));
        assert_eq!(t.remaining, 25 * MIN - Duration::from_secs(10));
    }

    #[test]
    fn irregular_tick_spacing_does_not_drift() {
        let t0 = Instant::now();
        let mut t = Timer::new(TimerPhase::Focus, 25 * MIN);
        t.start_at(t0);
        // Ticks arrive at wildly uneven intervals; total elapsed is 7.3 s.
        let mut now = t0;
        for ms in [130u64, 900, 40, 2_000, 4_230] {
            now += Duration::from_millis(ms);
            t.tick_at(now);
        }
        assert_eq!(t.remaining, 25 * MIN - Duration::from_millis(7_300));
    }

    #[test]
    fn paused_timer_ignores_time() {
        let t0 = Instant::now();
        let mut t = Timer::new(TimerPhase::Focus, 25 * MIN);
        assert!(!t.tick_at(t0 + 5 * MIN));
        assert_eq!(t.remaining, 25 * MIN);
    }

    #[test]
    fn pause_and_resume_continue_from_exact_remaining() {
        let t0 = Instant::now();
        let mut t = Timer::new(TimerPhase::Focus, 25 * MIN);
        t.start_at(t0);
        t.tick_at(t0 + Duration::from_secs(10));
        t.pause();
        // An hour passes while paused – nothing may change.
        assert!(!t.tick_at(t0 + Duration::from_secs(3_610)));
        assert_eq!(t.remaining, 25 * MIN - Duration::from_secs(10));
        // Resume and run another 5 s.
        let resume = t0 + Duration::from_secs(3_610);
        t.start_at(resume);
        t.tick_at(resume + Duration::from_secs(5));
        assert_eq!(t.remaining, 25 * MIN - Duration::from_secs(15));
    }

    #[test]
    fn completion_is_reported_exactly_once() {
        let t0 = Instant::now();
        let mut t = Timer::new(TimerPhase::Focus, 5 * MIN);
        t.start_at(t0);
        assert!(t.tick_at(t0 + 5 * MIN));
        assert_eq!(t.state, TimerState::Completed);
        assert_eq!(t.remaining, Duration::ZERO);
        assert!(!t.tick_at(t0 + 6 * MIN));
    }

    #[test]
    fn long_stall_completes_once_and_never_underflows() {
        // e.g. the laptop was suspended for hours.
        let t0 = Instant::now();
        let mut t = Timer::new(TimerPhase::Focus, 25 * MIN);
        t.start_at(t0);
        assert!(t.tick_at(t0 + Duration::from_secs(10 * 3600)));
        assert_eq!(t.remaining, Duration::ZERO);
    }

    #[test]
    fn reset_restores_full_duration_and_zero_progress() {
        let t0 = Instant::now();
        let mut t = Timer::new(TimerPhase::Focus, 25 * MIN);
        t.start_at(t0);
        t.tick_at(t0 + 7 * MIN);
        t.reset();
        assert_eq!(t.remaining, 25 * MIN);
        assert_eq!(t.progress(), 0.0);
        assert!(!t.is_running());
        assert!(!t.has_progress());
    }

    #[test]
    fn progress_tracks_the_whole_session() {
        let t0 = Instant::now();
        let mut t = Timer::new(TimerPhase::Focus, 50 * MIN);
        assert_eq!(t.progress(), 0.0);
        t.start_at(t0);
        t.tick_at(t0 + 25 * MIN);
        assert!((t.progress() - 0.5).abs() < 1e-9);
        t.tick_at(t0 + 50 * MIN);
        assert_eq!(t.progress(), 1.0);
    }

    #[test]
    fn display_rounds_up_like_a_normal_countdown() {
        let t0 = Instant::now();
        let mut t = Timer::new(TimerPhase::Focus, 25 * MIN);
        assert_eq!(t.format_time(), "25:00");
        t.start_at(t0);
        t.tick_at(t0 + Duration::from_millis(400));
        assert_eq!(t.format_time(), "25:00");
        t.tick_at(t0 + Duration::from_millis(1_000));
        assert_eq!(t.format_time(), "24:59");
        t.tick_at(t0 + 25 * MIN - Duration::from_millis(300));
        assert_eq!(t.format_time(), "00:01");
    }

    #[test]
    fn extended_durations_format_as_mm_ss() {
        let t = Timer::new(TimerPhase::LongBreak, 45 * MIN);
        assert_eq!(t.format_time(), "45:00");
        let t = Timer::new(TimerPhase::Focus, 75 * MIN);
        assert_eq!(t.format_time(), "75:00");
    }

    #[test]
    fn zero_duration_is_clamped() {
        let t = Timer::new(TimerPhase::Focus, Duration::ZERO);
        assert_eq!(t.duration, Duration::from_secs(1));
        assert!(t.progress().is_finite());
    }
}
