use crate::timer::TimerPhase;
use notify_rust::{Notification, Timeout, Urgency};

pub struct NotificationManager {
    enabled: bool,
}

impl NotificationManager {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn send_phase_complete(&self, phase: TimerPhase) {
        if !self.enabled {
            return;
        }

        let (summary, body, urgency) = match phase {
            TimerPhase::Focus => (
                "Focus Time Complete! 🎉",
                "Great work! Time for a break.",
                Urgency::Normal,
            ),
            TimerPhase::ShortBreak => (
                "Break Over",
                "Ready to focus again?",
                Urgency::Normal,
            ),
            TimerPhase::LongBreak => (
                "Long Break Complete",
                "Feeling refreshed? Let's go!",
                Urgency::Low,
            ),
        };

        if let Err(e) = Notification::new()
            .summary(summary)
            .body(body)
            .icon("clock")
            .urgency(urgency)
            .timeout(Timeout::Milliseconds(5000))
            .show()
        {
            eprintln!("Failed to send notification: {}", e);
        }
    }
}
