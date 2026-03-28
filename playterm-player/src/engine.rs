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
    /// Append the next track to the player queue for gapless playback.
    ///
    /// Must only be sent in response to `PlayerEvent::AboutToFinish`.
    /// Does NOT stop current playback.
    EnqueueNext { url: String, duration: Option<Duration> },
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
    /// Fired ~5 s before the current track ends. The TUI should respond with
    /// `PlayerCommand::EnqueueNext` to enable gapless playback.
    AboutToFinish,
    /// Fired when a gaplessly enqueued track begins playing (elapsed resets).
    TrackAdvanced,
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
    // Gapless state.
    let mut next_total: Option<Duration> = None;
    let mut next_queued = false;
    let mut about_to_finish_sent = false;
    let mut prev_elapsed = Duration::ZERO;

    loop {
        // ── Drain all pending commands (non-blocking) ─────────────────────────
        loop {
            use mpsc::TryRecvError;
            match cmd_rx.try_recv() {
                Ok(cmd) => handle_command(
                    cmd,
                    &player,
                    &evt_tx,
                    &mut current_total,
                    &mut was_playing,
                    &mut next_total,
                    &mut next_queued,
                    &mut about_to_finish_sent,
                    &mut prev_elapsed,
                ),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        // ── Progress tick ─────────────────────────────────────────────────────
        if !player.is_paused() && !player.empty() {
            let elapsed = player.get_pos();

            // Detect gapless track transition: elapsed resets to near zero
            // while we know a next track was appended.  Use a 2 s window rather
            // than 500 ms to tolerate rodio's first-tick imprecision.
            if next_queued
                && prev_elapsed > Duration::from_secs(2)
                && elapsed < Duration::from_secs(2)
            {
                eprintln!(
                    "[player] TrackAdvanced: prev={:.1?} → elapsed={:.1?}",
                    prev_elapsed, elapsed
                );
                current_total = next_total.take();
                next_queued = false;
                about_to_finish_sent = false;
                let _ = evt_tx.send(PlayerEvent::TrackAdvanced);
            }
            prev_elapsed = elapsed;

            let _ = evt_tx.send(PlayerEvent::Progress {
                elapsed,
                total: current_total,
            });

            // Send AboutToFinish ~10 s before the end so the TUI can enqueue next.
            // 10 s gives enough headroom for: player-thread sleep (≤500 ms) +
            // TUI dispatch latency + open_stream 256 KB prebuffer + decode.
            if !about_to_finish_sent && !next_queued {
                if let Some(total) = current_total {
                    let remaining = total.saturating_sub(elapsed);
                    if remaining <= Duration::from_secs(10) && remaining > Duration::ZERO {
                        eprintln!(
                            "[player] AboutToFinish: elapsed={:.1?}, remaining={:.1?}",
                            elapsed, remaining
                        );
                        about_to_finish_sent = true;
                        let _ = evt_tx.send(PlayerEvent::AboutToFinish);
                    }
                }
            }

            was_playing = true;
        }

        // ── Natural track end detection (no next track was enqueued) ──────────
        if was_playing && player.empty() {
            was_playing = false;
            current_total = None;
            next_total = None;
            next_queued = false;
            about_to_finish_sent = false;
            prev_elapsed = Duration::ZERO;
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
    next_total: &mut Option<Duration>,
    next_queued: &mut bool,
    about_to_finish_sent: &mut bool,
    prev_elapsed: &mut Duration,
) {
    match cmd {
        PlayerCommand::PlayUrl { url, duration } => {
            player.stop();
            *was_playing = false;
            *next_total = None;
            *next_queued = false;
            *about_to_finish_sent = false;
            *prev_elapsed = Duration::ZERO;

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
        PlayerCommand::EnqueueNext { url, duration } => {
            eprintln!("[player] EnqueueNext: fetching stream…");
            match download_and_decode(&url) {
                Ok(source) => {
                    *next_total = duration;
                    *next_queued = true;
                    player.append(source);
                    eprintln!("[player] EnqueueNext: appended, next_queued=true");
                }
                Err(e) => {
                    let _ = evt_tx.send(PlayerEvent::Error(format!("enqueue error: {e}")));
                }
            }
        }
        PlayerCommand::Pause => player.pause(),
        PlayerCommand::Resume => player.play(),
        PlayerCommand::Stop => {
            player.stop();
            *current_total = None;
            *next_total = None;
            *next_queued = false;
            *about_to_finish_sent = false;
            *prev_elapsed = Duration::ZERO;
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
