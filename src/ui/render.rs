use super::theme::Theme;
use crate::timer::session::DaySummary;
use crate::timer::{PomodoroTimer, TimerPhase};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

// ─── Big-digit block art ─────────────────────────────────────────────────────

const DIGITS: [&[&str]; 10] = [
    &["█████", "█   █", "█   █", "█   █", "█████"], // 0
    &["  █  ", "  █  ", "  █  ", "  █  ", "  █  "], // 1
    &["█████", "    █", "█████", "█    ", "█████"], // 2
    &["█████", "    █", "█████", "    █", "█████"], // 3
    &["█   █", "█   █", "█████", "    █", "    █"], // 4
    &["█████", "█    ", "█████", "    █", "█████"], // 5
    &["█████", "█    ", "█████", "█   █", "█████"], // 6
    &["█████", "    █", "    █", "    █", "    █"], // 7
    &["█████", "█   █", "█████", "█   █", "█████"], // 8
    &["█████", "█   █", "█████", "    █", "█████"], // 9
];

const COLON: &[&str] = &["   ", " █ ", "   ", " █ ", "   "];

fn big_digit_lines(time_str: &str, color: ratatui::style::Color) -> Vec<Line<'static>> {
    let mut rows: [String; 5] = Default::default();
    for ch in time_str.chars() {
        let pattern: &[&str] = if ch == ':' {
            COLON
        } else if ch.is_ascii_digit() {
            DIGITS[ch as usize - '0' as usize]
        } else {
            continue;
        };
        for (i, row) in pattern.iter().enumerate() {
            rows[i].push_str(row);
            rows[i].push(' ');
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

// ─── Public entry point ──────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    timer: &PomodoroTimer,
    theme: &Theme,
    show_help: bool,
    minimal_mode: bool,
    celebration_ticks: u8,
    task_input_buf: Option<&str>,
    show_task_input: bool,
    profile_name: &str,
    current_task: Option<&str>,
    show_history: bool,
    auto_start_countdown: u64,
    quit_confirm: bool,
    quit_confirm_ticks: u8,
    sound_enabled: bool,
) {
    let size = f.size();

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
    } else {
        render_full(
            f,
            size,
            timer,
            theme,
            celebration_ticks,
            profile_name,
            current_task,
            auto_start_countdown,
            sound_enabled,
        );
    }

    // Overlays render on top of everything (quit confirm is topmost).
    if show_history {
        render_history(f, size, timer, theme);
    }
    if show_task_input {
        render_task_input(f, size, theme, task_input_buf.unwrap_or(""));
    }
    if quit_confirm {
        render_quit_confirm(f, size, theme, quit_confirm_ticks);
    }
}

// ─── Full layout ─────────────────────────────────────────────────────────────

fn render_full(
    f: &mut Frame,
    size: Rect,
    timer: &PomodoroTimer,
    theme: &Theme,
    celebration_ticks: u8,
    profile_name: &str,
    current_task: Option<&str>,
    auto_start_countdown: u64,
    sound_enabled: bool,
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
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(7),
        ])
        .split(inner);

    render_header(f, chunks[0], timer, theme, profile_name, current_task);
    render_timer(
        f,
        chunks[1],
        timer,
        theme,
        celebration_ticks,
        auto_start_countdown,
    );
    render_footer(f, chunks[2], timer, theme, sound_enabled);
}

// ─── Header ──────────────────────────────────────────────────────────────────

fn render_header(
    f: &mut Frame,
    area: Rect,
    timer: &PomodoroTimer,
    theme: &Theme,
    profile_name: &str,
    current_task: Option<&str>,
) {
    let phase_color = get_phase_color(timer.current_timer.phase, theme);
    let is_paused = !timer.is_running();

    let (phase_kanji, phase_name) = match timer.current_timer.phase {
        TimerPhase::Focus => ("焦 点", "FOCUS"),
        TimerPhase::ShortBreak => ("小休憩", "SHORT BREAK"),
        TimerPhase::LongBreak => ("長休憩", "LONG BREAK"),
    };

    let mut row1: Vec<Span> = vec![
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
        row1.push(Span::styled(
            "  ⏸ PAUSED",
            Style::default()
                .fg(theme.paused_color)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(task) = current_task {
        let truncated = if task.len() > 28 {
            format!("{}…", &task[..27])
        } else {
            task.to_string()
        };
        row1.push(Span::styled("  ·  ", Style::default().fg(theme.border)));
        row1.push(Span::styled(
            truncated,
            Style::default()
                .fg(theme.text)
                .add_modifier(Modifier::ITALIC),
        ));
    }

    let session_text = format!(
        "{:02}/{:02}",
        timer.cycle_count + 1,
        timer.cycles_before_long_break
    );
    let profile_label = format!(" [{}] ", profile_name.to_uppercase());
    let right_section = format!(" {}  [SESSION]{}", session_text, profile_label);
    let sep_width = (area.width as usize).saturating_sub(right_section.len() + 2);

    let header_text = vec![
        Line::from(row1),
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
            Span::styled(" [SESSION]", Style::default().fg(theme.text)),
            Span::styled(
                profile_label,
                Style::default()
                    .fg(theme.dim)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]),
    ];

    f.render_widget(
        Paragraph::new(header_text).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.border)),
        ),
        area,
    );
}

