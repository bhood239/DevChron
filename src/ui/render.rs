use super::ring::{self, Ring};
use super::style;
use super::theme::{blend_linear, Theme};
use crate::timer::session::{AnalyticsSummary, SessionStats, SystemIdentity, GRID_DAYS_PER_WEEK};
use crate::timer::{PomodoroMode, PomodoroTimer, TimerPhase};
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
    /// The phase reached 00:00 and the alarm is ringing.
    pub alarm_ringing: bool,
    /// Flashes on/off (twice a second) while the alarm rings.
    pub alarm_flash: bool,
    /// Offline system username and computer hostname.
    pub system_identity: &'a SystemIdentity,
}

// ─── Big-digit block art ────────────────────────────────────────────────────
//
// Large digits are 5 rows × 5 cols, medium digits are 5 rows × 3 cols. Both use
// full-block (█) and space characters. `choose_digit_style` picks the largest
// one that fits inside the progress ring.

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

/// Does a block `width_cols` wide, whose farthest edge is `edge_rows` rows from
/// the ring centre, fit inside the inner circle (radius `r_in`, in dots)?
/// One cell is 2 dots wide and 4 dots tall.
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
    colon_color: Color,
) -> Vec<Line<'static>> {
    let mut rows: [Vec<Span<'static>>; 5] = Default::default();
    let mut first = true;
    let d_style = Style::default().fg(digit_color).add_modifier(Modifier::BOLD);
    let c_style = Style::default().fg(colon_color).add_modifier(Modifier::BOLD);

    for ch in time_str.chars() {
        let (pattern, st): (&[&str], Style) = if ch == ':' {
            (colon, c_style)
        } else if ch.is_ascii_digit() {
            (digits[ch as usize - '0' as usize], d_style)
        } else {
            continue;
        };

        for (i, row) in pattern.iter().enumerate() {
            if !first {
                rows[i].push(Span::styled(" ", d_style));
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
    digit_lines_styled(time_str, style, color, color)
}

fn digit_lines_styled(
    time_str: &str,
    style: DigitStyle,
    digit_color: Color,
    colon_color: Color,
) -> Vec<Line<'static>> {
    match style {
        DigitStyle::Large => {
            block_lines_with_colon(time_str, &DIGITS, COLON, digit_color, colon_color)
        }
        DigitStyle::Medium => block_lines_with_colon(
            time_str,
            &DIGITS_MEDIUM,
            COLON_MEDIUM,
            digit_color,
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

    // Minimum terminal size guard
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

    let compact = inner.height < FULL_FOOTER_MIN_HEIGHT;
    let footer_height = if compact { 1 } else { 3 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),             // Header (brand + phase + mode tabs)
            Constraint::Min(0),                // Spacious central timer ring
            Constraint::Length(footer_height), // Quiet borderless keycap footer
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
    let phase_color = get_phase_color(timer.current_timer.phase, theme);
    let is_paused = !timer.is_running();
    let (phase_title, _) = phase_labels(timer.current_timer.phase);

    // Row 1: DevChron · Phase [paused / ALARM] ... g Analytics on the right
    let mut row1_spans = vec![
        Span::raw("  "),
        Span::styled("DevChron", style::value_bold(theme)),
        Span::styled("  ·  ", style::label(theme)),
        Span::styled(phase_title, style::accent_bold(phase_color)),
    ];

    if timer.alarm_ringing {
        row1_spans.push(Span::styled(
            "  ALARM",
            Style::default()
                .fg(if ui.alarm_flash { theme.text } else { phase_color })
                .add_modifier(Modifier::BOLD),
        ));
    } else if is_paused {
        row1_spans.push(Span::styled("  paused", style::label(theme)));
    }

    let mut row1 = Line::from(row1_spans);
    let right_key = "g";
    let right_lbl = " Analytics  ";
    let right_w = right_key.len() + right_lbl.len();
    let row1_left_w = row1.width();
    let width = area.width as usize;
    if row1_left_w + right_w + 2 <= width {
        let pad = width - row1_left_w - right_w;
        row1.spans.push(Span::raw(" ".repeat(pad)));
        row1.spans.push(Span::styled(right_key, style::keycap(phase_color)));
        row1.spans.push(Span::styled(right_lbl, style::label(theme)));
    }

    // Row 2: mode tabs on the left, session counter on the right.
    let mut row2 = mode_tabs(ui, phase_color);
    let session_text = format!(
        "{:02}/{:02}",
        timer.cycle_count + 1,
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
            .push(Span::styled(session_text, style::value_bold(theme)));
        row2.spans.push(Span::styled(suffix, style::label(theme)));
    }

    let header = Paragraph::new(vec![row1, row2]).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(style::rule_style(theme)),
    );

    f.render_widget(header, area);
}

/// `[ 1 Classic · 25/5/15 ]  2 Session  3 Extended`
///
/// The selected mode is a filled chip (reverse video + bold) that also shows
/// its focus/short/long minutes; the others stay quiet and plain.
fn mode_tabs(ui: &UiState, accent: Color) -> Line<'static> {
    let mut spans = vec![Span::raw("  ")];

    for (i, mode) in ui.modes.iter().enumerate() {
        if i == ui.mode_index {
            spans.push(Span::styled(
                format!(" {} {} · {} ", i + 1, mode.name, mode.summary()),
                Style::default()
                    .fg(accent)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} {} ", i + 1, mode.name),
                style::label(ui.theme),
            ));
        }
        spans.push(Span::raw(" "));
    }

    Line::from(spans)
}

// ─── Timer area ──────────────────────────────────────────────────────────────

/// Colour of the digits: phase colour while running, gently muted phase tint
/// while paused, and flashing between the phase colour and bright text while
/// the alarm rings.
fn digit_color(ui: &UiState) -> Color {
    let phase_color = get_phase_color(ui.timer.current_timer.phase, ui.theme);
    if ui.alarm_ringing {
        if ui.alarm_flash {
            ui.theme.text
        } else {
            phase_color
        }
    } else if ui.timer.is_running() {
        phase_color
    } else {
        blend_linear(phase_color, ui.theme.paused_color, 0.58)
    }
}

/// `(from, to)` colours of the progress ring for the current state: gently
/// softened while paused, flashing brighter while the alarm rings.
fn ring_colors(ui: &UiState) -> (Color, Color) {
    let (mut from, mut to) = ui.theme.ring_gradient(ui.timer.current_timer.phase);
    if ui.alarm_ringing {
        if ui.alarm_flash {
            from = blend_linear(from, ui.theme.text, 0.6);
            to = blend_linear(to, ui.theme.text, 0.6);
        }
    } else if !ui.timer.is_running() {
        from = blend_linear(from, ui.theme.paused_color, 0.45);
        to = blend_linear(to, ui.theme.paused_color, 0.45);
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
            style::accent_bold(theme.focus_color),
        )))
        .alignment(Alignment::Center);
        f.render_widget(celebration, hint_area);
    } else if never_started {
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("Press ", style::label(theme)),
            Span::styled("Space", style::body(theme)),
            Span::styled(" to start  ·  ", style::label(theme)),
            Span::styled("G", style::body(theme)),
            Span::styled(" for analytics", style::label(theme)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(hint, hint_area);
    }
}

