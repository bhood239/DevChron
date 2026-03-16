use super::theme::Theme;
use crate::timer::{PomodoroTimer, TimerPhase};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

// ─── Big-digit block art ────────────────────────────────────────────────────
//
// Each digit is rendered as a 5-row × 4-col pattern using full-block (█) and
// space characters. The colon separator is 5-row × 2-col.

const DIGITS: [&[&str]; 10] = [
    // 0
    &["█████", "█   █", "█   █", "█   █", "█████"],
    // 1
    &["  █  ", "  █  ", "  █  ", "  █  ", "  █  "],
    // 2
    &["█████", "    █", "█████", "█    ", "█████"],
    // 3
    &["█████", "    █", "█████", "    █", "█████"],
    // 4
    &["█   █", "█   █", "█████", "    █", "    █"],
    // 5
    &["█████", "█    ", "█████", "    █", "█████"],
    // 6
    &["█████", "█    ", "█████", "█   █", "█████"],
    // 7
    &["█████", "    █", "    █", "    █", "    █"],
    // 8
    &["█████", "█   █", "█████", "█   █", "█████"],
    // 9
    &["█████", "█   █", "█████", "    █", "█████"],
];

const COLON: &[&str] = &["   ", " █ ", "   ", " █ ", "   "];

/// Render a big-digit time string (e.g. "25:04") into a vec of 5 Lines.
fn big_digit_lines(time_str: &str, color: ratatui::style::Color) -> Vec<Line<'static>> {
    let mut rows: [String; 5] = Default::default();

    for ch in time_str.chars() {
        let pattern: &[&str] = if ch == ':' {
            COLON
        } else if ch.is_ascii_digit() {
            let d = ch as usize - '0' as usize;
            DIGITS[d]
        } else {
            continue;
        };

        for (i, row) in pattern.iter().enumerate() {
            rows[i].push_str(row);
            rows[i].push(' '); // inter-character gap
        }
    }

    rows.iter()
        .map(|row| {
            Line::from(Span::styled(
                row.clone(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ))
        })
        .collect()
}

// ─── Public entry point ─────────────────────────────────────────────────────

pub fn render(
    f: &mut Frame,
    timer: &PomodoroTimer,
    theme: &Theme,
    show_help: bool,
    minimal_mode: bool,
    celebration_ticks: u8,
) {
    let size = f.size();

    // Minimum terminal size guard
    if size.width < 60 || size.height < 16 {
        render_too_small(f, size, theme);
        return;
    }

    if show_help {
        render_help(f, size, theme);
        return;
    }

    if minimal_mode {
        render_minimal(f, size, timer, theme);
        return;
    }

    render_full(f, size, timer, theme, celebration_ticks);
}

// ─── Full layout ─────────────────────────────────────────────────────────────

fn render_full(
    f: &mut Frame,
    size: Rect,
    timer: &PomodoroTimer,
    theme: &Theme,
    celebration_ticks: u8,
) {
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme.border));

    let inner = main_block.inner(size);
    f.render_widget(main_block, size);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Timer
            Constraint::Length(7), // Footer
        ])
        .split(inner);

    render_header(f, chunks[0], timer, theme);
    render_timer(f, chunks[1], timer, theme, celebration_ticks);
    render_footer(f, chunks[2], timer, theme);
}

// ─── Header ──────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = get_phase_color(timer.current_timer.phase, theme);
    let is_paused = !timer.is_running();

    let (phase_kanji, phase_name) = match timer.current_timer.phase {
        TimerPhase::Focus => ("焦 点", "FOCUS"),
        TimerPhase::ShortBreak => ("小休憩", "SHORT BREAK"),
        TimerPhase::LongBreak => ("長休憩", "LONG BREAK"),
    };

    let session_text = format!(
        "{:02}/{:02}",
        timer.cycle_count + 1,
        timer.cycles_before_long_break
    );

    // Row 1: branding │ phase  [PAUSED badge if paused]
    let mut row1_spans = vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            "DevChron",
            Style::default()
                .fg(theme.focus_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", Style::default().fg(theme.border)),
        Span::styled(
            phase_kanji,
            Style::default()
                .fg(phase_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" [", Style::default().fg(theme.text)),
        Span::styled(
            phase_name,
            Style::default()
                .fg(phase_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("]", Style::default().fg(theme.text)),
    ];

    if is_paused {
        row1_spans.push(Span::styled(
            "  ⏸ PAUSED",
            Style::default()
                .fg(theme.paused_color)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Row 3: separator ─── session counter
    let session_label = format!(
        " {:02}/{:02} [SESSION] ",
        timer.cycle_count + 1,
        timer.cycles_before_long_break
    );
    let used = session_label.len() + 2;
    let sep_width = (area.width as usize).saturating_sub(used);

    let header_text = vec![
        Line::from(row1_spans),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("{:─<width$}", "", width = sep_width),
                Style::default().fg(theme.border),
            ),
            Span::styled(" ", Style::default()),
            Span::styled(
                &session_text,
                Style::default()
                    .fg(phase_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" [SESSION] ", Style::default().fg(theme.text)),
        ]),
    ];

    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.border)),
    );

    f.render_widget(header, area);
}

