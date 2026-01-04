use std::time::Duration;
use chrono::{DateTime, Local};

#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total_focus_time: Duration,
    pub total_break_time: Duration,
    pub sessions_completed: u32,
    pub today_focus_time: Duration,
    pub today_sessions: u32,
    pub session_start: Option<DateTime<Local>>,
}

impl Default for SessionStats {
    fn default() -> Self {
        Self {
            total_focus_time: Duration::ZERO,
            total_break_time: Duration::ZERO,
            sessions_completed: 0,
            today_focus_time: Duration::ZERO,
            today_sessions: 0,
            session_start: None,
        }
    }
}

impl SessionStats {
    pub fn start_session(&mut self) {
        self.session_start = Some(Local::now());
    }

    pub fn complete_focus_session(&mut self, duration: Duration) {
        self.sessions_completed += 1;
        self.today_sessions += 1;
        self.total_focus_time += duration;
        self.today_focus_time += duration;
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