/// Phase name, MM:SS and run state, centred inside the ring.
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
    let status = if ui.alarm_ringing {
        "time's up"
    } else if timer.is_running() {
        "running"
    } else {
        "paused"
    };

    // Subtle 1 Hz colon breath while running: uses remaining_secs_ceil() which
    // already drives the 1-second repaint cadence in main.rs, so zero extra
    // wakeups occur when idle.
    let colon_color = if timer.is_running() && !ui.alarm_ringing {
        if timer.current_timer.remaining_secs_ceil() % 2 == 1 {
            blend_linear(digit_color, theme.dim, 0.32)
        } else {
            digit_color
        }
    } else {
        digit_color
    };

    let label_width = phase_upper.len().max(status.chars().count() + 1);
    let label_edge = style.rows() as f64 / 2.0 + 2.0;
    let show_extras = with_extras && fits_in_ring(label_width, label_edge, geo.r_in);

    let mut lines: Vec<Line> = Vec::new();
    if show_extras {
        lines.push(Line::from(Span::styled(
            phase_upper,
            Style::default().fg(get_phase_color(timer.current_timer.phase, theme)),
        )));
        lines.push(Line::from(""));
    }
    lines.extend(digit_lines_styled(&time_str, style, digit_color, colon_color));
    if show_extras {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(status, style::label(theme))));
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

// ─── Footer (Quiet, Borderless Keycap List) ──────────────────────────────────

fn render_footer(f: &mut Frame, area: Rect, ui: &UiState) {
    render_controls(f, area, ui.timer, ui.theme);
}

/// One-line key reference used when the terminal is too short for the full footer.
fn render_compact_footer(f: &mut Frame, area: Rect, ui: &UiState) {
    let phase_color = get_phase_color(ui.timer.current_timer.phase, ui.theme);
    const HINTS: [(&str, &str); 8] = [
        ("p", "pause"),
        ("r", "reset"),
        ("s", "skip"),
        ("1-3", "mode"),
        ("g", "analytics"),
        ("t", "theme"),
        ("h", "help"),
        ("q", "quit"),
    ];
    const DROP_ORDER: [&str; 5] = ["t", "s", "r", "h", "g"];
    const RINGING_HINTS: [(&str, &str); 4] = [
        ("a", "stop alarm"),
        ("g", "analytics"),
        ("t", "theme"),
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

    let mut spans = Vec::new();
    for (i, (k, d)) in active.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(k.to_string(), style::keycap(phase_color)));
        spans.push(Span::styled(format!(" {d}"), style::label(ui.theme)));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

fn render_controls(f: &mut Frame, area: Rect, timer: &PomodoroTimer, theme: &Theme) {
    let phase_color = get_phase_color(timer.current_timer.phase, theme);
    let pause_label = if timer.is_running() { "pause" } else { "start" };

    let key = |k: &'static str| Span::styled(k, style::keycap(phase_color));
    let desc = |d: &'static str| Span::styled(format!(" {d}"), style::label(theme));
    let sep = || Span::raw("   ");

    if timer.alarm_ringing {
        let lines = vec![Line::from(vec![
            key("a"),
            Span::styled(" Stop alarm", style::accent_bold(phase_color)),
            sep(),
            key("g"),
            desc("analytics"),
            sep(),
            key("t"),
            desc("theme"),
            sep(),
            key("m"),
            desc("minimal"),
            sep(),
            key("h"),
            desc("help"),
            sep(),
            key("q"),
            desc("quit"),
        ])];
        f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
        return;
    }

    let lines = vec![
        Line::from(vec![
            key("space"),
            desc(pause_label),
            sep(),
            key("r"),
            desc("reset"),
            sep(),
            key("s"),
            desc("skip"),
            sep(),
            key("+/-"),
            desc("±5m"),
            sep(),
            key("1-3"),
            desc("mode"),
        ]),
        Line::from(vec![
            key("g"),
            desc("analytics"),
            sep(),
            key("t"),
            desc("theme"),
            sep(),
            key("m"),
            desc("minimal"),
            sep(),
            key("h"),
            desc("help"),
            sep(),
            key("q"),
            desc("quit"),
        ]),
    ];

    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}

// ─── Dedicated Offline Analytics Page ────────────────────────────────────────

/// Derived empty-cell colour from the theme (no hard-coded background RGB).
pub fn empty_cell_color(theme: &Theme) -> Color {
    theme.empty_cell()
}

/// Perceptual colour of a daily cell in the 365-day focus grid.
///
/// * `focus_secs == 0`: `theme.empty_cell()`.
/// * `focus_secs > 0`: perceptual linear-light ramp from a clearly visible
///   non-zero floor up to `theme.focus_color`.
pub fn heatmap_cell_color(theme: &Theme, focus_secs: u64, max_daily_secs: u64) -> Color {
    if focus_secs == 0 {
        return empty_cell_color(theme);
    }
    let ratio = SessionStats::intensity_for(focus_secs, max_daily_secs);
    theme.heatmap_ramp(ratio)
}

/// Format a two-part row (`left_label`, `bar`, `right_value`) that dynamically
/// expands the bar width so the row always fills 100% of `width` columns.
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
    let summary = ui.timer.stats.analytics_summary_with_context(
        now.date_naive(),
        now.hour() as usize,
        ui.timer.live_focus_secs(),
        ui.timer.mode.name,
    );

    let outer = style::screen_block(theme);
    let inner = outer.inner(size);
    f.render_widget(outer, size);

    if inner.height < 20 {
        let padded = style::inset(inner, 1, 0);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Compact header + user@host + close hint
                Constraint::Length(1), // Compact headline numbers strip
                Constraint::Min(9),    // Yearly heatmap grid
                Constraint::Length(1), // Bottom keycap bar
            ])
            .split(padded);

        render_analytics_compact_header(f, chunks[0], ui);
        render_analytics_compact_metrics(f, chunks[1], &summary, theme);
        render_analytics_heatmap(f, chunks[2], &summary, theme, false);
        render_analytics_footer(f, chunks[3], theme);
    } else if inner.height < 28 {
        let padded = style::inset(inner, style::PAD_X, 0);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Header + subtle divider
                Constraint::Length(4), // Borderless headline numbers strip
                Constraint::Min(10),   // Yearly heatmap + optional hourly chart
                Constraint::Length(1), // Bottom keycap bar
            ])
            .split(padded);

        render_analytics_header(f, chunks[0], ui);
        render_analytics_headline_strip(f, chunks[1], &summary, theme);
        render_analytics_middle_tier(f, chunks[2], &summary, theme);
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
                Constraint::Length(2),          // Header + subtle divider
                Constraint::Length(headline_h), // Borderless headline numbers strip (+ subtle bar when tall)
                Constraint::Length(mid_h),      // Hero: Yearly heatmap grid (+ By hour on wide screens)
                Constraint::Min(6),             // Dynamic full-height 3-column bottom deck
                Constraint::Length(1),          // Bottom keycap bar
            ])
            .split(padded);

        render_analytics_header(f, chunks[0], ui);
        render_analytics_headline_strip(f, chunks[1], &summary, theme);
        render_analytics_middle_tier(f, chunks[2], &summary, theme);
        render_analytics_bottom_deck(f, chunks[3], &summary, ui);
        render_analytics_footer(f, chunks[4], theme);
    }
}

