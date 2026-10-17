use super::ring::{self, Ring};
use super::style;
use super::theme::{blend_linear, Theme};
use crate::timer::session::{
    category_index, AnalyticsSummary, SessionStats, SystemIdentity, GRID_DAYS_PER_WEEK,
};
use crate::timer::{CycleReward, FocusCategory, PomodoroMode, PomodoroTimer, TimerPhase};
use chrono::{Datelike, Local, Timelike};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Everything the renderer needs for one frame (built by `App::view`).
pub struct UiState<'a> {
    pub timer: &'a PomodoroTimer,
    pub theme: &'a Theme,
    pub show_help: bool,
    pub show_analytics: bool,
    pub minimal_mode: bool,
    /// Show the "CYCLE COMPLETE" banner.
    pub celebrating: bool,
    pub modes: &'a [PomodoroMode],
    pub mode_index: usize,
    /// A mode switch that is waiting for the user's confirmation.
    pub pending_mode: Option<usize>,
    /// Full-cycle reward awaiting acknowledgment before the final long break begins.
    pub reward: Option<&'a CycleReward>,
    /// Normalized reveal animation progress (`0.0..=1.0`) for the reward modal.
    pub reward_anim_progress: f64,
    /// Milliseconds elapsed since the reward was unlocked.
    pub reward_elapsed_ms: u64,
    /// The phase reached 00:00 and the alarm is ringing.
    pub alarm_ringing: bool,
    /// Flashes on/off (twice a second) while the alarm rings.
    pub alarm_flash: bool,
    /// Offline system username and computer hostname.
    pub system_identity: &'a SystemIdentity,
    /// Optional category filter on the Analytics page (`None` = All categories).
    pub analytics_filter: Option<FocusCategory>,
    /// Scroll offset in the Analytics Recent sessions table.
    pub analytics_scroll: usize,
}

// ─── Big-digit block art ────────────────────────────────────────────────────

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

const DIGITS_MEDIUM: [&[&str]; 10] = [
    &["███", "█ █", "█ █", "█ █", "███"],
    &[" █ ", "██ ", " █ ", " █ ", "███"],
    &["███", "  █", "███", "█  ", "███"],
    &["███", "  █", "███", "  █", "███"],
    &["█ █", "█ █", "███", "  █", "  █"],
    &["███", "█  ", "███", "  █", "███"],
    &["███", "█  ", "███", "█ █", "███"],
    &["███", "  █", "  █", "  █", "  █"],
    &["███", "█ █", "███", "█ █", "███"],
    &["███", "█ █", "███", "  █", "███"],
];

const COLON_MEDIUM: &[&str] = &[" ", "█", " ", "█", " "];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DigitStyle {
    /// 5×5 block digits.
    Large,
    /// 5×3 block digits.
    Medium,
    /// Plain bold text on one row (very small terminals).
    Compact,
}

impl DigitStyle {
    fn rows(self) -> usize {
        match self {
            DigitStyle::Compact => 1,
            _ => 5,
        }
    }

    fn glyph_width(self, ch: char) -> usize {
        match (self, ch) {
            (DigitStyle::Large, ':') => 3,
            (DigitStyle::Large, _) => 5,
            (DigitStyle::Medium, ':') => 1,
            (DigitStyle::Medium, _) => 3,
            (DigitStyle::Compact, _) => 1,
        }
    }

    /// Display width in terminal columns of `time_str` (1 column between glyphs).
    fn text_width(self, time_str: &str) -> usize {
        let n = time_str.chars().count();
        let glyphs: usize = time_str.chars().map(|c| self.glyph_width(c)).sum();
        match self {
            DigitStyle::Compact => glyphs,
            _ => glyphs + n.saturating_sub(1),
        }
    }
}

fn fits_in_ring(width_cols: usize, edge_rows: f64, r_in: f64) -> bool {
    let corner = (width_cols as f64).hypot(edge_rows * 4.0);
    corner + 1.5 <= r_in
}

fn choose_digit_style(time_str: &str, r_in: f64) -> DigitStyle {
    for style in [DigitStyle::Large, DigitStyle::Medium] {
        let edge = style.rows() as f64 / 2.0;
        if fits_in_ring(style.text_width(time_str), edge, r_in) {
            return style;
        }
    }
    DigitStyle::Compact
}

fn block_lines_with_colon(
    time_str: &str,
    digits: &[&[&str]; 10],
    colon: &[&str],
    digit_color: Color,
    digit_end_color: Color,
    colon_color: Color,
) -> Vec<Line<'static>> {
    let mut rows: [Vec<Span<'static>>; 5] = Default::default();
    let mut first = true;
    let c_style = Style::default().fg(colon_color).add_modifier(Modifier::BOLD);
    let row_styles: [Style; 5] = std::array::from_fn(|r| {
        let t = (r as f64 / 4.0) * 0.65;
        Style::default()
            .fg(blend_linear(digit_color, digit_end_color, t))
            .add_modifier(Modifier::BOLD)
    });

    for ch in time_str.chars() {
        let is_colon = ch == ':';
        let pattern: &[&str] = if is_colon {
            colon
        } else if ch.is_ascii_digit() {
            digits[ch as usize - '0' as usize]
        } else {
            continue;
        };

        for (i, row) in pattern.iter().enumerate() {
            let st = if is_colon { c_style } else { row_styles[i] };
            if !first {
                rows[i].push(Span::styled(" ", row_styles[i]));
            }
            rows[i].push(Span::styled((*row).to_string(), st));
        }
        first = false;
    }

    rows.into_iter().map(Line::from).collect()
}

/// Render a time string (e.g. "25:04") into lines using the given style.
#[cfg(test)]
fn digit_lines(time_str: &str, style: DigitStyle, color: Color) -> Vec<Line<'static>> {
    digit_lines_styled(time_str, style, color, color, color)
}

fn digit_lines_styled(
    time_str: &str,
    style: DigitStyle,
    digit_color: Color,
    digit_end_color: Color,
    colon_color: Color,
) -> Vec<Line<'static>> {
    match style {
        DigitStyle::Large => block_lines_with_colon(
            time_str,
            &DIGITS,
            COLON,
            digit_color,
            digit_end_color,
            colon_color,
        ),
        DigitStyle::Medium => block_lines_with_colon(
            time_str,
            &DIGITS_MEDIUM,
            COLON_MEDIUM,
            digit_color,
            digit_end_color,
            colon_color,
        ),
        DigitStyle::Compact => vec![Line::from(Span::styled(
            time_str.to_string(),
            Style::default().fg(digit_color).add_modifier(Modifier::BOLD),
        ))],
    }
}

// ─── Public entry point ─────────────────────────────────────────────────────

pub fn render(f: &mut Frame, ui: &UiState) {
    let size = f.size();

    if size.width < 60 || size.height < 16 {
        render_too_small(f, size, ui.theme);
        return;
    }

    if ui.show_help {
        render_help(f, size, ui);
        return;
    }

    if ui.show_analytics {
        render_analytics(f, size, ui);
        return;
    }

    if ui.minimal_mode {
        render_minimal(f, size, ui);
    } else {
        render_full(f, size, ui);
    }

    if let Some(reward) = ui.reward {
        render_reward(f, size, ui, reward);
    }

    if let Some(idx) = ui.pending_mode {
        if let Some(mode) = ui.modes.get(idx) {
            render_confirm(f, size, ui, mode);
        }
    }
}

// ─── Full Timer Layout (Calm, Spacious, Single Rounded Frame) ───────────────

const FULL_FOOTER_MIN_HEIGHT: u16 = 22;

fn render_full(f: &mut Frame, size: Rect, ui: &UiState) {
    let main_block = style::screen_block(ui.theme);
    let inner = main_block.inner(size);
    f.render_widget(main_block, size);

    let header_height = if inner.height >= 18 { 4 } else { 3 };
    let compact = inner.height < FULL_FOOTER_MIN_HEIGHT;
    let footer_height = if compact { 1 } else { 3 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(0),
            Constraint::Length(footer_height),
        ])
        .split(inner);

    render_header(f, chunks[0], ui);
    render_timer(f, chunks[1], ui);
    if compact {
        render_compact_footer(f, chunks[2], ui);
    } else {
        render_footer(f, chunks[2], ui);
    }
}

// ─── Header ──────────────────────────────────────────────────────────────────

fn phase_labels(phase: TimerPhase) -> (&'static str, &'static str) {
    match phase {
        TimerPhase::Focus => ("Focus", "FOCUS"),
        TimerPhase::ShortBreak => ("Short break", "SHORT BREAK"),
        TimerPhase::LongBreak => ("Long break", "LONG BREAK"),
    }
}

fn render_header(f: &mut Frame, area: Rect, ui: &UiState) {
    let theme = ui.theme;
    let timer = ui.timer;
    let is_paused = !timer.is_running();
    let width = area.width as usize;

    // Row 1: ◆ DevChron · Phase selector chips ([f Focus]  b Short break  l Long break) ... g Analytics
    let mut row1_spans = vec![
        Span::raw("  "),
        Span::styled("◆ ", style::accent_bold(theme.gold)),
        Span::styled("DevChron", style::value_bold(theme)),
        Span::styled("  ·  ", style::label(theme)),
    ];
    row1_spans.extend(phase_tabs_spans(ui, width >= 78));

    if area.height < 4 {
        let cat_col = theme.category_color(timer.category);
        row1_spans.push(Span::styled(" · ", style::label(theme)));
        row1_spans.push(Span::styled("c ", style::keycap(theme.gold)));
        row1_spans.push(Span::styled(timer.category.label(), style::accent_bold(cat_col)));
    } else if ui.reward.is_some() {
        row1_spans.push(Span::styled(
            "  ✦ CYCLE COMPLETE",
            Style::default()
                .fg(theme.today_color())
                .add_modifier(Modifier::BOLD),
        ));
    } else if timer.alarm_ringing {
        row1_spans.push(Span::styled(
            "  ALARM",
            Style::default()
                .fg(if ui.alarm_flash { theme.gold } else { theme.rose })
                .add_modifier(Modifier::BOLD),
        ));
    } else if is_paused {
        row1_spans.push(Span::styled(
            "  paused",
            Style::default().fg(theme.amber),
        ));
    } else {
        row1_spans.push(Span::styled(
            "  ● live",
            Style::default().fg(theme.emerald).add_modifier(Modifier::BOLD),
        ));
    }

    let mut row1 = Line::from(row1_spans);
    let right_key = "g";
    let right_lbl = " Analytics  ";
    let right_w = right_key.len() + right_lbl.len();
    let row1_left_w = row1.width();
    if row1_left_w + right_w + 2 <= width {
        let pad = width - row1_left_w - right_w;
        row1.spans.push(Span::raw(" ".repeat(pad)));
        row1.spans
            .push(Span::styled(right_key, style::keycap(theme.long_break_color)));
        row1.spans.push(Span::styled(
            right_lbl,
            Style::default().fg(theme.section_accent),
        ));
    }

    // Row 2: colorful mode tabs on the left, cycle dots + session counter on the right.
    let active_mode_col = theme.mode_color(ui.mode_index);
    let mut row2 = mode_tabs(ui, active_mode_col);
    let session_text = format!(
        "{:02}/{:02}",
        timer.display_cycle_number(),
        timer.cycles_before_long_break
    );
    let left_width = row2.width();
    let suffix = if left_width + session_text.len() + 10 <= width {
        Some(" session  ")
    } else if left_width + session_text.len() + 2 <= width {
        Some("  ")
    } else {
        None
    };
    if let Some(suffix) = suffix {
        let right_width = session_text.len() + suffix.len();
        row2.spans
            .push(Span::raw(" ".repeat(width - left_width - right_width)));
        row2.spans
            .push(Span::styled(session_text, style::accent_bold(theme.gold)));
        row2.spans.push(Span::styled(suffix, style::label(theme)));
    }

    let mut rows = vec![row1, row2];

    // Row 3: Focus category selector chips (Coding · Learning · Reading), each with its distinct color
    if area.height >= 4 {
        let mut row3 = category_tabs(ui);
        let today_secs = timer
            .stats
            .today_focus_time
            .as_secs()
            .saturating_add(timer.live_focus_secs());
        let cat_secs = timer
            .stats
            .category_focus_secs
            .get(timer.category.label())
            .copied()
            .unwrap_or(0)
            .saturating_add(timer.live_focus_secs());
        let cat_col = theme.category_color(timer.category);

        if width >= 88 {
            let s1 = format!("{} ", timer.category.label());
            let s2 = SessionStats::format_duration_secs(cat_secs);
            let s3 = "  ·  today ";
            let s4 = format!("{}  ", SessionStats::format_duration_secs(today_secs));
            let r3_right = s1.chars().count() + s2.chars().count() + s3.chars().count() + s4.chars().count();
            let r3_left = row3.width();
            if r3_left + r3_right + 2 <= width {
                row3.spans.push(Span::raw(" ".repeat(width - r3_left - r3_right)));
                row3.spans.push(Span::styled(s1, Style::default().fg(cat_col)));
                row3.spans.push(Span::styled(s2, style::accent_bold(cat_col)));
                row3.spans.push(Span::styled(s3, style::label(theme)));
                row3.spans.push(Span::styled(s4, style::accent_bold(theme.gold)));
            }
        } else {
            let s1 = "today ";
            let s2 = format!("{}  ", SessionStats::format_duration_secs(today_secs));
            let r3_right = s1.chars().count() + s2.chars().count();
            let r3_left = row3.width();
            if r3_left + r3_right + 2 <= width {
                row3.spans.push(Span::raw(" ".repeat(width - r3_left - r3_right)));
                row3.spans.push(Span::styled(s1, style::label(theme)));
                row3.spans.push(Span::styled(s2, style::accent_bold(theme.gold)));
            }
        }
        rows.push(row3);
    }

    let header = Paragraph::new(rows).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(style::rule_style(theme)),
    );

    f.render_widget(header, area);
}

fn phase_tabs_spans(ui: &UiState, full_labels: bool) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(TimerPhase::ALL.len() * 3);
    let active_phase = ui.timer.current_timer.phase;

    for (i, phase) in TimerPhase::ALL.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let p_color = get_phase_color(phase, ui.theme);
        let name = if full_labels {
            phase.label()
        } else {
            phase.short_label()
        };
        if phase == active_phase {
            spans.push(Span::styled(
                format!(" {} {} ", phase.key_hint(), name),
                Style::default()
                    .fg(p_color)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {}", phase.key_hint()),
                style::keycap(p_color),
            ));
            spans.push(Span::styled(format!(" {name} "), style::label(ui.theme)));
        }
    }

    spans
}

fn mode_tabs(ui: &UiState, accent: Color) -> Line<'static> {
    let mut spans = vec![Span::raw("  ")];

    for (i, mode) in ui.modes.iter().enumerate() {
        let mode_col = ui.theme.mode_color(i);
        if i == ui.mode_index {
            spans.push(Span::styled(
                format!(" {} {} · {} ", i + 1, mode.name, mode.summary()),
                Style::default()
                    .fg(accent)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {}", i + 1),
                style::keycap(mode_col),
            ));
            spans.push(Span::styled(
                format!(" {} ", mode.name),
                style::label(ui.theme),
            ));
        }
        spans.push(Span::raw(" "));
    }

    Line::from(spans)
}

/// Focus category selector row with distinct per-category colors.
fn category_tabs(ui: &UiState) -> Line<'static> {
    let theme = ui.theme;
    let active_cat = ui.timer.category;

    let mut spans = vec![
        Span::raw("  "),
        Span::styled("Category ", style::section_title(theme)),
        Span::styled("c", style::keycap(theme.gold)),
        Span::raw("  "),
    ];

    for cat in FocusCategory::ALL {
        let cat_col = theme.category_color(cat);
        if cat == active_cat {
            spans.push(Span::styled(
                format!(" ● {} ", cat.label()),
                Style::default()
                    .fg(cat_col)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ));
        } else {
            spans.push(Span::styled(" ●", Style::default().fg(cat_col)));
            spans.push(Span::styled(
                format!(" {} ", cat.label()),
                style::label(theme),
            ));
        }
        spans.push(Span::raw(" "));
    }

    Line::from(spans)
}

