use crate::timer::TimerPhase;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    /// Esc: closes the open dialog/help/analytics first; quits when nothing is open.
    Escape,
    TogglePause,
    Reset,
    Skip,
    /// Directly choose Focus (`F`) or Long Break (`L`) on the timer page.
    SelectPhase(TimerPhase),
    /// Choose Break time (`B`): switches to Short Break (or Long Break if already on Short Break).
    ToggleBreak,
    /// Cycle the Focus category (`C` / `Shift+Tab`): Coding -> Learning -> Reading -> Coding
    /// (or cycle the category filter on the Analytics page).
    CycleCategory,
    /// Scroll up in Analytics session history (`Up` / `K`).
    ScrollUp,
    /// Scroll down in Analytics session history (`Down` / `J`).
    ScrollDown,
    ToggleHelp,
    /// Open or close the dedicated Analytics page (G / D / I)
    ToggleAnalytics,
    /// Add 5 minutes to the current phase (+)
    AddTime,
    /// Subtract 5 minutes from the current phase (-)
    SubtractTime,
    /// Toggle minimal mode (M)
    ToggleMinimal,
    /// Select the Nth Pomodoro mode (keys 1-9, zero-based index)
    SelectMode(usize),
    /// Select the next Pomodoro mode (Tab)
    CycleMode,
    /// Stop the ringing alarm (A) – only does anything while it rings
    StopAlarm,
    /// Confirm a pending mode switch (Y / Enter)
    Confirm,
    /// Decline a pending mode switch (N)
    Cancel,
    None,
}

pub fn handle_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,
        KeyCode::Char('q') | KeyCode::Char('Q') => Action::Quit,
        KeyCode::Esc => Action::Escape,
        KeyCode::Char(' ') | KeyCode::Char('p') | KeyCode::Char('P') => Action::TogglePause,
        KeyCode::Char('r') | KeyCode::Char('R') => Action::Reset,
        KeyCode::Char('s') | KeyCode::Char('S') => Action::Skip,
        KeyCode::Char('f') | KeyCode::Char('F') => Action::SelectPhase(TimerPhase::Focus),
        KeyCode::Char('b') | KeyCode::Char('B') => Action::ToggleBreak,
        KeyCode::Char('l') | KeyCode::Char('L') => Action::SelectPhase(TimerPhase::LongBreak),
        KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::BackTab => Action::CycleCategory,
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => Action::ScrollUp,
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => Action::ScrollDown,
        KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Char('g')
        | KeyCode::Char('G')
        | KeyCode::Char('d')
        | KeyCode::Char('D')
        | KeyCode::Char('i')
        | KeyCode::Char('I') => Action::ToggleAnalytics,
        KeyCode::Char('+') | KeyCode::Char('=') => Action::AddTime,
        KeyCode::Char('-') | KeyCode::Char('_') => Action::SubtractTime,
        KeyCode::Char('m') | KeyCode::Char('M') => Action::ToggleMinimal,
        KeyCode::Char(c @ '1'..='9') => Action::SelectMode(c as usize - '1' as usize),
        KeyCode::Char('a') | KeyCode::Char('A') => Action::StopAlarm,
        KeyCode::Tab => Action::CycleMode,
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => Action::Confirm,
        KeyCode::Char('n') | KeyCode::Char('N') => Action::Cancel,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn number_keys_select_modes_by_zero_based_index() {
        assert_eq!(handle_key(key(KeyCode::Char('1'))), Action::SelectMode(0));
        assert_eq!(handle_key(key(KeyCode::Char('2'))), Action::SelectMode(1));
        assert_eq!(handle_key(key(KeyCode::Char('3'))), Action::SelectMode(2));
        assert_eq!(handle_key(key(KeyCode::Char('9'))), Action::SelectMode(8));
        assert_eq!(handle_key(key(KeyCode::Char('0'))), Action::None);
    }

    #[test]
    fn phase_category_and_scroll_keys_map_to_actions() {
        assert_eq!(
            handle_key(key(KeyCode::Char('f'))),
            Action::SelectPhase(TimerPhase::Focus)
        );
        assert_eq!(
            handle_key(key(KeyCode::Char('F'))),
            Action::SelectPhase(TimerPhase::Focus)
        );
        assert_eq!(handle_key(key(KeyCode::Char('b'))), Action::ToggleBreak);
        assert_eq!(handle_key(key(KeyCode::Char('B'))), Action::ToggleBreak);
        assert_eq!(
            handle_key(key(KeyCode::Char('l'))),
            Action::SelectPhase(TimerPhase::LongBreak)
        );
        assert_eq!(handle_key(key(KeyCode::Char('c'))), Action::CycleCategory);
        assert_eq!(handle_key(key(KeyCode::Char('C'))), Action::CycleCategory);
        assert_eq!(handle_key(key(KeyCode::BackTab)), Action::CycleCategory);
        assert_eq!(handle_key(key(KeyCode::Up)), Action::ScrollUp);
        assert_eq!(handle_key(key(KeyCode::Down)), Action::ScrollDown);
        assert_eq!(handle_key(key(KeyCode::Char('k'))), Action::ScrollUp);
        assert_eq!(handle_key(key(KeyCode::Char('j'))), Action::ScrollDown);
    }

    #[test]
    fn existing_bindings_are_unchanged() {
        assert_eq!(handle_key(key(KeyCode::Char(' '))), Action::TogglePause);
        assert_eq!(handle_key(key(KeyCode::Char('p'))), Action::TogglePause);
        assert_eq!(handle_key(key(KeyCode::Char('r'))), Action::Reset);
        assert_eq!(handle_key(key(KeyCode::Char('s'))), Action::Skip);
        assert_eq!(handle_key(key(KeyCode::Char('m'))), Action::ToggleMinimal);
        assert_eq!(handle_key(key(KeyCode::Char('+'))), Action::AddTime);
        assert_eq!(handle_key(key(KeyCode::Char('-'))), Action::SubtractTime);
        assert_eq!(handle_key(key(KeyCode::Char('q'))), Action::Quit);
        assert_eq!(handle_key(key(KeyCode::Char('?'))), Action::ToggleHelp);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(handle_key(ctrl_c), Action::Quit);
    }

    #[test]
    fn a_stops_the_alarm() {
        assert_eq!(handle_key(key(KeyCode::Char('a'))), Action::StopAlarm);
        assert_eq!(handle_key(key(KeyCode::Char('A'))), Action::StopAlarm);
    }

    #[test]
    fn g_toggles_the_analytics_page() {
        assert_eq!(
            handle_key(key(KeyCode::Char('g'))),
            Action::ToggleAnalytics
        );
        assert_eq!(
            handle_key(key(KeyCode::Char('G'))),
            Action::ToggleAnalytics
        );
        assert_eq!(
            handle_key(key(KeyCode::Char('d'))),
            Action::ToggleAnalytics
        );
    }

    #[test]
    fn confirmation_and_navigation_keys() {
        assert_eq!(handle_key(key(KeyCode::Tab)), Action::CycleMode);
        assert_eq!(handle_key(key(KeyCode::Enter)), Action::Confirm);
        assert_eq!(handle_key(key(KeyCode::Char('y'))), Action::Confirm);
        assert_eq!(handle_key(key(KeyCode::Char('n'))), Action::Cancel);
        assert_eq!(handle_key(key(KeyCode::Esc)), Action::Escape);
    }
}
