pub mod albums;
pub mod artists;
pub mod browser;
pub mod kitty_art;
pub mod layout;
pub mod now_playing;
pub mod nowplaying_tab;
pub mod queue;
pub mod status_bar;
pub mod tracks;

use ratatui::Frame;
use crate::app::{App, Tab};

// Colour palette constants removed — all colours are now accessed via
// `app.theme.*` (see `playterm/src/theme.rs`) so that the [theme] config
// section can override them at runtime.

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