// ─── Timer area ──────────────────────────────────────────────────────────────

fn digit_color(ui: &UiState) -> Color {
    let (base_col, _) = ui
        .theme
        .active_timer_gradient(ui.timer.current_timer.phase, ui.timer.category);
    if ui.alarm_ringing {
        if ui.alarm_flash {
            ui.theme.gold
        } else {
            ui.theme.rose
        }
    } else if ui.timer.is_running() {
        base_col
    } else {
        blend_linear(base_col, ui.theme.paused_color, 0.42)
    }
}

fn ring_colors(ui: &UiState) -> (Color, Color) {
    if ui.reward.is_some() {
        let p = ui.reward_anim_progress.clamp(0.0, 1.0);
        let cat_col = ui.theme.category_color(ui.timer.category);
        let from = blend_linear(cat_col, ui.theme.today_color(), p);
        let to = blend_linear(ui.theme.emerald, ui.theme.gold, p);
        return (from, to);
    }
    let (mut from, mut to) = ui
        .theme
        .active_timer_gradient(ui.timer.current_timer.phase, ui.timer.category);
    if ui.alarm_ringing {
        if ui.alarm_flash {
            from = blend_linear(from, ui.theme.gold, 0.65);
            to = blend_linear(to, ui.theme.text, 0.65);
        }
    } else if !ui.timer.is_running() {
        from = blend_linear(from, ui.theme.paused_color, 0.35);
        to = blend_linear(to, ui.theme.paused_color, 0.35);
    }
    (from, to)
}

fn alarm_hint(ui: &UiState) -> Paragraph<'static> {
    let accent = get_phase_color(ui.timer.current_timer.phase, ui.theme);
    Paragraph::new(Line::from(vec![
        Span::styled("Time's up  ·  press ", style::label(ui.theme)),
        Span::styled("[A]", style::keycap(accent)),
        Span::styled(" to stop alarm", style::label(ui.theme)),
    ]))
    .alignment(Alignment::Center)
}

fn render_timer(f: &mut Frame, area: Rect, ui: &UiState) {
    let timer = ui.timer;
    let theme = ui.theme;
    let running = timer.is_running();
    let digit_color = digit_color(ui);

    let never_started = !running
        && timer.current_timer.remaining == timer.current_timer.duration
        && timer.stats.sessions_completed == 0;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let mut ring_area = chunks[0];
    if ring_area.height >= 16 {
        ring_area.y += 1;
        ring_area.height -= 2;
    } else if ring_area.height >= 10 {
        ring_area.y += 1;
        ring_area.height -= 1;
    }

    let (from, to) = ring_colors(ui);
    f.render_widget(
        Ring {
            progress: timer.current_timer.progress(),
            from,
            to,
            track: theme.border,
        },
        ring_area,
    );

    render_ring_contents(f, ring_area, ui, digit_color, true);

    let hint_area = chunks[1];
    if ui.alarm_ringing {
        f.render_widget(alarm_hint(ui), hint_area);
    } else if ui.celebrating {
        let celebration = Paragraph::new(Line::from(Span::styled(
            "✦  Cycle complete  ✦",
            style::accent_bold(theme.gold),
        )))
        .alignment(Alignment::Center);
        f.render_widget(celebration, hint_area);
    } else if never_started {
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("Press ", style::label(theme)),
            Span::styled("Space", style::keycap(theme.emerald)),
            Span::styled(" to start  ·  ", style::label(theme)),
            Span::styled("F/B/L", style::keycap(theme.cyan)),
            Span::styled(" phase  ·  ", style::label(theme)),
            Span::styled("C", style::keycap(theme.gold)),
            Span::styled(" category  ·  ", style::label(theme)),
            Span::styled("G", style::keycap(theme.long_break_color)),
            Span::styled(" analytics", style::label(theme)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(hint, hint_area);
    }
}

fn render_ring_contents(
    f: &mut Frame,
    ring_area: Rect,
    ui: &UiState,
    digit_color: Color,
    with_extras: bool,
) {
    let timer = ui.timer;
    let theme = ui.theme;
    let geo = ring::geometry(ring_area);

    let time_str = timer.current_timer.format_time();
    let style = choose_digit_style(&time_str, geo.r_in);
    let (_, phase_upper) = phase_labels(timer.current_timer.phase);
    let full_top_label = if timer.current_timer.phase == TimerPhase::Focus {
        format!("{phase_upper} · {}", timer.category.upper_label())
    } else {
        phase_upper.to_string()
    };
    let (status, status_col) = if ui.alarm_ringing {
        ("time's up", theme.rose)
    } else if timer.is_running() {
        ("running", theme.emerald)
    } else {
        ("paused", theme.amber)
    };

    let (_, grad_end) = theme.active_timer_gradient(timer.current_timer.phase, timer.category);
    let digit_end_color = if ui.alarm_ringing {
        digit_color
    } else if timer.is_running() {
        grad_end
    } else {
        blend_linear(grad_end, theme.paused_color, 0.42)
    };

    let colon_color = if timer.is_running() && !ui.alarm_ringing {
        if timer.current_timer.remaining_secs_ceil() % 2 == 1 {
            blend_linear(digit_color, theme.gold, 0.45)
        } else {
            digit_color
        }
    } else {
        digit_color
    };

    let label_edge = style.rows() as f64 / 2.0 + 2.0;
    let min_label_width = phase_upper.len().max(status.chars().count() + 1);
    let show_extras = with_extras && fits_in_ring(min_label_width, label_edge, geo.r_in);
    let show_full_top =
        show_extras && fits_in_ring(full_top_label.len(), label_edge, geo.r_in);

    let mut lines: Vec<Line> = Vec::new();
    if show_extras {
        if show_full_top && timer.current_timer.phase == TimerPhase::Focus {
            let cat_col = theme.category_color(timer.category);
            lines.push(Line::from(vec![
                Span::styled(phase_upper, style::accent_bold(theme.focus_color)),
                Span::styled(" · ", style::label(theme)),
                Span::styled(timer.category.upper_label(), style::accent_bold(cat_col)),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                phase_upper,
                style::accent_bold(get_phase_color(timer.current_timer.phase, theme)),
            )));
        }
        lines.push(Line::from(""));
    }
    lines.extend(digit_lines_styled(
        &time_str,
        style,
        digit_color,
        digit_end_color,
        colon_color,
    ));
    if show_extras {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            status,
            Style::default().fg(status_col),
        )));
    }

    let height = (lines.len() as u16).min(ring_area.height);
    let width = (lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16)
        .max(1)
        .min(ring_area.width);
    let rect = Rect {
        x: ring_area.x + (ring_area.width - width) / 2,
        y: ring_area.y + (ring_area.height - height) / 2,
        width,
        height,
    };

    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rect);
}

// ─── Footer (Vibrant Multi-Hue Keycap Bar) ──────────────────────────────────

fn render_footer(f: &mut Frame, area: Rect, ui: &UiState) {
    render_controls(f, area, ui.timer, ui.theme);
}

fn render_compact_footer(f: &mut Frame, area: Rect, ui: &UiState) {
    let theme = ui.theme;
    const HINTS: [(&str, &str); 9] = [
        ("p", "pause"),
        ("f/b", "phase"),
        ("c", "cat"),
        ("r", "reset"),
        ("s", "skip"),
        ("1-3", "mode"),
        ("g", "analytics"),
        ("h", "help"),
        ("q", "quit"),
    ];
    const DROP_ORDER: [&str; 6] = ["h", "s", "r", "c", "f/b", "g"];
    const RINGING_HINTS: [(&str, &str); 3] = [
        ("a", "stop alarm"),
        ("g", "analytics"),
        ("q", "quit"),
    ];

    let items: &[(&str, &str)] = if ui.alarm_ringing {
        &RINGING_HINTS
    } else {
        &HINTS
    };

    let width_of = |list: &[(&str, &str)]| -> usize {
        list.iter()
            .map(|(k, d)| k.chars().count() + 1 + d.len())
            .sum::<usize>()
            + list.len().saturating_sub(1) * 2
    };
    let mut active: Vec<(&str, &str)> = items.to_vec();
    for dropped in DROP_ORDER {
        if width_of(&active) <= area.width as usize {
            break;
        }
        active.retain(|(k, _)| *k != dropped);
    }

    let palette = [
        theme.emerald,
        theme.cyan,
        theme.gold,
        theme.amber,
        theme.focus_color,
        theme.long_break_color,
        theme.rose,
    ];
    let mut spans = Vec::new();
    for (i, (k, d)) in active.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let col = palette[i % palette.len()];
        spans.push(Span::styled(k.to_string(), style::keycap(col)));
        spans.push(Span::styled(format!(" {d}"), style::label(theme)));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

fn render_controls(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = get_phase_color(timer.current_timer.phase, theme);
    let pause_label = if timer.is_running() { "pause" } else { "start" };

    let desc = |d: &'static str| Span::styled(format!(" {d}"), style::label(theme));
    let sep = || Span::raw("   ");

    if timer.is_reward_pending() {
        let mut spans = vec![
            Span::styled("enter/space", style::keycap(theme.emerald)),
            Span::styled(" Start long break", style::accent_bold(theme.gold)),
            sep(),
        ];
        if timer.alarm_ringing {
            spans.push(Span::styled("a", style::keycap(theme.amber)));
            spans.push(desc("mute"));
            spans.push(sep());
        }
        spans.extend([
            Span::styled("r", style::keycap(theme.rose)),
            desc("reset"),
            sep(),
            Span::styled("g", style::keycap(theme.long_break_color)),
            desc("analytics"),
            sep(),
            Span::styled("q", style::keycap(theme.rose)),
            desc("quit"),
        ]);
        f.render_widget(
            Paragraph::new(vec![Line::from(spans)]).alignment(Alignment::Center),
            area,
        );
        return;
    }

    if timer.alarm_ringing {
        let lines = vec![Line::from(vec![
            Span::styled("a", style::keycap(theme.rose)),
            Span::styled(" Stop alarm", style::accent_bold(theme.gold)),
            sep(),
            Span::styled("g", style::keycap(theme.long_break_color)),
            desc("analytics"),
            sep(),
            Span::styled("m", style::keycap(theme.cyan)),
            desc("minimal"),
            sep(),
            Span::styled("h", style::keycap(theme.section_accent)),
            desc("help"),
            sep(),
            Span::styled("q", style::keycap(theme.rose)),
            desc("quit"),
        ])];
        f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
        return;
    }

    let row1_items: [(&str, &str); 6] = [
        ("space", pause_label),
        ("f/b/l", "focus/break"),
        ("c", "category"),
        ("1-3", "mode"),
        ("+/-", "±5m"),
        ("s", "skip"),
    ];
    let row1_drop: [&str; 2] = ["+/-", "s"];

    let row2_items: [(&str, &str); 5] = [
        ("r", "reset"),
        ("g", "analytics"),
        ("m", "minimal"),
        ("h", "help"),
        ("q", "quit"),
    ];
    let row2_drop: [&str; 1] = ["m"];

    let lines = vec![
        style::keycap_bar(&row1_items, &row1_drop, area.width, phase_color, theme),
        style::keycap_bar(&row2_items, &row2_drop, area.width, theme.cyan, theme),
    ];

    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}

// ─── Dedicated Offline Analytics Page ────────────────────────────────────────

pub fn empty_cell_color(theme: &Theme) -> Color {
    theme.empty_cell()
}

#[allow(dead_code)]
pub fn heatmap_cell_color(theme: &Theme, focus_secs: u64, max_daily_secs: u64) -> Color {
    if focus_secs == 0 {
        return empty_cell_color(theme);
    }
    let ratio = SessionStats::intensity_for(focus_secs, max_daily_secs);
    theme.heatmap_ramp(ratio)
}

/// Per-category heatmap cell colour: uses the dominant category's color ramp
/// (or the active filter category's ramp when a filter is active).
fn category_heatmap_color(
    theme: &Theme,
    category: FocusCategory,
    focus_secs: u64,
    max_daily_secs: u64,
) -> Color {
    if focus_secs == 0 {
        return empty_cell_color(theme);
    }
    let ratio = SessionStats::intensity_for(focus_secs, max_daily_secs);
    theme.category_ramp(category, ratio)
}


fn legend_intensity_color(
    theme: &Theme,
    category: Option<FocusCategory>,
    secs: u64,
    max_secs: u64,
) -> Color {
    let ratio = SessionStats::intensity_for(secs, max_secs);
    match category {
        Some(cat) => theme.category_ramp(cat, ratio),
        None => theme.heatmap_ramp(ratio),
    }
}

/// Format a single-color dynamic bar row (`left_label`, `bar`, `right_value`)
/// that expands to 100% of `width` columns.
fn dynamic_bar_row(
    left_label: String,
    left_style: Style,
    ratio: f64,
    bar_color: Color,
    empty_color: Color,
    right_text: String,
    right_style: Style,
    trailing_note: Option<(String, Style)>,
    width: u16,
) -> Line<'static> {
    let left_w = left_label.chars().count();
    let right_w = right_text.chars().count();
    let trail_w = trailing_note
        .as_ref()
        .map_or(0, |(s, _)| s.chars().count());
    let bar_w = (width as usize)
        .saturating_sub(left_w + right_w + trail_w)
        .max(4);
    let r = ratio.clamp(0.0, 1.0);
    let filled = if r <= 0.0 {
        0
    } else {
        ((r * bar_w as f64).round() as usize).clamp(1, bar_w)
    };
    let empty = bar_w.saturating_sub(filled);

    let mut spans = vec![
        Span::styled(left_label, left_style),
        Span::styled("▆".repeat(filled), Style::default().fg(bar_color)),
        Span::styled("·".repeat(empty), Style::default().fg(empty_color)),
        Span::styled(right_text, right_style),
    ];
    if let Some((t_str, t_style)) = trailing_note {
        spans.push(Span::styled(t_str, t_style));
    }
    Line::from(spans)
}

