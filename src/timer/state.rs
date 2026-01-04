use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    Running,
    Paused,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerPhase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl TimerPhase {
    pub fn display_name(&self) -> &str {
        match self {
            TimerPhase::Focus => "Focus Time",
            TimerPhase::ShortBreak => "Short Break",
            TimerPhase::LongBreak => "Long Break",
        }
    }

    pub fn emoji(&self) -> &str {
        match self {
            TimerPhase::Focus => "🍅",
            TimerPhase::ShortBreak => "☕",
            TimerPhase::LongBreak => "🌴",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Timer {
    pub phase: TimerPhase,
    pub state: TimerState,
    pub duration: Duration,
    pub remaining: Duration,
}

impl Timer {
    pub fn new(phase: TimerPhase, duration: Duration) -> Self {
        Self {
            phase,
            state: TimerState::Paused,
            duration,
            remaining: duration,
        }
    }

    pub fn start(&mut self) {
        self.state = TimerState::Running;
    }

    pub fn pause(&mut self) {
        self.state = TimerState::Paused;
    }

    pub fn reset(&mut self) {
        self.remaining = self.duration;
        self.state = TimerState::Paused;
    }

    pub fn tick(&mut self) -> bool {
        if self.state != TimerState::Running {
            return false;
        }

        if self.remaining > Duration::from_secs(1) {
            self.remaining -= Duration::from_secs(1);
            false
        } else {
            self.remaining = Duration::ZERO;
            self.state = TimerState::Completed;
            true
        }
    }

    pub fn is_running(&self) -> bool {
        self.state == TimerState::Running
    }

    pub fn is_completed(&self) -> bool {
        self.state == TimerState::Completed
    }

    pub fn percentage_complete(&self) -> u16 {
        let total = self.duration.as_secs() as f64;
        let elapsed = (self.duration.as_secs() - self.remaining.as_secs()) as f64;
        ((elapsed / total) * 100.0) as u16
    }

    pub fn format_time(&self) -> String {
        let total_secs = self.remaining.as_secs();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }
}
