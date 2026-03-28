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
    SwitchTab,
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
    Quit,
    None,
}
