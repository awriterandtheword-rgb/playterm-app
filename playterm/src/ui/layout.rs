use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct NowPlayingAreas {
    pub center: Rect,
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

/// NowPlaying tab: art + queue (fill) | now-playing bar (4) | status bar (1).
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
    }
}

/// Split the Now Playing tab center into `[album art | queue column]` (50/50).
pub fn now_playing_split_center(center: Rect) -> (Rect, Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(center);
    (cols[0], cols[1])
}

/// Queue widget area: matches `nowplaying_tab::render` (full right column, or top
/// 75% when lyrics / visualizer share that column).
pub fn now_playing_queue_widget_rect(
    center: Rect,
    lyrics_visible: bool,
    visualizer_visible: bool,
) -> Rect {
    let (_, queue_col) = now_playing_split_center(center);
    if lyrics_visible || visualizer_visible {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(75),
                Constraint::Percentage(25),
            ])
            .split(queue_col);
        rows[0]
    } else {
        queue_col
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
