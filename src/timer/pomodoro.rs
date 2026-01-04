use std::time::Duration;
use super::state::{Timer, TimerPhase, TimerState};
use super::session::SessionStats;

#[derive(Debug)]
pub struct PomodoroTimer {
    pub current_timer: Timer,
    pub cycle_count: u32,
    pub cycles_before_long_break: u32,
    pub focus_duration: Duration,
    pub short_break_duration: Duration,
    pub long_break_duration: Duration,
    pub stats: SessionStats,
}

impl PomodoroTimer {
    pub fn new(
        focus_mins: u64,
        short_break_mins: u64,
        long_break_mins: u64,
        cycles_before_long_break: u32,
    ) -> Self {
        let focus_duration = Duration::from_secs(focus_mins * 60);
        Self {
            current_timer: Timer::new(TimerPhase::Focus, focus_duration),
            cycle_count: 0,
            cycles_before_long_break,
            focus_duration,
            short_break_duration: Duration::from_secs(short_break_mins * 60),
            long_break_duration: Duration::from_secs(long_break_mins * 60),
            stats: SessionStats::default(),
        }
    }

    pub fn toggle(&mut self) {
        match self.current_timer.state {
            TimerState::Running => self.current_timer.pause(),
            TimerState::Paused | TimerState::Completed => {
                self.current_timer.start();
                if self.stats.session_start.is_none() {
                    self.stats.start_session();
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.current_timer.reset();
    }

    pub fn skip(&mut self) {
        self.advance_to_next_phase();
    }

    pub fn tick(&mut self) -> bool {
        let completed = self.current_timer.tick();
        if completed {
            self.advance_to_next_phase();
            true
        } else {
            false
        }
    }

    fn advance_to_next_phase(&mut self) {
        match self.current_timer.phase {
            TimerPhase::Focus => {
                self.stats.complete_focus_session(
                    self.focus_duration - self.current_timer.remaining
                );
                self.cycle_count += 1;
                
                let next_phase = if self.cycle_count >= self.cycles_before_long_break {
                    self.cycle_count = 0;
                    TimerPhase::LongBreak
                } else {
                    TimerPhase::ShortBreak
                };
                
                let duration = match next_phase {
                    TimerPhase::ShortBreak => self.short_break_duration,
                    TimerPhase::LongBreak => self.long_break_duration,
                    _ => unreachable!(),
                };
                
                self.current_timer = Timer::new(next_phase, duration);
            }
            TimerPhase::ShortBreak | TimerPhase::LongBreak => {
                self.stats.complete_break_session(
                    self.current_timer.duration - self.current_timer.remaining
                );
                self.current_timer = Timer::new(TimerPhase::Focus, self.focus_duration);
            }
        }
    }

    pub fn current_phase(&self) -> TimerPhase {
        self.current_timer.phase
    }

    pub fn is_running(&self) -> bool {
        self.current_timer.is_running()
    }

    pub fn session_info(&self) -> String {
        format!("{}/{}", self.cycle_count + 1, self.cycles_before_long_break)
    }
}