/// Format a **multi-color stacked category bar row** (`Coding` + `Learning` + `Reading`)
/// where each category segment is painted in its distinct theme color (`Coral`, `Aqua`, `Periwinkle`).
/// When `filter` is `Some(cat)`, renders only that category's portion in its signature color.
#[allow(clippy::too_many_arguments)]
fn stacked_category_bar_row(
    left_label: String,
    left_style: Style,
    cat_secs: [u64; 3],
    scale_max_secs: u64,
    filter: Option<FocusCategory>,
    theme: &Theme,
    right_text: String,
    right_style: Style,
    trailing_note: Option<(String, Style)>,
    width: u16,
) -> Line<'static> {
    if let Some(f_cat) = filter {
        let idx = category_index(f_cat.label());
        let secs = cat_secs[idx];
        let ratio = if scale_max_secs > 0 {
            (secs as f64 / scale_max_secs as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let col = theme.category_color(f_cat);
        return dynamic_bar_row(
            left_label,
            left_style,
            ratio,
            col,
            empty_cell_color(theme),
            right_text,
            right_style,
            trailing_note,
            width,
        );
    }

    let left_w = left_label.chars().count();
    let right_w = right_text.chars().count();
    let trail_w = trailing_note
        .as_ref()
        .map_or(0, |(s, _)| s.chars().count());
    let bar_w = (width as usize)
        .saturating_sub(left_w + right_w + trail_w)
        .max(4);

    let total_secs = cat_secs[0]
        .saturating_add(cat_secs[1])
        .saturating_add(cat_secs[2]);
    let ratio = if scale_max_secs > 0 {
        (total_secs as f64 / scale_max_secs as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let total_filled = if total_secs == 0 || ratio <= 0.0 {
        0
    } else {
        ((ratio * bar_w as f64).round() as usize).clamp(1, bar_w)
    };
    let empty = bar_w.saturating_sub(total_filled);

    let mut seg_cells = [0usize; 3];
    if total_filled > 0 && total_secs > 0 {
        let mut remaining_cells = total_filled;
        let non_zero_cats: Vec<usize> = (0..3).filter(|&i| cat_secs[i] > 0).collect();
        for (pos, &c_i) in non_zero_cats.iter().enumerate() {
            if pos + 1 == non_zero_cats.len() {
                seg_cells[c_i] = remaining_cells;
            } else {
                let share = cat_secs[c_i] as f64 / total_secs as f64;
                let cells = ((share * total_filled as f64).round() as usize)
                    .max(usize::from(total_filled >= non_zero_cats.len()))
                    .min(remaining_cells);
                seg_cells[c_i] = cells;
                remaining_cells = remaining_cells.saturating_sub(cells);
            }
        }
    }

    let mut spans = vec![Span::styled(left_label, left_style)];
    for (c_i, cat_enum) in FocusCategory::ALL.into_iter().enumerate() {
        if seg_cells[c_i] > 0 {
            spans.push(Span::styled(
                "▆".repeat(seg_cells[c_i]),
                Style::default().fg(theme.category_color(cat_enum)),
            ));
        }
    }
    if empty > 0 {
        spans.push(Span::styled(
            "·".repeat(empty),
            Style::default().fg(empty_cell_color(theme)),
        ));
    }
    spans.push(Span::styled(right_text, right_style));
    if let Some((t_str, t_style)) = trailing_note {
        spans.push(Span::styled(t_str, t_style));
    }
    Line::from(spans)
}

/// Format a key-value row with a subtle dot leader (`Key ·········· Value`)
/// spanning the exact width `width`.
fn dot_leader_row(
    left: &str,
    right: String,
    right_style: Style,
    width: u16,
    theme: &Theme,
) -> Line<'static> {
    let w = width as usize;
    let l_str = format!("{left} ");
    let r_str = format!(" {right}");
    let l_len = l_str.chars().count();
    let r_len = r_str.chars().count();
    let dots = w.saturating_sub(l_len + r_len).max(2);
    Line::from(vec![
        Span::styled(l_str, style::label(theme)),
        Span::styled("·".repeat(dots), Style::default().fg(empty_cell_color(theme))),
        Span::styled(r_str, right_style),
    ])
}

fn render_analytics(f: &mut Frame, size: Rect, ui: &UiState) {
    f.render_widget(Clear, size);

    let theme = ui.theme;
    let now = Local::now();
    let summary = ui.timer.stats.analytics_summary_with_category(
        now.date_naive(),
        now.hour() as usize,
        ui.timer.live_focus_secs(),
        ui.timer.mode.name,
        ui.timer.category.label(),
    );

    let outer = style::screen_block(theme);
    let inner = outer.inner(size);
    f.render_widget(outer, size);

    if inner.height < 20 {
        let padded = style::inset(inner, 1, 0);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(9),
                Constraint::Length(1),
            ])
            .split(padded);

        render_analytics_compact_header(f, chunks[0], ui);
        render_analytics_compact_metrics(f, chunks[1], &summary, theme);
        render_analytics_heatmap(f, chunks[2], &summary, ui, false);
        render_analytics_footer(f, chunks[3], theme);
    } else if inner.height < 28 {
        let padded = style::inset(inner, style::PAD_X, 0);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(4),
                Constraint::Min(10),
                Constraint::Length(1),
            ])
            .split(padded);

        render_analytics_header(f, chunks[0], ui);
        render_analytics_headline_strip(f, chunks[1], &summary, theme);
        render_analytics_middle_tier(f, chunks[2], &summary, ui);
        render_analytics_footer(f, chunks[3], theme);
    } else {
        let padded = style::inset(inner, style::PAD_X, 0);
        let headline_h = if padded.height >= 36 { 5 } else { 4 };
        let mid_h = if padded.height >= 42 {
            13
        } else if padded.height >= 34 {
            12
        } else {
            11
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(headline_h),
                Constraint::Length(mid_h),
                Constraint::Min(6),
                Constraint::Length(1),
            ])
            .split(padded);

        render_analytics_header(f, chunks[0], ui);
        render_analytics_headline_strip(f, chunks[1], &summary, theme);
        render_analytics_middle_tier(f, chunks[2], &summary, ui);
        render_analytics_bottom_deck(f, chunks[3], &summary, ui);
        render_analytics_footer(f, chunks[4], theme);
    }
}

fn render_analytics_header(f: &mut Frame, area: Rect, ui: &UiState) {
    let theme = ui.theme;
    let id = ui.system_identity;
    let width = area.width as usize;

    let mut spans = vec![
        Span::styled("◆ ", style::accent_bold(theme.gold)),
        Span::styled("DevChron", style::value_bold(theme)),
        Span::styled("  ·  ", style::label(theme)),
        Span::styled("Analytics", style::accent_bold(theme.focus_color)),
        Span::styled("  ·  ", style::label(theme)),
        Span::styled(
            format!("{}@{}", id.username, id.hostname),
            style::accent_bold(theme.cyan),
        ),
        Span::styled("  ·  ", style::label(theme)),
    ];

    // Interactive Category Filter chips in the Analytics header:
    // `c` [ All ]  ● Coding  ● Learning  ● Reading
    spans.push(Span::styled("c ", style::keycap(theme.gold)));
    if ui.analytics_filter.is_none() {
        spans.push(Span::styled(
            " All ",
            Style::default()
                .fg(theme.gold)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        ));
    } else {
        spans.push(Span::styled(" All ", style::label(theme)));
    }
    for cat in FocusCategory::ALL {
        let cat_col = theme.category_color(cat);
        spans.push(Span::raw(" "));
        if ui.analytics_filter == Some(cat) {
            spans.push(Span::styled(
                format!(" ● {} ", cat.label()),
                Style::default()
                    .fg(cat_col)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ));
        } else {
            spans.push(Span::styled("● ", Style::default().fg(cat_col)));
            spans.push(Span::styled(cat.label(), style::label(theme)));
        }
    }

    if ui.timer.is_running() {
        spans.push(Span::styled(
            "  ·  ● live",
            Style::default().fg(theme.emerald).add_modifier(Modifier::BOLD),
        ));
    }

    let mut row1 = Line::from(spans);
    let close_key = "g / esc";
    let close_lbl = " Close Analytics";
    let right_len = close_key.len() + close_lbl.len();
    let left_w = row1.width();
    if left_w + right_len + 2 <= width {
        let pad = width - left_w - right_len;
        row1.spans.push(Span::raw(" ".repeat(pad)));
        row1.spans
            .push(Span::styled(close_key, style::keycap(theme.long_break_color)));
        row1.spans.push(Span::styled(
            close_lbl,
            Style::default().fg(theme.section_accent),
        ));
    }

    let header = Paragraph::new(vec![row1]).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(style::rule_style(theme)),
    );
    f.render_widget(header, area);
}

fn render_analytics_compact_header(f: &mut Frame, area: Rect, ui: &UiState) {
    let theme = ui.theme;
    let id = ui.system_identity;
    let width = area.width as usize;
    let mut line = Line::from(vec![
        Span::styled("DevChron", style::value_bold(theme)),
        Span::styled(" · ", style::label(theme)),
        Span::styled("Analytics", style::accent_bold(theme.focus_color)),
        Span::styled(format!(" · {}@{}", id.username, id.hostname), style::label(theme)),
    ]);

    let close_key = "g/esc";
    let close_lbl = if line.width() + 23 <= width {
        " Close Analytics"
    } else {
        " close"
    };
    let right_w = close_key.len() + close_lbl.len();
    if line.width() + right_w + 1 <= width {
        let pad = width - line.width() - right_w;
        line.spans.push(Span::raw(" ".repeat(pad)));
        line.spans
            .push(Span::styled(close_key, style::keycap(theme.focus_color)));
        line.spans.push(Span::styled(close_lbl, style::label(theme)));
    }
    f.render_widget(Paragraph::new(line), area);
}

fn render_analytics_compact_metrics(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    theme: &Theme,
) {
    let best_part = if area.width >= 68 {
        format!(" (best {}d) · Avg ", summary.longest_streak)
    } else {
        " · Avg ".to_string()
    };
    let sep = " · ";
    let line = Line::from(vec![
        Span::styled("Streak ", style::label(theme)),
        Span::styled(
            format!("{}d", summary.current_streak),
            style::accent_bold(theme.focus_color),
        ),
        Span::styled(best_part, style::label(theme)),
        Span::styled(
            SessionStats::format_duration_secs(summary.avg_daily_secs),
            style::value_bold(theme),
        ),
        Span::styled(format!("{sep}Today "), style::label(theme)),
        Span::styled(
            SessionStats::format_duration_secs(summary.today_focus_secs),
            style::value_bold(theme),
        ),
        Span::styled(format!("{sep}Year "), style::label(theme)),
        Span::styled(
            SessionStats::format_duration_secs(summary.year_focus_secs),
            style::value_bold(theme),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Build a 3-color proportional under-bar (`━`) for a headline metric showing
/// the Coding / Learning / Reading category mix.
fn stacked_underbar_line(
    cat_secs: [u64; 3],
    ratio: f64,
    col_w: u16,
    theme: &Theme,
) -> Line<'static> {
    let bw = (col_w as usize).saturating_sub(2).max(4);
    let r = ratio.clamp(0.0, 1.0);
    let total_secs = cat_secs[0]
        .saturating_add(cat_secs[1])
        .saturating_add(cat_secs[2]);
    let filled = if r <= 0.0 || total_secs == 0 {
        0
    } else {
        ((r * bw as f64).round() as usize).clamp(1, bw)
    };
    let empty = bw.saturating_sub(filled);

    let mut spans = Vec::new();
    if filled > 0 && total_secs > 0 {
        let mut rem = filled;
        let non_zero: Vec<usize> = (0..3).filter(|&i| cat_secs[i] > 0).collect();
        for (pos, &c_i) in non_zero.iter().enumerate() {
            let cells = if pos + 1 == non_zero.len() {
                rem
            } else {
                let sh = cat_secs[c_i] as f64 / total_secs as f64;
                ((sh * filled as f64).round() as usize)
                    .max(usize::from(filled >= non_zero.len()))
                    .min(rem)
            };
            rem = rem.saturating_sub(cells);
            if cells > 0 {
                let cat = FocusCategory::ALL[c_i];
                spans.push(Span::styled(
                    "━".repeat(cells),
                    Style::default().fg(theme.category_color(cat)),
                ));
            }
        }
    }
    if empty > 0 {
        spans.push(Span::styled("─".repeat(empty), style::rule_style(theme)));
    }
    Line::from(spans)
}

fn render_analytics_headline_strip(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    theme: &Theme,
) {
    let show_bar = area.height >= 5;
    let metric = |title: &'static str,
                  primary: String,
                  subtitle: String,
                  ratio: f64,
                  cat_split: Option<[u64; 3]>,
                  col_w: u16,
                  accent_col: Color|
     -> Paragraph<'static> {
        let val_style = style::accent_bold(accent_col);
        let mut lines = vec![
            Line::from(Span::styled(
                title,
                Style::default().fg(theme.section_accent),
            )),
            Line::from(Span::styled(primary, val_style)),
            Line::from(Span::styled(subtitle, style::label(theme))),
        ];
        if show_bar {
            if let Some(split) = cat_split {
                lines.push(stacked_underbar_line(split, ratio, col_w, theme));
            } else {
                let bw = (col_w as usize).saturating_sub(2).max(4);
                let r = ratio.clamp(0.0, 1.0);
                let filled = if r <= 0.0 {
                    0
                } else {
                    ((r * bw as f64).round() as usize).clamp(1, bw)
                };
                let empty = bw.saturating_sub(filled);
                let bar_col = theme.color_ramp(accent_col, r.max(0.35));
                lines.push(Line::from(vec![
                    Span::styled("━".repeat(filled), Style::default().fg(bar_col)),
                    Span::styled("─".repeat(empty), style::rule_style(theme)),
                ]));
            }
        }
        Paragraph::new(lines)
    };

    let streak_ratio = if summary.longest_streak == 0 {
        0.0
    } else {
        summary.current_streak as f64 / summary.longest_streak as f64
    };
    let avg_ratio = SessionStats::intensity_for(summary.avg_daily_secs, summary.max_daily_secs);
    let today_ratio = SessionStats::intensity_for(summary.today_focus_secs, summary.max_daily_secs);
    let year_ratio = (summary.active_days_year as f64 / 365.0).clamp(0.0, 1.0);
    let comp_ratio = (summary.completion_rate_pct / 100.0).clamp(0.0, 1.0);
    let focus_ratio = (summary.focus_share_pct / 100.0).clamp(0.0, 1.0);
    let year_cat_split = [
        summary.category_stats[0].focus_secs,
        summary.category_stats[1].focus_secs,
        summary.category_stats[2].focus_secs,
    ];
    let top_cat_sub = match &summary.top_category {
        Some(cat) => format!("{} sess · top {cat}", summary.today_sessions),
        None => format!("{} sessions today", summary.today_sessions),
    };

    if area.width >= 108 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
            ])
            .split(area);

        f.render_widget(
            metric(
                "Current streak",
                format!("{} days", summary.current_streak),
                format!(
                    "best {}d · {} active",
                    summary.longest_streak, summary.active_days_year
                ),
                streak_ratio,
                None,
                cols[0].width,
                theme.gold,
            ),
            cols[0],
        );

        f.render_widget(
            metric(
                "Average / day",
                SessionStats::format_duration_secs(summary.avg_daily_secs),
                format!(
                    "active avg {}",
                    SessionStats::format_duration_secs(summary.avg_active_day_secs)
                ),
                avg_ratio,
                Some(year_cat_split),
                cols[1].width,
                theme.cyan,
            ),
            cols[1],
        );

        f.render_widget(
            metric(
                "Today's focus",
                SessionStats::format_duration_secs(summary.today_focus_secs),
                top_cat_sub,
                today_ratio,
                Some(summary.today_cat_secs),
                cols[2].width,
                theme.focus_color,
            ),
            cols[2],
        );

        f.render_widget(
            metric(
                "Yearly focus",
                SessionStats::format_duration_secs(summary.year_focus_secs),
                format!(
                    "{} sess · peak {}",
                    summary.year_sessions,
                    SessionStats::format_duration_secs(summary.peak_day_secs)
                ),
                year_ratio.max(0.35 * usize::from(summary.year_focus_secs > 0) as f64),
                Some(year_cat_split),
                cols[3].width,
                theme.long_break_color,
            ),
            cols[3],
        );

        f.render_widget(
            metric(
                "Completion",
                format!("{:.0}%", summary.completion_rate_pct),
                format!(
                    "{} done · {} reset",
                    summary.completed_focus_count, summary.interrupted_focus_count
                ),
                comp_ratio,
                None,
                cols[4].width,
                theme.emerald,
            ),
            cols[4],
        );

        let ratio_str = if summary.year_focus_secs == 0 && summary.year_break_secs == 0 {
            "0.0 : 1".to_string()
        } else {
            format!("{:.1} : 1", summary.work_rest_ratio)
        };
        f.render_widget(
            metric(
                "Work / break",
                ratio_str,
                format!(
                    "{:.0}% focus · {} cycles",
                    summary.focus_share_pct, summary.cycles_completed
                ),
                focus_ratio,
                None,
                cols[5].width,
                theme.short_break_color,
            ),
            cols[5],
        );
    } else {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(area);

        f.render_widget(
            metric(
                "Current streak",
                format!("{} days", summary.current_streak),
                format!(
                    "best {}d · {} active",
                    summary.longest_streak, summary.active_days_year
                ),
                streak_ratio,
                None,
                cols[0].width,
                theme.gold,
            ),
            cols[0],
        );

        f.render_widget(
            metric(
                "Average / day",
                SessionStats::format_duration_secs(summary.avg_daily_secs),
                format!(
                    "active avg {}",
                    SessionStats::format_duration_secs(summary.avg_active_day_secs)
                ),
                avg_ratio,
                Some(year_cat_split),
                cols[1].width,
                theme.cyan,
            ),
            cols[1],
        );

        f.render_widget(
            metric(
                "Today's focus",
                SessionStats::format_duration_secs(summary.today_focus_secs),
                top_cat_sub,
                today_ratio,
                Some(summary.today_cat_secs),
                cols[2].width,
                theme.focus_color,
            ),
            cols[2],
        );

        f.render_widget(
            metric(
                "Yearly focus",
                SessionStats::format_duration_secs(summary.year_focus_secs),
                format!(
                    "{} sess · peak {}",
                    summary.year_sessions,
                    SessionStats::format_duration_secs(summary.peak_day_secs)
                ),
                year_ratio,
                Some(year_cat_split),
                cols[3].width,
                theme.long_break_color,
            ),
            cols[3],
        );
    }
}

