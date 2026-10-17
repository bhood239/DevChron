//! Shared design tokens, typography hierarchy, spacing, and chrome helpers.
//!
//! Principles:
//! - At most one `BorderType::Rounded` outer frame per screen (plus modal dialogs).
//! - Never nest bordered boxes inside bordered boxes; separate inner sections
//!   with whitespace or a single quiet rule in `theme.border`.
//! - Vibrant typographic hierarchy: headline numbers shine in semantic accent
//!   hues, section titles use `theme.section_accent`, labels use `theme.dim`,
//!   and right-hand callout notes highlight key takeaways in `theme.gold`.
//! - Foreground colours only (no opaque background fills) so Hyprland
//!   transparency and blur work naturally.

use super::theme::Theme;
use crate::timer::session::SessionStats;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};

/// Horizontal padding inside the outer frame.
pub const PAD_X: u16 = 2;
/// Standard gutter between side-by-side columns.
pub const COL_GAP: u16 = 3;

// ─── Typography hierarchy ───────────────────────────────────────────────────

/// App title or primary headline value (`text` + bold).
pub fn value_bold(theme: &Theme) -> Style {
    Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
}

/// Primary hero accent value (`accent` + bold).
pub fn accent_bold(accent: Color) -> Style {
    Style::default().fg(accent).add_modifier(Modifier::BOLD)
}

/// Regular body text (`text`, never bold).
pub fn body(theme: &Theme) -> Style {
    Style::default().fg(theme.text)
}

/// Quiet label or secondary metadata (`dim`, never bold).
pub fn label(theme: &Theme) -> Style {
    Style::default().fg(theme.dim)
}

/// Section header title style (`section_accent` + bold).
pub fn section_title(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.section_accent)
        .add_modifier(Modifier::BOLD)
}

/// Subtle structural line or divider (`border`, never bold).
pub fn rule_style(theme: &Theme) -> Style {
    Style::default().fg(theme.border)
}

/// Keycap style (`accent` + bold).
pub fn keycap(accent: Color) -> Style {
    Style::default().fg(accent).add_modifier(Modifier::BOLD)
}

// ─── Chrome & Layout Helpers ────────────────────────────────────────────────

/// Single rounded outer frame used for every screen.
pub fn screen_block(theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(rule_style(theme))
}

/// Rounded frame for a floating modal dialog (e.g. mode-switch confirmation).
pub fn dialog_block(title: &'static str, accent: Color) -> Block<'static> {
    Block::default()
        .title(Span::styled(format!(" {title} "), accent_bold(accent)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
}

/// Inset `area` horizontally by `pad_x` and vertically by `pad_y`.
pub fn inset(area: Rect, pad_x: u16, pad_y: u16) -> Rect {
    let x = area.x.saturating_add(pad_x);
    let y = area.y.saturating_add(pad_y);
    let width = area.width.saturating_sub(pad_x * 2);
    let height = area.height.saturating_sub(pad_y * 2);
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Vibrant section header: ice-blue section title, subtle horizontal rule,
/// and warm gold right-hand callout badge.
pub fn section_header(
    title: &str,
    right_note: Option<&str>,
    width: u16,
    theme: &Theme,
) -> Line<'static> {
    section_header_colored(title, theme.section_accent, right_note, theme.gold, width, theme)
}

/// Section header with custom title and right-note accent colours.
pub fn section_header_colored(
    title: &str,
    title_col: Color,
    right_note: Option<&str>,
    note_col: Color,
    width: u16,
    theme: &Theme,
) -> Line<'static> {
    let w = width as usize;
    let title_str = format!("{title} ");
    let title_len = title_str.chars().count();
    let t_style = Style::default().fg(title_col).add_modifier(Modifier::BOLD);

    if let Some(note) = right_note {
        let note_str = format!(" {note}");
        let note_len = note_str.chars().count();
        if title_len + note_len + 2 <= w {
            let rule_len = w - title_len - note_len;
            return Line::from(vec![
                Span::styled(title_str, t_style),
                Span::styled("─".repeat(rule_len), rule_style(theme)),
                Span::styled(note_str, Style::default().fg(note_col)),
            ]);
        }
    }

    let rule_len = w.saturating_sub(title_len);
    Line::from(vec![
        Span::styled(title_str, t_style),
        Span::styled("─".repeat(rule_len), rule_style(theme)),
    ])
}

/// Fixed-width right-aligned duration (7 chars, e.g. `"  1h 05m"` or `"     25m"`)
/// so numeric columns never shift horizontally.
pub fn format_duration_fixed(secs: u64) -> String {
    format!("{:>7}", SessionStats::format_duration_secs(secs))
}

/// Build a responsive, centered keycap bar from `(key, description)` pairs.
/// If `area_width` is too narrow for all items, drops entries in `drop_order`
/// so no hint is ever clipped mid-word.
pub fn keycap_bar<'a>(
    items: &[(&'a str, &'a str)],
    drop_order: &[&str],
    area_width: u16,
    accent: Color,
    theme: &Theme,
) -> Line<'static> {
    let palette = [
        accent,
        theme.gold,
        theme.cyan,
        theme.emerald,
        theme.long_break_color,
        theme.rose,
    ];
    let gap = 3usize;
    let width_of = |list: &[(&str, &str)]| -> usize {
        list.iter()
            .map(|(k, d)| k.chars().count() + 1 + d.chars().count())
            .sum::<usize>()
            + list.len().saturating_sub(1) * gap
    };

    let mut active: Vec<(&str, &str)> = items.to_vec();
    for &dropped in drop_order {
        if width_of(&active) <= area_width as usize {
            break;
        }
        active.retain(|(k, _)| *k != dropped);
    }

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(active.len() * 3);
    for (i, (k, d)) in active.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        let col = palette[i % palette.len()];
        spans.push(Span::styled(k.to_string(), keycap(col)));
        spans.push(Span::styled(format!(" {d}"), label(theme)));
    }
    Line::from(spans)
}