// ─── Timer area ──────────────────────────────────────────────────────────────

fn render_timer(
    f: &mut Frame,
    area: Rect,
    timer: &PomodoroTimer,
    theme: &Theme,
    celebration_ticks: u8,
    auto_start_countdown: u64,
) {
    let phase_color = if timer.is_running() {
        get_phase_color(timer.current_timer.phase, theme)
    } else {
        theme.paused_color
    };

    let never_started = !timer.is_running()
        && timer.current_timer.remaining == timer.current_timer.duration
        && timer.stats.sessions_completed == 0;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Percentage(15),
        ])
        .split(area);

    let time_str = timer.current_timer.format_time();
    f.render_widget(
        Paragraph::new(big_digit_lines(&time_str, phase_color)).alignment(Alignment::Center),
        chunks[1],
    );

    render_progress_bar(f, chunks[3], timer, theme);

    // Bottom hint line: celebration > auto-start countdown > first-launch hint
    if celebration_ticks > 0 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "✦  CYCLE COMPLETE  ✦",
                Style::default()
                    .fg(theme.focus_color)
                    .add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Center),
            chunks[4],
        );
    } else if auto_start_countdown > 0 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("Starting in {}…", auto_start_countdown),
                Style::default()
                    .fg(theme.dim)
                    .add_modifier(Modifier::ITALIC),
            )))
            .alignment(Alignment::Center),
            chunks[4],
        );
    } else if never_started {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Press SPACE to start",
                Style::default()
                    .fg(theme.dim)
                    .add_modifier(Modifier::ITALIC),
            )))
            .alignment(Alignment::Center),
            chunks[4],
        );
    }
}

// ─── Progress bar ─────────────────────────────────────────────────────────────

fn render_progress_bar(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = if timer.is_running() {
        get_phase_color(timer.current_timer.phase, theme)
    } else {
        theme.paused_color
    };

    let progress = timer.current_timer.percentage_complete();
    let label_cols: usize = 19;
    let bar_width = (area.width as usize).saturating_sub(label_cols + 2);
    let filled = (bar_width * progress as usize) / 100;
    let empty = bar_width.saturating_sub(filled).saturating_sub(1);
    let bar = format!("{}◯{}", "━".repeat(filled), "─".repeat(empty));

    f.render_widget(
        Paragraph::new(vec![Line::from(vec![
            Span::styled("  進捗 [PROGRESS]  ", Style::default().fg(theme.text)),
            Span::styled(bar, Style::default().fg(phase_color)),
        ])])
        .alignment(Alignment::Center),
        area,
    );
}

// ─── Footer ───────────────────────────────────────────────────────────────────

fn render_footer(
    f: &mut Frame,
    area: Rect,
    timer: &PomodoroTimer,
    theme: &Theme,
    sound_enabled: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    render_stats(f, chunks[0], timer, theme);
    render_controls(f, chunks[1], timer, theme, sound_enabled);
}

fn render_stats(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = get_phase_color(timer.current_timer.phase, theme);

    // Build a mini bar chart from the weekly data.
    // Each day gets one column of up to 3 block chars tall.
    let weekly = &timer.stats.weekly;

    // Only consider days that actually have focus time when computing the scale.
    // If nobody has any time yet, max_mins stays 0 and all bars are empty.
    let max_mins = weekly.iter().map(|d| d.focus_mins).max().unwrap_or(0);

    let bar_height = 3usize;
    let mut chart_rows: Vec<Line> = Vec::new();

    for row in (0..bar_height).rev() {
        // row 2 (top) fills when day >= 2/3 of max; row 1 >= 1/3; row 0 (bottom) > 0.
        let spans: Vec<Span> = std::iter::once(Span::styled(" ", Style::default()))
            .chain(weekly.iter().map(|day| {
                let filled = if max_mins == 0 {
                    false
                } else {
                    // Each row lights up when the day's minutes exceed that fraction of max.
                    // row 0 = bottom: any focus time at all
                    // row 1 = middle: > 1/3 of the best day
                    // row 2 = top:    > 2/3 of the best day
                    day.focus_mins * bar_height as u64 > max_mins * row as u64
                };
                let color = if filled { phase_color } else { theme.dim };
                Span::styled(
                    if filled { "██ " } else { "   " },
                    Style::default().fg(color),
                )
            }))
            .collect();
        chart_rows.push(Line::from(spans));
    }

    // Day labels row
    let label_spans: Vec<Span> = std::iter::once(Span::styled(" ", Style::default()))
        .chain(weekly.iter().map(|day| {
            Span::styled(
                format!("{} ", &day.label[..2]), // "Mo", "Tu", …
                Style::default().fg(theme.dim),
            )
        }))
        .collect();
    chart_rows.push(Line::from(label_spans));

    let stats_text = vec![
        Line::from(vec![
            Span::styled(
                " 統計 ",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled("[STATISTICS]", Style::default().fg(theme.text)),
        ]),
        chart_rows[0].clone(),
        chart_rows[1].clone(),
        chart_rows[2].clone(),
        chart_rows[3].clone(),
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
    ];

    f.render_widget(
        Paragraph::new(stats_text).block(
            Block::default()
                .borders(Borders::RIGHT | Borders::TOP)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.border)),
        ),
        area,
    );
}

