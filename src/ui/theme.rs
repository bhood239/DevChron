use crate::timer::{FocusCategory, TimerPhase};
use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    TokyoNight,
}

/// TokyoNight-inspired terminal palette: cool navy structure, vivid but disciplined accents.
///
/// The palette uses a deep, cool-violet structural base and vivid, high-separation
/// accents. Every hue has a job: category colours identify the kind of work,
/// semantic colours communicate state, and neutral colours preserve hierarchy.
/// Saturation is concentrated in meaningful data rather than spread uniformly
/// across every label and divider.
///
/// Category identities:
/// - Coding: warm orange
/// - Learning: bright cyan
/// - Reading: violet
/// - All-category heatmap: contribution-green ramp
/// - Today marker: solar yellow, independent of the selected category
///
/// Semantic accents:
/// - Gold = streaks, milestones, and records
/// - Emerald = completed/successful work
/// - Sky = averages and informational metrics
/// - Amber = paused/skipped/warning states
/// - Rose = reset/alarm/danger states
/// - Lavender = section headings
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: ThemeName,
    pub focus_color: Color,
    pub short_break_color: Color,
    pub long_break_color: Color,
    pub paused_color: Color,
    /// Far end of the progress-ring and block-digit gradient for each phase/category.
    pub focus_end: Color,
    pub short_break_end: Color,
    pub long_break_end: Color,
    /// Gamified semantic accents for streaks, completion, averages, and badges.
    pub gold: Color,
    pub emerald: Color,
    pub cyan: Color,
    pub amber: Color,
    pub rose: Color,
    pub section_accent: Color,
    pub text: Color,
    /// Frames, dividers, and the unfilled track of the progress ring.
    pub border: Color,
    /// Subtle dimmed text for labels, hints, and secondary info.
    pub dim: Color,
}