fn render_analytics_header(f: &mut Frame, area: Rect, ui: &UiState) {
    let theme = ui.theme;
    let id = ui.system_identity;
    let width = area.width as usize;

    let mut row1 = Line::from(vec![
        Span::styled("DevChron", style::value_bold(theme)),
        Span::styled("  ·  ", style::label(theme)),
        Span::styled("Analytics", style::accent_bold(theme.focus_color)),
        Span::styled("  ·  ", style::label(theme)),
        Span::styled(format!("{}@{}", id.username, id.hostname), style::body(theme)),
        Span::styled("  ·  local", style::label(theme)),
    ]);

    if ui.timer.is_running() {
        row1.spans
            .push(Span::styled("  ·  running", Style::default().fg(theme.focus_color)));
    }

    let close_key = "g / esc";
    let close_lbl = " Close Analytics";
    let right_len = close_key.len() + close_lbl.len();
    let left_w = row1.width();
    if left_w + right_len + 2 <= width {
        let pad = width - left_w - right_len;
        row1.spans.push(Span::raw(" ".repeat(pad)));
        row1.spans
            .push(Span::styled(close_key, style::keycap(theme.focus_color)));
        row1.spans.push(Span::styled(close_lbl, style::label(theme)));
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

/// Borderless headline metrics row: calm labels in `dim`, bold values in `text`
/// (with `focus_color` on the primary streak hero), and quiet sub-details below.
/// On tall screens (`area.height >= 5`), adds a subtle proportional under-bar
/// spanning each column so the strip fills the full width cleanly.
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
                  col_w: u16,
                  is_hero: bool|
     -> Paragraph<'static> {
        let val_style = if is_hero {
            style::accent_bold(theme.focus_color)
        } else {
            style::value_bold(theme)
        };
        let mut lines = vec![
            Line::from(Span::styled(title, style::label(theme))),
            Line::from(Span::styled(primary, val_style)),
            Line::from(Span::styled(subtitle, style::label(theme))),
        ];
        if show_bar {
            let bw = (col_w as usize).saturating_sub(2).max(4);
            let r = ratio.clamp(0.0, 1.0);
            let filled = if r <= 0.0 {
                0
            } else {
                ((r * bw as f64).round() as usize).clamp(1, bw)
            };
            let empty = bw.saturating_sub(filled);
            let bar_col = theme.heatmap_ramp(r.max(0.25));
            lines.push(Line::from(vec![
                Span::styled("━".repeat(filled), Style::default().fg(bar_col)),
                Span::styled("─".repeat(empty), style::rule_style(theme)),
            ]));
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
                cols[0].width,
                true,
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
                cols[1].width,
                false,
            ),
            cols[1],
        );

        f.render_widget(
            metric(
                "Today's focus",
                SessionStats::format_duration_secs(summary.today_focus_secs),
                format!("{} sessions today", summary.today_sessions),
                today_ratio,
                cols[2].width,
                false,
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
                cols[3].width,
                false,
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
                cols[4].width,
                false,
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
                cols[5].width,
                false,
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
                cols[0].width,
                true,
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
                cols[1].width,
                false,
            ),
            cols[1],
        );

        f.render_widget(
            metric(
                "Today's focus",
                SessionStats::format_duration_secs(summary.today_focus_secs),
                format!("{} sessions today", summary.today_sessions),
                today_ratio,
                cols[2].width,
                false,
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
                cols[3].width,
                false,
            ),
            cols[3],
        );
    }
}

/// Middle tier: dynamically sizes the 53-week Yearly Focus Grid and the
/// 24-Hour By-Hour distribution across narrow, standard (`>= 140` cols), and
/// ultra-wide (`>= 195` cols) terminals so there is no dead horizontal space.
fn render_analytics_middle_tier(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    theme: &Theme,
) {
    let total_weeks = summary.weeks.len() as u16; // 53
    let wide_grid_w = 5 + total_weeks * 3; // 164 cols (2-col block "▆▆" + 1-col gap)
    let std_grid_w = 5 + total_weeks * 2; // 111 cols (1-col block "▆"  + 1-col gap)

    if area.width >= wide_grid_w + style::COL_GAP + 28 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(wide_grid_w),
                Constraint::Length(style::COL_GAP),
                Constraint::Min(28),
            ])
            .split(area);

        render_analytics_heatmap(f, cols[0], summary, theme, true);
        render_analytics_by_hour(f, cols[2], summary, theme);
    } else if area.width >= std_grid_w + style::COL_GAP + 26 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(std_grid_w),
                Constraint::Length(style::COL_GAP),
                Constraint::Min(26),
            ])
            .split(area);

        render_analytics_heatmap(f, cols[0], summary, theme, true);
        render_analytics_by_hour(f, cols[2], summary, theme);
    } else {
        render_analytics_heatmap(f, area, summary, theme, true);
    }
}

