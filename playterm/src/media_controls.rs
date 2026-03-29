//! MPRIS / OS media-key integration via the `souvlaki` crate.
//!
//! `spawn_media_controls` registers the app with the OS media-control system
//! (MPRIS on Linux, MediaPlayer on macOS/Windows) and maps incoming hardware
//! or software media-key events to `Action` values sent on a channel.
//!
//! If the OS integration is unavailable for any reason the function returns
//! `None` and the rest of the app continues normally — media keys simply do
//! nothing.

use std::sync::mpsc;

use souvlaki::{MediaControlEvent, MediaControls, PlatformConfig, SeekDirection};

use crate::action::Action;

/// Register media controls with the OS and attach an event handler that
/// translates incoming events into `Action` values sent over `action_tx`.
///
/// Returns `None` (with a `warn:` line on stderr) if OS integration fails.
/// Never panics.
pub fn spawn_media_controls(action_tx: mpsc::Sender<Action>) -> Option<MediaControls> {
    let config = PlatformConfig {
        display_name: "playterm",
        dbus_name: "playterm",
        hwnd: None,
    };

    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warn: media controls unavailable: {e:?}");
            return None;
        }
    };

    let attach_result = controls.attach(move |event: MediaControlEvent| {
        let action = match event {
            MediaControlEvent::Play     => Action::PlayPause,
            MediaControlEvent::Pause    => Action::PlayPause,
            MediaControlEvent::Toggle   => Action::PlayPause,
            MediaControlEvent::Next     => Action::NextTrack,
            MediaControlEvent::Previous => Action::PrevTrack,
            MediaControlEvent::Seek(SeekDirection::Forward)  => Action::SeekForward,
            MediaControlEvent::Seek(SeekDirection::Backward) => Action::SeekBackward,
            MediaControlEvent::Stop     => Action::Quit,
            MediaControlEvent::Quit     => Action::Quit,
            // All other events (SetVolume, SetPosition, OpenUri, Raise, SeekBy…) — ignore.
            _ => return,
        };
        // Non-blocking: if the receiver is gone (app is shutting down), drop silently.
        let _ = action_tx.send(action);
    });

    if let Err(e) = attach_result {
        eprintln!("warn: media controls attach failed: {e:?}");
        return None;
    }

    Some(controls)
}
