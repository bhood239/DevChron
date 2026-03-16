use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub timer: TimerSettings,

    #[serde(default)]
    pub notifications: NotificationSettings,

    #[serde(default)]
    pub ui: UiSettings,

    #[serde(default)]
    pub integrations: IntegrationSettings,

    #[serde(default)]
    pub behaviour: BehaviourSettings,

    /// Named timer profiles. Keys are profile names (e.g. "work", "deep", "quick").
    /// If empty, a set of built-in defaults is used.
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileSettings>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timer: TimerSettings::default(),
            notifications: NotificationSettings::default(),
            ui: UiSettings::default(),
            integrations: IntegrationSettings::default(),
            behaviour: BehaviourSettings::default(),
            profiles: BTreeMap::new(),
        }
    }
}

// ── Timer ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimerSettings {
    #[serde(default = "default_focus_duration")]
    pub focus_duration: u64,

    #[serde(default = "default_short_break")]
    pub short_break_duration: u64,

    #[serde(default = "default_long_break")]
    pub long_break_duration: u64,

    #[serde(default = "default_cycles")]
    pub cycles_before_long_break: u32,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            focus_duration: default_focus_duration(),
            short_break_duration: default_short_break(),
            long_break_duration: default_long_break(),
            cycles_before_long_break: default_cycles(),
        }
    }
}

// ── Profile ──────────────────────────────────────────────────────────────────

/// A named timer profile — overrides the base [timer] durations.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileSettings {
    /// Focus duration in minutes.
    pub focus: u64,
    /// Short break duration in minutes.
    pub short_break: u64,
    /// Long break duration in minutes.
    pub long_break: u64,
    /// How many focus sessions before a long break (optional, falls back to [timer] value).
    pub cycles: Option<u32>,
}

impl ProfileSettings {
    /// Built-in "work" profile — classic Pomodoro.
    pub fn work() -> Self {
        Self {
            focus: 25,
            short_break: 5,
            long_break: 15,
            cycles: Some(4),
        }
    }

    /// Built-in "deep" profile — longer deep-work blocks.
    pub fn deep() -> Self {
        Self {
            focus: 50,
            short_break: 10,
            long_break: 20,
            cycles: Some(3),
        }
    }

    /// Built-in "quick" profile — short sprints.
    pub fn quick() -> Self {
        Self {
            focus: 15,
            short_break: 3,
            long_break: 10,
            cycles: Some(4),
        }
    }

    /// Returns the ordered list of built-in profiles used when none are defined in config.
    pub fn defaults() -> Vec<(String, ProfileSettings)> {
        vec![
            ("work".to_string(), Self::work()),
            ("deep".to_string(), Self::deep()),
            ("quick".to_string(), Self::quick()),
        ]
    }
}

// ── Notifications ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotificationSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Play a chime when each phase completes (default: true).
    #[serde(default = "default_true")]
    pub sound_enabled: bool,

    /// Sound volume from 0.0 (silent) to 1.0 (full). Default: 0.5.
    #[serde(default = "default_volume")]
    pub sound_volume: f32,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sound_enabled: true,
            sound_volume: default_volume(),
        }
    }
}

// ── Behaviour ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BehaviourSettings {
    /// Automatically start break timers when a focus session ends (default: false).
    #[serde(default)]
    pub auto_start_breaks: bool,

    /// Automatically start the next focus session when a break ends (default: false).
    #[serde(default)]
    pub auto_start_focus: bool,
}

impl Default for BehaviourSettings {
    fn default() -> Self {
        Self {
            auto_start_breaks: false,
            auto_start_focus: false,
        }
    }
}

// ── UI ────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
        }
    }
}

// ── Integrations ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IntegrationSettings {
    #[serde(default = "default_true")]
    pub hyprland_status_bar: bool,
}

impl Default for IntegrationSettings {
    fn default() -> Self {
        Self {
            hyprland_status_bar: true,
        }
    }
}

// ── Default value fns ─────────────────────────────────────────────────────────

fn default_focus_duration() -> u64 {
    25
}
fn default_short_break() -> u64 {
    5
}
fn default_long_break() -> u64 {
    15
}
fn default_cycles() -> u32 {
    4
}
fn default_theme() -> String {
    "nord".to_string()
}
fn default_true() -> bool {
    true
}
fn default_volume() -> f32 {
    0.5
}
