#[derive(Debug, Clone)]
pub enum Direction {
    Up,
    Down,
    Top,    // g
    Bottom, // G
}

use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Action {
    Navigate(Direction),
    Select,
    Back,
    /// Cycle tabs forward: Home → Browser → NowPlaying → Home (Tab key)
    SwitchTab,
    /// Cycle tabs backward: Home → NowPlaying → Browser → Home (Backtick / Shift+Tab)
    SwitchTabReverse,
    /// Jump directly to Home tab (key '1')
    GoToHome,
    /// Jump directly to Browser tab (key '2')
    GoToBrowser,
    /// Jump directly to NowPlaying tab (key '3')
    GoToNowPlaying,
    FocusLeft,
    FocusRight,
    AddToQueue,
    AddAllToQueue,
    PlayPause,
    NextTrack,
    PrevTrack,
    VolumeUp,
    VolumeDown,
    ClearQueue,
    Shuffle,
    Unshuffle,
    SeekForward,
    SeekBackward,
    /// Seek to an exact position (used by progress-bar clicks).
    SeekTo(Duration),
    SearchStart,
    SearchInput(char),
    SearchBackspace,
    SearchConfirm,
    SearchCancel,
    /// Toggle dynamic accent colour extraction from album art.
    ToggleDynamicTheme,
    /// Toggle the lyrics overlay on the NowPlaying tab.
    ToggleLyrics,
    /// Toggle the keybind reference popup.
    ToggleHelp,
    /// Move to the next section on the Home tab (RecentAlbums → RecentTracks → TopArtists → Rediscover).
    HomeSectionNext,
    /// Move to the previous section on the Home tab.
    HomeSectionPrev,
    /// Refresh Home tab data (re-rolls rediscover suggestions).
    HomeRefresh,
    /// Navigate the art strip left (decrement selected album).
    HomeAlbumLeft,
    /// Navigate the art strip right (increment selected album).
    HomeAlbumRight,
    /// Add the selected album (strip) to queue, replacing existing queue.
    HomeAlbumPlay,
    /// Append the selected album (strip) to queue without clearing.
    HomeAlbumAddToQueue,
    Quit,
    None,
}
