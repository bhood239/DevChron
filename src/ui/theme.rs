use crate::timer::TimerPhase;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Reef,
    Daybreak,
    Meadow,
    Tropic,
    Harbor,
    Sorbet,
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: ThemeName,
    pub focus_color: Color,
    pub short_break_color: Color,
    pub long_break_color: Color,
    pub paused_color: Color,
    /// Far end of the progress-ring gradient for each phase. The ring runs
    /// from the phase colour (12 o'clock) to this colour (full circle).
    pub focus_end: Color,
    pub short_break_end: Color,
    pub long_break_end: Color,
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

// Every theme is built around its own three hues (focus / short break / long
// break), picked so that no two themes share a palette, and tuned for dark
// terminal backgrounds: phase colours reach at least 6.2:1 contrast on a
// near-black background, dim labels sit comfortably around 4.8:1, and borders
// are a quiet tint of the theme's neutral hue.
impl Theme {
    /// Reef: coral / aqua / periwinkle – a bright lagoon.
    pub fn reef() -> Self {
        Self {
            name: ThemeName::Reef,
            focus_color: hex(0xFF806D),
            focus_end: hex(0xF4AE53),
            short_break_color: hex(0x17E5C7),
            short_break_end: hex(0xACEDA6),
            long_break_color: hex(0x899AF8),
            long_break_end: hex(0xED9AE3),
            paused_color: hex(0x8EA9BD),
            text: hex(0xE9F2FA),
            border: hex(0x3B5363),
            dim: hex(0x7A909E),
        }
    }

    /// Daybreak: gold / sky / rose – a sunny morning.
    pub fn daybreak() -> Self {
        Self {
            name: ThemeName::Daybreak,
            focus_color: hex(0xFFC53D),
            focus_end: hex(0xFF9650),
            short_break_color: hex(0x79D8FF),
            short_break_end: hex(0xA99BFF),
            long_break_color: hex(0xEE7AB2),
            long_break_end: hex(0xFF9B91),
            paused_color: hex(0x9DA5BE),
            text: hex(0xEEF0FA),
            border: hex(0x464E63),
            dim: hex(0x868C9F),
        }
    }

    /// Meadow: leaf green / wisteria / apricot – fresh air.
    pub fn meadow() -> Self {
        Self {
            name: ThemeName::Meadow,
            focus_color: hex(0x3ABC65),
            focus_end: hex(0x02D2C3),
            short_break_color: hex(0xB9A6FF),
            short_break_end: hex(0x9FC2FF),
            long_break_color: hex(0xEA865D),
            long_break_end: hex(0xD4B25C),
            paused_color: hex(0x94AC99),
            text: hex(0xEAF3EC),
            border: hex(0x3F5443),
            dim: hex(0x7E9182),
        }
    }

    /// Tropic: teal / flamingo / lime – a fruit-stand.
    pub fn tropic() -> Self {
        Self {
            name: ThemeName::Tropic,
            focus_color: hex(0x01B8C2),
            focus_end: hex(0x45C8FE),
            short_break_color: hex(0xFF9EBB),
            short_break_end: hex(0xFFC4E6),
            long_break_color: hex(0xB4DC4B),
            long_break_end: hex(0x6FD08F),
            paused_color: hex(0x85ACB4),
            text: hex(0xE5F3F6),
            border: hex(0x33555B),
            dim: hex(0x749196),
        }
    }

    /// Harbor: cobalt / peach / emerald – sea at dusk.
    pub fn harbor() -> Self {
        Self {
            name: ThemeName::Harbor,
            focus_color: hex(0x5A8DFF),
            focus_end: hex(0xBFA2FF),
            short_break_color: hex(0xFFBF90),
            short_break_end: hex(0xFFD2D0),
            long_break_color: hex(0x06B687),
            long_break_end: hex(0x16C9D4),
            paused_color: hex(0x91A9BE),
            text: hex(0xF5F0E9),
            border: hex(0x3D5264),
            dim: hex(0x7C8E9E),
        }
    }

    /// Sorbet: orchid pink / pistachio / cyan – sweet and cool.
    pub fn sorbet() -> Self {
        Self {
            name: ThemeName::Sorbet,
            focus_color: hex(0xF37AD8),
            focus_end: hex(0xFFA1AE),
            short_break_color: hex(0xAFD97A),
            short_break_end: hex(0xFDD989),
            long_break_color: hex(0x10AFD1),
            long_break_end: hex(0x7CBAFF),
            paused_color: hex(0xB2A0B6),
            text: hex(0xF5EEF7),
            border: hex(0x58495B),
            dim: hex(0x948697),
        }
    }

    /// The theme used when none (or an unknown one) is configured.
    pub fn default_theme() -> Self {
        Self::reef()
    }

