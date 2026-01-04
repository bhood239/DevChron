use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use crate::timer::{PomodoroTimer, TimerPhase};

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub phase: String,
    pub time_remaining: String,
    pub session: String,
    pub is_running: bool,
    pub percentage_complete: u16,
}

pub struct StatusWriter {
    enabled: bool,
    cache_path: PathBuf,
}

impl StatusWriter {
    pub fn new(enabled: bool) -> Result<Self, std::io::Error> {
        let cache_path = Self::get_cache_path()?;
        Ok(Self { enabled, cache_path })
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

    pub fn update(&self, timer: &PomodoroTimer) {
        if !self.enabled {
            return;
        }

        let status = StatusUpdate {
            phase: self.phase_to_string(timer.current_phase()),
            time_remaining: timer.current_timer.format_time(),
            session: timer.session_info(),
            is_running: timer.is_running(),
            percentage_complete: timer.current_timer.percentage_complete(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&status) {
            if let Err(e) = fs::write(&self.cache_path, json) {
                eprintln!("Failed to write status file: {}", e);
            }
        }
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
        // Clean up status file on exit
        let _ = fs::remove_file(&self.cache_path);
    }
}