// ─── Timer area ──────────────────────────────────────────────────────────────

fn render_timer(
    f: &mut Frame,
    area: Rect,
    timer: &PomodoroTimer,
    theme: &Theme,
    celebration_ticks: u8,
) {
    let phase_color = if timer.is_running() {
        get_phase_color(timer.current_timer.phase, theme)
    } else {
        theme.paused_color
    };

    let never_started = !timer.is_running()
        && timer.current_timer.remaining == timer.current_timer.duration
        && timer.stats.sessions_completed == 0;

    // Layout: padding / big digits (5 rows) / progress bar / hint / padding
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Length(5), // Big digit rows
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Progress bar
            Constraint::Length(1), // Hint / celebration
            Constraint::Percentage(15),
        ])
        .split(area);

    // Big digit clock
    let time_str = timer.current_timer.format_time();
    let digit_lines = big_digit_lines(&time_str, phase_color);
    let time_display = Paragraph::new(digit_lines).alignment(Alignment::Center);
    f.render_widget(time_display, chunks[1]);

    // Progress bar
    render_progress_bar(f, chunks[3], timer, theme);

    // Hint / celebration line
    if celebration_ticks > 0 {
        let celebration = Paragraph::new(Line::from(Span::styled(
            "✦  CYCLE COMPLETE  ✦",
            Style::default()
                .fg(theme.focus_color)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center);
        f.render_widget(celebration, chunks[4]);
    } else if never_started {
        let hint = Paragraph::new(Line::from(Span::styled(
            "Press SPACE to start",
            Style::default()
                .fg(theme.dim)
                .add_modifier(Modifier::ITALIC),
        )))
        .alignment(Alignment::Center);
        f.render_widget(hint, chunks[4]);
    }
}

// ─── Progress bar ────────────────────────────────────────────────────────────

fn render_progress_bar(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = if timer.is_running() {
        get_phase_color(timer.current_timer.phase, theme)
    } else {
        theme.paused_color
    };

    let progress = timer.current_timer.percentage_complete();

    // Label: "進捗 [PROGRESS]  " — measure actual char width
    let label = "  進捗 [PROGRESS]  ";
    // Each CJK char is 2 columns wide; ASCII is 1. Approximate:
    // "  " (2) + "進捗" (4) + " [PROGRESS]  " (13) = 19 visible cols
    let label_cols: usize = 19;
    let bar_width = (area.width as usize).saturating_sub(label_cols + 2);
    let filled = (bar_width * progress as usize) / 100;
    let empty = bar_width.saturating_sub(filled).saturating_sub(1);

    let bar = format!("{}◯{}", "━".repeat(filled), "─".repeat(empty),);

    let progress_text = vec![Line::from(vec![
        Span::styled(label, Style::default().fg(theme.text)),
        Span::styled(bar, Style::default().fg(phase_color)),
    ])];

    let progress_widget = Paragraph::new(progress_text).alignment(Alignment::Center);
    f.render_widget(progress_widget, area);
}

// ─── Footer ──────────────────────────────────────────────────────────────────

fn render_footer(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35), // Statistics
            Constraint::Percentage(65), // Controls
        ])
        .split(area);

    render_stats(f, chunks[0], timer, theme);
    render_controls(f, chunks[1], timer, theme);
}

