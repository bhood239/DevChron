use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph},
    Frame,
};
use crate::timer::{PomodoroTimer, TimerPhase};
use super::theme::Theme;

pub fn render(f: &mut Frame, timer: &PomodoroTimer, theme: &Theme, show_help: bool) {
    let size = f.size();
    
    if show_help {
        render_help(f, size, theme);
        return;
    }
    
    // Main layout with custom borders
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme.border));
    
    let inner = main_block.inner(size);
    f.render_widget(main_block, size);
    
    // Split into header, content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header with phase and session
            Constraint::Min(10),    // Main timer area
            Constraint::Length(7),  // Stats and controls
        ])
        .split(inner);
    
    render_header(f, chunks[0], timer, theme);
    render_timer(f, chunks[1], timer, theme);
    render_footer(f, chunks[2], timer, theme);
}

fn render_header(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = get_phase_color(timer.current_timer.phase, theme);
    
    let (phase_kanji, phase_name) = match timer.current_timer.phase {
        TimerPhase::Focus => ("焦 点", "FOCUS"),
        TimerPhase::ShortBreak => ("小休憩", "SHORT BREAK"),
        TimerPhase::LongBreak => ("長休憩", "LONG BREAK"),
    };
    
    let session_text = format!("{:02}/{:02}", 
        timer.cycle_count + 1, 
        timer.cycles_before_long_break
    );
    
    let branding = "DevChron";
    let phase_section = format!(" {} [{}] ", phase_kanji, phase_name);
    let session_section = format!(" {} [SESSION] ", session_text);
    
    // Calculate separator width to balance the header
    let used_width = branding.len() + phase_section.len() + session_section.len() + 4;
    let separator_width = (area.width as usize).saturating_sub(used_width);
    
    let header_text = vec![
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(branding, Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)),
            Span::styled(" │ ", Style::default().fg(theme.border)),
            Span::styled(phase_kanji, Style::default().fg(phase_color).add_modifier(Modifier::BOLD)),
            Span::styled(" [", Style::default().fg(theme.text)),
            Span::styled(phase_name, Style::default().fg(phase_color).add_modifier(Modifier::BOLD)),
            Span::styled("] ", Style::default().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("{:─<width$}", "", width = separator_width),
                Style::default().fg(theme.border)
            ),
            Span::styled(" ", Style::default()),
            Span::styled(&session_text, Style::default().fg(phase_color).add_modifier(Modifier::BOLD)),
            Span::styled(" [", Style::default().fg(theme.text)),
            Span::styled("SESSION", Style::default().fg(theme.text)),
            Span::styled("] ", Style::default().fg(theme.text)),
        ]),
    ];
    
    let header = Paragraph::new(header_text)
        .block(Block::default()
            .borders(Borders::BOTTOM)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.border))
        );
    
    f.render_widget(header, area);
}

fn render_timer(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = if timer.is_running() {
        get_phase_color(timer.current_timer.phase, theme)
    } else {
        theme.paused_color
    };
    
    // Center the timer display
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(5),  // Timer
            Constraint::Length(3),  // Progress bar
            Constraint::Percentage(30),
        ])
        .split(area);
    
    // Large time display with full-width numbers
    let time_str = timer.current_timer.format_time();
    let full_width_time = convert_to_fullwidth(&time_str);
    
    let time_text = vec![
        Line::from(""),
        Line::from(
            Span::styled(
                full_width_time,
                Style::default()
                    .fg(phase_color)
                    .add_modifier(Modifier::BOLD)
            )
        ),
        Line::from(""),
    ];
    
    let time_display = Paragraph::new(time_text)
        .alignment(Alignment::Center);
    
    f.render_widget(time_display, chunks[1]);
    
    // Progress bar with custom characters
    render_progress_bar(f, chunks[2], timer, theme);
}

fn render_progress_bar(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = if timer.is_running() {
        get_phase_color(timer.current_timer.phase, theme)
    } else {
        theme.paused_color
    };
    
    let progress = timer.current_timer.percentage_complete();
    let bar_width = (area.width as usize).saturating_sub(40);
    let filled = (bar_width * progress as usize) / 100;
    
    let progress_bar = "━".repeat(filled) + "◯" + &"─".repeat(bar_width.saturating_sub(filled));
    
    let progress_text = vec![
        Line::from(vec![
            Span::styled("        進捗 ", Style::default().fg(theme.text)),
            Span::styled("[PROGRESS]  ", Style::default().fg(theme.text)),
            Span::styled(&progress_bar, Style::default().fg(phase_color)),
            Span::styled("        ", Style::default()),
        ]),
    ];
    
    let progress = Paragraph::new(progress_text)
        .alignment(Alignment::Center);
    
    f.render_widget(progress, area);
}

