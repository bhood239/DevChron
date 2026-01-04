use crate::timer::{PomodoroTimer, TimerPhase};
use crate::error::Result;
use serde::Serialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

#[derive(Serialize)]
struct StatusJson {
    phase: String,
    time_remaining: String,
    session: String,
    is_running: bool,
    percentage_complete: u8,
}

pub struct StatusWriter {
    enabled: bool,
    status_path: PathBuf,
}

impl StatusWriter {
    pub fn new(enabled: bool) -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| crate::error::Error::Config("Could not determine cache directory".into()))?;
        
        let devchron_cache = cache_dir.join("devchron");
        if !devchron_cache.exists() {
            fs::create_dir_all(&devchron_cache)?;
        }

        let status_path = devchron_cache.join("status.json");

        Ok(Self {
            enabled,
            status_path,
        })
    }

    pub fn update(&self, timer: &PomodoroTimer) {
        if !self.enabled {
            return;
        }

        let phase_name = match timer.current_phase() {
            TimerPhase::Focus => "focus",
            TimerPhase::ShortBreak => "short_break",
            TimerPhase::LongBreak => "long_break",
        };

        let time_remaining = format!("{:02}:{:02}", timer.minutes, timer.seconds);
        let session = format!("{}/{}", timer.cycle_count + 1, timer.cycles_before_long_break);
        
        let total_seconds = match timer.current_phase() {
            TimerPhase::Focus => timer.focus_duration * 60,
            TimerPhase::ShortBreak => timer.short_break_duration * 60,
            TimerPhase::LongBreak => timer.long_break_duration * 60,
        };
        let remaining_seconds = timer.minutes * 60 + timer.seconds;
        let percentage_complete = if total_seconds > 0 {
            ((total_seconds - remaining_seconds) * 100 / total_seconds) as u8
        } else {
            0
        };

        let status = StatusJson {
            phase: phase_name.to_string(),
            time_remaining,
            session,
            is_running: timer.is_running(),
            percentage_complete,
        };

        if let Ok(json) = serde_json::to_string_pretty(&status) {
            if let Ok(mut file) = File::create(&self.status_path) {
                let _ = file.write_all(json.as_bytes());
            }
        }
    }
}
