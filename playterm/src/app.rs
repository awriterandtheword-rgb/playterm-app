use std::sync::{Arc, mpsc as std_mpsc};

use anyhow::Result;
use tokio::sync::mpsc;

use playterm_player::{PlayerCommand, PlayerEvent, spawn_player};
use playterm_subsonic::SubsonicClient;

use serde::{Deserialize, Serialize};

use crate::action::{Action, Direction};
use crate::config::Config;
use crate::state::{LibraryState, LoadingState, PlaybackState, QueueState};

// ── Tab ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    Browser,
    NowPlaying,
}

impl Tab {
    pub fn toggle(self) -> Self {
        match self {
            Tab::Browser => Tab::NowPlaying,
            Tab::NowPlaying => Tab::Browser,
        }
    }
}

// ── BrowserColumn ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserColumn {
    Artists,
    Albums,
    Tracks,
}

impl BrowserColumn {
    pub fn left(self) -> Self {
        match self {
            BrowserColumn::Artists => BrowserColumn::Artists,
            BrowserColumn::Albums => BrowserColumn::Artists,
            BrowserColumn::Tracks => BrowserColumn::Albums,
        }
    }

    pub fn right(self) -> Self {
        match self {
            BrowserColumn::Artists => BrowserColumn::Albums,
            BrowserColumn::Albums => BrowserColumn::Tracks,
            BrowserColumn::Tracks => BrowserColumn::Tracks,
        }
    }
}

// ── SearchMode ────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct SearchMode {
    pub active: bool,
    pub query: String,
    /// Index within the filtered result list that is currently selected.
    pub selected: usize,
}

// ── LibraryUpdate — messages sent back from background fetch tasks ─────────────