fn render_analytics_heatmap(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    theme: &Theme,
    with_section_header: bool,
) {
    if area.width < 10 || area.height < 8 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(13);

    if with_section_header {
        let meta = format!(
            "{} sessions · {} active days",
            summary.year_sessions, summary.active_days_year
        );
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

    // Dynamically scale cell width:
    // - cell_step == 3 ("▆▆ "): 2-col wide block + 1-col gap on large screens
    // - cell_step == 2 ("▆ "):  1-col wide block + 1-col gap on standard screens
    // - cell_step == 1 ("▆"):   1-col compact block on narrow screens
    // Uses U+2586 ('▆', Lower Three Quarters Block) which Kitty and modern
    // GPU terminals render via their native pixel-exact box-drawing shader
    // (100% width × 75% height with a clean 25% vertical gap between rows).
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

    // Month labels row aligned to week columns (calm dim style, never bold)
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

    // Weekday rows (Mon..Sun) — render all 7 × visible_weeks cells as a
    // complete, flush rectangular matrix (cells outside the 365-day window
    // render in empty_cell_color so the top-left and bottom-right corners
    // are never ragged or missing).
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
            let effective_secs = if cell.in_year_window {
                cell.focus_secs
            } else {
                0
            };
            let color = if is_today && effective_secs == 0 {
                theme.dim
            } else {
                heatmap_cell_color(theme, effective_secs, summary.max_daily_secs)
            };
            let cell_style = if is_today {
                Style::default()
                    .fg(color)
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

    // Legend row consistent with the perceptual ramp + today indicator
    if lines.len() < area.height as usize {
        let max_s = summary.max_daily_secs.max(3600);
        let scale_str = if area.width >= 66 {
            format!(
                "   ·   scale max {}   ·   ",
                SessionStats::format_duration_secs(max_s)
            )
        } else {
            format!("  ·  max {}  ·  ", SessionStats::format_duration_secs(max_s))
        };
        let mut legend_spans = vec![
            Span::styled("     Less ", style::label(theme)),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(heatmap_cell_color(theme, 0, max_s)),
            ),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(heatmap_cell_color(theme, max_s / 4, max_s)),
            ),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(heatmap_cell_color(theme, max_s / 2, max_s)),
            ),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(heatmap_cell_color(theme, (max_s * 3) / 4, max_s)),
            ),
            Span::styled(
                format!("{cell_glyph} "),
                Style::default().fg(heatmap_cell_color(theme, max_s, max_s)),
            ),
            Span::styled("More", style::label(theme)),
            Span::styled(scale_str, style::label(theme)),
            Span::styled(
                cell_glyph,
                Style::default()
                    .fg(theme.focus_color)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            Span::styled(" today", style::label(theme)),
        ];

        let legend_left_w: usize = legend_spans.iter().map(|s| s.content.chars().count()).sum();
        let right_summary = format!(
            "peak {} · {}/365 active ({:.0}%)",
            SessionStats::format_duration_secs(summary.peak_day_secs),
            summary.active_days_year,
            (summary.active_days_year as f64 / 365.0) * 100.0
        );
        let grid_right_edge = (label_prefix_width + visible_weeks * cell_step)
            .saturating_sub(usize::from(cell_step >= 2))
            .min(area.width as usize);
        if legend_left_w + right_summary.chars().count() + 4 <= grid_right_edge {
            let pad = grid_right_edge - legend_left_w - right_summary.chars().count();
            legend_spans.push(Span::raw(" ".repeat(pad)));
            legend_spans.push(Span::styled(right_summary, style::label(theme)));
        }

        lines.push(Line::from(legend_spans));
    }

    f.render_widget(Paragraph::new(lines), area);
}

/// Borderless 24-Hour distribution (`00h`..`23h` multi-row histogram + full-width
/// 4-window time-of-day split). Scales both horizontally (`step = 1, 2, or 3`) and
/// vertically (`chart_rows = 2..4`) to fill `area`.
fn render_analytics_by_hour(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    theme: &Theme,
) {
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

    // Dynamically scale histogram height (2 to 4 rows) to match the middle tier height
    let chart_rows = (area.height as usize).saturating_sub(7).clamp(2, 4);
    const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    for row_idx in 0..chart_rows {
        let low_band = (chart_rows - 1 - row_idx) as f64 / chart_rows as f64;
        let high_band = (chart_rows - row_idx) as f64 / chart_rows as f64;
        let mut row_spans: Vec<Span<'static>> = Vec::with_capacity(24);

        for h in 0..24 {
            let secs = summary.hourly_focus_secs[h];
            let ratio = if secs == 0 {
                0.0
            } else {
                (secs as f64 / max_h as f64).clamp(0.08, 1.0)
            };
            let color = heatmap_cell_color(theme, secs, max_h);

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
    lines.push(Line::from(Span::styled(axis, style::label(theme))));
    lines.push(Line::from(""));

    let (m_pct, a_pct, e_pct, n_pct) = summary.chronotype_pcts;
    let empty_col = empty_cell_color(theme);
    let period_row = |name: &'static str, pct: f64| -> Line<'static> {
        let r = (pct / 100.0).clamp(0.0, 1.0);
        let col = theme.heatmap_ramp(r);
        dynamic_bar_row(
            format!("{name:<15}"),
            style::label(theme),
            r,
            col,
            empty_col,
            format!(" {:>5.1}%", pct),
            style::body(theme),
            None,
            area.width,
        )
    };

    lines.push(period_row("Morning 06–12", m_pct));
    lines.push(period_row("Afternoon 12–18", a_pct));
    lines.push(period_row("Evening 18–24", e_pct));
    lines.push(period_row("Night 00–06", n_pct));

    f.render_widget(Paragraph::new(lines), area);
}

/// Calm, borderless bottom deck separated by whitespace gutters (`style::COL_GAP`).
/// Every column dynamically expands its bars horizontally to 100% of its column
/// width and scales its rows vertically to fill `area.height`.
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

        render_analytics_weekday_and_7d(f, cols[0], summary, ui.theme);
        render_analytics_modes_and_months(f, cols[2], summary, ui.theme);
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

        render_analytics_weekday_and_7d(f, cols[0], summary, ui.theme);
        render_analytics_recent_sessions(f, cols[2], summary, ui);
    }
}