/// `0xRRGGBB` → colour.
fn hex(rgb: u32) -> Color {
    Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

impl Theme {
    /// The single master TokyoNight-inspired palette for DevChron.
    pub fn reef() -> Self {
        Self {
            name: ThemeName::TokyoNight,
            focus_color: hex(0xFF9E64),
            focus_end: hex(0xFFC777),
            short_break_color: hex(0x2AC3DE),
            short_break_end: hex(0x73DACA),
            long_break_color: hex(0xBB9AF7),
            long_break_end: hex(0x9D7CD8),
            paused_color: hex(0x565F89),
            gold: hex(0xE0AF68),
            emerald: hex(0x9ECE6A),
            cyan: hex(0x7AA2F7),
            amber: hex(0xE0AF68),
            rose: hex(0xF7768E),
            section_accent: hex(0x7DCFFF),
            text: hex(0xC0CAF5),
            border: hex(0x292E42),
            dim: hex(0x565F89),
        }
    }

    /// The default (and only) application theme.
    pub fn default_theme() -> Self {
        Self::reef()
    }

    /// Single-element slice of themes for compatibility.
    #[allow(dead_code)]
    pub fn all() -> [Self; 1] {
        [Self::reef()]
    }

    /// Resolves any configured theme name to the unified DevChron palette.
    pub fn from_name(_name: &str) -> Self {
        Self::default_theme()
    }

    /// Solid colour for a phase (digits, labels, accents).
    pub fn phase_color(&self, phase: TimerPhase) -> Color {
        match phase {
            TimerPhase::Focus => self.focus_color,
            TimerPhase::ShortBreak => self.short_break_color,
            TimerPhase::LongBreak => self.long_break_color,
        }
    }

    /// Distinct accent colour for each FocusCategory (`Coding`, `Learning`, `Reading`).
    pub fn category_color(&self, category: FocusCategory) -> Color {
        match category {
            FocusCategory::Coding => self.focus_color,
            FocusCategory::Learning => self.short_break_color,
            FocusCategory::Reading => self.long_break_color,
        }
    }

    /// `(start, end)` gradient colours for a FocusCategory.
    pub fn category_gradient(&self, category: FocusCategory) -> (Color, Color) {
        match category {
            FocusCategory::Coding => (self.focus_color, self.focus_end),
            FocusCategory::Learning => (self.short_break_color, self.short_break_end),
            FocusCategory::Reading => (self.long_break_color, self.long_break_end),
        }
    }

    /// Active `(start, end)` gradient for the Timer ring and block digits:
    /// reflects the selected `FocusCategory` during Focus phases and the break
    /// palette during Short/Long breaks.
    pub fn active_timer_gradient(
        &self,
        phase: TimerPhase,
        category: FocusCategory,
    ) -> (Color, Color) {
        match phase {
            TimerPhase::Focus => self.category_gradient(category),
            TimerPhase::ShortBreak => (self.short_break_color, self.short_break_end),
            TimerPhase::LongBreak => (self.long_break_color, self.long_break_end),
        }
    }

    /// Distinct accent colour for each Pomodoro mode (`Classic`, `Session`, `Extended`).
    pub fn mode_color(&self, mode_index: usize) -> Color {
        match mode_index {
            0 => self.focus_color,
            1 => self.cyan,
            2 => self.long_break_color,
            _ => self.gold,
        }
    }

    /// Distinct accent colour for a mode by its name.
    pub fn mode_name_color(&self, name: &str) -> Color {
        match name {
            "Classic" => self.focus_color,
            "Session" => self.cyan,
            "Extended" => self.long_break_color,
            _ => self.gold,
        }
    }

    /// Perceptual colour ramp (`0.0..=1.0`) for a specific `FocusCategory`.
    pub fn category_ramp(&self, category: FocusCategory, ratio: f64) -> Color {
        self.color_ramp(self.category_color(category), ratio)
    }

    /// Perceptual colour ramp (`0.0..=1.0`) toward any target accent colour.
    pub fn color_ramp(&self, target: Color, ratio: f64) -> Color {
        if ratio <= 0.0 {
            return self.empty_cell();
        }
        let r = ratio.clamp(0.0, 1.0);
        let low_floor = blend_linear(self.border, target, 0.38);
        blend_linear(low_floor, target, r)
    }

    /// `(start, end)` colours of the progress-ring gradient for a phase.
    #[allow(dead_code)]
    pub fn ring_gradient(&self, phase: TimerPhase) -> (Color, Color) {
        match phase {
            TimerPhase::Focus => (self.focus_color, self.focus_end),
            TimerPhase::ShortBreak => (self.short_break_color, self.short_break_end),
            TimerPhase::LongBreak => (self.long_break_color, self.long_break_end),
        }
    }

    #[allow(dead_code)]
    pub fn display_name(&self) -> &'static str {
        match self.name {
            ThemeName::TokyoNight => "TokyoNight",
        }
    }

    /// Derived empty-cell / track surface colour from the theme's own border
    /// hue (no hard-coded background RGB).
    pub fn empty_cell(&self) -> Color {
        match self.border {
            Color::Rgb(r, g, b) => Color::Rgb(
                ((r as f64) * 0.62).round() as u8,
                ((g as f64) * 0.62).round() as u8,
                ((b as f64) * 0.62).round() as u8,
            ),
            other => other,
        }
    }

    /// Perceptual all-category heatmap ramp derived from the structural base and Coding accent.
    ///
    /// * `ratio <= 0.0` returns `self.empty_cell()`.
    /// * `ratio > 0.0` starts at a clearly visible floor (`t = 0.38`) and scales
    ///   smoothly in linear light up to `self.focus_color` (`t = 1.0`).
    pub fn heatmap_ramp(&self, ratio: f64) -> Color {
        // GitHub/LeetCode-style green intensity ramp when All categories are selected.
        self.color_ramp(hex(0x39D353), ratio)
    }

    /// Dedicated Today marker, intentionally independent from category hues.
    pub fn today_color(&self) -> Color {
        hex(0xF0D75E)
    }
}

