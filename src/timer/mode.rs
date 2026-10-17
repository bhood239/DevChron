//! Centralised Pomodoro mode definitions.
//!
//! This is the *only* place that knows which modes exist. The timer, the UI
//! (mode tabs, confirmation dialog, help screen) and the keyboard handler
//! (number keys, Tab) are all driven by [`MODES`], so adding a mode is a
//! one-line change:
//!
//! ```ignore
//! PomodoroMode::new("Sprint", 15, 3, 10),
//! ```
//!
//! Modes are selected with the number keys `1`..`9`, so at most
//! [`MAX_MODES`] entries are reachable.

use std::time::Duration;

/// Durations (in whole minutes) for one Pomodoro mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PomodoroMode {
    pub name: &'static str,
    pub focus_mins: u64,
    pub short_break_mins: u64,
    pub long_break_mins: u64,
}

impl PomodoroMode {
    pub const fn new(
        name: &'static str,
        focus_mins: u64,
        short_break_mins: u64,
        long_break_mins: u64,
    ) -> Self {
        Self {
            name,
            focus_mins,
            short_break_mins,
            long_break_mins,
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

    /// Compact `focus/short/long` label, e.g. `25/5/15`.
    pub fn summary(&self) -> String {
        format!(
            "{}/{}/{}",
            self.focus_mins, self.short_break_mins, self.long_break_mins
        )
    }
}

/// Every selectable mode, in display order.
pub const MODES: &[PomodoroMode] = &[
    PomodoroMode::new("Classic", 25, 5, 15),
    PomodoroMode::new("Session", 50, 10, 30),
    PomodoroMode::new("Extended", 75, 15, 45),
];

/// Index into [`MODES`] that is active at start-up (Classic).
pub const DEFAULT_MODE: usize = 0;

/// Number keys 1-9 select modes, so only this many can be addressed directly.
pub const MAX_MODES: usize = 9;

/// Build the runtime mode list.
///
/// The `[timer]` durations from `config.toml` define the *default* mode
/// (Classic). With an untouched config that is exactly 25/5/15, so existing
/// users keep their customised durations while the other modes stay fixed.
pub fn build_modes(focus: u64, short_break: u64, long_break: u64) -> Vec<PomodoroMode> {
    let mut modes: Vec<PomodoroMode> = MODES.iter().copied().take(MAX_MODES).collect();
    modes[DEFAULT_MODE] = modes[DEFAULT_MODE].with_durations(focus, short_break, long_break);
    modes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_modes_have_the_specified_durations() {
        let expected = [
            ("Classic", 25, 5, 15),
            ("Session", 50, 10, 30),
            ("Extended", 75, 15, 45),
        ];
        assert_eq!(MODES.len(), expected.len());
        for (mode, (name, f, s, l)) in MODES.iter().zip(expected) {
            assert_eq!(mode.name, name);
            assert_eq!(mode.focus(), Duration::from_secs(f * 60));
            assert_eq!(mode.short_break(), Duration::from_secs(s * 60));
            assert_eq!(mode.long_break(), Duration::from_secs(l * 60));
        }
    }

    #[test]
    fn classic_is_the_default_mode() {
        assert_eq!(MODES[DEFAULT_MODE].name, "Classic");
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
        assert_eq!(modes[1], MODES[1]);
        assert_eq!(modes[2], MODES[2]);
    }

    #[test]
    fn zero_durations_are_clamped() {
        let m = MODES[0].with_durations(0, 0, 0);
        assert_eq!(m.summary(), "1/1/1");
    }
}