/// Column 1: By weekday (`Mon`–`Sun`) + Dynamic Recent Days log (expands from
/// `Last 7 days` up to as many historical days as fit `area.height`, with
/// full-width bars aligned flush to the right margin).
fn render_analytics_weekday_and_7d(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    theme: &Theme,
) {
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
        .max(3600);
    const DOW_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let empty_col = empty_cell_color(theme);

    for ws in &summary.weekday_stats {
        let ratio = SessionStats::intensity_for(ws.focus_secs, max_dow);
        let col = heatmap_cell_color(theme, ws.focus_secs, max_dow);
        let is_peak = summary.peak_weekday == Some(ws.weekday_index) && ws.focus_secs > 0;
        let lbl_style = if is_peak {
            Style::default().fg(theme.focus_color)
        } else {
            style::label(theme)
        };

        lines.push(dynamic_bar_row(
            format!("{}  ", DOW_LABELS[ws.weekday_index]),
            lbl_style,
            ratio,
            col,
            empty_col,
            format!(" {}", style::format_duration_fixed(ws.focus_secs)),
            style::body(theme),
            Some((format!(" {:>3}s", ws.sessions), style::label(theme))),
            area.width,
        ));
    }

    if area.height >= 11 {
        lines.push(Line::from(""));

        // Collect all valid calendar days up to today from the 53-week grid so tall
        // terminals can show more than 7 days and fill the vertical height cleanly.
        let mut recent_days: Vec<(chrono::NaiveDate, u64, u32)> = summary
            .weeks
            .iter()
            .flat_map(|w| w.iter())
            .filter(|c| c.in_year_window && c.date <= summary.today_date)
            .map(|c| (c.date, c.focus_secs, c.sessions))
            .collect();
        recent_days.sort_by_key(|(d, _, _)| *d);
        if recent_days.is_empty() {
            recent_days.extend_from_slice(&summary.last_7_days);
        }

        let avail_rows = (area.height as usize).saturating_sub(lines.len() + 1);
        let num_days = avail_rows.clamp(1, 28).min(recent_days.len());
        let slice_start = recent_days.len().saturating_sub(num_days);
        let shown_days = &recent_days[slice_start..];

        let section_title = if num_days <= 7 {
            "Last 7 days".to_string()
        } else {
            format!("Last {num_days} days")
        };
        let active_in_window = shown_days.iter().filter(|(_, s, _)| *s > 0).count();
        let window_note = format!("{active_in_window}/{num_days} active");
        lines.push(style::section_header(
            &section_title,
            Some(&window_note),
            area.width,
            theme,
        ));

        let max_nd = shown_days
            .iter()
            .map(|(_, s, _)| *s)
            .max()
            .unwrap_or(0)
            .max(3600);

        for &(date, secs, sessions) in shown_days.iter().rev() {
            if lines.len() >= area.height as usize {
                break;
            }
            let ratio = SessionStats::intensity_for(secs, max_nd);
            let bar_color = heatmap_cell_color(theme, secs, max_nd);
            let dow = DOW_LABELS[date.weekday().num_days_from_monday() as usize];

            lines.push(dynamic_bar_row(
                format!("{:02}-{:02} {dow} ", date.month(), date.day()),
                style::label(theme),
                ratio,
                bar_color,
                empty_col,
                format!(" {}", style::format_duration_fixed(secs)),
                style::body(theme),
                Some((format!(" {:>3}s", sessions), style::label(theme))),
                area.width,
            ));
        }
    }

    f.render_widget(Paragraph::new(lines), area);
}

