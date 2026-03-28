use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use playterm_subsonic::Song;

use crate::app::{App, BrowserColumn, Tab};

// ── Saved state ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedState {
    pub active_tab: Tab,
    pub browser_focus: BrowserColumn,
    pub selected_artist: Option<usize>,
    pub selected_album: Option<usize>,
    pub selected_track: Option<usize>,
    pub queue: Vec<Song>,
    pub queue_cursor: usize,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn state_path() -> Result<PathBuf> {
    let dir = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("playterm")
    } else {
        let home = std::env::var("HOME").context("HOME env var not set")?;
        PathBuf::from(home).join(".config").join("playterm")
    };
    Ok(dir.join("state.json"))
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Serialize current UI state to `~/.config/playterm/state.json`.
pub fn save_state(app: &App) -> Result<()> {
    let state = SavedState {
        active_tab: app.active_tab,
        browser_focus: app.browser_focus,
        selected_artist: app.library.selected_artist,
        selected_album: app.library.selected_album,
        selected_track: app.library.selected_track,
        queue: app.queue.songs.clone(),
        queue_cursor: app.queue.cursor,
    };
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating state dir {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(&state)?;
    std::fs::write(&path, json)
        .with_context(|| format!("writing state to {}", path.display()))?;
    Ok(())
}

/// Restore previously saved state into `app`. No playback is started.
pub fn restore_state(app: &mut App) -> Result<()> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let state: SavedState = serde_json::from_str(&text)
        .with_context(|| format!("parsing {}", path.display()))?;

    app.active_tab = state.active_tab;
    app.browser_focus = state.browser_focus;
    app.library.selected_artist = state.selected_artist;
    app.library.selected_album = state.selected_album;
    app.library.selected_track = state.selected_track;
    app.queue.songs = state.queue;
    app.queue.cursor = state.queue_cursor.min(app.queue.songs.len().saturating_sub(1));
    app.queue.scroll = app.queue.cursor;
    Ok(())
}