fn render_analytics_middle_tier(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    ui: &UiState,
) {
    let total_weeks = summary.weeks.len() as u16; // 53
    let wide_grid_w = 5 + total_weeks * 3; // 164 cols
    let std_grid_w = 5 + total_weeks * 2; // 111 cols

    if area.width >= wide_grid_w + style::COL_GAP + 28 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(wide_grid_w),
                Constraint::Length(style::COL_GAP),
                Constraint::Min(28),
            ])
            .split(area);

        render_analytics_heatmap(f, cols[0], summary, ui, true);
        render_analytics_by_hour(f, cols[2], summary, ui);
    } else if area.width >= std_grid_w + style::COL_GAP + 26 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(std_grid_w),
                Constraint::Length(style::COL_GAP),
                Constraint::Min(26),
            ])
            .split(area);

        render_analytics_heatmap(f, cols[0], summary, ui, true);
        render_analytics_by_hour(f, cols[2], summary, ui);
    } else {
        render_analytics_heatmap(f, area, summary, ui, true);
    }
}

fn render_analytics_heatmap(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    ui: &UiState,
    with_section_header: bool,
) {
    let theme = ui.theme;
    if area.width < 10 || area.height < 8 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(13);

    if with_section_header {
        let meta = match ui.analytics_filter {
            Some(cat) => {
                let cs = &summary.category_stats[category_index(cat.label())];
                format!(
                    "{} · {} ({} sessions · {} active days)",
                    cat.label(),
                    SessionStats::format_duration_secs(cs.focus_secs),
                    cs.sessions,
                    cs.active_days
                )
            }
            None => format!(
                "{} sessions · {} active days · colored by category",
                summary.year_sessions, summary.active_days_year
            ),
        };
        lines.push(style::section_header(
            "Yearly Focus Grid",
            Some(&meta),
            area.width,
            theme,
        ));
    }

    let total_weeks = summary.weeks.len();
    let label_prefix_width = 5usize; // "Mon  "
    let avail_cols = (area.width as usize).saturating_sub(label_prefix_width);

    let cell_step = if avail_cols >= total_weeks * 3 {
        3
    } else if avail_cols >= total_weeks * 2 {
        2
    } else {
        1
    };
    let visible_weeks = (avail_cols / cell_step).clamp(1, total_weeks);
    let start_week = total_weeks.saturating_sub(visible_weeks);
    let cell_glyph = if cell_step == 3 { "▆▆" } else { "▆" };

    // Month labels row aligned to week columns
    let mut month_chars: Vec<char> = vec![' '; label_prefix_width + visible_weeks * cell_step];
    let mut next_free_col = label_prefix_width;
    for &(week_idx, m_name) in &summary.month_labels {
        if week_idx < start_week {
            continue;
        }
        let rel_week = week_idx - start_week;
        let pos = label_prefix_width + rel_week * cell_step;
        if pos >= next_free_col && pos + m_name.len() <= month_chars.len() {
            for (i, ch) in m_name.chars().enumerate() {
                month_chars[pos + i] = ch;
            }
            next_free_col = pos + m_name.len() + 1;
        }
    }
    let month_line: String = month_chars.into_iter().collect();
    lines.push(Line::from(Span::styled(month_line, style::label(theme))));

    const DAY_NAMES: [&str; GRID_DAYS_PER_WEEK] =
        ["Mon  ", "Tue  ", "Wed  ", "Thu  ", "Fri  ", "Sat  ", "Sun  "];

    for (day_row, &day_label) in DAY_NAMES.iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(visible_weeks * 2 + 2);
        let shown_label = if day_row % 2 == 0 || area.height >= 13 {
            day_label
        } else {
            "     "
        };
        spans.push(Span::styled(shown_label, style::label(theme)));

        for week in &summary.weeks[start_week..] {
            let cell = week[day_row];
            let is_today = cell.in_year_window && cell.date == summary.today_date;
            let color = if !cell.in_year_window {
                empty_cell_color(theme)
            } else if let Some(filter_cat) = ui.analytics_filter {
                let secs = cell.cat_secs[category_index(filter_cat.label())];
                category_heatmap_color(theme, filter_cat, secs, summary.max_daily_secs)
            } else {
                // All-category view intentionally uses a unified contribution-green scale.
                theme.heatmap_ramp(SessionStats::intensity_for(
                    cell.focus_secs,
                    summary.max_daily_secs,
                ))
            };

            // Today is always solar yellow, even when empty or when a category filter is active.
            let cell_style = if is_today {
                Style::default()
                    .fg(theme.today_color())
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(color)
            };
            spans.push(Span::styled(cell_glyph, cell_style));
            if cell_step >= 2 {
                spans.push(Span::raw(" "));
            }
        }
        lines.push(Line::from(spans));
    }

    if area.height >= 12 && lines.len() + 1 < area.height as usize {
        lines.push(Line::from(""));
    }

    // Legend row showing both Intensity Ramp AND Category Swatches (Coding / Learning / Reading)
    if lines.len() < area.height as usize {
        let max_s = summary.max_daily_secs.max(3600);
        let ramp_cat = ui.analytics_filter;
        let mut legend_spans = vec![
            Span::styled("     Less ", style::label(theme)),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(legend_intensity_color(theme, ramp_cat, 0, max_s)),
            ),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(legend_intensity_color(theme, ramp_cat, max_s / 4, max_s)),
            ),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(legend_intensity_color(theme, ramp_cat, max_s / 2, max_s)),
            ),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(legend_intensity_color(
                    theme,
                    ramp_cat,
                    (max_s * 3) / 4,
                    max_s,
                )),
            ),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(legend_intensity_color(theme, ramp_cat, max_s, max_s)),
            ),
            Span::styled("More  ·  ", style::label(theme)),
        ];

        if area.width >= 68 {
            for (i, cat) in FocusCategory::ALL.into_iter().enumerate() {
                if i > 0 {
                    legend_spans.push(Span::raw("  "));
                }
                legend_spans.push(Span::styled(
                    "▆ ",
                    Style::default().fg(theme.category_color(cat)),
                ));
                legend_spans.push(Span::styled(cat.label(), style::label(theme)));
            }
            legend_spans.push(Span::styled("  ·  ", style::label(theme)));
        }

        legend_spans.push(Span::styled(
            cell_glyph,
            Style::default()
                .fg(theme.today_color())
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ));
        legend_spans.push(Span::styled(" today", style::label(theme)));

        let legend_left_w: usize = legend_spans.iter().map(|s| s.content.chars().count()).sum();
        let right_summary = format!(
            "peak {} · {}/365d ({:.0}%)",
            SessionStats::format_duration_secs(summary.peak_day_secs),
            summary.active_days_year,
            (summary.active_days_year as f64 / 365.0) * 100.0
        );
        let grid_right_edge = (label_prefix_width + visible_weeks * cell_step)
            .saturating_sub(usize::from(cell_step >= 2))
            .min(area.width as usize);
        if legend_left_w + right_summary.chars().count() + 3 <= grid_right_edge {
            let pad = grid_right_edge - legend_left_w - right_summary.chars().count();
            legend_spans.push(Span::raw(" ".repeat(pad)));
            legend_spans.push(Span::styled(right_summary, style::label(theme)));
        }

        lines.push(Line::from(legend_spans));
    }

    f.render_widget(Paragraph::new(lines), area);
}

/// Borderless 24-Hour distribution colored by each hour's dominant category,
/// plus 4 time-of-day windows rendered as multi-color stacked category bars.
fn render_analytics_by_hour(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    ui: &UiState,
) {
    let theme = ui.theme;
    if area.width < 16 || area.height < 4 {
        return;
    }

    let peak_note = match summary.peak_hour {
        Some(h) => format!("peak {:02}:00–{:02}:00", h, (h + 1) % 24),
        None => "00h–23h".to_string(),
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(style::section_header(
        "By hour",
        Some(&peak_note),
        area.width,
        theme,
    ));

    let max_h = summary
        .hourly_focus_secs
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .max(1800);
    let step = if area.width >= 72 {
        3
    } else if area.width >= 48 {
        2
    } else {
        1
    };
    let bar_chars = if step == 3 { 2 } else { 1 };
    let pad = if step >= 2 { " " } else { "" };

    let chart_rows = (area.height as usize).saturating_sub(7).clamp(2, 4);
    const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    for row_idx in 0..chart_rows {
        let low_band = (chart_rows - 1 - row_idx) as f64 / chart_rows as f64;
        let high_band = (chart_rows - row_idx) as f64 / chart_rows as f64;
        let mut row_spans: Vec<Span<'static>> = Vec::with_capacity(24);

        for h in 0..24 {
            let [c_s, l_s, r_s] = summary.hourly_cat_secs[h];
            let (secs, dom_cat) = if let Some(f_cat) = ui.analytics_filter {
                (summary.hourly_cat_secs[h][category_index(f_cat.label())], f_cat)
            } else {
                let dom = if l_s > c_s && l_s >= r_s {
                    FocusCategory::Learning
                } else if r_s > c_s && r_s > l_s {
                    FocusCategory::Reading
                } else {
                    FocusCategory::Coding
                };
                (summary.hourly_focus_secs[h], dom)
            };

            let ratio = if secs == 0 {
                0.0
            } else {
                (secs as f64 / max_h as f64).clamp(0.08, 1.0)
            };
            let color = category_heatmap_color(theme, dom_cat, secs, max_h);

            if ratio >= high_band {
                row_spans.push(Span::styled(
                    format!("{}{pad}", "█".repeat(bar_chars)),
                    Style::default().fg(color),
                ));
            } else if ratio > low_band {
                let frac = ((ratio - low_band) / (high_band - low_band)).clamp(0.0, 1.0);
                let idx = ((frac * 7.0).round() as usize).min(7);
                let glyph = SPARK[idx].to_string().repeat(bar_chars);
                row_spans.push(Span::styled(
                    format!("{glyph}{pad}"),
                    Style::default().fg(color),
                ));
            } else if row_idx == chart_rows - 1 && secs > 0 {
                let glyph = "▁".repeat(bar_chars);
                row_spans.push(Span::styled(
                    format!("{glyph}{pad}"),
                    Style::default().fg(color),
                ));
            } else {
                let dots = "·".repeat(bar_chars);
                row_spans.push(Span::styled(
                    format!("{dots}{pad}"),
                    Style::default().fg(empty_cell_color(theme)),
                ));
            }
        }
        lines.push(Line::from(row_spans));
    }

    let axis = if step == 3 {
        "00    02    04    06    08    10    12    14    16    18    20    22h"
    } else if step == 2 {
        "00  02  04  06  08  10  12  14  16  18  20  22h"
    } else {
        "00 03 06 09 12 15 18 21h"
    };
    lines.push(Line::from(Span::styled(
        axis,
        Style::default().fg(theme.section_accent),
    )));
    lines.push(Line::from(""));

    let (m_pct, a_pct, e_pct, n_pct) = summary.chronotype_pcts;
    let total_hourly = summary.hourly_focus_secs.iter().sum::<u64>().max(1);
    let period_row =
        |name: &'static str, lbl_col: Color, pct: f64, p_cats: [u64; 3]| -> Line<'static> {
            stacked_category_bar_row(
                format!("{name:<15}"),
                Style::default().fg(lbl_col),
                p_cats,
                total_hourly,
                ui.analytics_filter,
                theme,
                format!(" {:>5.1}%", pct),
                if pct > 0.0 {
                    style::accent_bold(lbl_col)
                } else {
                    style::label(theme)
                },
                None,
                area.width,
            )
        };

    lines.push(period_row(
        "Morning 06–12",
        theme.gold,
        m_pct,
        summary.chronotype_cat_secs[0],
    ));
    lines.push(period_row(
        "Afternoon 12–18",
        theme.focus_color,
        a_pct,
        summary.chronotype_cat_secs[1],
    ));
    lines.push(period_row(
        "Evening 18–24",
        theme.cyan,
        e_pct,
        summary.chronotype_cat_secs[2],
    ));
    lines.push(period_row(
        "Night 00–06",
        theme.long_break_color,
        n_pct,
        summary.chronotype_cat_secs[3],
    ));

    f.render_widget(Paragraph::new(lines), area);
}

/// Decluttered, category-rich 3-column bottom deck.
fn render_analytics_bottom_deck(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    ui: &UiState,
) {
    if area.width >= 110 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Length(style::COL_GAP),
                Constraint::Ratio(1, 3),
                Constraint::Length(style::COL_GAP),
                Constraint::Min(32),
            ])
            .split(area);

        render_analytics_weekday_and_7d(f, cols[0], summary, ui, false);
        render_analytics_categories_and_modes(f, cols[2], summary, ui);
        render_analytics_recent_sessions(f, cols[4], summary, ui);
    } else {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(48),
                Constraint::Length(style::COL_GAP),
                Constraint::Min(28),
            ])
            .split(area);

        render_analytics_weekday_and_7d(f, cols[0], summary, ui, true);
        render_analytics_recent_sessions(f, cols[2], summary, ui);
    }
}

/// Append the 3-row `By category` (`Coding`, `Learning`, `Reading`) breakdown to `lines`,
/// using each category's distinct color (`Coral`, `Aqua`, `Periwinkle`).
fn push_category_section(
    lines: &mut Vec<Line<'static>>,
    summary: &AnalyticsSummary,
    active_category: FocusCategory,
    width: u16,
    theme: &Theme,
) {
    let top_note = match &summary.top_category {
        Some(top) => format!("top {top}"),
        None => format!("active {}", active_category.label()),
    };
    lines.push(style::section_header(
        "By category",
        Some(&top_note),
        width,
        theme,
    ));

    let empty_col = empty_cell_color(theme);
    for cs in &summary.category_stats {
        let r = (cs.share_pct / 100.0).clamp(0.0, 1.0);
        let cat_col = theme.category_color(cs.category);
        let is_active = cs.category == active_category;
        let lbl_style = if is_active {
            Style::default().fg(cat_col).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(cat_col)
        };

        lines.push(dynamic_bar_row(
            format!("{:<9} ", cs.name),
            lbl_style,
            r,
            cat_col,
            empty_col,
            format!(" {:>4.0}%", cs.share_pct),
            style::label(theme),
            Some((
                format!(
                    " {} {:>3}s",
                    style::format_duration_fixed(cs.focus_secs),
                    cs.sessions
                ),
                style::body(theme),
            )),
            width,
        ));
    }
}

