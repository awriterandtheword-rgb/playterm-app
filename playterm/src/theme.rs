/// Runtime theme — resolved ratatui `Color` values built from `ThemeSection`.
///
/// All fields default to the current hardcoded palette so the appearance is
/// identical when no `[theme]` section is present in config.toml.
use ratatui::style::Color;

use crate::config::ThemeSection;

#[derive(Debug, Clone)]
pub struct Theme {
    /// Orange accent: active borders, highlighted items, progress bar fill. (#ff8c00)
    pub accent:        Color,
    /// Outer background (status bar, now-playing bar). (#1a1a1a)
    pub background:    Color,
    /// Panel backgrounds (browser columns, queue block). (#161616)
    pub surface:       Color,
    /// Primary text. (#d4d0c8)
    pub foreground:    Color,
    /// Secondary / muted text. (#5a5858)
    pub dimmed:        Color,
    /// Inactive pane borders. (#252525)
    pub border:        Color,
    /// Active pane borders. (#3a3a3a)
    pub border_active: Color,
}

impl Theme {
    pub fn from_section(sec: &ThemeSection) -> Self {
        fn p(opt: Option<&str>, default: Color) -> Color {
            opt.and_then(parse_hex).unwrap_or(default)
        }
        Self {
            accent:        p(sec.accent.as_deref(),        Color::Rgb(255, 140,   0)),
            background:    p(sec.background.as_deref(),    Color::Rgb( 26,  26,  26)),
            surface:       p(sec.surface.as_deref(),       Color::Rgb( 22,  22,  22)),
            foreground:    p(sec.foreground.as_deref(),    Color::Rgb(212, 208, 200)),
            dimmed:        p(sec.dimmed.as_deref(),        Color::Rgb( 90,  88,  88)),
            border:        p(sec.border.as_deref(),        Color::Rgb( 37,  37,  37)),
            border_active: p(sec.border_active.as_deref(), Color::Rgb( 58,  58,  58)),
        }
    }
}

/// Parse a 6-digit hex colour string (with or without leading `#`).
fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}
