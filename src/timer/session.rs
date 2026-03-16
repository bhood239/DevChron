use chrono::{DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

// ── Persisted types ───────────────────────────────────────────────────────────

/// One completed focus session stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedSession {
    /// Wall-clock time the session ended (HH:MM).
    pub ended_at: String,
    /// Duration of the focus block in seconds.
    pub duration_secs: u64,
    /// Optional task tag the user set before starting.
    pub task: Option<String>,
}

/// Today's stats file (~/.local/share/devchron/stats.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedStats {
    date: String,
    today_focus_secs: u64,
    today_sessions: u32,
    #[serde(default)]
    sessions: Vec<CompletedSession>,
}

/// Rolling 7-day history file (~/.local/share/devchron/history.json).
/// Keys are ISO date strings ("2026-03-15"), values are focus minutes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HistoryFile {
    /// date → total focus seconds that day
    days: BTreeMap<String, u64>,
}

/// One day's data for the weekly chart.
#[derive(Debug, Clone)]
pub struct DaySummary {
    /// Short label e.g. "Mon"
    pub label: String,
    /// Total focus minutes
    pub focus_mins: u64,
}

// ── In-memory stats ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total_focus_time: Duration,
    pub total_break_time: Duration,
    pub sessions_completed: u32,
    pub today_focus_time: Duration,
    pub today_sessions: u32,
    pub session_start: Option<DateTime<Local>>,
    /// Completed sessions for today — used for the history view.
    pub completed_sessions: Vec<CompletedSession>,
    /// Last 7 days for the weekly bar chart (oldest first).
    pub weekly: Vec<DaySummary>,
    stats_path: Option<PathBuf>,
    history_path: Option<PathBuf>,
}

impl Default for SessionStats {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStats {
    pub fn new() -> Self {
        let stats_path = Self::data_dir().map(|d| d.join("stats.json"));
        let history_path = Self::data_dir().map(|d| d.join("history.json"));

        let (today_focus_time, today_sessions, completed_sessions) = Self::load_today(&stats_path);
        let weekly = Self::load_weekly(&history_path);

        Self {
            total_focus_time: today_focus_time,
            total_break_time: Duration::ZERO,
            sessions_completed: today_sessions,
            today_focus_time,
            today_sessions,
            session_start: None,
            completed_sessions,
            weekly,
            stats_path,
            history_path,
        }
    }

    // ── Path helpers ──────────────────────────────────────────────────────────

    fn data_dir() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| {
            let dir = d.join("devchron");
            let _ = fs::create_dir_all(&dir);
            dir
        })
    }

    fn date_str(date: NaiveDate) -> String {
        format!("{}-{:02}-{:02}", date.year(), date.month(), date.day())
    }

    fn today() -> NaiveDate {
        Local::now().date_naive()
    }

    fn today_str() -> String {
        Self::date_str(Self::today())
    }

    // ── Load ──────────────────────────────────────────────────────────────────

    fn load_today(path: &Option<PathBuf>) -> (Duration, u32, Vec<CompletedSession>) {
        let path = match path {
            Some(p) => p,
            None => return (Duration::ZERO, 0, vec![]),
        };
        let data = match fs::read_to_string(path) {
            Ok(d) => d,
            Err(_) => return (Duration::ZERO, 0, vec![]),
        };
        let persisted: PersistedStats = match serde_json::from_str(&data) {
            Ok(p) => p,
            Err(_) => return (Duration::ZERO, 0, vec![]),
        };
        if persisted.date == Self::today_str() {
            (
                Duration::from_secs(persisted.today_focus_secs),
                persisted.today_sessions,
                persisted.sessions,
            )
        } else {
            (Duration::ZERO, 0, vec![])
        }
    }

    fn load_weekly(path: &Option<PathBuf>) -> Vec<DaySummary> {
        let history = path
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<HistoryFile>(&s).ok())
            .unwrap_or_default();

        let today = Self::today();
        (0..7)
            .rev()
            .map(|days_ago| {
                let date = today - ChronoDuration::days(days_ago);
                let key = Self::date_str(date);
                let secs = history.days.get(&key).copied().unwrap_or(0);
                DaySummary {
                    label: date.format("%a").to_string(), // "Mon", "Tue", …
                    focus_mins: secs / 60,
                }
            })
            .collect()
    }

    // ── Persist ───────────────────────────────────────────────────────────────

    fn persist_today(&self) {
        let path = match &self.stats_path {
            Some(p) => p,
            None => return,
        };
        let data = PersistedStats {
            date: Self::today_str(),
            today_focus_secs: self.today_focus_time.as_secs(),
            today_sessions: self.today_sessions,
            sessions: self.completed_sessions.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = fs::write(path, json);
        }
    }

    fn persist_history(&self) {
        let path = match &self.history_path {
            Some(p) => p,
            None => return,
        };

        // Load existing history, update today's entry, prune to 90 days.
        let mut history: HistoryFile = fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        history
            .days
            .insert(Self::today_str(), self.today_focus_time.as_secs());

        // Keep only the last 90 days to avoid unbounded growth.
        let cutoff = Self::date_str(Self::today() - ChronoDuration::days(90));
        history.days.retain(|k, _| k.as_str() >= cutoff.as_str());

        if let Ok(json) = serde_json::to_string_pretty(&history) {
            let _ = fs::write(path, json);
        }
    }

    // ── Public API ────────────────────────────────────────────────────────────

    pub fn start_session(&mut self) {
        self.session_start = Some(Local::now());
    }

    pub fn complete_focus_session(&mut self, duration: Duration, task: Option<String>) {
        self.sessions_completed += 1;
        self.today_sessions += 1;
        self.total_focus_time += duration;
        self.today_focus_time += duration;

        let now = Local::now();
        self.completed_sessions.push(CompletedSession {
            ended_at: now.format("%H:%M").to_string(),
            duration_secs: duration.as_secs(),
            task,
        });

        // Update today's entry in the weekly summary too.
        if let Some(today_entry) = self.weekly.last_mut() {
            today_entry.focus_mins = self.today_focus_time.as_secs() / 60;
        }

        self.persist_today();
        self.persist_history();
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