/// Column 1: `By weekday` (multi-color stacked category bars) +
/// `Last 7 days` (clean 7-day window, never 28 rows of zeros!) +
/// `Category × weekday` matrix showing which days each category is practiced.
fn render_analytics_weekday_and_7d(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    ui: &UiState,
    include_categories: bool,
) {
    let theme = ui.theme;
    if area.height == 0 || area.width < 18 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    let peak_dow_note = summary.peak_weekday.map(|idx| {
        const FULL_DOW: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        format!("best {}", FULL_DOW[idx % 7])
    });
    lines.push(style::section_header(
        "By weekday",
        peak_dow_note.as_deref(),
        area.width,
        theme,
    ));

    let max_dow = summary
        .weekday_stats
        .iter()
        .map(|w| w.focus_secs)
        .max()
        .unwrap_or(0)
        .max(1800);
    const DOW_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

    for ws in &summary.weekday_stats {
        let is_peak = summary.peak_weekday == Some(ws.weekday_index) && ws.focus_secs > 0;
        let lbl_style = if is_peak {
            style::accent_bold(theme.gold)
        } else if ws.focus_secs > 0 {
            Style::default().fg(theme.section_accent)
        } else {
            style::label(theme)
        };
        let display_secs = match ui.analytics_filter {
            Some(cat) => ws.cat_secs[category_index(cat.label())],
            None => ws.focus_secs,
        };
        let sess_style = if ws.sessions > 0 {
            Style::default().fg(theme.cyan)
        } else {
            style::label(theme)
        };

        lines.push(stacked_category_bar_row(
            format!("{}  ", DOW_LABELS[ws.weekday_index]),
            lbl_style,
            ws.cat_secs,
            max_dow,
            ui.analytics_filter,
            theme,
            format!(" {}", style::format_duration_fixed(display_secs)),
            if display_secs > 0 {
                style::value_bold(theme)
            } else {
                style::label(theme)
            },
            Some((format!(" {:>3}s", ws.sessions), sess_style)),
            area.width,
        ));
    }

    if include_categories && (area.height as usize).saturating_sub(lines.len()) >= 5 {
        lines.push(Line::from(""));
        push_category_section(&mut lines, summary, ui.timer.category, area.width, theme);
    }

    // Clean Last 7 days breakdown (never a 28-day wall of 0m 0s!)
    if (area.height as usize).saturating_sub(lines.len()) >= 4 {
        lines.push(Line::from(""));
        let active_7d = summary
            .last_7_days
            .iter()
            .filter(|(_, s, _, _)| *s > 0)
            .count();
        let window_note = format!("{active_7d}/7 active");
        lines.push(style::section_header(
            "Last 7 days",
            Some(&window_note),
            area.width,
            theme,
        ));

        let max_7d = summary
            .last_7_days
            .iter()
            .map(|(_, s, _, _)| *s)
            .max()
            .unwrap_or(0)
            .max(1800);

        for &(date, secs, sessions, cat_split) in summary.last_7_days.iter().rev() {
            if lines.len() >= area.height as usize {
                break;
            }
            let dow = DOW_LABELS[date.weekday().num_days_from_monday() as usize];
            let display_secs = match ui.analytics_filter {
                Some(cat) => cat_split[category_index(cat.label())],
                None => secs,
            };
            let is_today = date == summary.today_date;
            let date_style = if is_today {
                style::accent_bold(theme.gold)
            } else if display_secs > 0 {
                Style::default().fg(theme.section_accent)
            } else {
                style::label(theme)
            };
            let sess_style = if sessions > 0 {
                Style::default().fg(theme.cyan)
            } else {
                style::label(theme)
            };

            lines.push(stacked_category_bar_row(
                format!("{:02}-{:02} {dow} ", date.month(), date.day()),
                date_style,
                cat_split,
                max_7d,
                ui.analytics_filter,
                theme,
                format!(" {}", style::format_duration_fixed(display_secs)),
                if display_secs > 0 {
                    style::value_bold(theme)
                } else {
                    style::label(theme)
                },
                Some((format!(" {:>3}s", sessions), sess_style)),
                area.width,
            ));
        }
    }

    // Category × Weekday activity matrix when vertical room remains
    if (area.height as usize).saturating_sub(lines.len()) >= 6 {
        lines.push(Line::from(""));
        lines.push(style::section_header(
            "Category × weekday",
            Some("Mon–Sun rhythm"),
            area.width,
            theme,
        ));
        lines.push(Line::from(Span::styled(
            "Category    Mon  Tue  Wed  Thu  Fri  Sat  Sun    Best day",
            style::label(theme),
        )));

        for (c_i, cs) in summary.category_stats.iter().enumerate() {
            if lines.len() >= area.height as usize {
                break;
            }
            let cat_col = theme.category_color(cs.category);
            let max_cat_dow = summary
                .weekday_stats
                .iter()
                .map(|w| w.cat_secs[c_i])
                .max()
                .unwrap_or(0)
                .max(1800);

            let mut row_spans = vec![
                Span::styled("● ", Style::default().fg(cat_col)),
                Span::styled(format!("{:<10}", cs.name), Style::default().fg(cat_col)),
            ];
            for dow_i in 0..7 {
                let d_secs = summary.weekday_stats[dow_i].cat_secs[c_i];
                let cell_col = category_heatmap_color(theme, cs.category, d_secs, max_cat_dow);
                row_spans.push(Span::styled("▆▆   ", Style::default().fg(cell_col)));
            }
            let best_str = cs
                .best_weekday
                .map_or_else(|| "—".to_string(), |d| DOW_LABELS[d % 7].to_string());
            row_spans.push(Span::styled(format!("  {best_str:>6}"), style::accent_bold(theme.gold)));
            lines.push(Line::from(row_spans));
        }
    }

    f.render_widget(Paragraph::new(lines), area);
}

/// Column 2: `By category` (colored bars) + `Category telemetry` (Today, 7-day,
/// Avg session, Active days, Peak hour, and Today/7-day stacked category bars) +
/// `Modes & pacing` + `Recent months` (stacked category bars).
fn render_analytics_categories_and_modes(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    ui: &UiState,
) {
    let theme = ui.theme;
    if area.height == 0 || area.width < 18 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    push_category_section(&mut lines, summary, ui.timer.category, area.width, theme);

    // Dedicated Category Telemetry section (replaces the redundant Focus rhythm / All-time dump!)
    if (area.height as usize).saturating_sub(lines.len()) >= 7 {
        lines.push(Line::from(""));
        lines.push(style::section_header(
            "Category telemetry",
            Some("today · 7d · pace"),
            area.width,
            theme,
        ));

        lines.push(Line::from(Span::styled(
            "Category     Today      7-day   Avg/sess   Active   Peak hr",
            style::label(theme),
        )));

        for cs in &summary.category_stats {
            if lines.len() >= area.height as usize {
                break;
            }
            let cat_col = theme.category_color(cs.category);
            let peak_h = cs
                .peak_hour
                .map_or_else(|| "  —  ".to_string(), |h| format!("{:02}:00", h));
            let row_spans = vec![
                Span::styled("● ", Style::default().fg(cat_col)),
                Span::styled(format!("{:<8}", cs.name), style::accent_bold(cat_col)),
                Span::styled(
                    format!(" {:>7}", SessionStats::format_duration_secs(cs.today_secs)),
                    style::value_bold(theme),
                ),
                Span::styled(
                    format!("  {:>9}", SessionStats::format_duration_secs(cs.week_secs)),
                    style::body(theme),
                ),
                Span::styled(
                    format!(
                        "    {:>7}",
                        SessionStats::format_duration_secs(cs.avg_session_secs)
                    ),
                    Style::default().fg(theme.cyan),
                ),
                Span::styled(
                    format!("   {:>5}d", cs.active_days),
                    Style::default().fg(theme.emerald),
                ),
                Span::styled(format!("     {peak_h}"), Style::default().fg(theme.gold)),
            ];
            lines.push(Line::from(row_spans));
        }

        if lines.len() + 2 < area.height as usize {
            lines.push(Line::from(""));
            let today_tot = summary.today_focus_secs.max(1);
            lines.push(stacked_category_bar_row(
                "Today mix ".to_string(),
                style::label(theme),
                summary.today_cat_secs,
                today_tot,
                ui.analytics_filter,
                theme,
                format!(" {}", style::format_duration_fixed(summary.today_focus_secs)),
                style::body(theme),
                Some((
                    format!(" {:>3}s", summary.today_sessions),
                    style::label(theme),
                )),
                area.width,
            ));

            let week_cats = [
                summary.category_stats[0].week_secs,
                summary.category_stats[1].week_secs,
                summary.category_stats[2].week_secs,
            ];
            let week_tot = week_cats.iter().sum::<u64>();
            let week_sess: u32 = summary.last_7_days.iter().map(|(_, _, s, _)| *s).sum();
            lines.push(stacked_category_bar_row(
                "7-day mix ".to_string(),
                style::label(theme),
                week_cats,
                week_tot.max(1),
                ui.analytics_filter,
                theme,
                format!(" {}", style::format_duration_fixed(week_tot)),
                style::body(theme),
                Some((format!(" {:>3}s", week_sess), style::label(theme))),
                area.width,
            ));
        }
    }

    if (area.height as usize).saturating_sub(lines.len()) >= 6 {
        lines.push(Line::from(""));
        lines.push(style::section_header(
            "Modes & pacing",
            None,
            area.width,
            theme,
        ));

        let empty_col = empty_cell_color(theme);
        for ms in &summary.mode_stats {
            if lines.len() >= area.height as usize {
                break;
            }
            let r = (ms.share_pct / 100.0).clamp(0.0, 1.0);
            let mode_col = theme.mode_name_color(&ms.name);
            let col = theme.color_ramp(mode_col, r);

            lines.push(dynamic_bar_row(
                format!("{:<9} ", ms.name),
                Style::default().fg(mode_col),
                r,
                col,
                empty_col,
                format!(" {:>4.0}%", ms.share_pct),
                Style::default().fg(mode_col),
                Some((
                    format!(
                        " {} {:>3}s",
                        style::format_duration_fixed(ms.focus_secs),
                        ms.sessions
                    ),
                    style::body(theme),
                )),
                area.width,
            ));
        }

        if lines.len() + 1 < area.height as usize {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Focus ", style::label(theme)),
                Span::styled(
                    SessionStats::format_duration_secs(summary.year_focus_secs),
                    style::accent_bold(theme.focus_color),
                ),
                Span::styled("  ·  Break ", style::label(theme)),
                Span::styled(
                    SessionStats::format_duration_secs(summary.year_break_secs),
                    style::accent_bold(theme.cyan),
                ),
                Span::styled("  ·  Cycles ", style::label(theme)),
                Span::styled(
                    format!("{}", summary.cycles_completed),
                    style::accent_bold(theme.gold),
                ),
            ]));
        }
    }

    if (area.height as usize).saturating_sub(lines.len()) >= 4 {
        lines.push(Line::from(""));
        lines.push(style::section_header(
            "Recent months",
            Some("6m velocity"),
            area.width,
            theme,
        ));

        let max_m = summary
            .monthly_velocity
            .iter()
            .map(|(_, s, _, _)| *s)
            .max()
            .unwrap_or(0)
            .max(1800);

        for (m_label, m_secs, m_sess, m_cats) in &summary.monthly_velocity {
            if lines.len() >= area.height as usize {
                break;
            }
            let disp_secs = match ui.analytics_filter {
                Some(cat) => m_cats[category_index(cat.label())],
                None => *m_secs,
            };

            lines.push(stacked_category_bar_row(
                format!("{m_label:<8} "),
                style::label(theme),
                *m_cats,
                max_m,
                ui.analytics_filter,
                theme,
                format!(" {}", style::format_duration_fixed(disp_secs)),
                style::body(theme),
                Some((format!(" {:>3}s", m_sess), style::label(theme))),
                area.width,
            ));
        }
    }

    f.render_widget(Paragraph::new(lines), area);
}

