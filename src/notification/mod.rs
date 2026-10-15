use crate::timer::pomodoro::PhaseEvent;
use notify_rust::{Notification, Timeout, Urgency};

pub struct NotificationManager {
    enabled: bool,
}

impl NotificationManager {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn send_phase_complete(&self, event: PhaseEvent) {
        if !self.enabled {
            return;
        }

        let (summary, body, urgency) = match event {
            PhaseEvent::FocusComplete => (
                "🍅 Focus Time Complete!",
                "Great work! Time for a break.",
                Urgency::Normal,
            ),
            PhaseEvent::ShortBreakComplete => {
                ("☕ Break Over", "Ready to focus again?", Urgency::Normal)
            }
            PhaseEvent::LongBreakComplete => (
                "🌴 Long Break Complete",
                "Feeling refreshed? Let's get back to work!",
                Urgency::Low,
            ),
            PhaseEvent::CycleComplete => (
                "🎉 Cycle Complete!",
                "Full Pomodoro cycle done. Outstanding focus!",
                Urgency::Normal,
            ),
        };

        // `show()` talks to the notification daemon over D-Bus and blocks until
        // it answers – seconds, if no daemon is running. Doing that on the UI
        // thread froze the whole app (including the `A` key) right when the
        // alarm started, so it runs on its own short-lived thread. Errors are
        // ignored on purpose: printing to stderr would garble the TUI.
        std::thread::spawn(move || {
            let _ = Notification::new()
                .summary(summary)
                .body(body)
                .icon("clock")
                .urgency(urgency)
                .timeout(Timeout::Milliseconds(5000))
                .show();
        });
    }
}