fn render_controls(
    f: &mut Frame,
    area: Rect,
    timer: &PomodoroTimer,
    theme: &Theme,
    sound_enabled: bool,
) {
    let phase_color = get_phase_color(timer.current_timer.phase, theme);
    let pause_label = if timer.is_running() {
        "静 [Pause]"
    } else {
        "再開 [Start]"
    };
    let sound_label = if sound_enabled {
        "音 [Sound ✓]"
    } else {
        "音 [Sound ✗]"
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
            key(" ［ｎ］ "),
            desc("作業 [Task]"),
            sep(),
            key("［ｖ］ "),
            desc("履歴 [Hist]"),
        ]),
        Line::from(vec![
            key(" ［ｂ］ "),
            Span::styled(sound_label, Style::default().fg(theme.text)),
            sep(),
            key("１２３  "),
            desc("Profile"),
        ]),
    ];

    f.render_widget(
        Paragraph::new(controls_text).block(
            Block::default()
                .borders(Borders::TOP)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.border)),
        ),
        area,
    );
}

// ─── Session history overlay ──────────────────────────────────────────────────

fn render_history(f: &mut Frame, size: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let popup_w = size.width.min(62);
    let sessions = &timer.stats.completed_sessions;
    // Height: title + divider + up to 12 sessions + empty-state + footer = dynamic
    let content_rows = (sessions.len().max(1) + 4) as u16;
    let popup_h = content_rows.min(size.height.saturating_sub(4));
    let x = (size.width.saturating_sub(popup_w)) / 2;
    let y = (size.height.saturating_sub(popup_h)) / 2;
    let area = Rect::new(x, y, popup_w, popup_h);

    f.render_widget(Clear, area);

    let dim_s = Style::default().fg(theme.dim);
    let text_s = Style::default().fg(theme.text);
    let phase_c = get_phase_color(timer.current_timer.phase, theme);

    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Today's Sessions",
            Style::default()
                .fg(theme.focus_color)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  ─────────────────────────────────────────────────",
            dim_s,
        )),
    ];

    if sessions.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No sessions completed yet today.",
            Style::default()
                .fg(theme.dim)
                .add_modifier(Modifier::ITALIC),
        )));
    } else {
        // Show most recent first, up to 12 entries.
        let visible: Vec<_> = sessions.iter().rev().take(12).collect();
        for s in visible {
            let mins = s.duration_secs / 60;
            let task_str = s.task.as_deref().unwrap_or("—");
            let task_display = if task_str.len() > 36 {
                format!("{}…", &task_str[..35])
            } else {
                task_str.to_string()
            };
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{}", s.ended_at),
                    Style::default().fg(phase_c).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(format!("{:>2}m", mins), text_s),
                Span::styled("  ", Style::default()),
                Span::styled(
                    task_display,
                    Style::default()
                        .fg(theme.text)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }
        if sessions.len() > 12 {
            lines.push(Line::from(Span::styled(
                format!("  … and {} more", sessions.len() - 12),
                dim_s,
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press V to close",
        Style::default()
            .fg(theme.dim)
            .add_modifier(Modifier::ITALIC),
    )));

    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" 履歴 History ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.border)),
        ),
        area,
    );
}

// ─── Task input overlay ───────────────────────────────────────────────────────

fn render_task_input(f: &mut Frame, size: Rect, theme: &Theme, buf: &str) {
    let popup_w = size.width.min(54);
    let popup_h = 7u16;
    let x = (size.width.saturating_sub(popup_w)) / 2;
    let y = (size.height.saturating_sub(popup_h)) / 2;
    let area = Rect::new(x, y, popup_w, popup_h);

    f.render_widget(Clear, area);

    let display = format!("{}▋", buf);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  What are you working on?",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                display,
                Style::default()
                    .fg(theme.focus_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Enter to confirm · Esc to cancel",
            Style::default()
                .fg(theme.dim)
                .add_modifier(Modifier::ITALIC),
        )),
    ];

    f.render_widget(
        Paragraph::new(content).block(
            Block::default()
                .title(" 作業 Task ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(theme.focus_color)),
        ),
        area,
    );
}

