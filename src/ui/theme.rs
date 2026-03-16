use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Nord,
    Catppuccin,
    Classic,
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: ThemeName,
    pub focus_color: Color,
    pub short_break_color: Color,
    pub long_break_color: Color,
    pub paused_color: Color,
    pub text: Color,
    pub border: Color,
    /// Subtle dimmed text for hints / secondary info
    pub dim: Color,
}

impl Theme {
    pub fn nord() -> Self {
        Self {
            name: ThemeName::Nord,
            focus_color: Color::Rgb(191, 97, 106), // Aurora red
            short_break_color: Color::Rgb(163, 190, 140), // Aurora green
            long_break_color: Color::Rgb(129, 161, 193), // Frost blue
            paused_color: Color::Rgb(216, 222, 233), // Snow storm
            text: Color::Rgb(236, 239, 244),       // Snow storm bright
            border: Color::Rgb(76, 86, 106),       // Polar night lighter
            dim: Color::Rgb(67, 76, 94),           // Polar night mid
        }
    }

    pub fn catppuccin() -> Self {
        Self {
            name: ThemeName::Catppuccin,
            focus_color: Color::Rgb(243, 139, 168), // Pink
            short_break_color: Color::Rgb(166, 227, 161), // Green
            long_break_color: Color::Rgb(137, 180, 250), // Blue
            paused_color: Color::Rgb(186, 194, 222), // Overlay2
            text: Color::Rgb(205, 214, 244),        // Text
            border: Color::Rgb(88, 91, 112),        // Surface2
            dim: Color::Rgb(69, 71, 90),            // Surface0
        }
    }

    pub fn classic() -> Self {
        Self {
            name: ThemeName::Classic,
            focus_color: Color::Rgb(220, 53, 69),       // Red
            short_break_color: Color::Rgb(40, 167, 69), // Green
            long_break_color: Color::Rgb(0, 123, 255),  // Blue
            paused_color: Color::Rgb(108, 117, 125),    // Gray
            text: Color::Rgb(248, 249, 250),            // Light
            border: Color::Rgb(73, 80, 87),             // Border
            dim: Color::Rgb(52, 58, 64),                // Dark
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "catppuccin" | "mocha" => Self::catppuccin(),
            "classic" | "tomato" => Self::classic(),
            _ => Self::nord(),
        }
    }

    /// Cycle to the next theme in the sequence: Nord → Catppuccin → Classic → Nord
    pub fn cycle_next(&self) -> Self {
        match self.name {
            ThemeName::Nord => Self::catppuccin(),
            ThemeName::Catppuccin => Self::classic(),
            ThemeName::Classic => Self::nord(),
        }
    }

    #[allow(dead_code)]
    pub fn display_name(&self) -> &'static str {
        match self.name {
            ThemeName::Nord => "Nord",
            ThemeName::Catppuccin => "Catppuccin",
            ThemeName::Classic => "Classic",
        }
    }
}