#[derive(Debug)]
pub enum LibraryUpdate {
    Artists(Result<Vec<playterm_subsonic::Artist>, String>),
    Albums {
        artist_id: String,
        result: Result<Vec<playterm_subsonic::Album>, String>,
    },
    Tracks {
        album_id: String,
        result: Result<Vec<playterm_subsonic::Song>, String>,
    },
    /// All tracks across every album for an artist; carries whether playback
    /// should auto-start (true when the queue was empty at dispatch time).
    AllTracksForArtist {
        songs: Vec<playterm_subsonic::Song>,
        start_playing: bool,
    },
    /// Raw image bytes for a cover art ID fetched from Navidrome.
    CoverArt { cover_id: String, bytes: Vec<u8> },
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub active_tab: Tab,
    pub browser_focus: BrowserColumn,
    pub library: LibraryState,
    pub queue: QueueState,
    pub playback: PlaybackState,
    pub config: Config,
    pub subsonic: Arc<SubsonicClient>,
    /// Receives library data from background tokio tasks.
    pub library_rx: mpsc::Receiver<LibraryUpdate>,
    library_tx: mpsc::Sender<LibraryUpdate>,
    /// Send commands to the audio engine thread.
    pub player_tx: std_mpsc::Sender<PlayerCommand>,
    /// Receive events from the audio engine thread.
    pub player_rx: std_mpsc::Receiver<PlayerEvent>,
    pub should_quit: bool,
    pub search_mode: SearchMode,
    /// Active filter applied to the current browser column after a search confirm.
    /// `None` = show all items; `Some(q)` = show only items whose name contains `q`.
    pub search_filter: Option<String>,
    /// Whether the running terminal supports the Kitty graphics protocol.
    /// Set once by `main` before the TUI loop starts.
    pub kitty_supported: bool,
    /// Cached cover art: `(cover_art_id, raw_image_bytes)`.
    /// Updated whenever a new track starts with a different cover ID.
    pub art_cache: Option<(String, Vec<u8>)>,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let subsonic =
            SubsonicClient::new(&config.subsonic_url, &config.subsonic_user, &config.subsonic_pass)?;
        let (library_tx, library_rx) = mpsc::channel(64);
        let (player_tx, player_rx) = spawn_player();
        // Apply configured default volume immediately.
        let _ = player_tx.send(PlayerCommand::SetVolume(config.default_volume as f32 / 100.0));
        Ok(Self {
            active_tab: Tab::Browser,
            browser_focus: BrowserColumn::Artists,
            library: LibraryState::default(),
            queue: QueueState::default(),
            playback: PlaybackState::default(),
            subsonic: Arc::new(subsonic),
            library_rx,
            library_tx,
            player_tx,
            player_rx,
            config,
            should_quit: false,
            search_mode: SearchMode::default(),
            search_filter: None,
            kitty_supported: false,
            art_cache: None,
        })
    }

    // ── Background fetch helpers ──────────────────────────────────────────────

    /// Spawn a task to fetch the artist list.
    pub fn fetch_artists(&self) {
        let client = self.subsonic.clone();
        let tx = self.library_tx.clone();
        tokio::spawn(async move {
            let result = playterm_subsonic::fetch_library(&client)
                .await
                .map(|lib| lib.artists)
                .map_err(|e| e.to_string());
            let _ = tx.send(LibraryUpdate::Artists(result)).await;
        });
    }

    /// Spawn a task to fetch albums for the given artist.
    pub fn fetch_albums(&self, artist_id: String) {
        let client = self.subsonic.clone();
        let tx = self.library_tx.clone();
        tokio::spawn(async move {
            let result = client
                .get_artist(&artist_id)
                .await
                .map(|a| a.album)
                .map_err(|e| e.to_string());
            let _ = tx.send(LibraryUpdate::Albums { artist_id, result }).await;
        });
    }

    /// Spawn a task to fetch the track list for the given album.
    pub fn fetch_tracks(&self, album_id: String) {
        let client = self.subsonic.clone();
        let tx = self.library_tx.clone();
        tokio::spawn(async move {
            let result = client
                .get_album(&album_id)
                .await
                .map(|a| a.song)
                .map_err(|e| e.to_string());
            let _ = tx.send(LibraryUpdate::Tracks { album_id, result }).await;
        });
    }

    /// Spawn a task to fetch raw cover art bytes for the given cover art ID.
    pub fn fetch_cover_art(&self, cover_id: String) {
        let client = self.subsonic.clone();
        let tx = self.library_tx.clone();
        tokio::spawn(async move {
            match client.get_cover_art(&cover_id).await {
                Ok(bytes) => {
                    let _ = tx.send(LibraryUpdate::CoverArt { cover_id, bytes }).await;
                }
                Err(e) => eprintln!("fetch_cover_art({cover_id}): {e}"),
            }
        });
    }

    /// Spawn a task that fetches every album + every track for the given artist,
    /// then delivers them as a flat sorted `AllTracksForArtist` update.
    pub fn fetch_all_tracks_for_artist(&self, artist_id: String, start_playing: bool) {
        let client = self.subsonic.clone();
        let tx = self.library_tx.clone();
        tokio::spawn(async move {
            let artist = match client.get_artist(&artist_id).await {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("fetch_all_tracks_for_artist({}): {e}", artist_id);
                    return;
                }
            };
            let mut songs = Vec::new();
            for album in &artist.album {
                match client.get_album(&album.id).await {
                    Ok(a) => songs.extend(a.song),
                    Err(e) => eprintln!("get_album({}): {e}", album.id),
                }
            }
            songs.sort_by_key(|s| (s.disc_number.unwrap_or(1), s.track.unwrap_or(0)));
            let _ = tx
                .send(LibraryUpdate::AllTracksForArtist { songs, start_playing })
                .await;
        });
    }

    // ── Library update ingestion ──────────────────────────────────────────────

    pub fn apply_library_update(&mut self, update: LibraryUpdate) {
        match update {
            LibraryUpdate::Artists(result) => {
                self.library.artists = match result {
                    Ok(artists) => {
                        if self.library.selected_artist.is_none() && !artists.is_empty() {
                            self.library.selected_artist = Some(0);
                            // Proactively fetch albums for the first artist.
                            let first_id = artists[0].id.clone();
                            self.library.albums.insert(first_id.clone(), LoadingState::Loading);
                            self.fetch_albums(first_id);
                        }
                        LoadingState::Loaded(artists)
                    }
                    Err(e) => LoadingState::Error(e),
                };
            }
            LibraryUpdate::Albums { artist_id, result } => {
                self.library.albums.insert(
                    artist_id,
                    match result {
                        Ok(albums) => {
                            // Proactively fetch tracks for the first album.
                            if !albums.is_empty() {
                                let first_id = albums[0].id.clone();
                                if !self.library.tracks.contains_key(&first_id) {
                                    self.library
                                        .tracks
                                        .insert(first_id.clone(), LoadingState::Loading);
                                    self.fetch_tracks(first_id);
                                }
                            }
                            LoadingState::Loaded(albums)
                        }
                        Err(e) => LoadingState::Error(e),
                    },
                );
                if self.library.selected_album.is_none() {
                    self.library.selected_album = Some(0);
                }
            }
            LibraryUpdate::Tracks { album_id, result } => {
                self.library.tracks.insert(
                    album_id,
                    match result {
                        Ok(songs) => LoadingState::Loaded(songs),
                        Err(e) => LoadingState::Error(e),
                    },
                );
                if self.library.selected_track.is_none() {
                    self.library.selected_track = Some(0);
                }
            }
            LibraryUpdate::AllTracksForArtist { songs, start_playing } => {
                let was_empty = self.queue.songs.is_empty();
                for song in songs {
                    self.queue.push(song);
                }
                if start_playing && was_empty && !self.queue.songs.is_empty() {
                    self.queue.cursor = 0;
                    self.play_current();
                }
            }
            LibraryUpdate::CoverArt { cover_id, bytes } => {
                self.art_cache = Some((cover_id, bytes));
            }
        }
    }

    // ── Player event ingestion ────────────────────────────────────────────────

    pub fn handle_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::TrackStarted => {
                self.playback.paused = false;
                if let Some(song) = self.queue.current().cloned() {
                    // Fetch cover art when the track has one and it differs from cache.
                    if self.kitty_supported {
                        if let Some(cover_id) = &song.cover_art {
                            let needs_fetch = self.art_cache
                                .as_ref()
                                .map(|(cached_id, _)| cached_id != cover_id)
                                .unwrap_or(true);
                            if needs_fetch {
                                self.fetch_cover_art(cover_id.clone());
                            }
                        }
                    }
                    self.playback.current_song = Some(song);
                }
            }
            PlayerEvent::Progress { elapsed, total } => {
                self.playback.elapsed = elapsed;
                self.playback.total = total;
            }
            PlayerEvent::AboutToFinish => {
                // Pre-load the next track for gapless playback.
                if let Some(next) = self.queue.peek_next().cloned() {
                    let url = self.subsonic.stream_url(&next.id, self.config.max_bit_rate);
                    let duration =
                        next.duration.map(|s| std::time::Duration::from_secs(u64::from(s)));
                    let _ = self
                        .player_tx
                        .send(PlayerCommand::EnqueueNext { url, duration });
                }
            }
            PlayerEvent::TrackAdvanced => {
                // The gapless transition happened — advance the queue cursor.
                self.queue.next();
                self.playback.paused = false;
                self.playback.elapsed = std::time::Duration::ZERO;
                if let Some(song) = self.queue.current().cloned() {
                    if self.kitty_supported {
                        if let Some(cover_id) = &song.cover_art {
                            let needs_fetch = self.art_cache
                                .as_ref()
                                .map(|(cached_id, _)| cached_id != cover_id)
                                .unwrap_or(true);
                            if needs_fetch {
                                self.fetch_cover_art(cover_id.clone());
                            }
                        }
                    }
                    self.playback.current_song = Some(song);
                }
            }
            PlayerEvent::TrackEnded => {
                if self.queue.next() {
                    self.play_current();
                } else {
                    self.playback.current_song = None;
                    self.playback.elapsed = std::time::Duration::ZERO;
                }
            }
            PlayerEvent::Error(e) => {
                eprintln!("player error: {e}");
            }
        }
    }

    /// Send a PlayUrl command for the song the queue cursor points at.
    fn play_current(&mut self) {
        if let Some(song) = self.queue.current().cloned() {
            let url = self.subsonic.stream_url(&song.id, self.config.max_bit_rate);
            let duration = song.duration.map(|s| std::time::Duration::from_secs(u64::from(s)));
            self.playback.current_song = Some(song);
            let _ = self.player_tx.send(PlayerCommand::PlayUrl { url, duration });
        }
    }

    // ── Action dispatch ───────────────────────────────────────────────────────

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::SwitchTab => {
                self.active_tab = self.active_tab.toggle();
                self.search_filter = None;
            }
            Action::FocusLeft => self.handle_focus_left(),
            Action::FocusRight => self.handle_focus_right(),
            Action::Navigate(dir) => self.handle_navigate(dir),
            Action::Select => self.handle_select(),
            Action::Back => self.handle_focus_left(),
            Action::AddToQueue => self.handle_add_to_queue(),
            Action::AddAllToQueue => self.handle_add_all_to_queue(),
            Action::PlayPause => {
                if self.playback.paused {
                    self.playback.paused = false;
                    let _ = self.player_tx.send(PlayerCommand::Resume);
                } else {
                    self.playback.paused = true;
                    let _ = self.player_tx.send(PlayerCommand::Pause);
                }
            }
            Action::NextTrack => {
                if self.queue.next() {
                    self.play_current();
                }
            }
            Action::PrevTrack => {
                if self.queue.prev() {
                    self.play_current();
                }
            }
            Action::VolumeUp => {
                self.config.default_volume = self.config.default_volume.saturating_add(5).min(100);
                let _ = self.player_tx.send(PlayerCommand::SetVolume(self.config.default_volume as f32 / 100.0));
            }
            Action::VolumeDown => {
                self.config.default_volume = self.config.default_volume.saturating_sub(5);
                let _ = self.player_tx.send(PlayerCommand::SetVolume(self.config.default_volume as f32 / 100.0));
            }
            Action::ClearQueue => self.handle_clear_queue(),
            Action::Shuffle => self.handle_shuffle(),
            Action::SearchStart => {
                self.search_mode.active = true;
                self.search_mode.query.clear();
                self.search_mode.selected = 0;
                // Starting a new search clears the previous filter.
                self.search_filter = None;
            }
            Action::SearchInput(ch) => {
                if self.search_mode.active {
                    self.search_mode.query.push(ch);
                    self.search_mode.selected = 0;
                }
            }
            Action::SearchBackspace => {
                if self.search_mode.active {
                    self.search_mode.query.pop();
                    self.search_mode.selected = 0;
                }
            }
            Action::SearchConfirm => {
                if self.search_mode.active {
                    let q = self.search_mode.query.to_lowercase();
                    if q.is_empty() {
                        self.search_filter = None;
                    } else {
                        self.search_filter = Some(q);
                    }
                    self.handle_search_confirm();
                    self.search_mode.active = false;
                    self.search_mode.query.clear();
                }
            }
            Action::SearchCancel => {
                self.search_mode.active = false;
                self.search_mode.query.clear();
                self.search_mode.selected = 0;
                self.search_filter = None;
            }
            Action::None => {}
        }
    }

    // ── Focus movement ────────────────────────────────────────────────────────

    fn handle_focus_right(&mut self) {
        if self.active_tab != Tab::Browser {
            return;
        }
        self.search_filter = None;
        match self.browser_focus {
            BrowserColumn::Artists => {
                if let Some(artist) = self.library.current_artist() {
                    let artist_id = artist.id.clone();
                    if !self.library.albums.contains_key(&artist_id) {
                        self.library.albums.insert(artist_id.clone(), LoadingState::Loading);
                        self.fetch_albums(artist_id);
                    }
                }
                self.browser_focus = BrowserColumn::Albums;
            }
            BrowserColumn::Albums => {
                if let Some(album) = self.library.current_album() {
                    let album_id = album.id.clone();
                    if !self.library.tracks.contains_key(&album_id) {
                        self.library.tracks.insert(album_id.clone(), LoadingState::Loading);
                        self.fetch_tracks(album_id);
                    }
                }
                self.browser_focus = BrowserColumn::Tracks;
            }
            BrowserColumn::Tracks => {} // already rightmost
        }
    }

    fn handle_focus_left(&mut self) {
        if self.active_tab != Tab::Browser {
            return;
        }
        self.search_filter = None;
        self.browser_focus = self.browser_focus.left();
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    fn handle_navigate(&mut self, dir: Direction) {
        match self.active_tab {
            Tab::Browser => self.handle_navigate_browser(dir),
            Tab::NowPlaying => self.handle_navigate_queue(dir),
        }
    }

    fn handle_navigate_browser(&mut self, dir: Direction) {
        match self.browser_focus {
            BrowserColumn::Artists => {
                let result = if let LoadingState::Loaded(artists) = &self.library.artists {
                    // Build navigable index set — filtered or full.
                    let indices: Vec<usize> = if let Some(q) = &self.search_filter {
                        artists.iter().enumerate()
                            .filter(|(_, a)| a.name.to_lowercase().contains(q.as_str()))
                            .map(|(i, _)| i)
                            .collect()
                    } else {
                        (0..artists.len()).collect()
                    };
                    if indices.is_empty() { return; }
                    let cur_pos = self.library.selected_artist
                        .and_then(|sel| indices.iter().position(|&i| i == sel))
                        .unwrap_or(0);
                    let new_pos = match dir {
                        Direction::Up => cur_pos.saturating_sub(1),
                        Direction::Down => (cur_pos + 1).min(indices.len() - 1),
                        Direction::Top => 0,
                        Direction::Bottom => indices.len() - 1,
                    };
                    let new_orig = indices[new_pos];
                    Some((new_orig, artists[new_orig].id.clone()))
                } else {
                    None
                };
                if let Some((new_idx, artist_id)) = result {
                    self.library.selected_artist = Some(new_idx);
                    self.library.selected_album = Some(0);
                    self.library.selected_track = Some(0);
                    if !self.library.albums.contains_key(&artist_id) {
                        self.library.albums.insert(artist_id.clone(), LoadingState::Loading);
                        self.fetch_albums(artist_id);
                    }
                }
            }
            BrowserColumn::Albums => {
                let result = {
                    let artist_id = match self.library.current_artist() {
                        Some(a) => a.id.clone(),
                        None => return,
                    };
                    if let Some(LoadingState::Loaded(albums)) = self.library.albums.get(&artist_id) {
                        let indices: Vec<usize> = if let Some(q) = &self.search_filter {
                            albums.iter().enumerate()
                                .filter(|(_, a)| a.name.to_lowercase().contains(q.as_str()))
                                .map(|(i, _)| i)
                                .collect()
                        } else {
                            (0..albums.len()).collect()
                        };
                        if indices.is_empty() { return; }
                        let cur_pos = self.library.selected_album
                            .and_then(|sel| indices.iter().position(|&i| i == sel))
                            .unwrap_or(0);
                        let new_pos = match dir {
                            Direction::Up => cur_pos.saturating_sub(1),
                            Direction::Down => (cur_pos + 1).min(indices.len() - 1),
                            Direction::Top => 0,
                            Direction::Bottom => indices.len() - 1,
                        };
                        let new_orig = indices[new_pos];
                        Some((new_orig, albums[new_orig].id.clone()))
                    } else {
                        None
                    }
                };
                if let Some((new_idx, album_id)) = result {
                    self.library.selected_album = Some(new_idx);
                    self.library.selected_track = Some(0);
                    if !self.library.tracks.contains_key(&album_id) {
                        self.library.tracks.insert(album_id.clone(), LoadingState::Loading);
                        self.fetch_tracks(album_id);
                    }
                }
            }
            BrowserColumn::Tracks => {
                let album_id = match self.library.current_album() {
                    Some(a) => a.id.clone(),
                    None => return,
                };
                if let Some(LoadingState::Loaded(songs)) = self.library.tracks.get(&album_id) {
                    let indices: Vec<usize> = if let Some(q) = &self.search_filter {
                        songs.iter().enumerate()
                            .filter(|(_, s)| s.title.to_lowercase().contains(q.as_str()))
                            .map(|(i, _)| i)
                            .collect()
                    } else {
                        (0..songs.len()).collect()
                    };
                    if indices.is_empty() { return; }
                    let cur_pos = self.library.selected_track
                        .and_then(|sel| indices.iter().position(|&i| i == sel))
                        .unwrap_or(0);
                    let new_pos = match dir {
                        Direction::Up => cur_pos.saturating_sub(1),
                        Direction::Down => (cur_pos + 1).min(indices.len() - 1),
                        Direction::Top => 0,
                        Direction::Bottom => indices.len() - 1,
                    };
                    self.library.selected_track = Some(indices[new_pos]);
                }
            }
        }
    }

    fn handle_navigate_queue(&mut self, dir: Direction) {
        let len = self.queue.songs.len();
        if len == 0 {
            return;
        }
        self.queue.cursor = match dir {
            Direction::Up => self.queue.cursor.saturating_sub(1),
            Direction::Down => (self.queue.cursor + 1).min(len - 1),
            Direction::Top => 0,
            Direction::Bottom => len - 1,
        };
        self.queue.scroll = self.queue.cursor;
    }

    // ── Select ────────────────────────────────────────────────────────────────

    fn handle_select(&mut self) {
        match self.active_tab {
            Tab::Browser => match self.browser_focus {
                // Enter on Artists or Albums: same as pressing l
                BrowserColumn::Artists | BrowserColumn::Albums => self.handle_focus_right(),
                // Enter on Tracks: add the highlighted track to the queue
                BrowserColumn::Tracks => self.handle_add_to_queue(),
            },
            Tab::NowPlaying => {} // nothing to select in queue view
        }
    }

    // ── Queue helpers ─────────────────────────────────────────────────────────

    fn handle_add_to_queue(&mut self) {
        if let Some(song) = self.library.current_track().cloned() {
            let was_empty = self.queue.songs.is_empty();
            self.queue.push(song);
            if was_empty {
                self.queue.cursor = 0;
                self.play_current();
            }
        }
    }

    fn handle_add_all_to_queue(&mut self) {
        match self.browser_focus {
            BrowserColumn::Artists | BrowserColumn::Albums => {
                // Fetch every album and every track for the selected artist,
                // then push them all to the queue via AllTracksForArtist.
                if let Some(artist) = self.library.current_artist() {
                    let artist_id = artist.id.clone();
                    let start_playing = self.queue.songs.is_empty();
                    self.fetch_all_tracks_for_artist(artist_id, start_playing);
                }
            }
            BrowserColumn::Tracks => {
                // Add every track in the selected album to the queue.
                let album_id = match self.library.current_album() {
                    Some(a) => a.id.clone(),
                    None => return,
                };
                if let Some(LoadingState::Loaded(songs)) = self.library.tracks.get(&album_id) {
                    let was_empty = self.queue.songs.is_empty();
                    for song in songs.clone() {
                        self.queue.push(song);
                    }
                    if was_empty && !self.queue.songs.is_empty() {
                        self.queue.cursor = 0;
                        self.play_current();
                    }
                }
                // If tracks not loaded yet: no-op; proactive loading makes this rare.
            }
        }
    }

    fn handle_shuffle(&mut self) {
        let len = self.queue.songs.len();
        if len < 2 {
            return;
        }
        // LCG seeded from system time — no external crate needed.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(12345) as u64;
        let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);

        let next_lcg = |state: &mut u64| -> usize {
            *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (*state >> 33) as usize
        };

        // If something is playing, pull the current track to index 0 first,
        // then Fisher-Yates shuffle indices 1..len.
        if self.playback.current_song.is_some() && self.queue.cursor < len {
            self.queue.songs.swap(0, self.queue.cursor);
            for i in (2..len).rev() {
                let j = next_lcg(&mut rng) % i + 1; // range [1, i]
                self.queue.songs.swap(i, j);
            }
        } else {
            // Nothing playing — shuffle the whole vec.
            for i in (1..len).rev() {
                let j = next_lcg(&mut rng) % (i + 1);
                self.queue.songs.swap(i, j);
            }
        }
        self.queue.cursor = 0;
        self.queue.scroll = 0;
    }

    /// Apply the current search: move selection to the first filtered result.
    fn handle_search_confirm(&mut self) {
        let q = self.search_mode.query.to_lowercase();
        if q.is_empty() {
            return;
        }
        match self.active_tab {
            Tab::Browser => match self.browser_focus {
                BrowserColumn::Artists => {
                    if let crate::state::LoadingState::Loaded(artists) = &self.library.artists {
                        if let Some(idx) = artists.iter().position(|a| a.name.to_lowercase().contains(&q)) {
                            self.library.selected_artist = Some(idx);
                        }
                    }
                }
                BrowserColumn::Albums => {
                    let artist_id = match self.library.current_artist() {
                        Some(a) => a.id.clone(),
                        None => return,
                    };
                    if let Some(crate::state::LoadingState::Loaded(albums)) = self.library.albums.get(&artist_id) {
                        if let Some(idx) = albums.iter().position(|a| a.name.to_lowercase().contains(&q)) {
                            self.library.selected_album = Some(idx);
                        }
                    }
                }
                BrowserColumn::Tracks => {
                    let album_id = match self.library.current_album() {
                        Some(a) => a.id.clone(),
                        None => return,
                    };
                    if let Some(crate::state::LoadingState::Loaded(songs)) = self.library.tracks.get(&album_id) {
                        if let Some(idx) = songs.iter().position(|s| s.title.to_lowercase().contains(&q)) {
                            self.library.selected_track = Some(idx);
                        }
                    }
                }
            },
            Tab::NowPlaying => {
                if let Some(idx) = self.queue.songs.iter().position(|s| s.title.to_lowercase().contains(&q)) {
                    self.queue.cursor = idx;
                    self.queue.scroll = idx;
                }
            }
        }
    }

    fn handle_clear_queue(&mut self) {
        self.queue.songs.clear();
        self.queue.cursor = 0;
        self.queue.scroll = 0;
        let _ = self.player_tx.send(PlayerCommand::Stop);
        self.playback.current_song = None;
        self.playback.elapsed = std::time::Duration::ZERO;
        self.playback.paused = false;
    }
}