// ─── Minimal mode ─────────────────────────────────────────────────────────────

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
    f.render_widget(
        Paragraph::new(big_digit_lines(&time_str, phase_color)).alignment(Alignment::Center),
        chunks[1],
    );

    let (phase_kanji, phase_name) = match timer.current_timer.phase {
        TimerPhase::Focus => ("焦 点", "FOCUS"),
        TimerPhase::ShortBreak => ("小休憩", "SHORT BREAK"),
        TimerPhase::LongBreak => ("長休憩", "LONG BREAK"),
    };
    let paused_suffix = if !timer.is_running() { "  ⏸" } else { "" };

    f.render_widget(
        Paragraph::new(Line::from(vec![
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
        .alignment(Alignment::Center),
        chunks[2],
    );
}

// ─── Help screen ──────────────────────────────────────────────────────────────

fn render_help(f: &mut Frame, area: Rect, theme: &Theme) {
    f.render_widget(Clear, area);

    let key_s = Style::default()
        .fg(theme.focus_color)
        .add_modifier(Modifier::BOLD);
    let desc_s = Style::default().fg(theme.text);
    let dim_s = Style::default().fg(theme.dim);

    let row = |k: &'static str, d: &'static str| {
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(k, key_s),
            Span::styled("  ", Style::default()),
            Span::styled(d, desc_s),
        ])
    };
    let divider = || Line::from(Span::styled("  ─────────────────────────────", dim_s));

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
        Line::from(Span::styled("  Timer", dim_s)),
        divider(),
        row("Space / P", "Start / Pause timer"),
        row("R        ", "Reset current phase"),
        row("S        ", "Skip to next phase"),
        row("+ / =    ", "Add 5 minutes"),
        row("- / _    ", "Subtract 5 minutes"),
        Line::from(""),
        divider(),
        Line::from(Span::styled("  Task & Profiles", dim_s)),
        divider(),
        row("N        ", "Set task tag for this session"),
        row("V        ", "View today's session history"),
        row("1 / 2 / 3", "Switch timer profile"),
        Line::from(""),
        divider(),
        Line::from(Span::styled("  Display", dim_s)),
        divider(),
        row("T        ", "Cycle colour theme"),
        row("B        ", "Toggle sound on/off"),
        row("M        ", "Toggle minimal mode"),
        row("H / ?    ", "Toggle this help screen"),
        Line::from(""),
        divider(),
        Line::from(Span::styled("  App", dim_s)),
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

    f.render_widget(
        Paragraph::new(help_text)
            .block(
                Block::default()
                    .title(" Help ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(theme.border)),
            )
            .alignment(Alignment::Left),
        area,
    );
}

// ─── Too small ────────────────────────────────────────────────────────────────

fn render_too_small(f: &mut Frame, area: Rect, theme: &Theme) {
    f.render_widget(
        Paragraph::new(vec![
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
        .alignment(Alignment::Center),
        area,
    );
}

// ─── Quit confirm popup ───────────────────────────────────────────────────────

fn render_quit_confirm(f: &mut Frame, size: Rect, theme: &Theme, ticks_left: u8) {
    let popup_w = size.width.min(44);
    let popup_h = 5u16;
    let x = (size.width.saturating_sub(popup_w)) / 2;
    let y = (size.height.saturating_sub(popup_h)) / 2;
    let area = Rect::new(x, y, popup_w, popup_h);

    f.render_widget(Clear, area);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Press Q or Esc again to quit",
            Style::default()
                .fg(theme.focus_color)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Any other key to cancel  ({}s)", ticks_left),
            Style::default()
                .fg(theme.dim)
                .add_modifier(Modifier::ITALIC),
        )),
    ];

    f.render_widget(
        Paragraph::new(content)
            .block(
                Block::default()
                    .title(" Quit? ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(theme.focus_color)),
            )
            .alignment(Alignment::Center),
        area,
    );
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn get_phase_color(phase: TimerPhase, theme: &Theme) -> ratatui::style::Color {
    match phase {
        TimerPhase::Focus => theme.focus_color,
        TimerPhase::ShortBreak => theme.short_break_color,
        TimerPhase::LongBreak => theme.long_break_color,
    }
}

// Silence unused import warning — DaySummary is used via timer.stats.weekly
#[allow(dead_code)]
fn _use_day_summary(_: &DaySummary) {}