fn render_stats(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = get_phase_color(timer.current_timer.phase, theme);

    let stats_text = vec![
        Line::from(vec![
            Span::styled(
                " 統計 ",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled("[STATISTICS]", Style::default().fg(theme.text)),
        ]),
        Line::from(Span::styled(
            " ━━━━━━━━━━━━",
            Style::default().fg(theme.border),
        )),
        Line::from(vec![
            Span::styled(" 今日 ", Style::default().fg(theme.text)),
            Span::styled("[Daily]  ", Style::default().fg(theme.dim)),
            Span::styled(
                timer.stats.format_today_time(),
                Style::default()
                    .fg(phase_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" 完了 ", Style::default().fg(theme.text)),
            Span::styled("[Done]   ", Style::default().fg(theme.dim)),
            Span::styled(
                format!("{:02}", timer.stats.sessions_completed),
                Style::default()
                    .fg(phase_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" 連勝 ", Style::default().fg(theme.text)),
            Span::styled("[Streak] ", Style::default().fg(theme.dim)),
            Span::styled(
                format!("{:02}", timer.cycle_count),
                Style::default()
                    .fg(phase_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let stats = Paragraph::new(stats_text).block(
        Block::default()
            .borders(Borders::RIGHT | Borders::TOP)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.border)),
    );

    f.render_widget(stats, area);
}

fn render_controls(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = get_phase_color(timer.current_timer.phase, theme);
    let pause_label = if timer.is_running() {
        "静 [Pause]"
    } else {
        "再開 [Start]"
    };

    let key = |k: &'static str| {
        Span::styled(
            k,
            Style::default()
                .fg(phase_color)
                .add_modifier(Modifier::BOLD),
        )
    };
    let desc = |d: &'static str| Span::styled(d, Style::default().fg(theme.text));
    let sep = || Span::styled("  ", Style::default());

    let controls_text = vec![
        Line::from(vec![
            Span::styled(
                " 操作 ",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled("[CONTROLS]", Style::default().fg(theme.text)),
        ]),
        Line::from(Span::styled(
            " ━━━━━━━━━━━━",
            Style::default().fg(theme.border),
        )),
        Line::from(vec![
            key(" ［ｐ］ "),
            Span::styled(pause_label, Style::default().fg(theme.text)),
            sep(),
            key("［ｒ］ "),
            desc("戻 [Reset]"),
        ]),
        Line::from(vec![
            key(" ［ｓ］ "),
            desc("進 [Skip] "),
            sep(),
            key("［ｔ］ "),
            desc("色 [Theme]"),
        ]),
        Line::from(vec![
            key(" ［＋］ "),
            desc("+5m      "),
            sep(),
            key("［－］ "),
            desc("-5m      "),
        ]),
        Line::from(vec![
            key(" ［ｍ］ "),
            desc("最小 [Minimal]"),
            sep(),
            key("［ｑ］ "),
            desc("終 [Quit]"),
        ]),
    ];

    let controls = Paragraph::new(controls_text).block(
        Block::default()
            .borders(Borders::TOP)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.border)),
    );

    f.render_widget(controls, area);
}

// ─── Minimal mode ────────────────────────────────────────────────────────────

fn render_minimal(f: &mut Frame, size: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = if timer.is_running() {
        get_phase_color(timer.current_timer.phase, theme)
    } else {
        theme.paused_color
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme.border));
    let inner = outer.inner(size);
    f.render_widget(outer, size);

    // Vertically center the 5-row big digit display
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let time_str = timer.current_timer.format_time();
    let digit_lines = big_digit_lines(&time_str, phase_color);
    let time_display = Paragraph::new(digit_lines).alignment(Alignment::Center);
    f.render_widget(time_display, chunks[1]);

    // Phase indicator below the clock
    let (phase_kanji, phase_name) = match timer.current_timer.phase {
        TimerPhase::Focus => ("焦 点", "FOCUS"),
        TimerPhase::ShortBreak => ("小休憩", "SHORT BREAK"),
        TimerPhase::LongBreak => ("長休憩", "LONG BREAK"),
    };
    let paused_suffix = if !timer.is_running() { "  ⏸" } else { "" };
    let phase_line = Paragraph::new(Line::from(vec![
        Span::styled(
            phase_kanji,
            Style::default()
                .fg(phase_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" [{}]{}", phase_name, paused_suffix),
            Style::default().fg(theme.text),
        ),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(phase_line, chunks[2]);
}

// ─── Help screen ─────────────────────────────────────────────────────────────

fn render_help(f: &mut Frame, area: Rect, theme: &Theme) {
    // Clear the background first for a clean overlay
    f.render_widget(Clear, area);

    let key_col = Style::default()
        .fg(theme.focus_color)
        .add_modifier(Modifier::BOLD);
    let desc_col = Style::default().fg(theme.text);
    let dim_col = Style::default().fg(theme.dim);

    let row = |k: &'static str, d: &'static str| {
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(k, key_col),
            Span::styled("  ", Style::default()),
            Span::styled(d, desc_col),
        ])
    };
    let divider = || Line::from(Span::styled("  ─────────────────────────────", dim_col));

    let help_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  DevChron — Keyboard Reference",
            Style::default()
                .fg(theme.focus_color)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        divider(),
        Line::from(Span::styled("  Timer", dim_col)),
        divider(),
        row("Space / P", "Start / Pause timer"),
        row("R        ", "Reset current phase"),
        row("S        ", "Skip to next phase"),
        row("+ / =    ", "Add 5 minutes"),
        row("- / _    ", "Subtract 5 minutes"),
        Line::from(""),
        divider(),
        Line::from(Span::styled("  Display", dim_col)),
        divider(),
        row("T        ", "Cycle colour theme"),
        row("M        ", "Toggle minimal mode"),
        row("H / ?    ", "Toggle this help screen"),
        Line::from(""),
        divider(),
        Line::from(Span::styled("  App", dim_col)),
        divider(),
        row("Q / Esc  ", "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to return...",
            Style::default()
                .fg(theme.dim)
                .add_modifier(Modifier::ITALIC),
        )),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.border)),
        )
        .alignment(Alignment::Left);

    f.render_widget(help, area);
}

// ─── Too small ───────────────────────────────────────────────────────────────

fn render_too_small(f: &mut Frame, area: Rect, theme: &Theme) {
    let msg = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "Terminal too small",
            Style::default()
                .fg(theme.focus_color)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Please resize to at least 60×16",
            Style::default().fg(theme.text),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.border)),
    )
    .alignment(Alignment::Center);

    f.render_widget(msg, area);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn get_phase_color(phase: TimerPhase, theme: &Theme) -> ratatui::style::Color {
    match phase {
        TimerPhase::Focus => theme.focus_color,
        TimerPhase::ShortBreak => theme.short_break_color,
        TimerPhase::LongBreak => theme.long_break_color,
    }
}