/// Column 3: Scrollable `Recent sessions` table with per-category color-coded
/// badges + concise `Category milestones` summary when vertical room remains.
fn render_analytics_recent_sessions(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    ui: &UiState,
) {
    let theme = ui.theme;
    if area.height == 0 || area.width < 20 {
        return;
    }

    // Filter recent_log when a category filter is active
    let filtered_log: Vec<_> = summary
        .recent_log
        .iter()
        .filter(|e| match ui.analytics_filter {
            Some(cat) => e.phase == "FOCUS" && e.category.eq_ignore_ascii_case(cat.label()),
            None => true,
        })
        .collect();

    let mut lines: Vec<Line<'static>> = Vec::new();
    let log_note = if filtered_log.len() > 8 {
        format!("{} logged · j/k scroll", filtered_log.len())
    } else {
        format!("{} logged", filtered_log.len())
    };
    lines.push(style::section_header(
        "Recent sessions",
        Some(&log_note),
        area.width,
        theme,
    ));

    let w = area.width as usize;
    let show_mode_and_cat = w >= 50;
    let show_cat_only = !show_mode_and_cat && w >= 40;

    let (g1, g2, g3, g4, g5) = if show_mode_and_cat {
        let base_cols = 11 + 5 + 8 + 8 + 7 + 7; // 46 chars
        let total_gap = w.saturating_sub(base_cols).max(5);
        let a = total_gap / 5;
        let b = (total_gap - a) / 4;
        let c = (total_gap - a - b) / 3;
        let d = (total_gap - a - b - c) / 2;
        let e = total_gap - a - b - c - d;
        (a, b, c, d, e)
    } else if show_cat_only {
        let base_cols = 11 + 5 + 8 + 7 + 7; // 38 chars
        let total_gap = w.saturating_sub(base_cols).max(4);
        let a = total_gap / 4;
        let b = (total_gap - a) / 3;
        let c = (total_gap - a - b) / 2;
        let d = total_gap - a - b - c;
        (a, b, c, 0, d)
    } else {
        let base_cols = 11 + 5 + 7 + 7; // 30 chars
        let total_gap = w.saturating_sub(base_cols).max(3);
        let a = total_gap / 3;
        let b = (total_gap - a) / 2;
        let c = total_gap - a - b;
        (a, b, 0, 0, c)
    };

    let hdr = if show_mode_and_cat {
        format!(
            "{:<11}{}{:<5}{}{:<8}{}{:<8}{}{:>7}{}{:>7}",
            "Time",
            " ".repeat(g1),
            "Phase",
            " ".repeat(g2),
            "Category",
            " ".repeat(g3),
            "Mode",
            " ".repeat(g4),
            "Dur",
            " ".repeat(g5),
            "Status"
        )
    } else if show_cat_only {
        format!(
            "{:<11}{}{:<5}{}{:<8}{}{:>7}{}{:>7}",
            "Time",
            " ".repeat(g1),
            "Phase",
            " ".repeat(g2),
            "Category",
            " ".repeat(g3),
            "Dur",
            " ".repeat(g5),
            "Status"
        )
    } else {
        format!(
            "{:<11}{}{:<5}{}{:>7}{}{:>7}",
            "Time",
            " ".repeat(g1),
            "Phase",
            " ".repeat(g2),
            "Dur",
            " ".repeat(g5),
            "Status"
        )
    };
    lines.push(Line::from(Span::styled(hdr, style::label(theme))));

    // Current timer state row
    let phase_short = match ui.timer.current_timer.phase {
        TimerPhase::Focus => "Focus",
        TimerPhase::ShortBreak => "Short",
        TimerPhase::LongBreak => "Long ",
    };
    let (live_cat, live_cat_style) = if ui.timer.current_timer.phase == TimerPhase::Focus {
        (
            ui.timer.category.label(),
            Style::default().fg(theme.category_color(ui.timer.category)),
        )
    } else {
        ("—", style::label(theme))
    };
    let live_elapsed = ui
        .timer
        .current_timer
        .duration
        .saturating_sub(ui.timer.current_timer.remaining)
        .as_secs();
    let live_state = if ui.alarm_ringing {
        "alarm"
    } else if ui.timer.is_running() {
        "running"
    } else if live_elapsed > 0 {
        "paused"
    } else {
        "ready"
    };
    let live_phase_col = get_phase_color(ui.timer.current_timer.phase, theme);

    let mut now_spans = vec![
        Span::styled(format!("{:<11}", "Now"), style::body(theme)),
        Span::raw(" ".repeat(g1)),
        Span::styled(
            format!("{phase_short:<5}"),
            Style::default().fg(live_phase_col),
        ),
        Span::raw(" ".repeat(g2)),
    ];
    if show_mode_and_cat || show_cat_only {
        now_spans.push(Span::styled(format!("{live_cat:<8}"), live_cat_style));
        now_spans.push(Span::raw(" ".repeat(g3)));
    }
    if show_mode_and_cat {
        now_spans.push(Span::styled(
            format!("{:<8}", ui.timer.mode.name),
            style::label(theme),
        ));
        now_spans.push(Span::raw(" ".repeat(g4)));
    }
    now_spans.push(Span::styled(
        format!("{:>7}", SessionStats::format_mm_ss(live_elapsed)),
        style::body(theme),
    ));
    now_spans.push(Span::raw(" ".repeat(g5)));
    now_spans.push(Span::styled(
        format!("{live_state:>7}"),
        style::label(theme),
    ));
    lines.push(Line::from(now_spans));

    // Reserve 7 rows at bottom of Column 3 for concise Category milestones when area is tall
    let reserve_bottom = if area.height >= 20 { 7 } else { 0 };
    let max_log_rows = (area.height as usize)
        .saturating_sub(lines.len() + reserve_bottom)
        .max(3);
    let scroll_start = ui
        .analytics_scroll
        .min(filtered_log.len().saturating_sub(1));

    for entry in filtered_log.iter().skip(scroll_start).take(max_log_rows) {
        if lines.len() >= area.height as usize {
            break;
        }
        let (phase_display, phase_col) = match entry.phase.as_str() {
            "FOCUS" => ("Focus", theme.focus_color),
            "SHORT BRK" => ("Short", theme.short_break_color),
            "LONG BRK" => ("Long ", theme.long_break_color),
            other => (other, theme.text),
        };
        let (status_display, status_col) = match entry.status.as_str() {
            "COMPLETE" => ("done", theme.emerald),
            "RESTED" => ("rested", theme.cyan),
            "SKIPPED" => ("skipped", theme.amber),
            "ABORTED" => ("reset", theme.rose),
            other => (other, theme.dim),
        };
        let (cat_display, cat_style) = if entry.phase == "FOCUS" {
            let c_enum = FocusCategory::from_name(&entry.category);
            (
                c_enum.label(),
                style::accent_bold(theme.category_color(c_enum)),
            )
        } else {
            ("—", style::label(theme))
        };

        let mut row_spans = vec![
            Span::styled(format!("{:<11}", entry.timestamp), style::label(theme)),
            Span::raw(" ".repeat(g1)),
            Span::styled(
                format!("{phase_display:<5}"),
                Style::default().fg(phase_col),
            ),
            Span::raw(" ".repeat(g2)),
        ];
        if show_mode_and_cat || show_cat_only {
            row_spans.push(Span::styled(format!("{cat_display:<8}"), cat_style));
            row_spans.push(Span::raw(" ".repeat(g3)));
        }
        if show_mode_and_cat {
            let m_col = theme.mode_name_color(&entry.mode);
            row_spans.push(Span::styled(
                format!("{:<8}", entry.mode),
                Style::default().fg(m_col),
            ));
            row_spans.push(Span::raw(" ".repeat(g4)));
        }
        row_spans.push(Span::styled(
            format!("{:>7}", SessionStats::format_mm_ss(entry.duration_secs)),
            style::body(theme),
        ));
        row_spans.push(Span::raw(" ".repeat(g5)));
        row_spans.push(Span::styled(
            format!("{status_display:>7}"),
            Style::default().fg(status_col),
        ));
        lines.push(Line::from(row_spans));
    }

    // Concise Category Milestones & Records (5 clean rows, no duplicate clutter!)
    if (area.height as usize).saturating_sub(lines.len()) >= 5 {
        lines.push(Line::from(""));
        lines.push(style::section_header(
            "Milestones & records",
            Some("highlights"),
            area.width,
            theme,
        ));

        let top_cat_val = summary
            .top_category
            .clone()
            .unwrap_or_else(|| ui.timer.category.label().to_string());
        let peak_date_str = summary
            .peak_day_date
            .map_or_else(|| "—".to_string(), SessionStats::format_date);
        let peak_day_val = format!(
            "{} ({peak_date_str})",
            SessionStats::format_duration_secs(summary.peak_day_secs)
        );
        let peak_hour_val = match summary.peak_hour {
            Some(h) => format!("{:02}:00–{:02}:00", h, (h + 1) % 24),
            None => "—".to_string(),
        };
        const FULL_DOW: [&str; 7] = [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ];
        let best_dow_val = summary
            .peak_weekday
            .map_or_else(|| "—".to_string(), |i| FULL_DOW[i % 7].to_string());

        let records = [
            (
                "Top focus category",
                top_cat_val.clone(),
                style::accent_bold(theme.category_color(FocusCategory::from_name(&top_cat_val))),
            ),
            ("Peak focus day", peak_day_val, style::accent_bold(theme.gold)),
            ("Peak focus hour", peak_hour_val, style::accent_bold(theme.cyan)),
            ("Best weekday", best_dow_val, style::accent_bold(theme.emerald)),
        ];

        for (k, v, st) in records {
            if lines.len() >= area.height as usize {
                break;
            }
            lines.push(dot_leader_row(k, v, st, area.width, theme));
        }
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn render_analytics_footer(f: &mut Frame, area: Rect, theme: &Theme) {
    const ITEMS: [(&str, &str); 5] = [
        ("c / tab", "filter category"),
        ("j / k", "scroll log"),
        ("g / esc", "close"),
        ("h", "help"),
        ("q", "quit"),
    ];
    const DROP: [&str; 3] = ["h", "j / k", "c / tab"];
    let line = style::keycap_bar(&ITEMS, &DROP, area.width, theme.focus_color, theme);
    f.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
}

// ─── Minimal mode ────────────────────────────────────────────────────────────

fn render_minimal(f: &mut Frame, size: Rect, ui: &UiState) {
    let timer = ui.timer;
    let theme = ui.theme;
    let phase_color = digit_color(ui);

    let outer = style::screen_block(theme);
    let inner = outer.inner(size);
    f.render_widget(outer, size);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let ring_area = chunks[0];
    let (from, to) = ring_colors(ui);
    f.render_widget(
        Ring {
            progress: timer.current_timer.progress(),
            from,
            to,
            track: theme.border,
        },
        ring_area,
    );
    render_ring_contents(f, ring_area, ui, phase_color, false);

    if ui.alarm_ringing {
        f.render_widget(alarm_hint(ui), chunks[1]);
        return;
    }
    let (_, phase_upper) = phase_labels(timer.current_timer.phase);
    let mut spans = vec![Span::styled(
        phase_upper,
        Style::default().fg(get_phase_color(timer.current_timer.phase, theme)),
    )];
    if timer.current_timer.phase == TimerPhase::Focus {
        spans.push(Span::styled("  ·  ", style::label(theme)));
        spans.push(Span::styled(
            timer.category.label(),
            Style::default().fg(theme.category_color(timer.category)),
        ));
    }
    if !timer.is_running() {
        spans.push(Span::styled("  ·  paused", style::label(theme)));
    }
    let phase_line = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    f.render_widget(phase_line, chunks[1]);
}

// ─── Animated Full-Cycle Reward Modal ────────────────────────────────────────

fn render_reward(f: &mut Frame, area: Rect, ui: &UiState, reward: &CycleReward) {
    let theme = ui.theme;
    let p = ui.reward_anim_progress.clamp(0.0, 1.0);
    let shimmer_idx = ((ui.reward_elapsed_ms / 250) % 4) as usize;
    const SPARKS: [&str; 4] = ["✦", "★", "◈", "✦"];
    let spark = SPARKS[shimmer_idx];

    let cat_col = theme.category_color(reward.final_category);
    let border_col = if p < 0.5 {
        blend_linear(cat_col, theme.emerald, p * 2.0)
    } else {
        blend_linear(theme.emerald, theme.today_color(), (p - 0.5) * 2.0)
    };

    let max_w = area.width.saturating_sub(4).min(68);
    let min_w = max_w.min(58);
    let anim_w = (min_w as f64 + (max_w.saturating_sub(min_w) as f64) * p).round() as u16;
    let width = anim_w.clamp(min_w, max_w);

    let compact = area.height < 21 || width < 58;
    let height = if compact {
        9u16.min(area.height.saturating_sub(2)).max(7)
    } else {
        14u16.min(area.height.saturating_sub(2))
    };
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    // Build animated cycle-completion nodes (e.g. ✓ Cycle 1 ── ✓ Cycle 2 ...)
    let mut cycle_spans: Vec<Span<'static>> = Vec::new();
    let total_cycles = reward.cycles_completed.max(1);
    for idx in 0..total_cycles {
        if idx > 0 {
            cycle_spans.push(Span::styled(" ── ", style::rule_style(theme)));
        }
        let c_cat = ui
            .timer
            .cycle_categories
            .get(idx as usize)
            .copied()
            .unwrap_or(reward.final_category);
        let c_col = theme.category_color(c_cat);
        let threshold = (idx as f64) / (total_cycles as f64);
        let revealed = p >= threshold;
        let badge_col = if revealed { theme.emerald } else { theme.dim };
        cycle_spans.push(Span::styled("✓ ", style::accent_bold(badge_col)));
        cycle_spans.push(Span::styled(
            format!("Cycle {}", idx + 1),
            style::accent_bold(c_col),
        ));
    }

    let earned_str = format!(
        "+{} min ({})",
        reward.total_focus_mins,
        SessionStats::format_duration_secs(reward.total_focus_mins * 60)
    );
    let today_str = SessionStats::format_duration_secs(reward.today_focus_secs);
    let cats_str = reward.categories_summary();

    let mut cat_spans: Vec<Span<'static>> = vec![Span::styled("Categories: ", style::label(theme))];
    for (i, &cat) in reward.categories_used.iter().enumerate() {
        if i > 0 {
            cat_spans.push(Span::styled(" · ", style::label(theme)));
        }
        let cc = theme.category_color(cat);
        cat_spans.push(Span::styled(format!("● {}", cat.label()), style::accent_bold(cc)));
    }
    if reward.categories_used.is_empty() {
        cat_spans.push(Span::styled(cats_str.clone(), style::accent_bold(cat_col)));
    }

    let motivation = match reward.mode_name {
        "Session" => format!(
            "3 deep-work cycles complete — enjoy your {}m break.",
            reward.long_break_mins
        ),
        "Extended" => format!(
            "2 endurance cycles complete — enjoy your {}m break.",
            reward.long_break_mins
        ),
        _ => format!(
            "{} focus cycles complete — enjoy your {}m break.",
            reward.cycles_completed, reward.long_break_mins
        ),
    };

    let crest_arms = ((p * 6.0).round() as usize).clamp(1, 6);
    let crest_bar = "━".repeat(crest_arms);

    let text = if compact {
        let mut lines = vec![
            Line::from(vec![
                Span::styled(format!("{spark} "), style::accent_bold(theme.today_color())),
                Span::styled(
                    format!(
                        "{} Mode Complete ({}/{} cycles)",
                        reward.mode_name, reward.cycles_completed, reward.cycles_completed
                    ),
                    style::accent_bold(theme.gold),
                ),
                Span::styled(format!(" {spark}"), style::accent_bold(theme.today_color())),
            ]),
            Line::from(vec![
                Span::styled("Earned: ", style::label(theme)),
                Span::styled(earned_str, style::accent_bold(theme.emerald)),
                Span::styled(" · ", style::label(theme)),
                Span::styled(cats_str, style::accent_bold(cat_col)),
            ]),
            Line::from(vec![
                Span::styled("Today: ", style::label(theme)),
                Span::styled(today_str, style::value_bold(theme)),
                Span::styled(
                    format!(" ({} sess)", reward.today_sessions),
                    style::label(theme),
                ),
                Span::styled(
                    format!("  ·  Streak {}d", reward.current_streak),
                    style::accent_bold(theme.gold),
                ),
            ]),
            Line::from(Span::styled(motivation, style::label(theme))),
            Line::from(vec![
                Span::styled("[Enter / Space]", style::keycap(theme.emerald)),
                Span::styled(
                    format!(" Start {}m Break   ", reward.long_break_mins),
                    style::body(theme),
                ),
                Span::styled("[R]", style::keycap(theme.rose)),
                Span::styled(" Reset", style::label(theme)),
            ]),
        ];
        if rect.height <= 6 {
            lines.remove(3);
        }
        lines
    } else {
        let mut streak_spans = vec![
            Span::styled("Today: ", style::label(theme)),
            Span::styled(today_str, style::value_bold(theme)),
            Span::styled(
                format!(" ({} sess)", reward.today_sessions),
                style::label(theme),
            ),
            Span::styled(" · Streak: ", style::label(theme)),
            Span::styled(
                format!("{}d", reward.current_streak),
                style::accent_bold(theme.gold),
            ),
            Span::styled(
                format!(" · Cycle #{}", reward.all_time_cycles),
                style::accent_bold(theme.cyan),
            ),
        ];
        if reward.is_new_daily_peak {
            streak_spans.push(Span::styled(
                " ★ Peak",
                style::accent_bold(theme.today_color()),
            ));
        }

        let mut action_spans = vec![
            Span::styled("[Enter / Space]", style::keycap(theme.emerald)),
            Span::styled(
                format!(" Start {}m Break   ", reward.long_break_mins),
                style::value_bold(theme),
            ),
        ];
        if ui.alarm_ringing {
            action_spans.push(Span::styled("[A]", style::keycap(theme.gold)));
            action_spans.push(Span::styled(" Mute   ", style::label(theme)));
        }
        action_spans.push(Span::styled("[R]", style::keycap(theme.rose)));
        action_spans.push(Span::styled(" Reset", style::label(theme)));

        vec![
            Line::from(vec![
                Span::styled(
                    format!("{spark} {crest_bar}  "),
                    style::accent_bold(theme.today_color()),
                ),
                Span::styled(
                    "FULL FOCUS CYCLE COMPLETE",
                    style::accent_bold(theme.gold),
                ),
                Span::styled(
                    format!("  {crest_bar} {spark}"),
                    style::accent_bold(theme.today_color()),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("{} Mode", reward.mode_name),
                    style::accent_bold(theme.mode_name_color(reward.mode_name)),
                ),
                Span::styled("  ·  ", style::label(theme)),
                Span::styled(
                    format!(
                        "{} of {} focus cycles completed",
                        reward.cycles_completed, reward.cycles_completed
                    ),
                    style::accent_bold(theme.emerald),
                ),
            ]),
            Line::from(""),
            Line::from(cycle_spans),
            Line::from(vec![
                Span::styled("Focus earned: ", style::label(theme)),
                Span::styled(earned_str, style::accent_bold(theme.gold)),
                Span::styled("  ·  ", style::label(theme)),
                Span::styled(
                    format!("Next: {}m Break", reward.long_break_mins),
                    style::accent_bold(theme.long_break_color),
                ),
            ]),
            Line::from(cat_spans),
            Line::from(streak_spans),
            Line::from(Span::styled(motivation, style::label(theme))),
            Line::from(""),
            Line::from(action_spans),
        ]
    };

    f.render_widget(Clear, rect);
    let dialog = Paragraph::new(text)
        .block(style::dialog_block("Cycle Complete · Reward Unlocked", border_col))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(dialog, rect);
}

// ─── Mode-switch confirmation ────────────────────────────────────────────────

fn render_confirm(f: &mut Frame, area: Rect, ui: &UiState, target: &PomodoroMode) {
    let theme = ui.theme;
    let accent = theme.amber;
    let target_col = theme.mode_name_color(target.name);

    let width = area.width.saturating_sub(4).min(66);
    let compact = area.height < 20 || width < 56;
    let height = if compact {
        8u16.min(area.height.saturating_sub(2)).max(6)
    } else {
        11u16.min(area.height.saturating_sub(2))
    };
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    let done_cycles = ui.timer.completed_cycles_in_sequence();
    let total_cycles = ui.timer.cycles_before_long_break;
    let current_mode = ui.timer.mode.name;
    let progress_line = format!(
        "You have completed {} of {} focus cycles in {} mode.",
        done_cycles, total_cycles, current_mode
    );

    let state_detail = if ui.timer.is_reward_pending() {
        "Full-cycle reward is awaiting acknowledgment and will be dismissed.".to_string()
    } else if ui.timer.is_running() {
        format!(
            "Active {} session is running and will be interrupted.",
            ui.timer.current_timer.phase.label()
        )
    } else if ui.timer.current_timer.has_progress() {
        format!(
            "Paused {} session has unfinished progress that will be discarded.",
            ui.timer.current_timer.phase.label()
        )
    } else {
        format!(
            "Unfinished cycle sequence in {} mode will be abandoned.",
            current_mode
        )
    };

    let preservation_line = format!(
        "Switching to {} mode ({} cycles) resets cycle progression. Recorded focus sessions in your statistics are preserved.",
        target.name, target.focus_cycles
    );

    let key_confirm = |k: &'static str| Span::styled(k, style::keycap(theme.emerald));
    let key_cancel = |k: &'static str| Span::styled(k, style::keycap(theme.rose));

    let text = if compact {
        vec![
            Line::from(vec![
                Span::styled("Switch to ", style::body(theme)),
                Span::styled(
                    format!("{} ({} min)", target.name, target.summary()),
                    style::accent_bold(target_col),
                ),
                Span::styled("?", style::body(theme)),
            ]),
            Line::from(Span::styled(progress_line, style::accent_bold(theme.gold))),
            Line::from(Span::styled(
                "Resets cycle progression; recorded statistics are preserved.",
                style::label(theme),
            )),
            Line::from(vec![
                key_confirm("[Y / Enter]"),
                Span::styled(" Confirm switch   ", style::body(theme)),
                key_cancel("[N / Esc]"),
                Span::styled(" Cancel", style::label(theme)),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Switch to ", style::body(theme)),
                Span::styled(
                    format!("{} ({} min)", target.name, target.summary()),
                    style::accent_bold(target_col),
                ),
                Span::styled(
                    format!(" · {} focus cycles?", target.focus_cycles),
                    style::body(theme),
                ),
            ]),
            Line::from(Span::styled(progress_line, style::accent_bold(theme.gold))),
            Line::from(Span::styled(state_detail, Style::default().fg(theme.rose))),
            Line::from(""),
            Line::from(Span::styled(preservation_line, style::label(theme))),
            Line::from(""),
            Line::from(vec![
                key_confirm("[Y / Enter]"),
                Span::styled(" Confirm switch    ", style::value_bold(theme)),
                key_cancel("[N / Esc]"),
                Span::styled(" Cancel and continue", style::label(theme)),
            ]),
        ]
    };

    f.render_widget(Clear, rect);
    let dialog = Paragraph::new(text)
        .block(style::dialog_block("Switch focus mode?", accent))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(dialog, rect);
}

// ─── Help screen ─────────────────────────────────────────────────────────────

fn render_help(f: &mut Frame, area: Rect, ui: &UiState) {
    let theme = ui.theme;
    f.render_widget(Clear, area);

    let outer = style::screen_block(theme);
    let roomy = area.height >= 28;
    let inner = style::inset(outer.inner(area), style::PAD_X, u16::from(roomy));
    f.render_widget(outer, area);

    let row = |k: String, d: String| {
        Line::from(vec![
            Span::styled(format!("  {k:<12}"), style::keycap(theme.focus_color)),
            Span::styled(d, style::body(theme)),
        ])
    };

    let mut help_text = vec![Line::from(vec![
        Span::styled("DevChron", style::value_bold(theme)),
        Span::styled("  ·  Keyboard shortcuts", style::label(theme)),
    ])];
    if roomy {
        help_text.push(Line::from(""));
    }
    help_text.extend([
        style::section_header("Timer & phases", None, inner.width.min(54), theme),
        row("Space / P".into(), "Start or pause timer".into()),
        row("F / B / L".into(), "Choose Focus, Short break, or Long break".into()),
        row(
            "C / S-Tab".into(),
            "Cycle focus category (Coding, Learning, Reading)".into(),
        ),
        row("R".into(), "Reset current phase".into()),
        row("S".into(), "Skip to next phase".into()),
        row("A".into(), "Stop alarm at 00:00".into()),
        row("+ / -".into(), "Add or subtract 5 minutes".into()),
    ]);
    if roomy {
        help_text.push(Line::from(""));
    }
    help_text.push(style::section_header(
        "Modes",
        None,
        inner.width.min(54),
        theme,
    ));

    for (i, mode) in ui.modes.iter().enumerate() {
        let current = if i == ui.mode_index { "  · current" } else { "" };
        help_text.push(row(
            format!("{}", i + 1),
            format!("{:<9} {} min{}", mode.name, mode.summary(), current),
        ));
    }
    help_text.push(row("Tab".into(), "Cycle to next mode".into()));

    if roomy {
        help_text.push(Line::from(""));
    }
    help_text.extend([
        style::section_header("Analytics & view", None, inner.width.min(54), theme),
        row("G / D".into(), "Open or close Analytics".into()),
        row("C / Tab".into(), "Filter Analytics by category (All / Coding / Learning / Reading)".into()),
        row("J / K".into(), "Scroll Recent sessions on Analytics page".into()),
        row("M".into(), "Toggle minimal mode".into()),
        row("H / ?".into(), "Toggle help screen".into()),
        row("Q / Esc".into(), "Quit (Esc closes overlays first)".into()),
        Line::from(Span::styled(
            "  Press any key to return",
            style::label(theme),
        )),
    ]);

    f.render_widget(Paragraph::new(help_text).alignment(Alignment::Left), inner);
}

// ─── Too small ───────────────────────────────────────────────────────────────

fn render_too_small(f: &mut Frame, area: Rect, theme: &Theme) {
    let msg = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "Terminal too small",
            style::accent_bold(theme.focus_color),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Resize to at least 60×16",
            style::label(theme),
        )),
    ])
    .block(style::screen_block(theme))
    .alignment(Alignment::Center);

    f.render_widget(msg, area);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn get_phase_color(phase: TimerPhase, theme: &Theme) -> Color {
    theme.phase_color(phase)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::timer::mode::MODES;
    use crate::timer::session::{DailyStat, SessionStats, SystemIdentity};
    use chrono::NaiveDate;
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use std::time::{Duration, Instant};

    fn test_identity() -> SystemIdentity {
        SystemIdentity {
            username: "devuser".to_string(),
            hostname: "archbox".to_string(),
        }
    }

    pub(crate) fn make_timer(
        mode: usize,
        phase: TimerPhase,
        progress: f64,
        running: bool,
    ) -> PomodoroTimer {
        let mut t = PomodoroTimer::with_stats(
            MODES[mode],
            MODES[mode].focus_cycles,
            SessionStats::in_memory(),
        );
        if t.current_phase() != phase {
            t.set_phase(phase);
        }
        let dur = t.current_timer.duration;
        t.current_timer.remaining = dur.mul_f64(1.0 - progress);
        if running {
            t.toggle_at(Instant::now());
        }
        t
    }

    pub(crate) fn draw(
        w: u16,
        h: u16,
        timer: &PomodoroTimer,
        theme: &Theme,
        mode_index: usize,
        pending: Option<usize>,
        help: bool,
        minimal: bool,
    ) -> Buffer {
        draw_with_analytics(w, h, timer, theme, mode_index, pending, help, minimal, false)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_with_analytics(
        w: u16,
        h: u16,
        timer: &PomodoroTimer,
        theme: &Theme,
        mode_index: usize,
        pending: Option<usize>,
        help: bool,
        minimal: bool,
        show_analytics: bool,
    ) -> Buffer {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let identity = test_identity();
        let ui = UiState {
            timer,
            theme,
            show_help: help,
            show_analytics,
            minimal_mode: minimal,
            celebrating: false,
            modes: MODES,
            mode_index,
            pending_mode: pending,
            reward: timer.reward.as_ref(),
            reward_anim_progress: 1.0,
            reward_elapsed_ms: 1200,
            alarm_ringing: timer.alarm_ringing,
            alarm_flash: false,
            system_identity: &identity,
            analytics_filter: None,
            analytics_scroll: 0,
        };
        term.draw(|f| render(f, &ui)).unwrap();
        term.backend().buffer().clone()
    }

    fn text(buf: &Buffer) -> String {
        let w = buf.area.width as usize;
        buf.content()
            .chunks(w)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn buffer_to_ansi(buf: &Buffer) -> String {
        let w = buf.area.width as usize;
        let mut out = String::new();
        for row in buf.content().chunks(w) {
            for cell in row {
                let mut codes = String::from("\x1b[0m");
                if cell.modifier.contains(Modifier::BOLD) {
                    codes.push_str("\x1b[1m");
                }
                if cell.modifier.contains(Modifier::UNDERLINED) {
                    codes.push_str("\x1b[4m");
                }
                if cell.modifier.contains(Modifier::REVERSED) {
                    codes.push_str("\x1b[7m");
                }
                if let Color::Rgb(r, g, b) = cell.fg {
                    codes.push_str(&format!("\x1b[38;2;{r};{g};{b}m"));
                }
                out.push_str(&codes);
                out.push_str(cell.symbol());
            }
            out.push_str("\x1b[0m\n");
        }
        out
    }

    pub(crate) fn buffer_to_html(buf: &Buffer, title: &str) -> String {
        let w = buf.area.width as usize;
        let mut html = format!(
            "<section style=\"margin:18px 0;\"><h3 style=\"font-family:monospace;color:#9ab;\">{title}</h3>\
             <pre style=\"background:#0d1117;color:#e6edf3;padding:12px;border-radius:8px;line-height:1.15;font-family:'JetBrains Mono',monospace;font-size:12px;display:inline-block;\">"
        );
        for row in buf.content().chunks(w) {
            for cell in row {
                let fg = match cell.fg {
                    Color::Rgb(r, g, b) => format!("rgb({r},{g},{b})"),
                    _ => "#e6edf3".to_string(),
                };
                let weight = if cell.modifier.contains(Modifier::BOLD) {
                    "font-weight:bold;"
                } else {
                    ""
                };
                let deco = if cell.modifier.contains(Modifier::UNDERLINED) {
                    "text-decoration:underline;"
                } else {
                    ""
                };
                let style_attr = if cell.modifier.contains(Modifier::REVERSED) {
                    format!("background:{fg};color:#0d1117;{weight}{deco}")
                } else {
                    format!("color:{fg};{weight}{deco}")
                };
                let sym = match cell.symbol() {
                    "&" => "&amp;",
                    "<" => "&lt;",
                    ">" => "&gt;",
                    s => s,
                };
                html.push_str(&format!("<span style=\"{style_attr}\">{sym}</span>"));
            }
            html.push('\n');
        }
        html.push_str("</pre></section>\n");
        html
    }

    #[test]
    #[ignore]
    fn visual_preview_gallery() {
        let mut full_html = String::from(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>DevChron Visual Gallery</title></head>\
             <body style=\"background:#06090f;color:#c9d1d9;padding:24px;\">",
        );

        let sizes = [
            (60u16, 16u16),
            (80, 24),
            (100, 30),
            (120, 40),
            (160, 50),
            (220, 52),
        ];
        for theme in Theme::all() {
            let mut t = make_timer(0, TimerPhase::Focus, 0.42, true);
            let today = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
            for offset in [0i64, 1, 2, 4, 7, 11, 18, 25, 40] {
                t.stats.complete_focus_session_on(
                    today - chrono::Duration::days(offset),
                    Duration::from_secs(1800 + (offset as u64 % 4) * 1200),
                );
            }
            let ringing = ringing_timer();

            for &(w, h) in &sizes {
                let screens = [
                    ("Timer", draw(w, h, &t, &theme, 0, None, false, false)),
                    ("Minimal", draw(w, h, &t, &theme, 0, None, false, true)),
                    (
                        "Analytics",
                        draw_with_analytics(w, h, &t, &theme, 0, None, false, false, true),
                    ),
                    ("Help", draw(w, h, &t, &theme, 0, None, true, false)),
                    ("Confirm", draw(w, h, &t, &theme, 0, Some(2), false, false)),
                    ("Alarm", draw(w, h, &ringing, &theme, 0, None, false, false)),
                ];
                for (name, buf) in screens {
                    let title = format!("{} — {} ({}×{})", theme.display_name(), name, w, h);
                    if w == 100 && h == 30 && theme.name == super::super::theme::ThemeName::TokyoNight {
                        println!("=== {title} ===\n{}", buffer_to_ansi(&buf));
                    }
                    full_html.push_str(&buffer_to_html(&buf, &title));
                }
            }
        }
        full_html.push_str("</body></html>");
        let _ = std::fs::write("target/visual_preview.html", full_html);
    }

    #[test]
    fn renders_without_panicking_at_every_size_and_state() {
        let theme = Theme::default_theme();
        for mode in 0..MODES.len() {
            for (phase, p, running) in [
                (TimerPhase::Focus, 0.0, false),
                (TimerPhase::Focus, 0.42, true),
                (TimerPhase::ShortBreak, 0.9, false),
                (TimerPhase::LongBreak, 1.0, true),
            ] {
                let t = make_timer(mode, phase, p, running);
                for w in [40u16, 59, 60, 61, 80, 100, 120, 160, 200, 226] {
                    for h in [10u16, 15, 16, 17, 24, 29, 30, 31, 40, 52, 64] {
                        for (pending, help, minimal, analytics) in [
                            (None, false, false, false),
                            (Some((mode + 1) % 3), false, false, false),
                            (None, true, false, false),
                            (None, false, true, false),
                            (Some(0), false, true, false),
                            (None, false, false, true),
                        ] {
                            draw_with_analytics(
                                w, h, &t, &theme, mode, pending, help, minimal, analytics,
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn timer_page_shows_phase_controls_and_focus_categories() {
        let theme = Theme::default_theme();
        let mut t = make_timer(0, TimerPhase::Focus, 0.25, false);
        t.category = FocusCategory::Learning;
        let screen = text(&draw(100, 32, &t, &theme, 0, None, false, false));
        assert!(!screen.contains("[STATISTICS]"), "old statistics panel must be removed:\n{screen}");
        assert!(screen.contains("Analytics"), "analytics hint must be shown:\n{screen}");
        assert!(screen.contains("Focus") && screen.contains("Short break") && screen.contains("Long break"), "phase chips missing:\n{screen}");
        assert!(screen.contains("Coding") && screen.contains("Learning") && screen.contains("Reading"), "category chips missing:\n{screen}");
        assert!(screen.contains("f/b/l focus/break"), "phase control hint missing:\n{screen}");
        assert!(screen.contains("c category"), "category control hint missing:\n{screen}");
        assert!(screen.contains("╭") && screen.contains("╯"), "outer frame must be rounded:\n{screen}");
    }

    #[test]
    fn analytics_page_shows_color_coded_category_telemetry_and_no_clutter() {
        let theme = Theme::default_theme();
        let mut t = make_timer(0, TimerPhase::Focus, 0.0, false);
        let today = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        t.stats.complete_focus_session_with_category(
            today,
            10,
            Duration::from_secs(3600),
            "Classic",
            "Coding",
            true,
        );
        t.stats.complete_focus_session_with_category(
            today,
            14,
            Duration::from_secs(1800),
            "Session",
            "Learning",
            true,
        );
        t.stats.complete_focus_session_with_category(
            today,
            20,
            Duration::from_secs(1800),
            "Classic",
            "Reading",
            true,
        );

        let buf = draw_with_analytics(160, 42, &t, &theme, 0, None, false, false, true);
        let screen = text(&buf);
        assert!(screen.contains("Analytics"), "{screen}");
        assert!(screen.contains("devuser@archbox"), "user@host missing:\n{screen}");
        assert!(screen.contains("Close Analytics"), "close hint missing:\n{screen}");
        assert!(screen.contains("Current streak"), "{screen}");
        assert!(screen.contains("Yearly Focus Grid"), "{screen}");
        assert!(screen.contains("By hour"), "{screen}");
        assert!(screen.contains("By weekday"), "{screen}");
        assert!(screen.contains("Last 7 days"), "{screen}");
        assert!(!screen.contains("Last 28 days"), "28-day zero wall must be gone:\n{screen}");
        assert!(screen.contains("By category"), "category analytics section missing:\n{screen}");
        assert!(screen.contains("Category telemetry"), "Category telemetry missing:\n{screen}");
        assert!(screen.contains("Today mix") && screen.contains("7-day mix"), "Category mix bars missing:\n{screen}");
        assert!(screen.contains("Coding") && screen.contains("Learning") && screen.contains("Reading"), "categories missing on analytics:\n{screen}");
        assert!(screen.contains("Modes & pacing"), "{screen}");
        assert!(screen.contains("Recent sessions"), "{screen}");
        assert!(screen.contains("Category"), "Category column missing in Recent sessions:\n{screen}");

        // Verify that all three distinct category colors are actually painted in the Analytics buffer
        let has_coding_col = buf.content().iter().any(|c| c.fg == theme.focus_color);
        let has_learning_col = buf.content().iter().any(|c| c.fg == theme.short_break_color);
        let has_reading_col = buf.content().iter().any(|c| c.fg == theme.long_break_color);
        assert!(has_coding_col && has_learning_col && has_reading_col, "all 3 category colors must be rendered on Analytics page");

        // Wide & tall screen (e.g. 226x52) scales grid to 2-col cells ("▆▆") and shows Category × weekday matrix
        let wide_screen = text(&draw_with_analytics(
            226, 52, &t, &theme, 0, None, false, false, true,
        ));
        assert!(wide_screen.contains("▆▆"), "wide screen must use 2-col grid blocks:\n{wide_screen}");
        assert!(wide_screen.contains("Category × weekday"), "tall screen must show Category × weekday:\n{wide_screen}");
        assert!(wide_screen.contains("Category telemetry"), "tall screen must show Category telemetry:\n{wide_screen}");
    }

    #[test]
    fn heatmap_color_intensity_is_directly_proportional_to_focus_time() {
        let theme = Theme::reef();
        let max_s = 3600;
        let c0 = heatmap_cell_color(&theme, 0, max_s);
        let c1 = heatmap_cell_color(&theme, 900, max_s);
        let c2 = heatmap_cell_color(&theme, 1800, max_s);
        let c3 = heatmap_cell_color(&theme, 2700, max_s);
        let c4 = heatmap_cell_color(&theme, 3600, max_s);

        assert_ne!(c0, c1);
        if let (
            Color::Rgb(r1, _, _),
            Color::Rgb(r2, _, _),
            Color::Rgb(r3, _, _),
            Color::Rgb(r4, _, _),
        ) = (c1, c2, c3, c4)
        {
            assert!(r1 < r2 && r2 < r3 && r3 < r4, "{r1} < {r2} < {r3} < {r4}");
        } else {
            panic!("expected RGB colors");
        }
        let _ = DailyStat::default();
    }

    #[test]
    fn mode_tabs_list_every_mode_and_mark_only_the_selected_one() {
        let theme = Theme::default_theme();
        for (selected, summary) in [(0, "25/5/15"), (1, "50/10/30"), (2, "75/15/45")] {
            let t = make_timer(selected, TimerPhase::Focus, 0.0, false);
            let screen = text(&draw(80, 24, &t, &theme, selected, None, false, false));
            for name in ["Classic", "Session", "Extended"] {
                assert!(screen.contains(name), "{name} tab missing");
            }
            assert!(screen.contains(&format!("· {summary}")), "selected chip shows {summary}");
            for (i, other) in ["25/5/15", "50/10/30", "75/15/45"].iter().enumerate() {
                if i != selected {
                    assert!(!screen.contains(other), "unselected summary {other} leaked");
                }
            }
        }
    }

    #[test]
    fn selected_tab_is_marked_by_more_than_colour() {
        use ratatui::style::Modifier;
        let theme = Theme::default_theme();
        let t = make_timer(1, TimerPhase::Focus, 0.0, false);
        let buf = draw(80, 24, &t, &theme, 1, None, false, false);
        let screen = text(&buf);
        let row = screen.lines().position(|l| l.contains("Session")).unwrap();
        let line: Vec<char> = screen.lines().nth(row).unwrap().chars().collect();
        let col = |needle: &str| {
            let s: Vec<char> = needle.chars().collect();
            (0..line.len()).find(|&i| line[i..].starts_with(&s)).unwrap()
        };
        let selected = buf.get(col("Session") as u16, row as u16);
        let other = buf.get(col("Classic") as u16, row as u16);
        assert!(selected.modifier.contains(Modifier::REVERSED | Modifier::BOLD));
        assert!(!other.modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn confirm_dialog_names_the_target_mode_progress_and_both_choices() {
        let theme = Theme::default_theme();
        let mut t = make_timer(0, TimerPhase::Focus, 0.3, true);
        t.cycle_count = 2;
        let screen = text(&draw(80, 24, &t, &theme, 0, Some(2), false, false));
        assert!(screen.contains("Switch to Extended (75/15/45 min)"), "{screen}");
        assert!(
            screen.contains("You have completed 2 of 4 focus cycles in Classic mode."),
            "{screen}"
        );
        assert!(screen.contains("statistics are preserved"), "{screen}");
        assert!(screen.contains("[Y / Enter]"), "{screen}");
        assert!(screen.contains("[N / Esc]"), "{screen}");

        // Also fits cleanly at minimum supported terminal size (60x16)
        let min_screen = text(&draw(60, 16, &t, &theme, 0, Some(1), false, false));
        assert!(min_screen.contains("Switch to Session (50/10/30 min)?"), "{min_screen}");
        assert!(min_screen.contains("2 of 4 focus cycles"), "{min_screen}");
        assert!(min_screen.contains("[Y / Enter]"), "{min_screen}");
        assert!(min_screen.contains("[N / Esc]"), "{min_screen}");

        let idle_screen = text(&draw(80, 24, &t, &theme, 0, None, false, false));
        assert!(!idle_screen.contains("Switch focus mode?"));
    }

    #[test]
    fn full_cycle_reward_modal_renders_at_all_sizes_and_animation_stages() {
        let theme = Theme::default_theme();
        let mut t = make_timer(2, TimerPhase::Focus, 0.0, false); // Extended (2 cycles, 150m)
        let mut now = Instant::now();
        t.toggle_at(now);
        now += Duration::from_secs(75 * 60);
        t.tick_at(now);
        t.dismiss_alarm();
        t.toggle_at(now);
        now += Duration::from_secs(15 * 60);
        t.tick_at(now);
        t.dismiss_alarm();
        t.cycle_category(); // Learning
        t.toggle_at(now);
        now += Duration::from_secs(75 * 60);
        t.tick_at(now);
        assert!(t.is_reward_pending());

        for &(w, h) in &[(60u16, 16u16), (80, 24), (100, 30), (160, 42)] {
            let screen = text(&draw(w, h, &t, &theme, 2, None, false, false));
            assert!(screen.contains("Extended"), "{w}x{h}:
{screen}");
            assert!(screen.contains("150"), "{w}x{h} total focus mins missing:
{screen}");
            assert!(screen.contains("45m"), "{w}x{h} long break mins missing:
{screen}");
            assert!(screen.contains("[Enter / Space]"), "{w}x{h} action prompt missing:
{screen}");
        }
    }

    #[test]
    fn help_lists_every_mode_from_the_central_table() {
        let theme = Theme::default_theme();
        let t = make_timer(0, TimerPhase::Focus, 0.0, false);
        let screen = text(&draw(80, 30, &t, &theme, 0, None, true, false));
        for m in MODES {
            assert!(screen.contains(m.name) && screen.contains(&m.summary()));
        }
        assert!(screen.contains("current"));
    }

    fn ring_share(buf: &Buffer, track: Color) -> f64 {
        let (mut active, mut total) = (0u32, 0u32);
        for c in buf.content() {
            let ch = c.symbol().chars().next().unwrap_or(' ');
            if ('\u{2801}'..='\u{28FF}').contains(&ch) {
                let dots = ((ch as u32) - 0x2800).count_ones();
                total += dots;
                if c.fg != track {
                    active += dots;
                }
            }
        }
        active as f64 / total as f64
    }

    #[test]
    fn ring_in_the_rendered_frame_tracks_the_timer() {
        let theme = Theme::default_theme();
        for mode in 0..MODES.len() {
            let mut last = -1.0;
            for p in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
                let t = make_timer(mode, TimerPhase::Focus, p, true);
                let share = ring_share(&draw(100, 40, &t, &theme, mode, None, false, false), theme.border);
                assert!((share - p).abs() < 0.05, "mode {mode} p={p}: ring share {share:.3}");
                assert!(share >= last, "ring must never shrink");
                last = share;
            }
        }
    }

    #[test]
    fn ring_is_empty_at_start_and_full_at_end() {
        let theme = Theme::default_theme();
        let start = make_timer(0, TimerPhase::Focus, 0.0, false);
        let end = make_timer(0, TimerPhase::Focus, 1.0, false);
        assert_eq!(ring_share(&draw(100, 40, &start, &theme, 0, None, false, false), theme.border), 0.0);
        assert_eq!(ring_share(&draw(100, 40, &end, &theme, 0, None, false, false), theme.border), 1.0);
    }

    #[test]
    fn digits_scale_down_with_the_ring_but_stay_readable() {
        assert_eq!(choose_digit_style("25:00", 60.0), DigitStyle::Large);
        assert_eq!(choose_digit_style("25:00", 28.0), DigitStyle::Medium);
        assert_eq!(choose_digit_style("25:00", 10.0), DigitStyle::Compact);
        for r in [10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 60.0] {
            let s = choose_digit_style("75:00", r);
            if s != DigitStyle::Compact {
                assert!(fits_in_ring(s.text_width("75:00"), s.rows() as f64 / 2.0, r));
            }
        }
    }

    #[test]
    fn block_digit_lines_are_rectangular_and_exact_width() {
        for style in [DigitStyle::Large, DigitStyle::Medium] {
            let lines = digit_lines("75:08", style, Color::White);
            assert_eq!(lines.len(), 5);
            for l in &lines {
                assert_eq!(l.width(), style.text_width("75:08"), "{style:?}");
            }
        }
    }

    #[test]
    fn nothing_is_clipped_at_the_minimum_size() {
        let theme = Theme::default_theme();
        let t = make_timer(0, TimerPhase::Focus, 0.5, true);
        let screen = text(&draw(60, 16, &t, &theme, 0, None, false, false));
        assert!(screen.contains("q quit"), "{screen}");
        assert!(screen.contains("1-3 mode"), "{screen}");
        assert!(screen.contains("01/04"), "{screen}");

        let analytics_min = text(&draw_with_analytics(60, 16, &t, &theme, 0, None, false, false, true));
        assert!(analytics_min.contains("today"), "{analytics_min}");
        assert!(analytics_min.contains("close"), "{analytics_min}");
    }

    #[test]
    fn remaining_time_is_shown_as_mm_ss_in_compact_style() {
        let theme = Theme::default_theme();
        let t = make_timer(0, TimerPhase::Focus, 0.5, false);
        let screen = text(&draw(60, 16, &t, &theme, 0, None, false, false));
        assert!(screen.contains("12:30") || screen.contains("12:31"), "{screen}");
        let _ = Duration::ZERO;
    }

    fn ringing_timer() -> PomodoroTimer {
        let mut t = make_timer(0, TimerPhase::Focus, 0.0, true);
        let t0 = Instant::now();
        t.tick_at(t0 + Duration::from_secs(3600));
        assert!(t.alarm_ringing);
        t
    }

    #[test]
    fn stop_alarm_button_appears_only_while_ringing() {
        let theme = Theme::default_theme();
        let quiet = make_timer(0, TimerPhase::Focus, 0.5, true);
        let ringing = ringing_timer();
        for (w, h) in [(80u16, 40u16), (80, 24), (60, 16)] {
            for minimal in [false, true] {
                let s = text(&draw(w, h, &quiet, &theme, 0, None, false, minimal));
                assert!(!s.contains("stop"), "{w}x{h} min={minimal}: stop shown while quiet\n{s}");
                assert!(!s.contains("Stop"), "{w}x{h} min={minimal}\n{s}");
                let s = text(&draw(w, h, &ringing, &theme, 0, None, false, minimal));
                assert!(
                    s.contains("00:00") || s.contains('█'),
                    "{w}x{h} min={minimal}: no countdown shown\n{s}"
                );
                assert!(s.contains("[A]") || s.contains("stop alarm"), "{w}x{h} min={minimal}\n{s}");
            }
        }
        let s = text(&draw(100, 40, &ringing, &theme, 0, None, false, false));
        assert!(s.contains("Stop alarm") && s.contains("ALARM"), "{s}");
        assert!(!s.contains("skip"), "inactive keys are hidden while ringing");
    }

    #[test]
    fn minimal_mode_is_the_ring_with_the_time_and_one_text_line() {
        let theme = Theme::default_theme();
        let t = make_timer(0, TimerPhase::Focus, 0.5, true);
        let buf = draw(80, 30, &t, &theme, 0, None, false, true);
        let s = text(&buf);
        let share = ring_share(&buf, theme.border);
        assert!((share - 0.5).abs() < 0.05, "ring share {share:.3}");
        assert!(s.contains("█"), "{s}");
        assert!(s.contains("FOCUS"), "{s}");
        for gone in ["DevChron", "STATISTICS", "CONTROLS", "Classic", "running", "01/04"] {
            assert!(!s.contains(gone), "{gone} should not be in minimal mode\n{s}");
        }
    }

    #[test]
    fn alarm_flash_changes_the_colours_but_not_the_layout() {
        let theme = Theme::default_theme();
        let t = ringing_timer();
        let identity = test_identity();
        let mut term_a = Terminal::new(TestBackend::new(80, 30)).unwrap();
        let mut term_b = Terminal::new(TestBackend::new(80, 30)).unwrap();
        for (term, flash) in [(&mut term_a, true), (&mut term_b, false)] {
            let ui = UiState {
                timer: &t,
                theme: &theme,
                show_help: false,
                show_analytics: false,
                minimal_mode: true,
                celebrating: false,
                modes: MODES,
                mode_index: 0,
                pending_mode: None,
                reward: t.reward.as_ref(),
                reward_anim_progress: 1.0,
                reward_elapsed_ms: 1200,
                alarm_ringing: true,
                alarm_flash: flash,
                system_identity: &identity,
                analytics_filter: None,
                analytics_scroll: 0,
            };
            term.draw(|f| render(f, &ui)).unwrap();
        }
        let (a, b) = (term_a.backend().buffer(), term_b.backend().buffer());
        assert_eq!(text(a), text(b), "only colours differ");
        assert_ne!(a, b);
    }
}
