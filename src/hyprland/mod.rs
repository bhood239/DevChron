use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use crate::timer::{PomodoroTimer, TimerPhase};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub phase: String,
    pub time_remaining: String,
    pub session: String,
    pub is_running: bool,
    pub percentage_complete: u16,
    /// Active Pomodoro mode ("classic", "session", ...). Added in addition to
    /// the original keys, so existing status-bar scripts keep working.
    #[serde(default)]
    pub mode: String,
    /// Active Focus category ("coding", "learning", "reading").
    #[serde(default)]
    pub category: String,
    /// True while the 00:00 alarm is ringing (additive key).
    #[serde(default)]
    pub alarm_ringing: bool,
}

pub struct StatusWriter {
    enabled: bool,
    cache_path: PathBuf,
    /// Last status written; the file is only rewritten when it changes
    /// (about once per second) instead of on every UI frame.
    last: Option<StatusUpdate>,
}

impl StatusWriter {
    pub fn new(enabled: bool) -> Result<Self, std::io::Error> {
        let cache_path = Self::get_cache_path()?;
        Ok(Self {
            enabled,
            cache_path,
            last: None,
        })
    }

    fn get_cache_path() -> Result<PathBuf, std::io::Error> {
        let proj_dirs = ProjectDirs::from("", "", "devchron")
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not determine cache directory",
                )
            })?;

        let cache_dir = proj_dirs.cache_dir();
        fs::create_dir_all(cache_dir)?;

        Ok(cache_dir.join("status.json"))
    }

    pub fn update(&mut self, timer: &PomodoroTimer) {
        if !self.enabled {
            return;
        }

        let status = StatusUpdate {
            phase: self.phase_to_string(timer.current_phase()),
            time_remaining: timer.current_timer.format_time(),
            session: timer.session_info(),
            is_running: timer.is_running(),
            percentage_complete: timer.current_timer.percentage_complete(),
            mode: timer.mode.name.to_lowercase(),
            category: timer.category.label().to_lowercase(),
            alarm_ringing: timer.alarm_ringing,
        };

        if self.last.as_ref() == Some(&status) {
            return;
        }

        if let Ok(json) = serde_json::to_string_pretty(&status) {
            if let Err(e) = fs::write(&self.cache_path, json) {
                eprintln!("Failed to write status file: {}", e);
            }
        }
        self.last = Some(status);
    }

    fn phase_to_string(&self, phase: TimerPhase) -> String {
        match phase {
            TimerPhase::Focus => "focus".to_string(),
            TimerPhase::ShortBreak => "short_break".to_string(),
            TimerPhase::LongBreak => "long_break".to_string(),
        }
    }
}

impl Drop for StatusWriter {
    fn drop(&mut self) {
        // Clean up status file on exit (only if this instance wrote it)
        if self.enabled {
            let _ = fs::remove_file(&self.cache_path);
        }
    }
}
