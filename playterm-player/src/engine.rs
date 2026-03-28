//! Audio playback engine.
//!
//! Runs entirely on a dedicated `std::thread` — no tokio inside this module.
//! The TUI communicates via two `std::sync::mpsc` channels:
//!
//! - `PlayerCommand` (TUI → engine): play a URL, pause, resume, stop, set volume.
//! - `PlayerEvent`  (engine → TUI): progress ticks, track-ended, errors.

use std::io::BufReader;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use rodio::{Decoder, DeviceSinkBuilder, Player};

use crate::stream::open_stream;

// ── Public channel types ──────────────────────────────────────────────────────

/// Commands sent from the TUI to the player thread.
#[derive(Debug)]
pub enum PlayerCommand {
    /// Start playing the track at `url`. `duration` is the expected total
    /// duration (from Subsonic metadata), used for progress display.
    PlayUrl { url: String, duration: Option<Duration> },
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
}

/// Events sent from the player thread back to the TUI.
#[derive(Debug)]
pub enum PlayerEvent {
    TrackStarted,
    /// Fired every ~500 ms. `total` is `None` when unknown.
    Progress { elapsed: Duration, total: Option<Duration> },
    TrackEnded,
    Error(String),
}

// ── Engine spawn ──────────────────────────────────────────────────────────────

/// Spawn the player thread. Returns the command sender and event receiver
/// for the TUI to use.
pub fn spawn_player() -> (mpsc::Sender<PlayerCommand>, mpsc::Receiver<PlayerEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<PlayerEvent>();

    std::thread::Builder::new()
        .name("playterm-player".into())
        .spawn(move || player_thread(cmd_rx, evt_tx))
        .expect("failed to spawn player thread");

    (cmd_tx, evt_rx)
}

// ── Player thread ─────────────────────────────────────────────────────────────

fn player_thread(cmd_rx: mpsc::Receiver<PlayerCommand>, evt_tx: mpsc::Sender<PlayerEvent>) {
    // MixerDeviceSink must live for the duration of playback.
    let device = match DeviceSinkBuilder::open_default_sink() {
        Ok(d) => d,
        Err(e) => {
            let _ = evt_tx.send(PlayerEvent::Error(format!("audio device error: {e}")));
            return;
        }
    };

    let player = Player::connect_new(&device.mixer());

    // State for the current track.
    let mut current_total: Option<Duration> = None;
    // Tracks whether the previous tick saw a non-empty player (to detect natural end).
    let mut was_playing = false;

    loop {
        // ── Drain all pending commands (non-blocking) ─────────────────────────
        loop {
            use mpsc::TryRecvError;
            match cmd_rx.try_recv() {
                Ok(cmd) => {
                    handle_command(cmd, &player, &evt_tx, &mut current_total, &mut was_playing)
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        // ── Progress tick ─────────────────────────────────────────────────────
        if !player.is_paused() && !player.empty() {
            let elapsed = player.get_pos();
            let _ = evt_tx.send(PlayerEvent::Progress {
                elapsed,
                total: current_total,
            });
            was_playing = true;
        }

        // ── Natural track end detection ───────────────────────────────────────
        if was_playing && player.empty() {
            was_playing = false;
            current_total = None;
            let _ = evt_tx.send(PlayerEvent::TrackEnded);
        }

        std::thread::sleep(Duration::from_millis(500));
    }
}

fn handle_command(
    cmd: PlayerCommand,
    player: &Player,
    evt_tx: &mpsc::Sender<PlayerEvent>,
    current_total: &mut Option<Duration>,
    was_playing: &mut bool,
) {
    match cmd {
        PlayerCommand::PlayUrl { url, duration } => {
            player.stop();
            *was_playing = false;

            match download_and_decode(&url) {
                Ok(source) => {
                    *current_total = duration;
                    player.append(source);
                    player.play();
                    let _ = evt_tx.send(PlayerEvent::TrackStarted);
                }
                Err(e) => {
                    let _ = evt_tx.send(PlayerEvent::Error(format!("playback error: {e}")));
                }
            }
        }
        PlayerCommand::Pause => player.pause(),
        PlayerCommand::Resume => player.play(),
        PlayerCommand::Stop => {
            player.stop();
            *current_total = None;
            *was_playing = false;
        }
        PlayerCommand::SetVolume(v) => player.set_volume(v),
    }
}

// ── Stream + decode ───────────────────────────────────────────────────────────

fn download_and_decode(url: &str) -> Result<Decoder<BufReader<crate::stream::StreamingReader>>> {
    let reader = open_stream(url)?;
    let decoder = Decoder::try_from(BufReader::new(reader))?;
    Ok(decoder)
}
