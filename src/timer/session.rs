use super::state::FocusCategory;
use chrono::{DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

pub const YEAR_DAYS: usize = 365;
pub const GRID_WEEKS: usize = 53;
pub const GRID_DAYS_PER_WEEK: usize = 7;
pub const MAX_FLIGHT_LOG: usize = 48;

fn default_flight_category() -> String {
    "Coding".to_string()
}

/// Map a category name to its index `0 = Coding, 1 = Learning, 2 = Reading`.
pub fn category_index(name: &str) -> usize {
    match FocusCategory::from_name(name) {
        FocusCategory::Coding => 0,
        FocusCategory::Learning => 1,
        FocusCategory::Reading => 2,
    }
}

/// Daily focus record stored per ISO date ("YYYY-MM-DD").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyStat {
    pub focus_secs: u64,
    pub sessions: u32,
    #[serde(default)]
    pub break_secs: u64,
    #[serde(default)]
    pub coding_secs: u64,
    #[serde(default)]
    pub learning_secs: u64,
    #[serde(default)]
    pub reading_secs: u64,
}

impl DailyStat {
    /// Returns `[coding_secs, learning_secs, reading_secs]`, automatically
    /// attributing any legacy uncategorized `focus_secs` to `Coding`.
    pub fn category_split(self) -> [u64; 3] {
        let cat_sum = self
            .coding_secs
            .saturating_add(self.learning_secs)
            .saturating_add(self.reading_secs);
        let uncategorized = self.focus_secs.saturating_sub(cat_sum);
        [
            self.coding_secs.saturating_add(uncategorized),
            self.learning_secs,
            self.reading_secs,
        ]
    }

    /// Dominant category for this day (defaults to `Coding` when tied or zero).
    pub fn dominant_category(self) -> FocusCategory {
        let [c, l, r] = self.category_split();
        if l > c && l >= r {
            FocusCategory::Learning
        } else if r > c && r > l {
            FocusCategory::Reading
        } else {
            FocusCategory::Coding
        }
    }

    pub fn add_category_secs(&mut self, category_name: &str, secs: u64) {
        match category_index(category_name) {
            0 => self.coding_secs = self.coding_secs.saturating_add(secs),
            1 => self.learning_secs = self.learning_secs.saturating_add(secs),
            _ => self.reading_secs = self.reading_secs.saturating_add(secs),
        }
    }
}

/// One chronological entry in the Session Flight Log audit trail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlightLogEntry {
    /// Local timestamp e.g. "10-15 09:30"
    pub timestamp: String,
    /// "FOCUS", "SHORT BRK", "LONG BRK"
    pub phase: String,
    /// Mode name e.g. "Classic", "Session", "Extended"
    pub mode: String,
    /// Focus category ("Coding", "Learning", "Reading") or "—" for breaks
    #[serde(default = "default_flight_category")]
    pub category: String,
    /// Elapsed duration in seconds
    pub duration_secs: u64,
    /// Outcome e.g. "COMPLETE", "SKIPPED", "ABORTED", "RESTED"
    pub status: String,
}

/// Aggregated weekday telemetry (Mon=0 .. Sun=6), including per-category split.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub struct WeekdayStat {
    pub weekday_index: usize,
    pub focus_secs: u64,
    pub sessions: u32,
    pub active_days: u32,
    pub avg_active_secs: u64,
    /// `[coding_secs, learning_secs, reading_secs]`
    pub cat_secs: [u64; 3],
}

/// Aggregated Pomodoro mode telemetry.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct ModeStat {
    pub name: String,
    pub focus_secs: u64,
    pub sessions: u32,
    pub share_pct: f64,
}

/// Rich per-category analytics for `Coding`, `Learning`, and `Reading`.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct CategoryStat {
    pub category: FocusCategory,
    pub name: String,
    pub focus_secs: u64,
    pub today_secs: u64,
    pub week_secs: u64,
    pub sessions: u32,
    pub interrupted: u32,
    pub completion_pct: f64,
    pub avg_session_secs: u64,
    pub active_days: u32,
    pub current_streak: u32,
    pub best_weekday: Option<usize>,
    pub peak_hour: Option<usize>,
    pub share_pct: f64,
}

/// Persisted daily & historical stats stored in ~/.local/share/devchron/stats.json
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PersistedStats {
    /// ISO date string e.g. "2026-03-15" (kept for backward compatibility)
    #[serde(default)]
    date: String,
    #[serde(default)]
    today_focus_secs: u64,
    #[serde(default)]
    today_sessions: u32,
    /// Full daily focus history keyed by ISO date ("YYYY-MM-DD") for offline analytics.
    #[serde(default)]
    history: BTreeMap<String, DailyStat>,
    /// Cumulative focus seconds by hour of day (24 slots, 00h..23h).
    #[serde(default)]
    hourly_focus_secs: Vec<u64>,
    #[serde(default)]
    hourly_coding_secs: Vec<u64>,
    #[serde(default)]
    hourly_learning_secs: Vec<u64>,
    #[serde(default)]
    hourly_reading_secs: Vec<u64>,
    /// Cumulative focus seconds per Pomodoro mode ("Classic", "Session", "Extended").
    #[serde(default)]
    mode_focus_secs: BTreeMap<String, u64>,
    /// Completed focus sessions per Pomodoro mode.
    #[serde(default)]
    mode_sessions: BTreeMap<String, u32>,
    /// Cumulative focus seconds per focus category ("Coding", "Learning", "Reading").
    #[serde(default)]
    category_focus_secs: BTreeMap<String, u64>,
    /// Completed focus sessions per focus category.
    #[serde(default)]
    category_sessions: BTreeMap<String, u32>,
    /// Interrupted focus sessions per focus category.
    #[serde(default)]
    category_interrupted: BTreeMap<String, u32>,
    /// Total focus sessions completed all the way to 00:00 (or finished).
    #[serde(default)]
    completed_focus_count: u32,
    /// Total focus sessions interrupted (reset or skipped mid-phase with progress > 0).
    #[serde(default)]
    interrupted_focus_count: u32,
    /// Total full Pomodoro cycles completed.
    #[serde(default)]
    cycles_completed: u32,
    /// Recent chronological session audit log.
    #[serde(default)]
    recent_log: Vec<FlightLogEntry>,
}

/// Offline system identity (username and computer name) retrieved from the local OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemIdentity {
    pub username: String,
    pub hostname: String,
}

