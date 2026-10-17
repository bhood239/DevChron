//! Centralised Pomodoro mode definitions.
//!
//! This is the *only* place that knows which modes exist and how their focus
//! cycle schedules are structured. The timer state machine, the UI (mode tabs,
//! confirmation modal, reward modal, help screen) and the keyboard handler
//! (number keys, Tab) are all driven by [`MODES`].
//!
//! Authoritative mode schedules:
//! - **Classic** (4 focus cycles, 100m total focus):
//!   25m Focus → 5m Short Break → 25m Focus → 5m Short Break → 25m Focus → 5m Short Break → 25m Focus → 15m Final Long Break
//! - **Session** (3 focus cycles, 150m total focus):
//!   50m Focus → 10m Short Break → 50m Focus → 10m Short Break → 50m Focus → 30m Final Long Break
//! - **Extended** (2 focus cycles, 150m total focus):
//!   75m Focus → 15m Short Break → 75m Focus → 45m Final Long Break

use std::time::Duration;

/// Durations (in whole minutes) and focus-cycle count for one Pomodoro mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PomodoroMode {
    pub name: &'static str,
    pub focus_mins: u64,
    pub short_break_mins: u64,
    pub long_break_mins: u64,
    pub focus_cycles: u32,
}

impl PomodoroMode {
    pub const fn new(
        name: &'static str,
        focus_mins: u64,
        short_break_mins: u64,
        long_break_mins: u64,
        focus_cycles: u32,
    ) -> Self {
        Self {
            name,
            focus_mins,
            short_break_mins,
            long_break_mins,
            focus_cycles: if focus_cycles == 0 { 1 } else { focus_cycles },
        }
    }

    pub fn focus(&self) -> Duration {
        Duration::from_secs(self.focus_mins * 60)
    }

    pub fn short_break(&self) -> Duration {
        Duration::from_secs(self.short_break_mins * 60)
    }

    pub fn long_break(&self) -> Duration {
        Duration::from_secs(self.long_break_mins * 60)
    }

    /// Number of short breaks in a full mode sequence (`focus_cycles - 1`).
    #[allow(dead_code)]
    pub const fn short_break_count(&self) -> u32 {
        self.focus_cycles.saturating_sub(1)
    }

    /// Total focus minutes across a full mode sequence (`focus_mins * focus_cycles`).
    pub const fn total_focus_mins(&self) -> u64 {
        self.focus_mins * (self.focus_cycles as u64)
    }

    /// Total focus [`Duration`] across a full mode sequence.
    #[allow(dead_code)]
    pub fn total_focus(&self) -> Duration {
        Duration::from_secs(self.total_focus_mins() * 60)
    }

    /// Same mode with different durations. Each value is clamped to at least
    /// one minute so a zero in the config can never create an empty phase.
    pub fn with_durations(self, focus: u64, short_break: u64, long_break: u64) -> Self {
        Self {
            focus_mins: focus.max(1),
            short_break_mins: short_break.max(1),
            long_break_mins: long_break.max(1),
            ..self
        }
    }

    /// Same mode with a custom focus-cycle count (clamped to at least 1).
    pub fn with_cycles(self, focus_cycles: u32) -> Self {
        Self {
            focus_cycles: focus_cycles.max(1),
            ..self
        }
    }

    /// Compact `focus/short/long` label, e.g. `25/5/15`.
    pub fn summary(&self) -> String {
        format!(
            "{}/{}/{}",
            self.focus_mins, self.short_break_mins, self.long_break_mins
        )
    }

    /// Extended schedule summary including cycle count and total focus minutes.
    #[allow(dead_code)]
    pub fn schedule_summary(&self) -> String {
        format!(
            "{} min · {} cycles ({}m focus)",
            self.summary(),
            self.focus_cycles,
            self.total_focus_mins()
        )
    }
}

/// Every selectable mode, in display order.
pub const MODES: &[PomodoroMode] = &[
    PomodoroMode::new("Classic", 25, 5, 15, 4),
    PomodoroMode::new("Session", 50, 10, 30, 3),
    PomodoroMode::new("Extended", 75, 15, 45, 2),
];

/// Index into [`MODES`] that is active at start-up (Classic).
pub const DEFAULT_MODE: usize = 0;

/// Number keys 1-9 select modes, so only this many can be addressed directly.
pub const MAX_MODES: usize = 9;

/// Build the runtime mode list.
///
/// The `[timer]` durations from `config.toml` define the *default* mode
/// (Classic). With an untouched config that is exactly 25/5/15 (4 cycles), so
/// existing users keep their customised durations while Session and Extended
/// follow their fixed authoritative schedules.
pub fn build_modes(focus: u64, short_break: u64, long_break: u64) -> Vec<PomodoroMode> {
    let mut modes: Vec<PomodoroMode> = MODES.iter().copied().take(MAX_MODES).collect();
    modes[DEFAULT_MODE] = modes[DEFAULT_MODE].with_durations(focus, short_break, long_break);
    modes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_modes_have_the_specified_durations_and_cycles() {
        let expected = [
            ("Classic", 25, 5, 15, 4, 3, 100),
            ("Session", 50, 10, 30, 3, 2, 150),
            ("Extended", 75, 15, 45, 2, 1, 150),
        ];
        assert_eq!(MODES.len(), expected.len());
        for (mode, (name, f, s, l, cycles, short_breaks, total_focus)) in
            MODES.iter().zip(expected)
        {
            assert_eq!(mode.name, name);
            assert_eq!(mode.focus(), Duration::from_secs(f * 60));
            assert_eq!(mode.short_break(), Duration::from_secs(s * 60));
            assert_eq!(mode.long_break(), Duration::from_secs(l * 60));
            assert_eq!(mode.focus_cycles, cycles);
            assert_eq!(mode.short_break_count(), short_breaks);
            assert_eq!(mode.total_focus_mins(), total_focus);
        }
    }

    #[test]
    fn classic_is_the_default_mode() {
        assert_eq!(MODES[DEFAULT_MODE].name, "Classic");
        assert_eq!(MODES[DEFAULT_MODE].focus_cycles, 4);
    }

    #[test]
    fn mode_list_fits_on_number_keys() {
        assert!(MODES.len() <= MAX_MODES);
    }

    #[test]
    fn untouched_config_reproduces_the_builtin_table() {
        assert_eq!(build_modes(25, 5, 15), MODES.to_vec());
    }

    #[test]
    fn config_overrides_only_the_default_mode() {
        let modes = build_modes(30, 6, 20);
        assert_eq!(modes[DEFAULT_MODE].summary(), "30/6/20");
        assert_eq!(modes[DEFAULT_MODE].focus_cycles, 4);
        assert_eq!(modes[1], MODES[1]);
        assert_eq!(modes[2], MODES[2]);
    }

    #[test]
    fn zero_durations_and_cycles_are_clamped() {
        let m = MODES[0].with_durations(0, 0, 0).with_cycles(0);
        assert_eq!(m.summary(), "1/1/1");
        assert_eq!(m.focus_cycles, 1);
    }
}
