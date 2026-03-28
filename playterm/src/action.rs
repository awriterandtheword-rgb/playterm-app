#[derive(Debug, Clone)]
pub enum Direction {
    Up,
    Down,
    Top,    // g
    Bottom, // G
}

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
    SearchStart,
    SearchInput(char),
    SearchBackspace,
    SearchConfirm,
    SearchCancel,
    Quit,
    None,
}