impl SystemIdentity {
    /// Detect the current user's system username and computer name completely offline.
    pub fn detect() -> Self {
        Self {
            username: Self::detect_username(),
            hostname: Self::detect_hostname(),
        }
    }

    fn detect_username() -> String {
        for key in ["USER", "LOGNAME", "USERNAME"] {
            if let Ok(val) = std::env::var(key) {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }

        // Offline Unix fallback: match current real UID from /proc/self/status against /etc/passwd
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            if let Some(uid_line) = status.lines().find(|l| l.starts_with("Uid:")) {
                if let Some(uid) = uid_line.split_whitespace().nth(1) {
                    if let Ok(passwd) = fs::read_to_string("/etc/passwd") {
                        for line in passwd.lines() {
                            let parts: Vec<&str> = line.split(':').collect();
                            if parts.len() >= 3 && parts[2] == uid && !parts[0].trim().is_empty() {
                                return parts[0].trim().to_string();
                            }
                        }
                    }
                }
            }
        }

        "user".to_string()
    }

    fn detect_hostname() -> String {
        for path in ["/etc/hostname", "/proc/sys/kernel/hostname"] {
            if let Ok(content) = fs::read_to_string(path) {
                if let Some(line) = content
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty() && !l.starts_with('#'))
                {
                    return line.to_string();
                }
            }
        }

        for key in ["HOSTNAME", "COMPUTERNAME", "HOST"] {
            if let Ok(val) = std::env::var(key) {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }

        "localhost".to_string()
    }
}

/// One cell in the 53-week × 7-day LeetCode-style yearly focus grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct DayCell {
    pub date: NaiveDate,
    pub focus_secs: u64,
    pub sessions: u32,
    /// `[coding_secs, learning_secs, reading_secs]`
    pub cat_secs: [u64; 3],
    pub dominant_category: FocusCategory,
    /// True if `date` falls within the 365-day window ending on `today`.
    pub in_year_window: bool,
}

/// Computed offline analytics snapshot for rendering the Analytics page.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AnalyticsSummary {
    pub today_date: NaiveDate,
    pub today_focus_secs: u64,
    pub today_sessions: u32,
    pub today_cat_secs: [u64; 3],
    pub year_focus_secs: u64,
    pub year_break_secs: u64,
    pub year_sessions: u32,
    pub all_time_focus_secs: u64,
    pub all_time_sessions: u32,
    pub active_days_year: u32,
    pub active_days_all_time: u32,
    /// Average daily focus seconds across the user's active calendar span.
    pub avg_daily_secs: u64,
    /// Average focus seconds on active days (days with focus_secs > 0).
    pub avg_active_day_secs: u64,
    /// Average daily focus seconds across the full 365-day window.
    pub avg_year_daily_secs: u64,
    /// Consecutive active days up to today (or yesterday if today hasn't started yet).
    pub current_streak: u32,
    /// Longest streak of consecutive active days in recorded history.
    pub longest_streak: u32,
    /// Highest single-day focus seconds in the past year.
    pub peak_day_secs: u64,
    pub peak_day_date: Option<NaiveDate>,
    /// Reference maximum daily seconds for proportional heatmap color scaling.
    pub max_daily_secs: u64,
    /// 53 week columns, each holding 7 weekday rows (Mon=0 .. Sun=6).
    pub weeks: Vec<[DayCell; GRID_DAYS_PER_WEEK]>,
    /// `(week_column_index, month_short_name)` for the month header above the grid.
    pub month_labels: Vec<(usize, &'static str)>,
    /// Last 7 calendar days `(date, focus_secs, sessions, [coding, learning, reading])` ending on `today`.
    pub last_7_days: Vec<(NaiveDate, u64, u32, [u64; 3])>,
    /// 24-hour diurnal focus distribution (`00h..=23h`).
    pub hourly_focus_secs: [u64; 24],
    /// 24-hour per-category focus distribution (`[coding, learning, reading]` for each hour).
    pub hourly_cat_secs: [[u64; 3]; 24],
    /// Hour of day (`0..24`) with highest focus activity, if any.
    pub peak_hour: Option<usize>,
    /// Chronotype split percentages: `(morning_06_12, afternoon_12_18, evening_18_24, night_00_06)`.
    pub chronotype_pcts: (f64, f64, f64, f64),
    /// Per-category seconds in each chronotype window `[morning, afternoon, evening, night]`.
    pub chronotype_cat_secs: [[u64; 3]; 4],
    /// Day-of-week rhythm across Mon (0) .. Sun (6).
    pub weekday_stats: [WeekdayStat; 7],
    /// Index (`0..7`) of the most productive day of the week, if any.
    pub peak_weekday: Option<usize>,
    /// Work-to-Rest telemetry: focus share %, break share %, work-to-rest ratio, cycles completed.
    pub focus_share_pct: f64,
    pub break_share_pct: f64,
    pub work_rest_ratio: f64,
    pub cycles_completed: u32,
    /// Mode distribution across Classic, Session, Extended.
    pub mode_stats: Vec<ModeStat>,
    /// Rich Category analytics across Coding, Learning, Reading.
    pub category_stats: Vec<CategoryStat>,
    /// Name of the category with the highest focus time, if any.
    pub top_category: Option<String>,
    /// Discipline & completion telemetry.
    pub completed_focus_count: u32,
    pub interrupted_focus_count: u32,
    pub completion_rate_pct: f64,
    /// Recent session flight log (newest first).
    pub recent_log: Vec<FlightLogEntry>,
    /// Last 6 months velocity `(month_label, focus_secs, sessions, [coding, learning, reading])`.
    pub monthly_velocity: Vec<(String, u64, u32, [u64; 3])>,
}

