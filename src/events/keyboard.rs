use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    TogglePause,
    Reset,
    Skip,
    ToggleHelp,
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
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,
        _ => Action::None,
    }
}
