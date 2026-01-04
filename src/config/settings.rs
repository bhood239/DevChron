use serde::{Deserialize, Serialize};

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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timer: TimerSettings::default(),
            notifications: NotificationSettings::default(),
            ui: UiSettings::default(),
            integrations: IntegrationSettings::default(),
        }
    }
}

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotificationSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    #[serde(default)]
    pub sound_enabled: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sound_enabled: false,
        }
    }
}

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

// Default value functions
fn default_focus_duration() -> u64 { 25 }
fn default_short_break() -> u64 { 5 }
fn default_long_break() -> u64 { 15 }
fn default_cycles() -> u32 { 4 }
fn default_theme() -> String { "nord".to_string() }
fn default_true() -> bool { true }
