use chrono::{DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

pub const YEAR_DAYS: usize = 365;
pub const GRID_WEEKS: usize = 53;
pub const GRID_DAYS_PER_WEEK: usize = 7;
pub const MAX_FLIGHT_LOG: usize = 24;

/// Daily focus record stored per ISO date ("YYYY-MM-DD").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyStat {
    pub focus_secs: u64,
    pub sessions: u32,
    #[serde(default)]
    pub break_secs: u64,
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
    /// Elapsed duration in seconds
    pub duration_secs: u64,
    /// Outcome e.g. "COMPLETE", "SKIPPED", "ABORTED", "RESTED"
    pub status: String,
}

/// Aggregated weekday telemetry (Mon=0 .. Sun=6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WeekdayStat {
    pub weekday_index: usize,
    pub focus_secs: u64,
    pub sessions: u32,
    pub active_days: u32,
    pub avg_active_secs: u64,
}

/// Aggregated Pomodoro mode telemetry.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeStat {
    pub name: String,
    pub focus_secs: u64,
    pub sessions: u32,
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
    /// Cumulative focus seconds per Pomodoro mode ("Classic", "Session", "Extended").
    #[serde(default)]
    mode_focus_secs: BTreeMap<String, u64>,
    /// Completed focus sessions per Pomodoro mode.
    #[serde(default)]
    mode_sessions: BTreeMap<String, u32>,
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
pub struct DayCell {
    pub date: NaiveDate,
    pub focus_secs: u64,
    pub sessions: u32,
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
    /// Last 7 calendar days `(date, focus_secs, sessions)` ending on `today`.
    pub last_7_days: Vec<(NaiveDate, u64, u32)>,
    /// 24-hour diurnal focus distribution (`00h..=23h`).
    pub hourly_focus_secs: [u64; 24],
    /// Hour of day (`0..24`) with highest focus activity, if any.
    pub peak_hour: Option<usize>,
    /// Chronotype split percentages: `(morning_06_12, afternoon_12_18, evening_18_24, night_00_06)`.
    pub chronotype_pcts: (f64, f64, f64, f64),
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
    /// Discipline & completion telemetry.
    pub completed_focus_count: u32,
    pub interrupted_focus_count: u32,
    pub completion_rate_pct: f64,
    /// Recent session flight log (newest first).
    pub recent_log: Vec<FlightLogEntry>,
    /// Last 6 months velocity `(month_label, focus_secs, sessions)`.
    pub monthly_velocity: Vec<(String, u64, u32)>,
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
    pub mode_focus_secs: BTreeMap<String, u64>,
    pub mode_sessions: BTreeMap<String, u32>,
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
            mode_focus_secs: BTreeMap::new(),
            mode_sessions: BTreeMap::new(),
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
            mode_focus_secs: BTreeMap::new(),
            mode_sessions: BTreeMap::new(),
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

        base.total_focus_time = today_focus_time;
        base.sessions_completed = today_sessions;
        base.today_focus_time = today_focus_time;
        base.today_sessions = today_sessions;
        base.history = history;
        base.hourly_focus_secs = hourly;
        base.mode_focus_secs = persisted.mode_focus_secs;
        base.mode_sessions = persisted.mode_sessions;
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
            mode_focus_secs: self.mode_focus_secs.clone(),
            mode_sessions: self.mode_sessions.clone(),
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