/// Linear blend between two colours (`t` = 0 → `a`, `t` = 1 → `b`).
/// Non-RGB colours cannot be blended and fall back to `a`.
#[allow(dead_code)]
pub fn blend(a: Color, b: Color, t: f64) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let t = t.clamp(0.0, 1.0);
            let mix = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
            Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
        }
        _ => a,
    }
}

/// Perceptual blend in linear light (gamma 2.2) so midtones stay rich and
/// never dip through muddy grey.
pub fn blend_linear(a: Color, b: Color, t: f64) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let t = t.clamp(0.0, 1.0);
            let to_lin = |u: u8| (u as f64 / 255.0).powf(2.2);
            let to_srgb = |l: f64| (l.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8;
            let mix = |x: u8, y: u8| {
                let lx = to_lin(x);
                let ly = to_lin(y);
                to_srgb(lx + (ly - lx) * t)
            };
            Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
        }
        _ => a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_endpoints_and_midpoint() {
        let a = Color::Rgb(0, 100, 200);
        let b = Color::Rgb(100, 200, 0);
        assert_eq!(blend(a, b, 0.0), a);
        assert_eq!(blend(a, b, 1.0), b);
        assert_eq!(blend(a, b, 0.5), Color::Rgb(50, 150, 100));
    }

    #[test]
    fn blend_clamps_and_tolerates_non_rgb() {
        let a = Color::Rgb(10, 10, 10);
        let b = Color::Rgb(20, 20, 20);
        assert_eq!(blend(a, b, 7.0), b);
        assert_eq!(blend(Color::Red, b, 0.5), Color::Red);
    }

    #[test]
    fn linear_blend_preserves_endpoints_and_brightens_midtones() {
        let a = Color::Rgb(40, 60, 80);
        let b = Color::Rgb(240, 120, 100);
        assert_eq!(blend_linear(a, b, 0.0), a);
        assert_eq!(blend_linear(a, b, 1.0), b);
        if let (Color::Rgb(r_lin, _, _), Color::Rgb(r_naive, _, _)) =
            (blend_linear(a, b, 0.5), blend(a, b, 0.5))
        {
            assert!(r_lin >= r_naive);
        }
    }

    #[test]
    fn heatmap_ramp_distinguishes_lowest_nonzero_from_empty() {
        let theme = Theme::default_theme();
        let empty = theme.empty_cell();
        let low = theme.heatmap_ramp(0.05);
        let mid = theme.heatmap_ramp(0.50);
        let high = theme.heatmap_ramp(1.0);
        assert_ne!(empty, low, "lowest active cell must stand out from empty");
        assert_ne!(low, mid);
        assert_ne!(mid, high);
        assert_eq!(high, theme.heatmap_ramp(1.0));
    }

    #[test]
    fn defines_a_gradient_for_every_phase_and_category() {
        let theme = Theme::default_theme();
        for phase in [TimerPhase::Focus, TimerPhase::ShortBreak, TimerPhase::LongBreak] {
            let (start, end) = theme.ring_gradient(phase);
            assert_eq!(start, theme.phase_color(phase));
            assert_ne!(start, end, "gradient should actually shift colour");
        }
        for cat in FocusCategory::ALL {
            let (start, end) = theme.category_gradient(cat);
            assert_eq!(start, theme.category_color(cat));
            assert_ne!(start, end);
        }
    }

    #[test]
    fn single_unified_theme_resolves_all_config_names() {
        assert_eq!(Theme::all().len(), 1);
        for name in ["reef", "daybreak", "meadow", "tropic", "harbor", "sorbet", "default", "custom"] {
            assert_eq!(Theme::from_name(name).name, ThemeName::TokyoNight);
        }
    }

    #[test]
    fn phases_categories_and_semantic_accents_are_distinct() {
        let theme = Theme::default_theme();
        let (f, s, l) = (
            theme.focus_color,
            theme.short_break_color,
            theme.long_break_color,
        );
        assert!(f != s && f != l && s != l);
        assert_ne!(theme.gold, theme.emerald);
        assert_ne!(theme.cyan, theme.focus_color);
        assert_ne!(theme.border, theme.dim);
        assert_ne!(theme.dim, theme.text);
    }
}