/// Column 2: Modes & pacing + Recent months (up to 12 months on tall screens) +
/// Focus rhythm & targets (fills remaining vertical space on tall terminals).
fn render_analytics_modes_and_months(
    f: &mut Frame,
    area: Rect,
    summary: &AnalyticsSummary,
    theme: &Theme,
) {
    if area.height == 0 || area.width < 18 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(style::section_header(
        "Modes & pacing",
        None,
        area.width,
        theme,
    ));

    let empty_col = empty_cell_color(theme);
    for ms in &summary.mode_stats {
        let r = (ms.share_pct / 100.0).clamp(0.0, 1.0);
        let col = theme.heatmap_ramp(r);

        lines.push(dynamic_bar_row(
            format!("{:<9} ", ms.name),
            style::label(theme),
            r,
            col,
            empty_col,
            format!(" {:>4.0}%", ms.share_pct),
            style::label(theme),
            Some((
                format!(" {}", style::format_duration_fixed(ms.focus_secs)),
                style::body(theme),
            )),
            area.width,
        ));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Focus ", style::label(theme)),
        Span::styled(
            SessionStats::format_duration_secs(summary.year_focus_secs),
            style::value_bold(theme),
        ),
        Span::styled("  ·  Break ", style::label(theme)),
        Span::styled(
            SessionStats::format_duration_secs(summary.year_break_secs),
            style::body(theme),
        ),
        Span::styled("  ·  Cycles ", style::label(theme)),
        Span::styled(format!("{}", summary.cycles_completed), style::body(theme)),
    ]));

    if area.height >= 11 {
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
            .map(|(_, s, _)| *s)
            .max()
            .unwrap_or(0)
            .max(3600);

        for (m_label, m_secs, m_sess) in &summary.monthly_velocity {
            if lines.len() >= area.height as usize {
                break;
            }
            let ratio = SessionStats::intensity_for(*m_secs, max_m);
            let col = heatmap_cell_color(theme, *m_secs, max_m);

            lines.push(dynamic_bar_row(
                format!("{m_label:<8} "),
                style::label(theme),
                ratio,
                col,
                empty_col,
                format!(" {}", style::format_duration_fixed(*m_secs)),
                style::body(theme),
                Some((format!(" {:>3}s", m_sess), style::label(theme))),
                area.width,
            ));
        }
    }

    // On tall terminals (e.g. >= 36 rows), fill remaining vertical space in Column 2
    // with Focus rhythm & consistency metrics so there is no dead void below Recent months.
    if (area.height as usize).saturating_sub(lines.len()) >= 4 {
        lines.push(Line::from(""));
        lines.push(style::section_header(
            "Focus rhythm",
            Some("consistency"),
            area.width,
            theme,
        ));

        let active_ratio = (summary.active_days_year as f64 / 365.0).clamp(0.0, 1.0);
        let comp_ratio = (summary.completion_rate_pct / 100.0).clamp(0.0, 1.0);
        let focus_share = (summary.focus_share_pct / 100.0).clamp(0.0, 1.0);
        let streak_ratio = if summary.longest_streak == 0 {
            0.0
        } else {
            (summary.current_streak as f64 / summary.longest_streak as f64).clamp(0.0, 1.0)
        };
        let scale_ref = summary.max_daily_secs.max(3600) as f64;
        let active_vs_scale = (summary.avg_active_day_secs as f64 / scale_ref).clamp(0.0, 1.0);
        let today_vs_scale = (summary.today_focus_secs as f64 / scale_ref).clamp(0.0, 1.0);

        let rhythm_rows: [(&str, f64, String); 5] = [
            (
                "Active days  ",
                active_ratio,
                format!(" {:>11}", format!("{}/365d", summary.active_days_year)),
            ),
            (
                "Completion   ",
                comp_ratio,
                format!(
                    " {:>11}",
                    format!(
                        "{:.0}% ({}ok)",
                        summary.completion_rate_pct, summary.completed_focus_count
                    )
                ),
            ),
            (
                "Work share   ",
                focus_share,
                format!(
                    " {:>11}",
                    format!("{:.0}% ({:.1}:1)", summary.focus_share_pct, summary.work_rest_ratio)
                ),
            ),
            (
                "Active avg   ",
                active_vs_scale,
                format!(
                    " {:>11}",
                    SessionStats::format_duration_secs(summary.avg_active_day_secs)
                ),
            ),
            (
                "Today pace   ",
                today_vs_scale,
                format!(
                    " {:>11}",
                    format!(
                        "{} ({}d strk)",
                        SessionStats::format_duration_secs(summary.today_focus_secs),
                        summary.current_streak
                    )
                ),
            ),
        ];
        let _ = streak_ratio;

        for (lbl, r, right_val) in rhythm_rows {
            if lines.len() >= area.height as usize {
                break;
            }
            let col = theme.heatmap_ramp(r);
            lines.push(dynamic_bar_row(
                lbl.to_string(),
                style::label(theme),
                r,
                col,
                empty_col,
                right_val,
                style::body(theme),
                None,
                area.width,
            ));
        }
    }

    // If the terminal is still taller, show additional historical records with dot leaders
    if (area.height as usize).saturating_sub(lines.len()) >= 3 {
        lines.push(Line::from(""));
        let extras = [
            (
                "All-time focus",
                SessionStats::format_duration_secs(summary.all_time_focus_secs),
            ),
            (
                "All-time sessions",
                format!("{} sessions", summary.all_time_sessions),
            ),
            (
                "All-time active days",
                format!("{} days", summary.active_days_all_time),
            ),
            (
                "365d daily average",
                SessionStats::format_duration_secs(summary.avg_year_daily_secs),
            ),
            (
                "Active day average",
                SessionStats::format_duration_secs(summary.avg_active_day_secs),
            ),
            (
                "Scale reference max",
                SessionStats::format_duration_secs(summary.max_daily_secs.max(3600)),
            ),
            (
                "Stats file",
                "~/.local/share/devchron/stats.json".to_string(),
            ),
        ];
        for (k, v) in extras {
            if lines.len() >= area.height as usize {
                break;
            }
            lines.push(dot_leader_row(k, v, style::body(theme), area.width, theme));
        }
    }

    f.render_widget(Paragraph::new(lines), area);
}