    pub fn record_focus_phase(
        &mut self,
        duration: Duration,
        mode_name: &str,
        reached_zero: bool,
    ) {
        let now = Local::now();
        self.complete_focus_session_with_details(
            now.date_naive(),
            now.hour() as usize,
            duration,
            mode_name,
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
        self.sessions_completed += 1;
        self.today_sessions += 1;
        self.total_focus_time += duration;
        self.today_focus_time += duration;

        let secs = duration.as_secs();
        let key = Self::format_date(date);
        let entry = self.history.entry(key).or_default();
        entry.focus_secs = entry.focus_secs.saturating_add(secs);
        if duration > Duration::ZERO {
            entry.sessions = entry.sessions.saturating_add(1);
            self.hourly_focus_secs[hour % 24] =
                self.hourly_focus_secs[hour % 24].saturating_add(secs);
            *self
                .mode_focus_secs
                .entry(mode_name.to_string())
                .or_insert(0) += secs;
            *self
                .mode_sessions
                .entry(mode_name.to_string())
                .or_insert(0) += 1;

            if reached_zero {
                self.completed_focus_count = self.completed_focus_count.saturating_add(1);
            } else {
                self.interrupted_focus_count = self.interrupted_focus_count.saturating_add(1);
            }

            self.push_log(FlightLogEntry {
                timestamp: Self::now_timestamp(),
                phase: "FOCUS".to_string(),
                mode: mode_name.to_string(),
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

    pub fn record_abandoned_focus(&mut self, duration: Duration, mode_name: &str) {
        if duration.is_zero() {
            return;
        }
        self.interrupted_focus_count = self.interrupted_focus_count.saturating_add(1);
        self.push_log(FlightLogEntry {
            timestamp: Self::now_timestamp(),
            phase: "FOCUS".to_string(),
            mode: mode_name.to_string(),
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
    /// Zero focus time always maps to `0.0`; non-zero focus time scales linearly
    /// in direct proportion to `focus_secs`.
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
        }

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
                    }
                }

                *cell_slot = DayCell {
                    date: cell_date,
                    focus_secs: stat.focus_secs,
                    sessions: stat.sessions,
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

        // Last 7 days breakdown
        let mut last_7_days = Vec::with_capacity(7);
        for i in (0..7).rev() {
            let d = today - ChronoDuration::days(i);
            let k = Self::format_date(d);
            let st = effective.get(&k).copied().unwrap_or_default();
            last_7_days.push((d, st.focus_secs, st.sessions));
        }

        // 24-Hour Diurnal Focus distribution + chronotype split
        let mut hourly_focus_secs = self.hourly_focus_secs;
        if live_focus_secs > 0 {
            hourly_focus_secs[current_hour % 24] =
                hourly_focus_secs[current_hour % 24].saturating_add(live_focus_secs);
        }
        let hourly_sum: u64 = hourly_focus_secs.iter().sum();
        if hourly_sum == 0 && year_focus_secs > 0 {
            hourly_focus_secs[current_hour % 24] = year_focus_secs;
        }
        let total_hourly: u64 = hourly_focus_secs.iter().sum();
        let peak_hour = hourly_focus_secs
            .iter()
            .enumerate()
            .filter(|(_, &s)| s > 0)
            .max_by_key(|(_, &s)| s)
            .map(|(h, _)| h);

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

        // Mode Distribution (Classic, Session, Extended)
        let mut mode_map = self.mode_focus_secs.clone();
        let mut mode_sess_map = self.mode_sessions.clone();
        if live_focus_secs > 0 {
            *mode_map.entry(active_mode_name.to_string()).or_insert(0) += live_focus_secs;
        }
        let recorded_mode_sum: u64 = mode_map.values().sum();
        if recorded_mode_sum == 0 && year_focus_secs > 0 {
            mode_map.insert(active_mode_name.to_string(), year_focus_secs);
            mode_sess_map.insert(active_mode_name.to_string(), year_sessions);
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
            for (k, st) in &effective {
                if k.starts_with(&prefix) {
                    m_secs = m_secs.saturating_add(st.focus_secs);
                    m_sess = m_sess.saturating_add(st.sessions);
                }
            }
            monthly_velocity.push((
                format!("{} '{:02}", month_short_name(m), y.rem_euclid(100)),
                m_secs,
                m_sess,
            ));
        }

        let max_daily_secs = peak_day_secs.max(3600);

        AnalyticsSummary {
            today_date: today,
            today_focus_secs: today_combined_secs,
            today_sessions: today_combined_sessions,
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
            peak_hour,
            chronotype_pcts,
            weekday_stats,
            peak_weekday,
            focus_share_pct,
            break_share_pct,
            work_rest_ratio,
            cycles_completed: self.cycles_completed,
            mode_stats,
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
                },
            );
        }
        stats.today_focus_time = Duration::from_secs(3600);
        stats.today_sessions = 2;
        stats.hourly_focus_secs[9] = 5400;
        stats.hourly_focus_secs[14] = 3600;
        stats.mode_focus_secs.insert("Classic".into(), 3600);
        stats.mode_focus_secs.insert("Session".into(), 7200);
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
