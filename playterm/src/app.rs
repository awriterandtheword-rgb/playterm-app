use std::sync::{Arc, mpsc as std_mpsc};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use anyhow::Result;
use ratatui::style::Color;
use tokio::sync::mpsc;

use playterm_player::{PlayerCommand, PlayerEvent, spawn_player};
use playterm_subsonic::SubsonicClient;

use serde::{Deserialize, Serialize};

use crate::action::{Action, Direction};
use crate::color::{extract_accent, lerp_color};
use crate::config::Config;
use crate::keybinds::Keybinds;
use crate::state::{LibraryState, LoadingState, PlaybackState, QueueState};
use crate::theme::Theme;
use playterm_subsonic::LyricLine;

// ── Tab ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    #[default]
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserColumn {
    #[default]
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

    #[allow(dead_code)]
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
    /// Lyrics fetched for a song; `lines` is empty when the track has no lyrics.
    Lyrics { song_id: String, lines: Vec<LyricLine> },
    /// Track bytes downloaded for offline caching.
    CacheTrack { song_id: String, album_id: String, bytes: Vec<u8> },
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
    /// Resolved keybindings (parsed from config.toml [keybinds]).
    pub keybinds: Keybinds,
    /// Resolved theme colours (parsed from config.toml [theme]).
    pub theme: Theme,
    /// Monotonically increasing counter sent with every `PlayerCommand::PlayUrl`.
    /// The engine uses it to discard stale downloads from rapid skips.
    play_gen: u64,

    // ── Offline cache (Feature 5.3) ───────────────────────────────────────────
    /// Track file cache (LRU, persisted to `~/.cache/playterm/`).
    pub cache: crate::cache::TrackCache,
    /// Monotonically increasing counter for background download tasks.
    /// Incremented on every `play_current()` call. Background tasks discard
    /// their result if the gen has advanced since they were spawned.
    prefetch_gen: Arc<AtomicU64>,

    // ── Help popup (Feature 5.2.1) ────────────────────────────────────────────
    /// Whether the keybind reference popup is open.
    pub help_visible: bool,

    // ── Lyrics (Feature 5.2) ──────────────────────────────────────────────────
    /// Whether the lyrics overlay is currently visible (NowPlaying tab only).
    pub lyrics_visible: bool,
    /// Cached lyrics: `(song_id, lines)`. Empty `lines` = server has no lyrics.
    /// `None` = not yet fetched for the current song.
    pub lyrics_cache: Option<(String, Vec<LyricLine>)>,
    /// Scroll offset for unsynced lyrics (manual j/k scrolling).
    pub lyrics_scroll: usize,
    /// True while an async lyrics fetch is in flight.
    pub lyrics_loading: bool,

    // ── Dynamic accent (Feature 5.1) ──────────────────────────────────────────
    /// Dominant colour extracted from the current track's album art.
    /// `None` = no art / no suitable colour found.
    pub dynamic_accent: Option<Color>,
    /// Currently displayed accent — interpolates toward `dynamic_accent` over 400 ms.
    /// Initialised to `theme.accent`; updated each render tick.
    pub accent_current: Color,
    /// Accent value at the start of the current transition.
    accent_lerp_from: Color,
    /// Target accent for the current transition.
    accent_target: Color,
    /// When the current colour transition started. `None` = no active transition.
    pub accent_transition_start: Option<Instant>,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let subsonic =
            SubsonicClient::new(&config.subsonic_url, &config.subsonic_user, &config.subsonic_pass)?;
        let (library_tx, library_rx) = mpsc::channel(64);
        let (player_tx, player_rx) = spawn_player();
        // Apply configured default volume immediately.
        let _ = player_tx.send(PlayerCommand::SetVolume(config.default_volume as f32 / 100.0));
        let keybinds = Keybinds::from_section(&config.keybinds);
        let theme    = Theme::from_section(&config.theme);
        let static_accent = theme.accent;
        let lyrics_visible = config.lyrics_visible;
        let track_cache = crate::cache::TrackCache::load(config.cache_enabled, config.cache_max_size_gb);
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
            keybinds,
            theme,
            play_gen: 0,
            cache: track_cache,
            prefetch_gen: Arc::new(AtomicU64::new(0)),
            help_visible: false,
            lyrics_visible,
            lyrics_cache: None,
            lyrics_scroll: 0,
            lyrics_loading: false,
            dynamic_accent: None,
            accent_current: static_accent,
            accent_lerp_from: static_accent,
            accent_target: static_accent,
            accent_transition_start: None,
        })
    }

    // ── Accent colour helpers ─────────────────────────────────────────────────

    /// The accent colour to use at render time.
    /// Returns `accent_current` (the OKLab-interpolated value) when dynamic
    /// mode is on, otherwise the static configured accent.
    pub fn accent(&self) -> Color {
        // Pass `accent_current` as the dynamic value — `effective_accent`
        // uses it when `theme.dynamic` is true, else falls back to static accent.
        self.theme.effective_accent(if self.theme.dynamic {
            Some(self.accent_current)
        } else {
            None
        })
    }

    /// Returns true while a colour transition is in progress.
    pub fn accent_transition_active(&self) -> bool {
        self.accent_transition_start.is_some()
    }

    /// Call once per render tick to advance the colour interpolation.
    pub fn tick_accent_transition(&mut self) {
        if let Some(start) = self.accent_transition_start {
            let t = start.elapsed().as_secs_f32() / 0.4;
            if t >= 1.0 {
                self.accent_current = self.accent_target;
                self.accent_transition_start = None;
            } else {
                self.accent_current = lerp_color(self.accent_lerp_from, self.accent_target, t);
            }
        }
    }

    /// Set the dynamic accent, kicking off a transition if dynamic mode is on.
    fn apply_dynamic_accent(&mut self, color: Option<Color>) {
        self.dynamic_accent = color;
        if self.theme.dynamic {
            let target = color.unwrap_or(self.theme.accent);
            if target != self.accent_target {
                self.accent_lerp_from = self.accent_current;
                self.accent_target = target;
                self.accent_transition_start = Some(Instant::now());
            }
        }
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

    /// Spawn a task to fetch lyrics for a song from LRCLib.
    ///
    /// Soft-fails silently — on any error an empty `lines` vec is delivered so
    /// the UI shows "No lyrics available" rather than a loading spinner forever.
    pub fn fetch_lyrics(
        &mut self,
        song_id: String,
        artist: String,
        title: String,
        album: String,
    ) {
        self.lyrics_loading = true;
        self.lyrics_scroll = 0;
        let tx = self.library_tx.clone();
        tokio::spawn(async move {
            let lines = crate::lyrics::fetch_lyrics(&artist, &title, &album).await;
            let _ = tx.send(LibraryUpdate::Lyrics { song_id, lines }).await;
        });
    }

    /// Return `true` when every line in the current lyrics cache has no timestamp
    /// (i.e. lyrics are plain-text and must be scrolled manually).
    pub fn lyrics_are_unsynced(&self) -> bool {
        self.lyrics_cache
            .as_ref()
            .map(|(_, lines)| lines.iter().all(|l| l.time.is_none()))
            .unwrap_or(false)
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
                        if !artists.is_empty() {
                            // Default to 0 on fresh start; restore keeps whatever was saved.
                            if self.library.selected_artist.is_none() {
                                self.library.selected_artist = Some(0);
                            }
                            // Clamp restored index to actual list size.
                            let idx = self.library.selected_artist.unwrap()
                                .min(artists.len() - 1);
                            self.library.selected_artist = Some(idx);
                            // Fetch albums for the selected (or restored) artist.
                            let artist_id = artists[idx].id.clone();
                            if !self.library.albums.contains_key(&artist_id) {
                                self.library.albums.insert(artist_id.clone(), LoadingState::Loading);
                                self.fetch_albums(artist_id);
                            }
                        }
                        LoadingState::Loaded(artists)
                    }
                    Err(e) => LoadingState::Error(e),
                };
            }
            LibraryUpdate::Albums { artist_id, result } => {
                // Is this update for the currently-selected artist?
                let is_selected_artist = self.library.selected_artist
                    .and_then(|idx| {
                        if let LoadingState::Loaded(artists) = &self.library.artists {
                            artists.get(idx).map(|a| a.id == artist_id)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(false);

                self.library.albums.insert(
                    artist_id,
                    match result {
                        Ok(albums) if !albums.is_empty() => {
                            if is_selected_artist {
                                // Use restored selection (default 0), clamped to list size.
                                if self.library.selected_album.is_none() {
                                    self.library.selected_album = Some(0);
                                }
                                let idx = self.library.selected_album.unwrap()
                                    .min(albums.len() - 1);
                                self.library.selected_album = Some(idx);
                                let album_id = albums[idx].id.clone();
                                if !self.library.tracks.contains_key(&album_id) {
                                    self.library.tracks.insert(album_id.clone(), LoadingState::Loading);
                                    self.fetch_tracks(album_id);
                                }
                            } else {
                                // Background prefetch: fetch tracks for first album.
                                if self.library.selected_album.is_none() {
                                    self.library.selected_album = Some(0);
                                }
                                let first_id = albums[0].id.clone();
                                if !self.library.tracks.contains_key(&first_id) {
                                    self.library.tracks.insert(first_id.clone(), LoadingState::Loading);
                                    self.fetch_tracks(first_id);
                                }
                            }
                            LoadingState::Loaded(albums)
                        }
                        Ok(albums) => LoadingState::Loaded(albums),
                        Err(e) => LoadingState::Error(e),
                    },
                );
            }
            LibraryUpdate::Tracks { album_id, result } => {
                // Is this update for the currently-selected album?
                let is_current_album = self.library.current_album()
                    .map(|a| a.id == album_id)
                    .unwrap_or(false);
                let loaded = match result {
                    Ok(songs) => {
                        if is_current_album && !songs.is_empty() {
                            // Clamp restored index (or default to 0) to actual song count.
                            if self.library.selected_track.is_none() {
                                self.library.selected_track = Some(0);
                            }
                            let idx = self.library.selected_track.unwrap()
                                .min(songs.len() - 1);
                            self.library.selected_track = Some(idx);
                        } else if self.library.selected_track.is_none() {
                            self.library.selected_track = Some(0);
                        }
                        LoadingState::Loaded(songs)
                    }
                    Err(e) => LoadingState::Error(e),
                };
                self.library.tracks.insert(album_id, loaded);
            }
            LibraryUpdate::AllTracksForArtist { mut songs, start_playing } => {
                let was_empty = self.queue.songs.is_empty();
                songs.sort_by_key(|s| (
                    s.album_id.clone().unwrap_or_default(),
                    s.disc_number.unwrap_or(1),
                    s.track.unwrap_or(0),
                ));
                for song in songs {
                    self.queue.push(song);
                }
                if start_playing && was_empty && !self.queue.songs.is_empty() {
                    self.queue.cursor = 0;
                    self.play_current();
                }
            }
            LibraryUpdate::CoverArt { cover_id, bytes } => {
                // Extract dynamic accent before storing (bytes are consumed here).
                let accent = extract_accent(&bytes);
                self.art_cache = Some((cover_id, bytes));
                self.apply_dynamic_accent(accent);
            }
            LibraryUpdate::Lyrics { song_id, lines } => {
                self.lyrics_loading = false;
                self.lyrics_cache = Some((song_id, lines));
                self.lyrics_scroll = 0;
            }
            LibraryUpdate::CacheTrack { song_id, album_id, bytes } => {
                let _ = self.cache.put(&song_id, &album_id, &bytes);
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
                    let cover_id = song.cover_art.clone();
                    if let Some(ref cid) = cover_id {
                        let needs_fetch = self.art_cache
                            .as_ref()
                            .map(|(cached_id, _)| cached_id != cid)
                            .unwrap_or(true);
                        if needs_fetch {
                            // Art will arrive via CoverArt library update — accent
                            // is applied there.  Clear stale dynamic accent for now.
                            self.apply_dynamic_accent(None);
                            self.fetch_cover_art(cid.clone());
                        } else if let Some((_, ref bytes)) = self.art_cache.clone() {
                            // Art already cached for this cover_id — extract immediately.
                            let accent = extract_accent(bytes);
                            self.apply_dynamic_accent(accent);
                        }
                    } else {
                        // Track has no cover art.
                        self.apply_dynamic_accent(None);
                    }
                    // Fetch lyrics if not already cached for this song.
                    let cached_for_song = self.lyrics_cache
                        .as_ref()
                        .map(|(id, _)| id == &song.id)
                        .unwrap_or(false);
                    if !cached_for_song {
                        self.fetch_lyrics(
                            song.id.clone(),
                            song.artist.clone().unwrap_or_default(),
                            song.title.clone(),
                            song.album.clone().unwrap_or_default(),
                        );
                    }
                    // Background-cache current track + prefetch next 2.
                    if self.config.cache_enabled {
                        // Collect (song_id, album_id) pairs to download, then spawn.
                        // We read from queue and cache separately to satisfy the borrow checker.
                        let mut to_download: Vec<(String, String)> = Vec::new();
                        // Current track.
                        if !self.cache.get_const(&song.id) {
                            to_download.push((song.id.clone(), song.album_id.clone().unwrap_or_default()));
                        }
                        // Next 2 tracks.
                        let cursor = self.queue.cursor;
                        for offset in 1..=2usize {
                            let idx = cursor + offset;
                            if idx < self.queue.songs.len() {
                                let s_id = self.queue.songs[idx].id.clone();
                                let a_id = self.queue.songs[idx].album_id.clone().unwrap_or_default();
                                if !self.cache.get_const(&s_id) {
                                    to_download.push((s_id, a_id));
                                }
                            }
                        }
                        for (s_id, a_id) in to_download {
                            self.spawn_cache_download(&s_id, &a_id);
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
                    let cover_id = song.cover_art.clone();
                    if let Some(ref cid) = cover_id {
                        let needs_fetch = self.art_cache
                            .as_ref()
                            .map(|(cached_id, _)| cached_id != cid)
                            .unwrap_or(true);
                        if needs_fetch {
                            self.apply_dynamic_accent(None);
                            self.fetch_cover_art(cid.clone());
                        } else if let Some((_, ref bytes)) = self.art_cache.clone() {
                            let accent = extract_accent(bytes);
                            self.apply_dynamic_accent(accent);
                        }
                    } else {
                        self.apply_dynamic_accent(None);
                    }
                    // Fetch lyrics if not already cached for this song.
                    let cached_for_song = self.lyrics_cache
                        .as_ref()
                        .map(|(id, _)| id == &song.id)
                        .unwrap_or(false);
                    if !cached_for_song {
                        self.fetch_lyrics(
                            song.id.clone(),
                            song.artist.clone().unwrap_or_default(),
                            song.title.clone(),
                            song.album.clone().unwrap_or_default(),
                        );
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
            self.play_gen += 1;
            // Advance the prefetch gen so stale background downloads are discarded.
            self.prefetch_gen.fetch_add(1, Ordering::Release);
            let url = self.resolve_stream_url(&song);
            let duration = song.duration.map(|s| std::time::Duration::from_secs(u64::from(s)));
            self.playback.current_song = Some(song);
            self.playback.player_loaded = true;
            let _ = self.player_tx.send(PlayerCommand::PlayUrl { url, duration, gen: self.play_gen });
        }
    }

    /// Return the URL to play for `song`.
    ///
    /// If the track is in the offline cache, starts a loopback HTTP server that
    /// serves the cached bytes and returns `http://127.0.0.1:{port}/`.
    /// Falls back to the Subsonic stream URL on any error or cache miss.
    fn resolve_stream_url(&mut self, song: &playterm_subsonic::Song) -> String {
        if self.config.cache_enabled {
            if let Some(path) = self.cache.get(&song.id) {
                self.cache.touch(&song.id);
                match crate::cache::serve_from_cache(path) {
                    Ok(local_url) => return local_url,
                    Err(e) => eprintln!("warn: could not serve from cache: {e}"),
                }
            }
        }
        self.subsonic.stream_url(&song.id, self.config.max_bit_rate)
    }

    /// Spawn a background task to download `song_id` for caching.
    ///
    /// Callers are responsible for checking `cache.get_const()` first.
    /// The task checks `prefetch_gen` after download and discards stale bytes
    /// (from rapid skips or queue changes since spawn time).
    fn spawn_cache_download(&self, song_id: &str, album_id: &str) {
        let url = self.subsonic.stream_url(song_id, self.config.max_bit_rate);
        let song_id  = song_id.to_string();
        let album_id = album_id.to_string();
        let tx       = self.library_tx.clone();
        let gen_arc  = self.prefetch_gen.clone();
        let expected = gen_arc.load(Ordering::Acquire);
        tokio::spawn(async move {
            if let Ok(resp) = reqwest::Client::new().get(&url).send().await {
                if let Ok(bytes) = resp.bytes().await {
                    if gen_arc.load(Ordering::Acquire) == expected {
                        let _ = tx.send(LibraryUpdate::CacheTrack {
                            song_id,
                            album_id,
                            bytes: bytes.to_vec(),
                        }).await;
                    }
                }
            }
        });
    }

    // ── Action dispatch ───────────────────────────────────────────────────────

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::ToggleHelp => self.help_visible = !self.help_visible,
            Action::Quit => self.should_quit = true,
            Action::SwitchTab => {
                self.active_tab = self.active_tab.toggle();
                self.search_filter = None;
            }
            Action::FocusLeft => self.handle_focus_left(),
            Action::FocusRight => self.handle_focus_right(),
            Action::Navigate(dir) => {
                // On NowPlaying tab with unsynced lyrics visible, j/k scroll
                // the lyrics pane instead of the queue.
                if self.active_tab == Tab::NowPlaying
                    && self.lyrics_visible
                    && self.lyrics_are_unsynced()
                {
                    match dir {
                        Direction::Up | Direction::Top => {
                            self.lyrics_scroll = self.lyrics_scroll.saturating_sub(1);
                        }
                        Direction::Down | Direction::Bottom => {
                            self.lyrics_scroll = self.lyrics_scroll.saturating_add(1);
                        }
                    }
                } else {
                    self.handle_navigate(dir);
                }
            }
            Action::Select => self.handle_select(),
            Action::Back => self.handle_focus_left(),
            Action::AddToQueue => self.handle_add_to_queue(),
            Action::AddAllToQueue => self.handle_add_all_to_queue(),
            Action::PlayPause => {
                if !self.playback.player_loaded && self.queue.current().is_some() {
                    // Restored queue: engine has no track yet — load and start playing.
                    self.play_current();
                } else if self.playback.paused {
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
            Action::Unshuffle => self.handle_unshuffle(),
            Action::SeekForward => {
                let new_pos = if let Some(total) = self.playback.total {
                    (self.playback.elapsed + std::time::Duration::from_secs(10)).min(total)
                } else {
                    self.playback.elapsed + std::time::Duration::from_secs(10)
                };
                let _ = self.player_tx.send(PlayerCommand::Seek(new_pos));
                self.playback.elapsed = new_pos;
            }
            Action::SeekBackward => {
                let new_pos = self.playback.elapsed.saturating_sub(std::time::Duration::from_secs(10));
                let _ = self.player_tx.send(PlayerCommand::Seek(new_pos));
                self.playback.elapsed = new_pos;
            }
            Action::SeekTo(pos) => {
                let new_pos = if let Some(total) = self.playback.total {
                    pos.min(total)
                } else {
                    pos
                };
                let _ = self.player_tx.send(PlayerCommand::Seek(new_pos));
                self.playback.elapsed = new_pos;
            }
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
            Action::ToggleLyrics => {
                // Only active on NowPlaying tab; silently ignored on Browser.
                if self.active_tab == Tab::NowPlaying {
                    self.lyrics_visible = !self.lyrics_visible;
                    // Trigger a lyrics fetch if we just enabled the overlay and
                    // nothing is cached for the current song yet.
                    if self.lyrics_visible {
                        if let Some(song) = self.playback.current_song.clone() {
                            let cached = self.lyrics_cache
                                .as_ref()
                                .map(|(id, _)| id == &song.id)
                                .unwrap_or(false);
                            if !cached {
                                self.fetch_lyrics(
                                    song.id.clone(),
                                    song.artist.clone().unwrap_or_default(),
                                    song.title.clone(),
                                    song.album.clone().unwrap_or_default(),
                                );
                            }
                        }
                    }
                }
            }
            Action::ToggleDynamicTheme => {
                if self.theme.dynamic {
                    // Disable: instant snap back to static accent.
                    self.theme.dynamic = false;
                    self.accent_current = self.theme.accent;
                    self.accent_target  = self.theme.accent;
                    self.accent_transition_start = None;
                } else {
                    // Enable: start transition from current to dynamic accent (if any).
                    self.theme.dynamic = true;
                    let target = self.dynamic_accent.unwrap_or(self.theme.accent);
                    self.accent_lerp_from = self.accent_current;
                    self.accent_target    = target;
                    self.accent_transition_start = Some(Instant::now());
                }
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
                    let mut sorted = songs.clone();
                    sorted.sort_by_key(|s| (s.disc_number.unwrap_or(1), s.track.unwrap_or(0)));
                    for song in sorted {
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
        // pre_shuffle_order is maintained by QueueState::push and must NOT be
        // overwritten here — it always holds the original add-order so Z can
        // revert regardless of how many times x is pressed.

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
        self.queue.pre_shuffle_order = None;
        let _ = self.player_tx.send(PlayerCommand::Stop);
        self.playback.current_song = None;
        self.playback.elapsed = std::time::Duration::ZERO;
        self.playback.paused = false;
        self.playback.player_loaded = false;
    }

    fn handle_unshuffle(&mut self) {
        let original = match &self.queue.pre_shuffle_order {
            Some(o) => o.clone(),
            None => return,
        };
        // Do NOT clear pre_shuffle_order — Z should always work, even after
        // reshuffling multiple times.
        let current_id = self.queue.current().map(|s| s.id.clone());
        self.queue.songs = original;
        if let Some(id) = current_id {
            if let Some(idx) = self.queue.songs.iter().position(|s| s.id == id) {
                self.queue.cursor = idx;
                self.queue.scroll = idx;
            }
        }
    }

    // ── Mouse-click helpers (called from main.rs event handler) ──────────────

    pub fn click_browser_artist(&mut self, orig_idx: usize) {
        if let LoadingState::Loaded(artists) = &self.library.artists {
            if orig_idx >= artists.len() {
                return;
            }
        } else {
            return;
        }
        self.library.selected_artist = Some(orig_idx);
        self.library.selected_album = Some(0);
        self.library.selected_track = Some(0);
        let artist_id = if let LoadingState::Loaded(artists) = &self.library.artists {
            artists[orig_idx].id.clone()
        } else {
            return;
        };
        if !self.library.albums.contains_key(&artist_id) {
            self.library.albums.insert(artist_id.clone(), LoadingState::Loading);
            self.fetch_albums(artist_id);
        }
    }

    pub fn click_browser_album(&mut self, orig_idx: usize) {
        let artist_id = match self.library.current_artist() {
            Some(a) => a.id.clone(),
            None => return,
        };
        let album_id = {
            let albums = match self.library.albums.get(&artist_id) {
                Some(LoadingState::Loaded(a)) => a,
                _ => return,
            };
            if orig_idx >= albums.len() {
                return;
            }
            albums[orig_idx].id.clone()
        };
        self.library.selected_album = Some(orig_idx);
        self.library.selected_track = Some(0);
        if !self.library.tracks.contains_key(&album_id) {
            self.library.tracks.insert(album_id.clone(), LoadingState::Loading);
            self.fetch_tracks(album_id);
        }
    }

    pub fn click_browser_track(&mut self, orig_idx: usize) {
        let album_id = match self.library.current_album() {
            Some(a) => a.id.clone(),
            None => return,
        };
        let valid = match self.library.tracks.get(&album_id) {
            Some(LoadingState::Loaded(songs)) => orig_idx < songs.len(),
            _ => false,
        };
        if valid {
            self.library.selected_track = Some(orig_idx);
        }
    }

    pub fn set_queue_cursor(&mut self, idx: usize) {
        if idx < self.queue.songs.len() {
            self.queue.cursor = idx;
            self.queue.scroll = idx;
        }
    }
}
