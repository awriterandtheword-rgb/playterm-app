use std::collections::HashMap;
use std::time::Duration;

use playterm_subsonic::models::{Album, Artist, Song};

// ── LoadingState ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LoadingState<T> {
    NotLoaded,
    Loading,
    Loaded(T),
    Error(String),
}

impl<T> Default for LoadingState<T> {
    fn default() -> Self {
        Self::NotLoaded
    }
}

// ── LibraryState ──────────────────────────────────────────────────────────────

/// Per-pane selection and lazy-loaded data cache for the library browser.
#[derive(Debug, Default)]
pub struct LibraryState {
    pub artists: LoadingState<Vec<Artist>>,
    pub selected_artist: Option<usize>,

    /// Albums keyed by artist ID.
    pub albums: HashMap<String, LoadingState<Vec<Album>>>,
    pub selected_album: Option<usize>,

    /// Songs keyed by album ID.
    pub tracks: HashMap<String, LoadingState<Vec<Song>>>,
    pub selected_track: Option<usize>,
}

impl LibraryState {
    /// The artist currently highlighted, if the artist list is loaded.
    pub fn current_artist(&self) -> Option<&Artist> {
        if let LoadingState::Loaded(artists) = &self.artists {
            self.selected_artist.and_then(|i| artists.get(i))
        } else {
            None
        }
    }

    /// The album currently highlighted for the selected artist, if loaded.
    pub fn current_album(&self) -> Option<&Album> {
        let artist_id = self.current_artist().map(|a| a.id.as_str())?;
        if let Some(LoadingState::Loaded(albums)) = self.albums.get(artist_id) {
            self.selected_album.and_then(|i| albums.get(i))
        } else {
            None
        }
    }

    /// The track currently highlighted for the selected album, if loaded.
    pub fn current_track(&self) -> Option<&Song> {
        let album_id = self.current_album().map(|a| a.id.as_str())?;
        if let Some(LoadingState::Loaded(songs)) = self.tracks.get(album_id) {
            self.selected_track.and_then(|i| songs.get(i))
        } else {
            None
        }
    }
}

// ── QueueState ────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct QueueState {
    pub songs: Vec<Song>,
    /// Index of the currently playing (or next-to-play) song.
    pub cursor: usize,
    /// Offset for list rendering (scroll).
    pub scroll: usize,
    /// Snapshot of the queue order taken just before the last shuffle.
    /// `None` means no unshuffle is available.
    pub pre_shuffle_order: Option<Vec<Song>>,
}

impl QueueState {
    pub fn push(&mut self, song: Song) {
        self.songs.push(song);
    }

    pub fn current(&self) -> Option<&Song> {
        self.songs.get(self.cursor)
    }

    /// Advance to the next song. Returns true if there is a next song.
    pub fn next(&mut self) -> bool {
        if self.cursor + 1 < self.songs.len() {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    /// Peek at the next song without advancing the cursor.
    pub fn peek_next(&self) -> Option<&Song> {
        self.songs.get(self.cursor + 1)
    }

    /// Move to the previous song. Returns true if there is a previous song.
    pub fn prev(&mut self) -> bool {
        if self.cursor > 0 {
            self.cursor -= 1;
            true
        } else {
            false
        }
    }
}

// ── PlaybackState ─────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct PlaybackState {
    pub current_song: Option<Song>,
    pub elapsed: Duration,
    pub total: Option<Duration>,
    pub paused: bool,
}