#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total_focus_time: Duration,
    pub total_break_time: Duration,
    pub sessions_completed: u32,
    pub today_focus_time: Duration,
    pub today_sessions: u32,
    pub session_start: Option<DateTime<Local>>,
    pub history: BTreeMap<String, DailyStat>,
    pub hourly_focus_secs: [u64; 24],
    pub hourly_cat_secs: [[u64; 3]; 24],
    pub mode_focus_secs: BTreeMap<String, u64>,
    pub mode_sessions: BTreeMap<String, u32>,
    pub category_focus_secs: BTreeMap<String, u64>,
    pub category_sessions: BTreeMap<String, u32>,
    pub category_interrupted: BTreeMap<String, u32>,
    pub completed_focus_count: u32,
    pub interrupted_focus_count: u32,
    pub cycles_completed: u32,
    pub recent_log: Vec<FlightLogEntry>,
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
        Self::load_from_path(stats_path)
    }

    /// Stats that never touch the disk – used by unit tests so that running
    /// `cargo test` can never modify the user's real `stats.json`.
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            total_focus_time: Duration::ZERO,
            total_break_time: Duration::ZERO,
            sessions_completed: 0,
            today_focus_time: Duration::ZERO,
            today_sessions: 0,
            session_start: None,
            history: BTreeMap::new(),
            hourly_focus_secs: [0; 24],
            hourly_cat_secs: [[0; 3]; 24],
            mode_focus_secs: BTreeMap::new(),
            mode_sessions: BTreeMap::new(),
            category_focus_secs: BTreeMap::new(),
            category_sessions: BTreeMap::new(),
            category_interrupted: BTreeMap::new(),
            completed_focus_count: 0,
            interrupted_focus_count: 0,
            cycles_completed: 0,
            recent_log: Vec::new(),
            stats_path: None,
        }
    }

    fn resolve_stats_path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| {
            let dir = d.join("devchron");
            let _ = fs::create_dir_all(&dir);
            dir.join("stats.json")
        })
    }

    pub fn format_date(date: NaiveDate) -> String {
        format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
    }

    fn today_date() -> String {
        Self::format_date(Local::now().date_naive())
    }

    fn now_timestamp() -> String {
        let now = Local::now();
        format!(
            "{:02}-{:02} {:02}:{:02}",
            now.month(),
            now.day(),
            now.hour(),
            now.minute()
        )
    }

    fn load_from_path(stats_path: Option<PathBuf>) -> Self {
        let mut base = Self {
            total_focus_time: Duration::ZERO,
            total_break_time: Duration::ZERO,
            sessions_completed: 0,
            today_focus_time: Duration::ZERO,
            today_sessions: 0,
            session_start: None,
            history: BTreeMap::new(),
            hourly_focus_secs: [0; 24],
            hourly_cat_secs: [[0; 3]; 24],
            mode_focus_secs: BTreeMap::new(),
            mode_sessions: BTreeMap::new(),
            category_focus_secs: BTreeMap::new(),
            category_sessions: BTreeMap::new(),
            category_interrupted: BTreeMap::new(),
            completed_focus_count: 0,
            interrupted_focus_count: 0,
            cycles_completed: 0,
            recent_log: Vec::new(),
            stats_path: stats_path.clone(),
        };

        let Some(path) = stats_path else {
            return base;
        };
        let Ok(data) = fs::read_to_string(&path) else {
            return base;
        };
        let Ok(persisted) = serde_json::from_str::<PersistedStats>(&data) else {
            return base;
        };

        let mut history = persisted.history;
        if !persisted.date.is_empty()
            && (persisted.today_focus_secs > 0 || persisted.today_sessions > 0)
        {
            let entry = history.entry(persisted.date.clone()).or_default();
            entry.focus_secs = entry.focus_secs.max(persisted.today_focus_secs);
            entry.sessions = entry.sessions.max(persisted.today_sessions);
        }

        let today = Self::today_date();
        let (today_focus_time, today_sessions) = if let Some(today_stat) = history.get(&today) {
            (
                Duration::from_secs(today_stat.focus_secs),
                today_stat.sessions,
            )
        } else if persisted.date == today {
            (
                Duration::from_secs(persisted.today_focus_secs),
                persisted.today_sessions,
            )
        } else {
            (Duration::ZERO, 0)
        };

        let mut hourly = [0u64; 24];
        for (i, &v) in persisted.hourly_focus_secs.iter().take(24).enumerate() {
            hourly[i] = v;
        }
        let mut hourly_cat = [[0u64; 3]; 24];
        for (i, &v) in persisted.hourly_coding_secs.iter().take(24).enumerate() {
            hourly_cat[i][0] = v;
        }
        for (i, &v) in persisted.hourly_learning_secs.iter().take(24).enumerate() {
            hourly_cat[i][1] = v;
        }
        for (i, &v) in persisted.hourly_reading_secs.iter().take(24).enumerate() {
            hourly_cat[i][2] = v;
        }

        base.total_focus_time = today_focus_time;
        base.sessions_completed = today_sessions;
        base.today_focus_time = today_focus_time;
        base.today_sessions = today_sessions;
        base.history = history;
        base.hourly_focus_secs = hourly;
        base.hourly_cat_secs = hourly_cat;
        base.mode_focus_secs = persisted.mode_focus_secs;
        base.mode_sessions = persisted.mode_sessions;
        base.category_focus_secs = persisted.category_focus_secs;
        base.category_sessions = persisted.category_sessions;
        base.category_interrupted = persisted.category_interrupted;
        base.completed_focus_count = persisted.completed_focus_count;
        base.interrupted_focus_count = persisted.interrupted_focus_count;
        base.cycles_completed = persisted.cycles_completed;
        base.recent_log = persisted.recent_log;
        base
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
            history: self.history.clone(),
            hourly_focus_secs: self.hourly_focus_secs.to_vec(),
            hourly_coding_secs: self.hourly_cat_secs.iter().map(|c| c[0]).collect(),
            hourly_learning_secs: self.hourly_cat_secs.iter().map(|c| c[1]).collect(),
            hourly_reading_secs: self.hourly_cat_secs.iter().map(|c| c[2]).collect(),
            mode_focus_secs: self.mode_focus_secs.clone(),
            mode_sessions: self.mode_sessions.clone(),
            category_focus_secs: self.category_focus_secs.clone(),
            category_sessions: self.category_sessions.clone(),
            category_interrupted: self.category_interrupted.clone(),
            completed_focus_count: self.completed_focus_count,
            interrupted_focus_count: self.interrupted_focus_count,
            cycles_completed: self.cycles_completed,
            recent_log: self.recent_log.clone(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = fs::write(path, json);
        }
    }

    fn push_log(&mut self, entry: FlightLogEntry) {
        self.recent_log.push(entry);
        if self.recent_log.len() > MAX_FLIGHT_LOG {
            let overflow = self.recent_log.len() - MAX_FLIGHT_LOG;
            self.recent_log.drain(0..overflow);
        }
    }

    pub fn start_session(&mut self) {
        self.session_start = Some(Local::now());
    }

    #[allow(dead_code)]
    pub fn complete_focus_session(&mut self, duration: Duration) {
        let now = Local::now();
        self.complete_focus_session_with_details(
            now.date_naive(),
            now.hour() as usize,
            duration,
            "Classic",
            true,
        );
    }

    #[allow(dead_code)]
    pub fn complete_focus_session_on(&mut self, date: NaiveDate, duration: Duration) {
        self.complete_focus_session_with_details(date, 10, duration, "Classic", true);
    }

    #[allow(dead_code)]
    pub fn record_focus_phase(
        &mut self,
        duration: Duration,
        mode_name: &str,
        reached_zero: bool,
    ) {
        self.record_focus_phase_with_category(duration, mode_name, "Coding", reached_zero);
    }

    pub fn record_focus_phase_with_category(
        &mut self,
        duration: Duration,
        mode_name: &str,
        category_name: &str,
        reached_zero: bool,
    ) {
        let now = Local::now();
        self.complete_focus_session_with_category(
            now.date_naive(),
            now.hour() as usize,
            duration,
            mode_name,
            category_name,
            reached_zero,
        );
    }

    pub fn complete_focus_session_with_details(
        &mut self,
        date: NaiveDate,
        hour: usize,
        duration: Duration,
        mode_name: &str,
        reached_zero: bool,
    ) {
        self.complete_focus_session_with_category(
            date,
            hour,
            duration,
            mode_name,
            "Coding",
            reached_zero,
        );
    }

    pub fn complete_focus_session_with_category(
        &mut self,
        date: NaiveDate,
        hour: usize,
        duration: Duration,
        mode_name: &str,
        category_name: &str,
        reached_zero: bool,
    ) {
        self.sessions_completed += 1;
        self.today_sessions += 1;
        self.total_focus_time += duration;
        self.today_focus_time += duration;

        let secs = duration.as_secs();
        let key = Self::format_date(date);
        let entry = self.history.entry(key).or_default();
        entry.focus_secs = entry.focus_secs.saturating_add(secs);
        entry.add_category_secs(category_name, secs);

        if duration > Duration::ZERO {
            entry.sessions = entry.sessions.saturating_add(1);
            let h = hour % 24;
            self.hourly_focus_secs[h] = self.hourly_focus_secs[h].saturating_add(secs);
            let c_idx = category_index(category_name);
            self.hourly_cat_secs[h][c_idx] = self.hourly_cat_secs[h][c_idx].saturating_add(secs);

            *self
                .mode_focus_secs
                .entry(mode_name.to_string())
                .or_insert(0) += secs;
            *self
                .mode_sessions
                .entry(mode_name.to_string())
                .or_insert(0) += 1;
            *self
                .category_focus_secs
                .entry(category_name.to_string())
                .or_insert(0) += secs;
            *self
                .category_sessions
                .entry(category_name.to_string())
                .or_insert(0) += 1;

            if reached_zero {
                self.completed_focus_count = self.completed_focus_count.saturating_add(1);
            } else {
                self.interrupted_focus_count = self.interrupted_focus_count.saturating_add(1);
                *self
                    .category_interrupted
                    .entry(category_name.to_string())
                    .or_insert(0) += 1;
            }

            self.push_log(FlightLogEntry {
                timestamp: Self::now_timestamp(),
                phase: "FOCUS".to_string(),
                mode: mode_name.to_string(),
                category: category_name.to_string(),
                duration_secs: secs,
                status: if reached_zero {
                    "COMPLETE".to_string()
                } else {
                    "SKIPPED".to_string()
                },
            });
        }

        self.persist();
    }

    #[allow(dead_code)]
    pub fn record_abandoned_focus(&mut self, duration: Duration, mode_name: &str) {
        self.record_abandoned_focus_with_category(duration, mode_name, "Coding");
    }

    pub fn record_abandoned_focus_with_category(
        &mut self,
        duration: Duration,
        mode_name: &str,
        category_name: &str,
    ) {
        if duration.is_zero() {
            return;
        }
        self.interrupted_focus_count = self.interrupted_focus_count.saturating_add(1);
        *self
            .category_interrupted
            .entry(category_name.to_string())
            .or_insert(0) += 1;
        self.push_log(FlightLogEntry {
            timestamp: Self::now_timestamp(),
            phase: "FOCUS".to_string(),
            mode: mode_name.to_string(),
            category: category_name.to_string(),
            duration_secs: duration.as_secs(),
            status: "ABORTED".to_string(),
        });
        self.persist();
    }

    pub fn record_cycle_complete(&mut self) {
        self.cycles_completed = self.cycles_completed.saturating_add(1);
        self.persist();
    }

    #[allow(dead_code)]
    pub fn complete_break_session(&mut self, duration: Duration) {
        self.record_break_phase("SHORT BRK", duration, "Classic");
    }

    pub fn record_break_phase(&mut self, phase_label: &str, duration: Duration, mode_name: &str) {
        self.total_break_time += duration;
        let secs = duration.as_secs();
        let key = Self::today_date();
        let entry = self.history.entry(key).or_default();
        entry.break_secs = entry.break_secs.saturating_add(secs);

        if secs > 0 {
            self.push_log(FlightLogEntry {
                timestamp: Self::now_timestamp(),
                phase: phase_label.to_string(),
                mode: mode_name.to_string(),
                category: "—".to_string(),
                duration_secs: secs,
                status: "RESTED".to_string(),
            });
            self.persist();
        }
    }

    #[allow(dead_code)]
    pub fn format_today_time(&self) -> String {
        Self::format_duration_secs(self.today_focus_time.as_secs())
    }

    pub fn format_duration_secs(secs: u64) -> String {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;

        if hours > 0 {
            format!("{}h {:02}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }

    pub fn format_mm_ss(secs: u64) -> String {
        let mins = secs / 60;
        let rem = secs % 60;
        if mins >= 60 {
            format!("{}h {:02}m", mins / 60, mins % 60)
        } else {
            format!("{:02}m {:02}s", mins, rem)
        }
    }

    /// Proportional intensity in `0.0..=1.0` for `focus_secs` relative to `max_daily_secs`.
    pub fn intensity_for(focus_secs: u64, max_daily_secs: u64) -> f64 {
        if focus_secs == 0 {
            return 0.0;
        }
        let scale = max_daily_secs.max(3600) as f64;
        (focus_secs as f64 / scale).clamp(0.0, 1.0)
    }

    /// Build a complete offline analytics snapshot as of today (`Local::now()`),
    /// including `live_focus_secs` from an in-progress focus session.
    #[allow(dead_code)]
    pub fn analytics_summary(&self, live_focus_secs: u64) -> AnalyticsSummary {
        let now = Local::now();
        self.analytics_summary_with_context(
            now.date_naive(),
            now.hour() as usize,
            live_focus_secs,
            "Classic",
        )
    }

    /// Deterministic analytics computation for a given `today` date.
    #[allow(dead_code)]
    pub fn analytics_summary_at(
        &self,
        today: NaiveDate,
        live_focus_secs: u64,
    ) -> AnalyticsSummary {
        self.analytics_summary_with_context(today, 10, live_focus_secs, "Classic")
    }

    pub fn analytics_summary_with_context(
        &self,
        today: NaiveDate,
        current_hour: usize,
        live_focus_secs: u64,
        active_mode_name: &str,
    ) -> AnalyticsSummary {
        self.analytics_summary_with_category(
            today,
            current_hour,
            live_focus_secs,
            active_mode_name,
            "Coding",
        )
    }

    pub fn analytics_summary_with_category(
        &self,
        today: NaiveDate,
        current_hour: usize,
        live_focus_secs: u64,
        active_mode_name: &str,
        active_category_name: &str,
    ) -> AnalyticsSummary {
        let mut effective = self.history.clone();
        let today_key = Self::format_date(today);
        let recorded_today_secs = self
            .today_focus_time
            .as_secs()
            .max(effective.get(&today_key).map_or(0, |d| d.focus_secs));
        let today_combined_secs = recorded_today_secs.saturating_add(live_focus_secs);
        let today_combined_sessions = self
            .today_sessions
            .max(effective.get(&today_key).map_or(0, |d| d.sessions));

        if today_combined_secs > 0 || today_combined_sessions > 0 {
            let entry = effective.entry(today_key.clone()).or_default();
            entry.focus_secs = today_combined_secs;
            entry.sessions = today_combined_sessions;
            if live_focus_secs > 0 {
                entry.add_category_secs(active_category_name, live_focus_secs);
            }
        }

        let today_cat_secs = effective
            .get(&today_key)
            .copied()
            .unwrap_or_default()
            .category_split();

        let window_start = today - ChronoDuration::days(YEAR_DAYS as i64 - 1);

        // Build 53-week × 7-day (Mon..Sun) LeetCode-style calendar grid
        let today_dow = today.weekday().num_days_from_monday() as i64;
        let current_week_monday = today - ChronoDuration::days(today_dow);

        let mut weeks = Vec::with_capacity(GRID_WEEKS);
        let mut month_labels = Vec::new();
        let mut last_month: Option<u32> = None;

        let mut year_focus_secs = 0u64;
        let mut year_break_secs = 0u64;
        let mut year_sessions = 0u32;
        let mut active_days_year = 0u32;
        let mut peak_day_secs = 0u64;
        let mut peak_day_date: Option<NaiveDate> = None;
        let mut year_cat_from_days = [0u64; 3];
        let mut cat_active_days = [0u32; 3];
        let mut cat_weekday_secs = [[0u64; 7]; 3];

        let mut weekday_stats = [WeekdayStat::default(); 7];
        for (i, ws) in weekday_stats.iter_mut().enumerate() {
            ws.weekday_index = i;
        }

        for col in 0..GRID_WEEKS {
            let weeks_ago = (GRID_WEEKS - 1 - col) as i64;
            let week_monday = current_week_monday - ChronoDuration::days(weeks_ago * 7);
            let mut week_cells = [DayCell {
                date: week_monday,
                focus_secs: 0,
                sessions: 0,
                cat_secs: [0; 3],
                dominant_category: FocusCategory::Coding,
                in_year_window: false,
            }; GRID_DAYS_PER_WEEK];

            for (row, cell_slot) in week_cells.iter_mut().enumerate() {
                let cell_date = week_monday + ChronoDuration::days(row as i64);
                let in_year_window = cell_date >= window_start && cell_date <= today;
                let key = Self::format_date(cell_date);
                let stat = if in_year_window {
                    effective.get(&key).copied().unwrap_or_default()
                } else {
                    DailyStat::default()
                };
                let split = stat.category_split();

                if in_year_window {
                    year_focus_secs = year_focus_secs.saturating_add(stat.focus_secs);
                    year_break_secs = year_break_secs.saturating_add(stat.break_secs);
                    year_sessions = year_sessions.saturating_add(stat.sessions);
                    if stat.focus_secs > 0 {
                        active_days_year += 1;
                        if stat.focus_secs > peak_day_secs {
                            peak_day_secs = stat.focus_secs;
                            peak_day_date = Some(cell_date);
                        }
                        weekday_stats[row].focus_secs =
                            weekday_stats[row].focus_secs.saturating_add(stat.focus_secs);
                        weekday_stats[row].sessions =
                            weekday_stats[row].sessions.saturating_add(stat.sessions);
                        weekday_stats[row].active_days += 1;
                        for c_i in 0..3 {
                            weekday_stats[row].cat_secs[c_i] =
                                weekday_stats[row].cat_secs[c_i].saturating_add(split[c_i]);
                            year_cat_from_days[c_i] =
                                year_cat_from_days[c_i].saturating_add(split[c_i]);
                            cat_weekday_secs[c_i][row] =
                                cat_weekday_secs[c_i][row].saturating_add(split[c_i]);
                            if split[c_i] > 0 {
                                cat_active_days[c_i] += 1;
                            }
                        }
                    }
                }

                *cell_slot = DayCell {
                    date: cell_date,
                    focus_secs: stat.focus_secs,
                    sessions: stat.sessions,
                    cat_secs: split,
                    dominant_category: stat.dominant_category(),
                    in_year_window,
                };
            }

            if let Some(first_in_window) = week_cells.iter().find(|c| c.in_year_window) {
                let m = first_in_window.date.month();
                if last_month != Some(m) {
                    month_labels.push((col, month_short_name(m)));
                    last_month = Some(m);
                }
            }

            weeks.push(week_cells);
        }

        year_break_secs = year_break_secs.max(self.total_break_time.as_secs());

        for ws in &mut weekday_stats {
            if ws.active_days > 0 {
                ws.avg_active_secs = ws.focus_secs / ws.active_days as u64;
            }
        }
        let peak_weekday = weekday_stats
            .iter()
            .filter(|w| w.focus_secs > 0)
            .max_by_key(|w| w.focus_secs)
            .map(|w| w.weekday_index);

        // All-time totals and parsed active dates for streaks & averages
        let mut all_time_focus_secs = 0u64;
        let mut all_time_sessions = 0u32;
        let mut active_dates: Vec<NaiveDate> = Vec::new();

        for (date_str, stat) in &effective {
            all_time_focus_secs = all_time_focus_secs.saturating_add(stat.focus_secs);
            all_time_sessions = all_time_sessions.saturating_add(stat.sessions);
            if stat.focus_secs > 0 {
                if let Ok(parsed) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    active_dates.push(parsed);
                }
            }
        }
        active_dates.sort_unstable();
        active_dates.dedup();

        let active_days_all_time = active_dates.len() as u32;

        // Longest streak across all recorded history
        let mut longest_streak = 0u32;
        let mut run = 0u32;
        let mut prev_date: Option<NaiveDate> = None;
        for &d in &active_dates {
            if let Some(prev) = prev_date {
                if d == prev + ChronoDuration::days(1) {
                    run += 1;
                } else {
                    run = 1;
                }
            } else {
                run = 1;
            }
            longest_streak = longest_streak.max(run);
            prev_date = Some(d);
        }

        // Current streak (counts backward from today if active today, else from yesterday)
        let is_active = |d: NaiveDate| -> bool {
            let k = Self::format_date(d);
            effective.get(&k).is_some_and(|s| s.focus_secs > 0)
        };
        let mut current_streak = 0u32;
        let mut cursor = if is_active(today) {
            Some(today)
        } else {
            let yesterday = today - ChronoDuration::days(1);
            is_active(yesterday).then_some(yesterday)
        };
        while let Some(d) = cursor {
            if is_active(d) {
                current_streak += 1;
                cursor = Some(d - ChronoDuration::days(1));
            } else {
                break;
            }
        }

        // Per-category current streaks
        let mut cat_streaks = [0u32; 3];
        for c_i in 0..3 {
            let is_cat_active = |d: NaiveDate| -> bool {
                let k = Self::format_date(d);
                effective
                    .get(&k)
                    .is_some_and(|s| s.category_split()[c_i] > 0)
            };
            let mut cur = if is_cat_active(today) {
                Some(today)
            } else {
                let yest = today - ChronoDuration::days(1);
                is_cat_active(yest).then_some(yest)
            };
            while let Some(d) = cur {
                if is_cat_active(d) {
                    cat_streaks[c_i] += 1;
                    cur = Some(d - ChronoDuration::days(1));
                } else {
                    break;
                }
            }
        }

        // Averages
        let avg_active_day_secs = if active_days_year > 0 {
            year_focus_secs / active_days_year as u64
        } else {
            0
        };
        let avg_year_daily_secs = year_focus_secs / YEAR_DAYS as u64;

        let first_active_in_year = active_dates
            .iter()
            .copied()
            .find(|&d| d >= window_start && d <= today);
        let span_days = first_active_in_year
            .map(|first| ((today - first).num_days().max(0) as u64) + 1)
            .unwrap_or(1)
            .clamp(1, YEAR_DAYS as u64);
        let avg_daily_secs = year_focus_secs / span_days;

        // Last 7 days breakdown + 7-day per-category totals
        let mut last_7_days = Vec::with_capacity(7);
        let mut week_cat_secs = [0u64; 3];
        for i in (0..7).rev() {
            let d = today - ChronoDuration::days(i);
            let k = Self::format_date(d);
            let st = effective.get(&k).copied().unwrap_or_default();
            let split = st.category_split();
            for c_i in 0..3 {
                week_cat_secs[c_i] = week_cat_secs[c_i].saturating_add(split[c_i]);
            }
            last_7_days.push((d, st.focus_secs, st.sessions, split));
        }

        // 24-Hour Diurnal Focus distribution + per-category hourly distribution
        let mut hourly_focus_secs = self.hourly_focus_secs;
        let mut hourly_cat_secs = self.hourly_cat_secs;
        let h_now = current_hour % 24;
        if live_focus_secs > 0 {
            hourly_focus_secs[h_now] = hourly_focus_secs[h_now].saturating_add(live_focus_secs);
            let c_idx = category_index(active_category_name);
            hourly_cat_secs[h_now][c_idx] =
                hourly_cat_secs[h_now][c_idx].saturating_add(live_focus_secs);
        }
        let hourly_sum: u64 = hourly_focus_secs.iter().sum();
        if hourly_sum == 0 && year_focus_secs > 0 {
            hourly_focus_secs[h_now] = year_focus_secs;
        }
        // Reconcile any legacy uncategorized hourly seconds into Coding (index 0)
        for h in 0..24 {
            let cat_h_sum = hourly_cat_secs[h][0]
                .saturating_add(hourly_cat_secs[h][1])
                .saturating_add(hourly_cat_secs[h][2]);
            if hourly_focus_secs[h] > cat_h_sum {
                hourly_cat_secs[h][0] =
                    hourly_cat_secs[h][0].saturating_add(hourly_focus_secs[h] - cat_h_sum);
            }
        }

        let total_hourly: u64 = hourly_focus_secs.iter().sum();
        let peak_hour = hourly_focus_secs
            .iter()
            .enumerate()
            .filter(|(_, &s)| s > 0)
            .max_by_key(|(_, &s)| s)
            .map(|(h, _)| h);

        let mut chronotype_cat_secs = [[0u64; 3]; 4];
        for h in 6..12 {
            for c_i in 0..3 {
                chronotype_cat_secs[0][c_i] += hourly_cat_secs[h][c_i];
            }
        }
        for h in 12..18 {
            for c_i in 0..3 {
                chronotype_cat_secs[1][c_i] += hourly_cat_secs[h][c_i];
            }
        }
        for h in 18..24 {
            for c_i in 0..3 {
                chronotype_cat_secs[2][c_i] += hourly_cat_secs[h][c_i];
            }
        }
        for h in 0..6 {
            for c_i in 0..3 {
                chronotype_cat_secs[3][c_i] += hourly_cat_secs[h][c_i];
            }
        }

        let chronotype_pcts = if total_hourly > 0 {
            let morning: u64 = hourly_focus_secs[6..12].iter().sum();
            let afternoon: u64 = hourly_focus_secs[12..18].iter().sum();
            let evening: u64 = hourly_focus_secs[18..24].iter().sum();
            let night: u64 = hourly_focus_secs[0..6].iter().sum();
            let tot = total_hourly as f64;
            (
                morning as f64 / tot * 100.0,
                afternoon as f64 / tot * 100.0,
                evening as f64 / tot * 100.0,
                night as f64 / tot * 100.0,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        // Work-to-Rest Telemetry
        let combined_work_rest = year_focus_secs.saturating_add(year_break_secs);
        let (focus_share_pct, break_share_pct) = if combined_work_rest > 0 {
            let f_pct = (year_focus_secs as f64 / combined_work_rest as f64) * 100.0;
            (f_pct, (100.0 - f_pct).max(0.0))
        } else {
            (0.0, 0.0)
        };
        let work_rest_ratio = if year_break_secs > 0 {
            year_focus_secs as f64 / year_break_secs as f64
        } else if year_focus_secs > 0 {
            (year_focus_secs as f64 / 300.0).min(99.9)
        } else {
            0.0
        };

        // Mode Distribution (Classic, Session, Extended) with legacy reconciliation
        let mut mode_map = self.mode_focus_secs.clone();
        let mut mode_sess_map = self.mode_sessions.clone();
        if live_focus_secs > 0 {
            *mode_map.entry(active_mode_name.to_string()).or_insert(0) += live_focus_secs;
        }
        let recorded_mode_sum: u64 = mode_map.values().sum();
        if year_focus_secs > recorded_mode_sum {
            *mode_map.entry(active_mode_name.to_string()).or_insert(0) +=
                year_focus_secs - recorded_mode_sum;
        }
        let recorded_mode_sess: u32 = mode_sess_map.values().sum();
        if year_sessions > recorded_mode_sess {
            *mode_sess_map
                .entry(active_mode_name.to_string())
                .or_insert(0) += year_sessions - recorded_mode_sess;
        }
        let mode_total_secs: u64 = mode_map.values().sum();
        let mut mode_stats = Vec::with_capacity(3);
        for name in ["Classic", "Session", "Extended"] {
            let f_secs = mode_map.get(name).copied().unwrap_or(0);
            let sess = mode_sess_map.get(name).copied().unwrap_or(0);
            let pct = if mode_total_secs > 0 {
                (f_secs as f64 / mode_total_secs as f64) * 100.0
            } else {
                0.0
            };
            mode_stats.push(ModeStat {
                name: name.to_string(),
                focus_secs: f_secs,
                sessions: sess,
                share_pct: pct,
            });
        }

        // Rich Category Distribution (Coding, Learning, Reading) with legacy reconciliation!
        // Any focus time or sessions recorded prior to category tracking are automatically
        // attributed to Coding so that category totals always match `year_focus_secs`.
        let mut cat_map = self.category_focus_secs.clone();
        let mut cat_sess_map = self.category_sessions.clone();
        if live_focus_secs > 0 {
            *cat_map
                .entry(active_category_name.to_string())
                .or_insert(0) += live_focus_secs;
        }
        for (idx, cat_enum) in FocusCategory::ALL.into_iter().enumerate() {
            let entry = cat_map.entry(cat_enum.label().to_string()).or_insert(0);
            if year_cat_from_days[idx] > *entry {
                *entry = year_cat_from_days[idx];
            }
        }
        let recorded_cat_sum: u64 = cat_map.values().sum();
        if year_focus_secs > recorded_cat_sum {
            *cat_map.entry("Coding".to_string()).or_insert(0) +=
                year_focus_secs - recorded_cat_sum;
        }
        let recorded_cat_sess: u32 = cat_sess_map.values().sum();
        if year_sessions > recorded_cat_sess {
            *cat_sess_map.entry("Coding".to_string()).or_insert(0) +=
                year_sessions - recorded_cat_sess;
        }

        let cat_total_secs: u64 = cat_map.values().sum();
        let mut category_stats = Vec::with_capacity(3);
        for (c_i, cat_enum) in FocusCategory::ALL.into_iter().enumerate() {
            let name = cat_enum.label();
            let f_secs = cat_map.get(name).copied().unwrap_or(0);
            let sess = cat_sess_map.get(name).copied().unwrap_or(0);
            let intr = self.category_interrupted.get(name).copied().unwrap_or(0);
            let attempts = sess.saturating_add(intr);
            let comp_pct = if attempts > 0 {
                (sess as f64 / attempts as f64) * 100.0
            } else {
                0.0
            };
            let avg_sess = if sess > 0 { f_secs / sess as u64 } else { 0 };
            let pct = if cat_total_secs > 0 {
                (f_secs as f64 / cat_total_secs as f64) * 100.0
            } else {
                0.0
            };
            let best_dow = cat_weekday_secs[c_i]
                .iter()
                .enumerate()
                .filter(|(_, &s)| s > 0)
                .max_by_key(|(_, &s)| s)
                .map(|(d, _)| d);
            let pk_hour = (0..24)
                .filter(|&h| hourly_cat_secs[h][c_i] > 0)
                .max_by_key(|&h| hourly_cat_secs[h][c_i]);

            category_stats.push(CategoryStat {
                category: cat_enum,
                name: name.to_string(),
                focus_secs: f_secs,
                today_secs: today_cat_secs[c_i],
                week_secs: week_cat_secs[c_i],
                sessions: sess,
                interrupted: intr,
                completion_pct: comp_pct,
                avg_session_secs: avg_sess,
                active_days: cat_active_days[c_i],
                current_streak: cat_streaks[c_i],
                best_weekday: best_dow,
                peak_hour: pk_hour,
                share_pct: pct,
            });
        }
        let top_category = category_stats
            .iter()
            .filter(|c| c.focus_secs > 0)
            .max_by_key(|c| c.focus_secs)
            .map(|c| c.name.clone());

        // Discipline & Completion Index
        let completed_focus_count = self.completed_focus_count.max(year_sessions);
        let interrupted_focus_count = self.interrupted_focus_count;
        let total_attempts = completed_focus_count.saturating_add(interrupted_focus_count);
        let completion_rate_pct = if total_attempts > 0 {
            (completed_focus_count as f64 / total_attempts as f64) * 100.0
        } else {
            0.0
        };

        // Monthly Velocity (last 6 calendar months ending in `today`'s month)
        let mut monthly_velocity = Vec::with_capacity(6);
        for m_offset in (0..6).rev() {
            let (y, m) = subtract_months(today.year(), today.month(), m_offset);
            let prefix = format!("{:04}-{:02}-", y, m);
            let mut m_secs = 0u64;
            let mut m_sess = 0u32;
            let mut m_cats = [0u64; 3];
            for (k, st) in &effective {
                if k.starts_with(&prefix) {
                    m_secs = m_secs.saturating_add(st.focus_secs);
                    m_sess = m_sess.saturating_add(st.sessions);
                    let sp = st.category_split();
                    for c_i in 0..3 {
                        m_cats[c_i] = m_cats[c_i].saturating_add(sp[c_i]);
                    }
                }
            }
            monthly_velocity.push((
                format!("{} '{:02}", month_short_name(m), y.rem_euclid(100)),
                m_secs,
                m_sess,
                m_cats,
            ));
        }

        let max_daily_secs = peak_day_secs.max(3600);

        AnalyticsSummary {
            today_date: today,
            today_focus_secs: today_combined_secs,
            today_sessions: today_combined_sessions,
            today_cat_secs,
            year_focus_secs,
            year_break_secs,
            year_sessions,
            all_time_focus_secs,
            all_time_sessions,
            active_days_year,
            active_days_all_time,
            avg_daily_secs,
            avg_active_day_secs,
            avg_year_daily_secs,
            current_streak,
            longest_streak,
            peak_day_secs,
            peak_day_date,
            max_daily_secs,
            weeks,
            month_labels,
            last_7_days,
            hourly_focus_secs,
            hourly_cat_secs,
            peak_hour,
            chronotype_pcts,
            chronotype_cat_secs,
            weekday_stats,
            peak_weekday,
            focus_share_pct,
            break_share_pct,
            work_rest_ratio,
            cycles_completed: self.cycles_completed,
            mode_stats,
            category_stats,
            top_category,
            completed_focus_count,
            interrupted_focus_count,
            completion_rate_pct,
            recent_log: self.recent_log.iter().rev().cloned().collect(),
            monthly_velocity,
        }
    }
}

fn subtract_months(year: i32, month: u32, offset: u32) -> (i32, u32) {
    let total_months = year * 12 + (month as i32 - 1) - offset as i32;
    let y = total_months.div_euclid(12);
    let m = (total_months.rem_euclid(12) + 1) as u32;
    (y, m)
}

fn month_short_name(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "   ",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_identity_detects_non_empty_username_and_hostname_offline() {
        let id = SystemIdentity::detect();
        assert!(!id.username.trim().is_empty());
        assert!(!id.hostname.trim().is_empty());
    }

    #[test]
    fn intensity_is_zero_when_idle_and_directly_proportional_to_time_spent() {
        let max_secs = 7200; // 2 hours
        assert_eq!(SessionStats::intensity_for(0, max_secs), 0.0);

        let i_30m = SessionStats::intensity_for(1800, max_secs);
        let i_60m = SessionStats::intensity_for(3600, max_secs);
        let i_90m = SessionStats::intensity_for(5400, max_secs);
        let i_120m = SessionStats::intensity_for(7200, max_secs);

        assert!((i_30m - 0.25).abs() < 1e-9);
        assert!((i_60m - 0.50).abs() < 1e-9);
        assert!((i_90m - 0.75).abs() < 1e-9);
        assert!((i_120m - 1.00).abs() < 1e-9);
        assert!(i_30m < i_60m && i_60m < i_90m && i_90m < i_120m);
    }

    #[test]
    fn legacy_uncategorized_history_is_reconciled_into_coding() {
        let mut stats = SessionStats::in_memory();
        let today = NaiveDate::from_ymd_opt(2026, 10, 17).unwrap();
        // Legacy day with 9 sessions (3h 45m = 13500s) and zero category breakdown,
        // plus 1 new session (25m = 1500s) recorded in category_focus_secs:
        stats.history.insert(
            SessionStats::format_date(today),
            DailyStat {
                focus_secs: 15000,
                sessions: 10,
                break_secs: 1800,
                coding_secs: 1500,
                learning_secs: 0,
                reading_secs: 0,
            },
        );
        stats.category_focus_secs.insert("Coding".into(), 1500);
        stats.category_sessions.insert("Coding".into(), 1);

        let summary = stats.analytics_summary_at(today, 0);
        assert_eq!(summary.category_stats[0].focus_secs, 15000);
        assert_eq!(summary.category_stats[0].sessions, 10);
        assert!((summary.category_stats[0].share_pct - 100.0).abs() < 1e-6);
    }

    #[test]
    fn streaks_averages_and_telemetry_are_computed_accurately() {
        let mut stats = SessionStats::in_memory();
        let today = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();

        // 3-day current streak ending today, plus an earlier 4-day streak
        for offset in [0, 1, 2, 10, 11, 12, 13] {
            let d = today - ChronoDuration::days(offset);
            stats.history.insert(
                SessionStats::format_date(d),
                DailyStat {
                    focus_secs: 3600,
                    sessions: 2,
                    break_secs: 600,
                    coding_secs: 1800,
                    learning_secs: 1200,
                    reading_secs: 600,
                },
            );
        }
        stats.today_focus_time = Duration::from_secs(3600);
        stats.today_sessions = 2;
        stats.hourly_focus_secs[9] = 5400;
        stats.hourly_focus_secs[14] = 3600;
        stats.mode_focus_secs.insert("Classic".into(), 3600);
        stats.mode_focus_secs.insert("Session".into(), 7200);
        stats.category_focus_secs.insert("Coding".into(), 7 * 1800);
        stats.category_focus_secs.insert("Learning".into(), 7 * 1200);
        stats.category_focus_secs.insert("Reading".into(), 7 * 600);
        stats.completed_focus_count = 14;
        stats.interrupted_focus_count = 1;

        let summary = stats.analytics_summary_at(today, 0);
        assert_eq!(summary.current_streak, 3);
        assert_eq!(summary.longest_streak, 4);
        assert_eq!(summary.active_days_year, 7);
        assert_eq!(summary.year_focus_secs, 7 * 3600);
        assert_eq!(summary.year_break_secs, 7 * 600);
        assert_eq!(summary.avg_active_day_secs, 3600);
        assert_eq!(summary.peak_hour, Some(9));
        assert!(summary.work_rest_ratio > 5.9 && summary.work_rest_ratio < 6.1);
        assert!(summary.completion_rate_pct > 93.0);
        assert_eq!(summary.mode_stats.len(), 3);
        assert_eq!(summary.category_stats.len(), 3);
        assert_eq!(summary.top_category.as_deref(), Some("Coding"));
        assert!((summary.category_stats[0].share_pct - 50.0).abs() < 1e-6);
        assert_eq!(summary.category_stats[0].active_days, 7);
        assert_eq!(summary.monthly_velocity.len(), 6);
        assert_eq!(summary.weeks.len(), GRID_WEEKS);

        // Count total in-window cells across the 53 weeks: must be exactly 365
        let in_window_count: usize = summary
            .weeks
            .iter()
            .flatten()
            .filter(|c| c.in_year_window)
            .count();
        assert_eq!(in_window_count, YEAR_DAYS);
    }
}