/// Column 3: Recent sessions table with dynamic full-width column spacing +
/// Mode presets & cycle progress + Milestones & records when vertical space allows.
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

    let mut lines: Vec<Line<'static>> = Vec::new();
    let log_note = format!("{} logged", summary.recent_log.len());
    lines.push(style::section_header(
        "Recent sessions",
        Some(&log_note),
        area.width,
        theme,
    ));

    let show_mode = area.width >= 42;
    let w = area.width as usize;

    // Distribute extra horizontal width across column gaps so every row in
    // Recent sessions spans the full width of the column flush to the right edge.
    let (g1, g2, g3, g4) = if show_mode {
        let base_cols = 11 + 5 + 8 + 7 + 7; // 38 chars
        let total_gap = w.saturating_sub(base_cols).max(4);
        let a = total_gap / 4;
        let b = (total_gap - a) / 3;
        let c = (total_gap - a - b) / 2;
        let d = total_gap - a - b - c;
        (a, b, c, d)
    } else {
        let base_cols = 11 + 5 + 7 + 7; // 30 chars
        let total_gap = w.saturating_sub(base_cols).max(3);
        let a = total_gap / 3;
        let b = (total_gap - a) / 2;
        let c = total_gap - a - b;
        (a, b, 0, c)
    };

    let hdr = if show_mode {
        format!(
            "{:<11}{}{:<5}{}{:<8}{}{:>7}{}{:>7}",
            "Time",
            " ".repeat(g1),
            "Phase",
            " ".repeat(g2),
            "Mode",
            " ".repeat(g3),
            "Dur",
            " ".repeat(g4),
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
            " ".repeat(g4),
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
    if show_mode {
        now_spans.push(Span::styled(
            format!("{:<8}", ui.timer.mode.name),
            style::label(theme),
        ));
        now_spans.push(Span::raw(" ".repeat(g3)));
    }
    now_spans.push(Span::styled(
        format!("{:>7}", SessionStats::format_mm_ss(live_elapsed)),
        style::body(theme),
    ));
    now_spans.push(Span::raw(" ".repeat(g4)));
    now_spans.push(Span::styled(
        format!("{live_state:>7}"),
        style::label(theme),
    ));
    lines.push(Line::from(now_spans));

    for entry in &summary.recent_log {
        if lines.len() >= area.height as usize {
            break;
        }
        let (phase_display, phase_col) = match entry.phase.as_str() {
            "FOCUS" => ("Focus", theme.focus_color),
            "SHORT BRK" => ("Short", theme.short_break_color),
            "LONG BRK" => ("Long ", theme.long_break_color),
            other => (other, theme.text),
        };
        let status_display = match entry.status.as_str() {
            "COMPLETE" => "done",
            "RESTED" => "rested",
            "SKIPPED" => "skipped",
            "ABORTED" => "reset",
            other => other,
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
        if show_mode {
            row_spans.push(Span::styled(
                format!("{:<8}", entry.mode),
                style::label(theme),
            ));
            row_spans.push(Span::raw(" ".repeat(g3)));
        }
        row_spans.push(Span::styled(
            format!("{:>7}", SessionStats::format_mm_ss(entry.duration_secs)),
            style::body(theme),
        ));
        row_spans.push(Span::raw(" ".repeat(g4)));
        row_spans.push(Span::styled(
            format!("{status_display:>7}"),
            style::label(theme),
        ));
        lines.push(Line::from(row_spans));
    }

    // When Recent sessions has few entries (or the terminal is tall), dynamically
    // populate the remaining space in Column 3 with Mode presets & cycle status
    // and Milestones & records so Column 3 is never an empty void.
    let empty_col = empty_cell_color(theme);
    if (area.height as usize).saturating_sub(lines.len()) >= 6 {
        lines.push(Line::from(""));
        let cyc_note = format!(
            "cycle {:02}/{:02}",
            ui.timer.cycle_count + 1,
            ui.timer.cycles_before_long_break
        );
        lines.push(style::section_header(
            "Active mode & presets",
            Some(&cyc_note),
            area.width,
            theme,
        ));

        for (i, mode) in ui.modes.iter().enumerate() {
            if lines.len() >= area.height as usize {
                break;
            }
            let is_active = i == ui.mode_index;
            let lbl = format!("{} {:<8}", i + 1, mode.name);
            let lbl_style = if is_active {
                Style::default()
                    .fg(theme.focus_color)
                    .add_modifier(Modifier::BOLD)
            } else {
                style::label(theme)
            };
            let right_spec = format!(
                " {}m / {:>2}m / {:>2}m{}",
                mode.focus_mins,
                mode.short_break_mins,
                mode.long_break_mins,
                if is_active { " · active" } else { "         " }
            );
            lines.push(dot_leader_row(
                &lbl,
                right_spec,
                lbl_style,
                area.width,
                theme,
            ));
        }

        if lines.len() < area.height as usize {
            let cyc_ratio = ((ui.timer.cycle_count as f64
                + ui.timer.current_timer.progress())
                / ui.timer.cycles_before_long_break.max(1) as f64)
                .clamp(0.0, 1.0);
            lines.push(dynamic_bar_row(
                "Cycle     ".to_string(),
                style::label(theme),
                cyc_ratio,
                theme.focus_color,
                empty_col,
                format!(
                    " {}/{} to long break",
                    ui.timer.cycle_count + 1,
                    ui.timer.cycles_before_long_break
                ),
                style::body(theme),
                None,
                area.width,
            ));
        }
    }

    if (area.height as usize).saturating_sub(lines.len()) >= 4 {
        lines.push(Line::from(""));
        lines.push(style::section_header(
            "Milestones & records",
            Some("all-time"),
            area.width,
            theme,
        ));

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
            ("Peak focus day", peak_day_val),
            ("Peak focus hour", peak_hour_val),
            ("Best weekday", best_dow_val),
            (
                "Longest streak",
                format!("{} days", summary.longest_streak),
            ),
            (
                "Current streak",
                format!("{} days", summary.current_streak),
            ),
            (
                "Completed / reset",
                format!(
                    "{} done · {} reset",
                    summary.completed_focus_count, summary.interrupted_focus_count
                ),
            ),
            (
                "Total break time",
                format!(
                    "{} ({} cycles)",
                    SessionStats::format_duration_secs(summary.year_break_secs),
                    summary.cycles_completed
                ),
            ),
            (
                "Active days (365d)",
                format!(
                    "{} / 365 ({:.1}%)",
                    summary.active_days_year,
                    (summary.active_days_year as f64 / 365.0) * 100.0
                ),
            ),
            (
                "Theme palette",
                format!("{} (press t)", theme.display_name()),
            ),
        ];

        for (k, v) in records {
            if lines.len() >= area.height as usize {
                break;
            }
            lines.push(dot_leader_row(k, v, style::body(theme), area.width, theme));
        }
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn render_analytics_footer(f: &mut Frame, area: Rect, theme: &Theme) {
    const ITEMS: [(&str, &str); 4] = [
        ("g / esc", "close"),
        ("t", "theme"),
        ("h", "help"),
        ("q", "quit"),
    ];
    const DROP: [&str; 2] = ["h", "t"];
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
    if !timer.is_running() {
        spans.push(Span::styled("  ·  paused", style::label(theme)));
    }
    let phase_line = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    f.render_widget(phase_line, chunks[1]);
}

// ─── Mode-switch confirmation ────────────────────────────────────────────────

fn render_confirm(f: &mut Frame, area: Rect, ui: &UiState, target: &PomodoroMode) {
    let theme = ui.theme;
    let accent = get_phase_color(ui.timer.current_timer.phase, theme);

    let width = area.width.saturating_sub(4).min(52);
    let height = 7u16.min(area.height);
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    let consequence = "This phase restarts at the new length, paused.";
    let key = |k: &'static str| Span::styled(k, style::keycap(accent));

    let text = vec![
        Line::from(vec![
            Span::styled("Switch to ", style::body(theme)),
            Span::styled(
                format!("{} ({} min)", target.name, target.summary()),
                style::accent_bold(accent),
            ),
            Span::styled("?", style::body(theme)),
        ]),
        Line::from(Span::styled(consequence, style::label(theme))),
        Line::from(""),
        Line::from(vec![
            key("[Y / Enter]"),
            Span::styled(" Switch    ", style::label(theme)),
            key("[N / Esc]"),
            Span::styled(" Keep current", style::label(theme)),
        ]),
    ];

    f.render_widget(Clear, rect);
    let dialog = Paragraph::new(text)
        .block(style::dialog_block("Switch mode", accent))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(dialog, rect);
}

// ─── Help screen ─────────────────────────────────────────────────────────────

fn render_help(f: &mut Frame, area: Rect, ui: &UiState) {
    let theme = ui.theme;
    f.render_widget(Clear, area);

    let outer = style::screen_block(theme);
    let roomy = area.height >= 26;
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
        style::section_header("Timer", None, inner.width.min(54), theme),
        row("Space / P".into(), "Start or pause timer".into()),
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
        style::section_header("View & app", None, inner.width.min(54), theme),
        row("G / D".into(), "Open or close Analytics".into()),
        row("T".into(), "Cycle colour theme".into()),
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

    /// Timer in the given mode, advanced to `phase`, `progress` of the way
    /// through it, and either running or paused.
    pub(crate) fn make_timer(
        mode: usize,
        phase: TimerPhase,
        progress: f64,
        running: bool,
    ) -> PomodoroTimer {
        let mut t = PomodoroTimer::with_stats(MODES[mode], 4, SessionStats::in_memory());
        while t.current_phase() != phase {
            t.skip();
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
            alarm_ringing: timer.alarm_ringing,
            alarm_flash: false,
            system_identity: &identity,
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

    /// Convert a rendered `Buffer` into truecolor ANSI escape sequences for
    /// terminal inspection (`cargo test -- --ignored --nocapture visual_preview_gallery`).
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

    /// Convert a rendered `Buffer` into an HTML `<pre>` block with exact RGB styling.
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
                    if w == 100 && h == 30 && theme.name == super::super::theme::ThemeName::Reef {
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
    fn timer_page_uses_single_rounded_frame_and_clean_english_labels() {
        let theme = Theme::default_theme();
        let t = make_timer(0, TimerPhase::Focus, 0.25, false);
        let screen = text(&draw(100, 32, &t, &theme, 0, None, false, false));
        assert!(!screen.contains("[STATISTICS]"), "old statistics panel must be removed:\n{screen}");
        assert!(screen.contains("Analytics"), "analytics hint must be shown:\n{screen}");
        assert!(screen.contains("╭") && screen.contains("╯"), "outer frame must be rounded:\n{screen}");
        assert!(!screen.contains("╔"), "double borders must be gone:\n{screen}");
        assert!(!screen.contains("焦"), "mixed CJK labels must be gone:\n{screen}");
        assert!(!screen.contains("［"), "fullwidth key brackets must be gone:\n{screen}");
    }

    #[test]
    fn analytics_page_shows_single_identity_headline_strip_heatmap_and_secondary_sections() {
        let theme = Theme::default_theme();
        let mut t = make_timer(0, TimerPhase::Focus, 0.0, false);
        let today = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        t.stats.complete_focus_session_on(today, Duration::from_secs(3600));

        let screen = text(&draw_with_analytics(
            160, 42, &t, &theme, 0, None, false, false, true,
        ));
        assert!(screen.contains("Analytics"), "{screen}");
        assert!(screen.contains("devuser@archbox"), "user@host missing:\n{screen}");
        assert!(!screen.contains("Computer:"), "redundant identity labels must be gone:\n{screen}");
        assert!(screen.contains("Close Analytics"), "close hint missing:\n{screen}");
        assert!(screen.contains("Current streak"), "{screen}");
        assert!(screen.contains("Average / day"), "{screen}");
        assert!(screen.contains("Completion"), "{screen}");
        assert!(screen.contains("Work / break"), "{screen}");
        assert!(screen.contains("Yearly Focus Grid"), "{screen}");
        assert!(screen.contains("By hour"), "{screen}");
        assert!(screen.contains("By weekday"), "{screen}");
        assert!(screen.contains("Modes & pacing"), "{screen}");
        assert!(screen.contains("Recent sessions"), "{screen}");
        assert!(
            screen.contains("▆") || screen.contains("■"),
            "heatmap blocks missing:\n{screen}"
        );
        assert!(screen.contains("Less") && screen.contains("More"), "{screen}");
        assert!(!screen.contains("╔"), "nested double borders must be gone:\n{screen}");

        // Wide & tall screen (e.g. 226x52) scales grid to 2-col cells ("▆▆") and fills bottom deck
        let wide_screen = text(&draw_with_analytics(
            226, 52, &t, &theme, 0, None, false, false, true,
        ));
        assert!(wide_screen.contains("▆▆"), "wide screen must use 2-col grid blocks:\n{wide_screen}");
        assert!(wide_screen.contains("Focus rhythm"), "tall screen must fill Col 2:\n{wide_screen}");
        assert!(wide_screen.contains("Milestones & records"), "tall screen must fill Col 3:\n{wide_screen}");
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
    fn confirm_dialog_names_the_target_mode_and_both_choices() {
        let theme = Theme::default_theme();
        let t = make_timer(0, TimerPhase::Focus, 0.3, true);
        let screen = text(&draw(80, 24, &t, &theme, 0, Some(2), false, false));
        assert!(screen.contains("Switch to Extended (75/15/45 min)?"));
        assert!(screen.contains("[Y / Enter]"));
        assert!(screen.contains("[N / Esc]"));
        let screen = text(&draw(80, 24, &t, &theme, 0, None, false, false));
        assert!(!screen.contains("Switch to"));
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

    /// Dots of the ring painted in the "active" (non-track) colour, as a share
    /// of all ring dots, measured on the finished frame.
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

        // Minimum size Analytics also fits cleanly without clipping "today" or "close"
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

    // ── Alarm / minimal mode ────────────────────────────────────────────────

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
                alarm_ringing: true,
                alarm_flash: flash,
                system_identity: &identity,
            };
            term.draw(|f| render(f, &ui)).unwrap();
        }
        let (a, b) = (term_a.backend().buffer(), term_b.backend().buffer());
        assert_eq!(text(a), text(b), "only colours differ");
        assert_ne!(a, b);
    }
}
