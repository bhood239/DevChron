use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub focus_color: Color,
    pub short_break_color: Color,
    pub long_break_color: Color,
    pub paused_color: Color,
    pub background: Color,
    pub text: Color,
    pub border: Color,
}

impl Theme {
    pub fn nord() -> Self {
        Self {
            focus_color: Color::Rgb(191, 97, 106),        // Aurora red
            short_break_color: Color::Rgb(163, 190, 140), // Aurora green
            long_break_color: Color::Rgb(129, 161, 193),  // Frost blue
            paused_color: Color::Rgb(216, 222, 233),      // Snow storm
            background: Color::Rgb(46, 52, 64),           // Polar night
            text: Color::Rgb(236, 239, 244),              // Snow storm
            border: Color::Rgb(76, 86, 106),              // Polar night lighter
        }
    }

    pub fn catppuccin() -> Self {
        Self {
            focus_color: Color::Rgb(243, 139, 168),       // Pink
            short_break_color: Color::Rgb(166, 227, 161), // Green
            long_break_color: Color::Rgb(137, 180, 250),  // Blue
            paused_color: Color::Rgb(186, 194, 222),      // Overlay2
            background: Color::Rgb(30, 30, 46),           // Base
            text: Color::Rgb(205, 214, 244),              // Text
            border: Color::Rgb(88, 91, 112),              // Surface1
        }
    }

    pub fn classic() -> Self {
        Self {
            focus_color: Color::Rgb(220, 53, 69),         // Red
            short_break_color: Color::Rgb(40, 167, 69),   // Green
            long_break_color: Color::Rgb(0, 123, 255),    // Blue
            paused_color: Color::Rgb(108, 117, 125),      // Gray
            background: Color::Rgb(33, 37, 41),           // Dark
            text: Color::Rgb(248, 249, 250),              // Light
            border: Color::Rgb(73, 80, 87),               // Border
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "catppuccin" | "mocha" => Self::catppuccin(),
            "classic" | "tomato" => Self::classic(),
            _ => Self::nord(),
        }
    }
}
