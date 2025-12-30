//! Color theme and styling for the terminal UI
//!
//! Aesthetic: Brutalist + Retro-Futuristic
//! - Sharp geometric forms
//! - Neon accents on dark backgrounds
//! - Monospace typography emphasis
//! - Scanline/CRT-inspired elements

use colored::{Color, ColoredString, Colorize};

/// Terminal color palette - Cyberpunk Brutalist
pub struct Theme;

impl Theme {
    // Primary colors - electric neon
    pub const NEON_CYAN: Color = Color::TrueColor {
        r: 0,
        g: 255,
        b: 255,
    };
    pub const NEON_MAGENTA: Color = Color::TrueColor {
        r: 255,
        g: 0,
        b: 255,
    };
    pub const NEON_GREEN: Color = Color::TrueColor {
        r: 57,
        g: 255,
        b: 20,
    };
    pub const NEON_ORANGE: Color = Color::TrueColor {
        r: 255,
        g: 165,
        b: 0,
    };

    // Score gradient colors
    pub const SCORE_HIGH: Color = Color::TrueColor {
        r: 0,
        g: 255,
        b: 136,
    }; // Mint green
    pub const SCORE_MED: Color = Color::TrueColor {
        r: 255,
        g: 215,
        b: 0,
    }; // Gold
    pub const SCORE_LOW: Color = Color::TrueColor {
        r: 255,
        g: 99,
        b: 71,
    }; // Tomato

    // UI elements
    pub const BORDER: Color = Color::TrueColor {
        r: 88,
        g: 88,
        b: 88,
    };
    pub const BORDER_ACCENT: Color = Color::TrueColor {
        r: 0,
        g: 200,
        b: 255,
    };
    pub const DIM: Color = Color::TrueColor {
        r: 100,
        g: 100,
        b: 100,
    };
    pub const SUBTLE: Color = Color::TrueColor {
        r: 140,
        g: 140,
        b: 140,
    };

    // Language colors
    pub const LANG_RUST: Color = Color::TrueColor {
        r: 255,
        g: 106,
        b: 0,
    };
    pub const LANG_TS: Color = Color::TrueColor {
        r: 0,
        g: 122,
        b: 204,
    };
    pub const LANG_JS: Color = Color::TrueColor {
        r: 247,
        g: 223,
        b: 30,
    };
    pub const LANG_PY: Color = Color::TrueColor {
        r: 55,
        g: 118,
        b: 171,
    };
    pub const LANG_GO: Color = Color::TrueColor {
        r: 0,
        g: 173,
        b: 216,
    };
    pub const LANG_MD: Color = Color::TrueColor {
        r: 150,
        g: 150,
        b: 150,
    };

    /// Get color for a language
    pub fn lang_color(lang: &str) -> Color {
        match lang.to_lowercase().as_str() {
            "rust" | "rs" => Self::LANG_RUST,
            "typescript" | "ts" => Self::LANG_TS,
            "javascript" | "js" => Self::LANG_JS,
            "python" | "py" => Self::LANG_PY,
            "go" => Self::LANG_GO,
            "markdown" | "md" => Self::LANG_MD,
            _ => Self::SUBTLE,
        }
    }

    /// Get gradient color for score (0.0 - 1.0)
    pub fn score_color(score: f32) -> Color {
        if score >= 0.7 {
            Self::SCORE_HIGH
        } else if score >= 0.4 {
            Self::SCORE_MED
        } else {
            Self::SCORE_LOW
        }
    }

    /// Create a score bar visualization
    pub fn score_bar(score: f32, width: usize) -> String {
        let filled = ((score * width as f32) as usize).min(width);
        let empty = width - filled;

        let color = Self::score_color(score);
        let bar_char = "█";
        let empty_char = "░";

        format!(
            "{}{}",
            bar_char.repeat(filled).color(color),
            empty_char.repeat(empty).color(Self::DIM)
        )
    }
}

/// Box drawing characters for UI frames
pub struct BoxChars;

impl BoxChars {
    // Heavy box drawing
    pub const H_LINE: &'static str = "━";
    pub const V_LINE: &'static str = "┃";
    pub const TL_CORNER: &'static str = "┏";
    pub const TR_CORNER: &'static str = "┓";
    pub const BL_CORNER: &'static str = "┗";
    pub const BR_CORNER: &'static str = "┛";
    pub const T_DOWN: &'static str = "┳";
    pub const T_UP: &'static str = "┻";
    pub const T_RIGHT: &'static str = "┣";
    pub const T_LEFT: &'static str = "┫";
    pub const CROSS: &'static str = "╋";

    // Light box drawing
    pub const L_H_LINE: &'static str = "─";
    pub const L_V_LINE: &'static str = "│";
    pub const L_TL: &'static str = "┌";
    pub const L_TR: &'static str = "┐";
    pub const L_BL: &'static str = "└";
    pub const L_BR: &'static str = "┘";

    // Double line
    pub const D_H_LINE: &'static str = "═";
    pub const D_V_LINE: &'static str = "║";
    pub const D_TL: &'static str = "╔";
    pub const D_TR: &'static str = "╗";
    pub const D_BL: &'static str = "╚";
    pub const D_BR: &'static str = "╝";

    // Block elements
    pub const FULL_BLOCK: &'static str = "█";
    pub const SHADE_LIGHT: &'static str = "░";
    pub const SHADE_MED: &'static str = "▒";
    pub const SHADE_DARK: &'static str = "▓";

    // Arrows and symbols
    pub const ARROW_RIGHT: &'static str = "▶";
    pub const ARROW_DOWN: &'static str = "▼";
    pub const DIAMOND: &'static str = "◆";
    pub const BULLET: &'static str = "●";
    pub const CIRCLE: &'static str = "○";
    pub const STAR: &'static str = "★";
    pub const CHECK: &'static str = "✓";
    pub const CROSS_MARK: &'static str = "✗";
}

/// Animated spinner frames
pub struct Spinner;

impl Spinner {
    pub const FRAMES: &'static [&'static str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    pub const BLOCKS: &'static [&'static str] = &[
        "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█", "▉", "▊", "▋", "▌", "▍", "▎", "▏",
    ];

    pub const DOTS: &'static [&'static str] = &["   ", ".  ", ".. ", "...", " ..", "  .", "   "];
}
