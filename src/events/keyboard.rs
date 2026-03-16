use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    TogglePause,
    Reset,
    Skip,
    ToggleHelp,
    /// Cycle to the next colour theme (T)
    CycleTheme,
    /// Add 5 minutes to the current phase (+)
    AddTime,
    /// Subtract 5 minutes from the current phase (-)
    SubtractTime,
    /// Toggle minimal mode (M)
    ToggleMinimal,
    /// Open the task-tag input overlay (N)
    OpenTaskInput,
    /// Switch to a numbered profile (1-based index)
    SwitchProfile(usize),
    /// Toggle the session history overlay (V)
    ToggleHistory,
    None,
}

pub fn handle_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Action::Quit,
        KeyCode::Esc => Action::Quit,
        KeyCode::Char(' ') | KeyCode::Char('p') | KeyCode::Char('P') => Action::TogglePause,
        KeyCode::Char('r') | KeyCode::Char('R') => Action::Reset,
        KeyCode::Char('s') | KeyCode::Char('S') => Action::Skip,
        KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Char('t') | KeyCode::Char('T') => Action::CycleTheme,
        KeyCode::Char('+') | KeyCode::Char('=') => Action::AddTime,
        KeyCode::Char('-') | KeyCode::Char('_') => Action::SubtractTime,
        KeyCode::Char('m') | KeyCode::Char('M') => Action::ToggleMinimal,
        KeyCode::Char('n') | KeyCode::Char('N') => Action::OpenTaskInput,
        KeyCode::Char('v') | KeyCode::Char('V') => Action::ToggleHistory,
        // Profile switching: 1, 2, 3 (up to 9)
        KeyCode::Char(c @ '1'..='9') => Action::SwitchProfile((c as usize) - ('0' as usize)),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,
        _ => Action::None,
    }
}
