use chrono::{DateTime, Datelike, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Persisted daily stats stored in ~/.local/share/devchron/stats.json
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedStats {
    /// ISO date string e.g. "2026-03-15"
    date: String,
    today_focus_secs: u64,
    today_sessions: u32,
}

#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total_focus_time: Duration,
    pub total_break_time: Duration,
    pub sessions_completed: u32,
    pub today_focus_time: Duration,
    pub today_sessions: u32,
    pub session_start: Option<DateTime<Local>>,
    stats_path: Option<PathBuf>,
}

impl Default for SessionStats {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStats {
    pub fn new() -> Self {
        let stats_path = Self::resolve_stats_path();
        let (today_focus_time, today_sessions) = Self::load_today(&stats_path);

        Self {
            total_focus_time: today_focus_time,
            total_break_time: Duration::ZERO,
            sessions_completed: today_sessions,
            today_focus_time,
            today_sessions,
            session_start: None,
            stats_path,
        }
    }

    fn resolve_stats_path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| {
            let dir = d.join("devchron");
            let _ = fs::create_dir_all(&dir);
            dir.join("stats.json")
        })
    }

    fn today_date() -> String {
        let now = Local::now();
        format!("{}-{:02}-{:02}", now.year(), now.month(), now.day())
    }

    fn load_today(path: &Option<PathBuf>) -> (Duration, u32) {
        let path = match path {
            Some(p) => p,
            None => return (Duration::ZERO, 0),
        };

        let data = match fs::read_to_string(path) {
            Ok(d) => d,
            Err(_) => return (Duration::ZERO, 0),
        };

        let persisted: PersistedStats = match serde_json::from_str(&data) {
            Ok(p) => p,
            Err(_) => return (Duration::ZERO, 0),
        };

        // Only restore if the saved date matches today
        if persisted.date == Self::today_date() {
            (
                Duration::from_secs(persisted.today_focus_secs),
                persisted.today_sessions,
            )
        } else {
            (Duration::ZERO, 0)
        }
    }

    fn persist(&self) {
        let path = match &self.stats_path {
            Some(p) => p,
            None => return,
        };

        let data = PersistedStats {
            date: Self::today_date(),
            today_focus_secs: self.today_focus_time.as_secs(),
            today_sessions: self.today_sessions,
        };

        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = fs::write(path, json);
        }
    }

    pub fn start_session(&mut self) {
        self.session_start = Some(Local::now());
    }

    pub fn complete_focus_session(&mut self, duration: Duration) {
        self.sessions_completed += 1;
        self.today_sessions += 1;
        self.total_focus_time += duration;
        self.today_focus_time += duration;
        self.persist();
    }

    pub fn complete_break_session(&mut self, duration: Duration) {
        self.total_break_time += duration;
    }

    pub fn format_today_time(&self) -> String {
        let hours = self.today_focus_time.as_secs() / 3600;
        let mins = (self.today_focus_time.as_secs() % 3600) / 60;

        if hours > 0 {
            format!("{}h {:02}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }
}