fn render_footer(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),  // Statistics
            Constraint::Percentage(65),  // Controls
        ])
        .split(area);
    
    // Statistics
    let stats_text = vec![
        Line::from(vec![
            Span::styled(" 統計 ", Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
            Span::styled("[STATISTICS]", Style::default().fg(theme.text)),
        ]),
        Line::from(
            Span::styled(" ━━━━━━━━━━━━", Style::default().fg(theme.border))
        ),
        Line::from(vec![
            Span::styled(" 今日 ", Style::default().fg(theme.text)),
            Span::styled("[Daily]  ", Style::default().fg(theme.text)),
            Span::styled(timer.stats.format_today_time(), Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(" 完了 ", Style::default().fg(theme.text)),
            Span::styled("[Done]   ", Style::default().fg(theme.text)),
            Span::styled(format!("{:02}", timer.stats.sessions_completed), Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(" 連勝 ", Style::default().fg(theme.text)),
            Span::styled("[Streak] ", Style::default().fg(theme.text)),
            Span::styled(format!("{:02}", timer.cycle_count), Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)),
        ]),
    ];
    
    let stats = Paragraph::new(stats_text)
        .block(Block::default()
            .borders(Borders::RIGHT | Borders::TOP)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.border))
        );
    
    f.render_widget(stats, chunks[0]);
    
    // Controls
    let state_text = if timer.is_running() { "静 [Pause]" } else { "再開 [Start]" };
    let phase_color = get_phase_color(timer.current_timer.phase, theme);
    
    let controls_text = vec![
        Line::from(vec![
            Span::styled(" 操作 ", Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
            Span::styled("[CONTROLS]", Style::default().fg(theme.text)),
        ]),
        Line::from(
            Span::styled(" ━━━━━━━━━━━━", Style::default().fg(theme.border))
        ),
        Line::from(vec![
            Span::styled(" ［ｐ］ ", Style::default().fg(phase_color).add_modifier(Modifier::BOLD)),
            Span::styled(state_text, Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled(" ［ｒ］ ", Style::default().fg(phase_color).add_modifier(Modifier::BOLD)),
            Span::styled("戻 [Reset]", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled(" ［ｓ］ ", Style::default().fg(phase_color).add_modifier(Modifier::BOLD)),
            Span::styled("進 [Skip] ", Style::default().fg(theme.text)),
            Span::styled("       ", Style::default()),
            Span::styled("［ｑ］ ", Style::default().fg(phase_color).add_modifier(Modifier::BOLD)),
            Span::styled("終 [Quit]", Style::default().fg(theme.text)),
        ]),
    ];
    
    let controls = Paragraph::new(controls_text)
        .block(Block::default()
            .borders(Borders::TOP)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.border))
        );
    
    f.render_widget(controls, chunks[1]);
}

fn render_help(f: &mut Frame, area: Rect, theme: &Theme) {
    let help_text = vec![
        Line::from(""),
        Line::from(
            Span::styled(
                "DevChron - Pomodoro Timer",
                Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)
            )
        ),
        Line::from(""),
        Line::from(vec![
            Span::styled("Space/P   ", Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)),
            Span::styled("Start/Pause timer", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("R         ", Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)),
            Span::styled("Reset current timer", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("S         ", Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)),
            Span::styled("Skip to next phase", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("Q / Esc   ", Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)),
            Span::styled("Quit application", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("H / ?     ", Style::default().fg(theme.focus_color).add_modifier(Modifier::BOLD)),
            Span::styled("Toggle this help screen", Style::default().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(
            Span::styled(
                "Press any key to return...",
                Style::default().fg(theme.paused_color).add_modifier(Modifier::ITALIC)
            )
        ),
    ];
    
    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title("Help")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.border))
        )
        .alignment(Alignment::Center);
    
    f.render_widget(help, area);
}

fn get_phase_color(phase: TimerPhase, theme: &Theme) -> ratatui::style::Color {
    match phase {
        TimerPhase::Focus => theme.focus_color,
        TimerPhase::ShortBreak => theme.short_break_color,
        TimerPhase::LongBreak => theme.long_break_color,
    }
}

fn convert_to_fullwidth(s: &str) -> String {
    s.chars().map(|c| match c {
        '0' => '０',
        '1' => '１',
        '2' => '２',
        '3' => '３',
        '4' => '４',
        '5' => '５',
        '6' => '６',
        '7' => '７',
        '8' => '８',
        '9' => '９',
        ':' => '：',
        ' ' => ' ',
        _ => c,
    }).collect()
}