    /// Every theme, in the order `T` cycles through them.
    pub fn all() -> [Self; 6] {
        [
            Self::reef(),
            Self::daybreak(),
            Self::meadow(),
            Self::tropic(),
            Self::harbor(),
            Self::sorbet(),
        ]
    }

    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().replace(['-', '_', ' '], "").as_str() {
            "reef" | "lagoon" => Self::reef(),
            "daybreak" | "sunrise" => Self::daybreak(),
            "meadow" => Self::meadow(),
            "tropic" | "tropical" => Self::tropic(),
            "harbor" | "harbour" => Self::harbor(),
            "sorbet" => Self::sorbet(),
            _ => Self::default_theme(),
        }
    }

    /// Cycle to the next theme: Reef → Daybreak → Meadow → Tropic → Harbor →
    /// Sorbet → Reef
    pub fn cycle_next(&self) -> Self {
        let all = Self::all();
        let i = all.iter().position(|t| t.name == self.name).unwrap_or(0);
        all[(i + 1) % all.len()]
    }

    /// Solid colour for a phase (digits, labels, accents).
    pub fn phase_color(&self, phase: TimerPhase) -> Color {
        match phase {
            TimerPhase::Focus => self.focus_color,
            TimerPhase::ShortBreak => self.short_break_color,
            TimerPhase::LongBreak => self.long_break_color,
        }
    }

    /// `(start, end)` colours of the progress-ring gradient for a phase.
    pub fn ring_gradient(&self, phase: TimerPhase) -> (Color, Color) {
        match phase {
            TimerPhase::Focus => (self.focus_color, self.focus_end),
            TimerPhase::ShortBreak => (self.short_break_color, self.short_break_end),
            TimerPhase::LongBreak => (self.long_break_color, self.long_break_end),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self.name {
            ThemeName::Reef => "Reef",
            ThemeName::Daybreak => "Daybreak",
            ThemeName::Meadow => "Meadow",
            ThemeName::Tropic => "Tropic",
            ThemeName::Harbor => "Harbor",
            ThemeName::Sorbet => "Sorbet",
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

    /// Perceptual heatmap ramp derived from the theme's `border` and `focus_color`.
    ///
    /// * `ratio <= 0.0` returns `self.empty_cell()`.
    /// * `ratio > 0.0` starts at a clearly visible floor (`t = 0.36`) and scales
    ///   smoothly in linear light up to `self.focus_color` (`t = 1.0`).
    pub fn heatmap_ramp(&self, ratio: f64) -> Color {
        if ratio <= 0.0 {
            return self.empty_cell();
        }
        let r = ratio.clamp(0.0, 1.0);
        let low_floor = blend_linear(self.border, self.focus_color, 0.36);
        blend_linear(low_floor, self.focus_color, r)
    }
}

/// Linear blend between two colours (`t` = 0 → `a`, `t` = 1 → `b`).
/// Non-RGB colours cannot be blended and fall back to `a`.
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
    fn heatmap_ramp_distinguishes_lowest_nonzero_from_empty_in_every_theme() {
        for theme in Theme::all() {
            let empty = theme.empty_cell();
            let low = theme.heatmap_ramp(0.05);
            let mid = theme.heatmap_ramp(0.50);
            let high = theme.heatmap_ramp(1.0);
            assert_ne!(empty, low, "{:?}: lowest active cell must stand out from empty", theme.name);
            assert_ne!(low, mid, "{:?}", theme.name);
            assert_ne!(mid, high, "{:?}", theme.name);
            assert_eq!(high, theme.focus_color, "{:?}", theme.name);
        }
    }

    #[test]
    fn every_theme_defines_a_gradient_for_every_phase() {
        for theme in Theme::all() {
            for phase in [TimerPhase::Focus, TimerPhase::ShortBreak, TimerPhase::LongBreak] {
                let (start, end) = theme.ring_gradient(phase);
                assert_eq!(start, theme.phase_color(phase));
                assert_ne!(start, end, "gradient should actually shift colour");
            }
        }
    }

    #[test]
    fn cycling_visits_every_theme_once_and_wraps() {
        let mut theme = Theme::default_theme();
        let mut seen = vec![theme.name];
        for _ in 0..Theme::all().len() - 1 {
            theme = theme.cycle_next();
            assert!(!seen.contains(&theme.name), "{:?} repeated", theme.name);
            seen.push(theme.name);
        }
        assert_eq!(theme.cycle_next().name, ThemeName::Reef);
    }

    #[test]
    fn theme_names_from_config_resolve() {
        assert_eq!(Theme::from_name("Reef").name, ThemeName::Reef);
        assert_eq!(Theme::from_name("daybreak").name, ThemeName::Daybreak);
        assert_eq!(Theme::from_name("MEADOW").name, ThemeName::Meadow);
        assert_eq!(Theme::from_name("tropic").name, ThemeName::Tropic);
        assert_eq!(Theme::from_name("harbor").name, ThemeName::Harbor);
        assert_eq!(Theme::from_name("sorbet").name, ThemeName::Sorbet);
        for old in ["nord", "catppuccin", "classic", "gruvbox", "dracula", "tokyo-night", "nonsense"] {
            assert_eq!(Theme::from_name(old).name, ThemeName::Reef, "{old}");
        }
    }

    #[test]
    fn every_name_the_config_accepts_round_trips_through_display_name() {
        for theme in Theme::all() {
            assert_eq!(Theme::from_name(theme.display_name()).name, theme.name);
        }
    }

    #[test]
    fn no_two_themes_share_a_phase_colour() {
        let all = Theme::all();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                for phase in [TimerPhase::Focus, TimerPhase::ShortBreak, TimerPhase::LongBreak] {
                    assert_ne!(a.phase_color(phase), b.phase_color(phase), "{:?} vs {:?}", a.name, b.name);
                }
                assert_ne!(a.text, b.text, "{:?} vs {:?}", a.name, b.name);
            }
        }
    }

    #[test]
    fn phases_are_distinct_within_a_theme_and_the_track_is_not_a_phase_colour() {
        for theme in Theme::all() {
            let (f, s, l) = (theme.focus_color, theme.short_break_color, theme.long_break_color);
            assert!(f != s && f != l && s != l, "{:?}", theme.name);
            for phase in [TimerPhase::Focus, TimerPhase::ShortBreak, TimerPhase::LongBreak] {
                let (a, b) = theme.ring_gradient(phase);
                assert!(theme.border != a && theme.border != b, "{:?}", theme.name);
            }
            assert_ne!(theme.border, theme.dim);
            assert_ne!(theme.dim, theme.text);
        }
    }
}
