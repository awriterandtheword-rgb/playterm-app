use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct BrowserAreas {
    pub now_playing: Rect,
    pub center: Rect,
    pub status_bar: Rect,
}

pub struct NowPlayingAreas {
    pub center: Rect,
    pub status_bar: Rect,
}

/// Browser tab: now-playing bar (1) | columns (fill) | status bar (1).
pub fn build_browser(area: Rect) -> BrowserAreas {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // now-playing bar
            Constraint::Min(0),    // three columns
            Constraint::Length(1), // status bar
        ])
        .split(area);

    BrowserAreas {
        now_playing: chunks[0],
        center: chunks[1],
        status_bar: chunks[2],
    }
}

/// NowPlaying tab: full content (fill) | status bar (1).
pub fn build_nowplaying(area: Rect) -> NowPlayingAreas {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // art + queue + progress strip
            Constraint::Length(1), // status bar
        ])
        .split(area);

    NowPlayingAreas {
        center: chunks[0],
        status_bar: chunks[1],
    }
}
