use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct BrowserAreas {
    pub center: Rect,
    pub now_playing: Rect,
    pub status_bar: Rect,
}

pub struct NowPlayingAreas {
    pub center: Rect,
    pub now_playing: Rect,
    pub status_bar: Rect,
}

/// Unified areas struct used by `build_layout` (all three tabs).
pub struct LayoutAreas {
    pub center:     Rect,
    pub now_playing: Rect,
    /// Tab indicator bar — height 1, between now-playing bar and status bar.
    pub tab_bar:    Rect,
    pub status_bar: Rect,
}

/// Unified layout for all tabs:
///   center (fill) | now-playing bar (4) | tab bar (1) | status bar (1)
pub fn build_layout(area: Rect) -> LayoutAreas {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // active tab content
            Constraint::Length(4), // persistent now-playing bar
            Constraint::Length(1), // tab indicator bar
            Constraint::Length(1), // status bar
        ])
        .split(area);

    LayoutAreas {
        center:      chunks[0],
        now_playing: chunks[1],
        tab_bar:     chunks[2],
        status_bar:  chunks[3],
    }
}

/// Browser tab: columns (fill) | now-playing bar (4) | status bar (1).
///
/// Kept for backwards compatibility with `handle_mouse_click` in `main.rs`
/// which calls this directly.
pub fn build_browser(area: Rect) -> BrowserAreas {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // three-column browser
            Constraint::Length(4), // persistent now-playing bar
            Constraint::Length(1), // status bar
        ])
        .split(area);

    BrowserAreas {
        center: chunks[0],
        now_playing: chunks[1],
        status_bar: chunks[2],
    }
}

/// NowPlaying tab: art + queue (fill) | now-playing bar (4) | status bar (1).
///
/// Kept for backwards compatibility with `handle_mouse_click` in `main.rs`
/// which calls this directly.
pub fn build_nowplaying(area: Rect) -> NowPlayingAreas {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // art placeholder + queue
            Constraint::Length(4), // persistent now-playing bar
            Constraint::Length(1), // status bar
        ])
        .split(area);

    NowPlayingAreas {
        center: chunks[0],
        now_playing: chunks[1],
        status_bar: chunks[2],
    }
}

/// Return the album-art widget rect given the full terminal size.
///
/// Replicates the NowPlaying layout calculation so that `main.rs` can compute
/// the same area without going through a ratatui `Frame`.
pub fn art_rect(terminal_size: Rect) -> Rect {
    let areas = build_nowplaying(terminal_size);
    Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(areas.center)[0]
}
