pub mod albums;
pub mod artists;
pub mod browser;
pub mod layout;
pub mod now_playing;
pub mod nowplaying_tab;
pub mod queue;
pub mod status_bar;
pub mod tracks;

use ratatui::Frame;
use crate::app::{App, Tab};

// ── Palette ───────────────────────────────────────────────────────────────────

use ratatui::style::Color;

pub const BG: Color = Color::Rgb(26, 26, 26);
pub const SURFACE: Color = Color::Rgb(22, 22, 22);
pub const ACCENT: Color = Color::Rgb(255, 140, 0);
pub const TEXT: Color = Color::Rgb(212, 208, 200);
pub const TEXT_MUTED: Color = Color::Rgb(90, 88, 88);
pub const BORDER: Color = Color::Rgb(37, 37, 37);
pub const BORDER_ACTIVE: Color = Color::Rgb(58, 58, 58);

// ── Top-level render ──────────────────────────────────────────────────────────

pub fn render(app: &App, frame: &mut Frame) {
    match app.active_tab {
        Tab::Browser => {
            let areas = layout::build_browser(frame.area());
            browser::render(app, frame, areas.center);
            now_playing::render(app, frame, areas.now_playing);
            status_bar::render(app, frame, areas.status_bar);
        }
        Tab::NowPlaying => {
            let areas = layout::build_nowplaying(frame.area());
            nowplaying_tab::render(app, frame, areas.center);
            now_playing::render(app, frame, areas.now_playing);
            status_bar::render(app, frame, areas.status_bar);
        }
    }
}
