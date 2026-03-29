//! Audio playback engine.
//!
//! Runs entirely on a dedicated `std::thread` — no tokio inside this module.
//! The TUI communicates via two `std::sync::mpsc` channels:
//!
//! - `PlayerCommand` (TUI → engine): play a URL, pause, resume, stop, set volume.
//! - `PlayerEvent`  (engine → TUI): progress ticks, track-ended, errors.

use std::collections::VecDeque;
use std::io::BufReader;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use anyhow::Result;
use rodio::{Decoder, DeviceSinkBuilder, Player};

use crate::stream::open_stream;
use crate::tap::SampleTap;

type SampleBuffer = Arc<Mutex<VecDeque<f32>>>;

// ── Public channel types ──────────────────────────────────────────────────────

/// Commands sent from the TUI to the player thread.
#[derive(Debug)]
pub enum PlayerCommand {
    /// Start playing the track at `url`. `duration` is the expected total
    /// duration (from Subsonic metadata), used for progress display.
    /// `gen` is a monotonically increasing counter from the TUI; the engine
    /// uses it to discard stale downloads when multiple skips arrive quickly.
    PlayUrl { url: String, duration: Option<Duration>, gen: u64 },
    /// Append the next track to the player queue for gapless playback.
    ///
    /// Must only be sent in response to `PlayerEvent::AboutToFinish`.
    /// Does NOT stop current playback.
    EnqueueNext { url: String, duration: Option<Duration> },
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
    /// Seek to an absolute position in the current track.
    Seek(Duration),
    /// Stop playback and shut down the player thread cleanly.
    Quit,
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

/// Spawn the player thread.
///
/// Returns `(cmd_tx, evt_rx, join_handle, sample_buffer)`.  The caller should send
/// `PlayerCommand::Quit` and then join the handle (with a timeout) on
/// shutdown to ensure the audio device is released cleanly.
///
/// `sample_buffer` is a ring buffer of the most recent decoded f32 samples;
/// the TUI reads it each frame to drive the visualizer FFT.
pub fn spawn_player() -> (
    mpsc::Sender<PlayerCommand>,
    mpsc::Receiver<PlayerEvent>,
    std::thread::JoinHandle<()>,
    SampleBuffer,
) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<PlayerEvent>();

    let sample_buffer: SampleBuffer = Arc::new(Mutex::new(VecDeque::with_capacity(4096)));
    let thread_buffer = sample_buffer.clone();

    let handle = std::thread::Builder::new()
        .name("playterm-player".into())
        .spawn(move || player_thread(cmd_rx, evt_tx, thread_buffer))
        .expect("failed to spawn player thread");

    (cmd_tx, evt_rx, handle, sample_buffer)
}

// ── Player thread ─────────────────────────────────────────────────────────────

fn player_thread(cmd_rx: mpsc::Receiver<PlayerCommand>, evt_tx: mpsc::Sender<PlayerEvent>, sample_buffer: SampleBuffer) {
    // MixerDeviceSink must live for the duration of playback.
    let mut device = match DeviceSinkBuilder::open_default_sink() {
        Ok(d) => d,
        Err(e) => {
            let _ = evt_tx.send(PlayerEvent::Error(format!("audio device error: {e}")));
            return;
        }
    };
    // Suppress the default stderr message on drop — we control shutdown explicitly.
    device.log_on_drop(false);

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
    // Skip-generation counter: updated every time a PlayUrl is received.
    // Used to discard stale downloads when the user skips rapidly.
    let mut skip_gen: u64 = 0;

    'outer: loop {
        // ── Drain all pending commands (non-blocking) ─────────────────────────
        loop {
            use mpsc::TryRecvError;
            match cmd_rx.try_recv() {
                Ok(PlayerCommand::Quit) => break 'outer,
                Ok(PlayerCommand::PlayUrl { url, duration, gen }) => {
                    // Before downloading, drain any further PlayUrl commands that
                    // are already queued.  This turns N rapid skips into one fetch.
                    play_url(
                        url, duration, gen,
                        &cmd_rx, &mut skip_gen,
                        &player, &evt_tx,
                        &mut current_total, &mut was_playing,
                        &mut next_total, &mut next_queued,
                        &mut about_to_finish_sent, &mut prev_elapsed,
                        &sample_buffer,
                    );
                }
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
                    &sample_buffer,
                ),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'outer,
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

    // Stop playback before releasing the audio device.
    player.stop();
    drop(player);
    drop(device);
}

/// Handle a `PlayUrl` command with skip-generation cancellation.
///
/// Before downloading, drains any further `PlayUrl` commands already queued
/// in the channel — turning N rapid skips into a single fetch.  After the
/// (blocking) download, checks the channel once more: if an even newer
/// `PlayUrl` arrived while we were fetching, the download is discarded and
/// we recurse for the new one.
#[allow(clippy::too_many_arguments)]
fn play_url(
    url: String,
    duration: Option<Duration>,
    gen: u64,
    cmd_rx: &mpsc::Receiver<PlayerCommand>,
    skip_gen: &mut u64,
    player: &Player,
    evt_tx: &mpsc::Sender<PlayerEvent>,
    current_total: &mut Option<Duration>,
    was_playing: &mut bool,
    next_total: &mut Option<Duration>,
    next_queued: &mut bool,
    about_to_finish_sent: &mut bool,
    prev_elapsed: &mut Duration,
    sample_buffer: &SampleBuffer,
) {
    // Update skip_gen for this command.
    *skip_gen = gen;

    // ── Pre-download drain ────────────────────────────────────────────────────
    // Consume any later PlayUrl commands already sitting in the channel.
    // Non-PlayUrl commands are dropped here; in practice the only commands
    // that arrive between rapid skips are more PlayUrl commands.
    let mut final_url = url;
    let mut final_duration = duration;
    let mut final_gen = gen;
    loop {
        match cmd_rx.try_recv() {
            Ok(PlayerCommand::PlayUrl { url: u, duration: d, gen: g }) => {
                final_url = u;
                final_duration = d;
                final_gen = g;
                *skip_gen = g;
            }
            Ok(_other) => break, // non-play command; stop lookahead
            Err(_) => break,
        }
    }

    // Stop current playback and reset all state before the (slow) fetch.
    player.stop();
    *was_playing = false;
    *next_total = None;
    *next_queued = false;
    *about_to_finish_sent = false;
    *prev_elapsed = Duration::ZERO;

    // ── Network fetch (blocking) ──────────────────────────────────────────────
    let source = match download_and_decode(&final_url) {
        Ok(s) => s,
        Err(e) => {
            let _ = evt_tx.send(PlayerEvent::Error(format!("playback error: {e}")));
            return;
        }
    };

    // ── Post-download drain ───────────────────────────────────────────────────
    // If the user skipped again while we were fetching, discard this result
    // and handle the newer command instead.
    let mut newer: Option<(String, Option<Duration>, u64)> = None;
    loop {
        match cmd_rx.try_recv() {
            Ok(PlayerCommand::PlayUrl { url: u, duration: d, gen: g }) => {
                *skip_gen = g;
                newer = Some((u, d, g));
            }
            Ok(_other) => break,
            Err(_) => break,
        }
    }

    if *skip_gen != final_gen {
        // A newer PlayUrl arrived mid-download — discard this source.
        drop(source);
        if let Some((u, d, g)) = newer {
            play_url(
                u, d, g, cmd_rx, skip_gen,
                player, evt_tx,
                current_total, was_playing,
                next_total, next_queued,
                about_to_finish_sent, prev_elapsed,
                sample_buffer,
            );
        }
        return;
    }

    // ── Commit ────────────────────────────────────────────────────────────────
    *current_total = final_duration;
    let tapped = SampleTap::new(source, sample_buffer.clone());
    player.append(tapped);
    player.play();
    let _ = evt_tx.send(PlayerEvent::TrackStarted);
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
    sample_buffer: &SampleBuffer,
) {
    match cmd {
        PlayerCommand::PlayUrl { .. } => {
            // Handled by play_url() in the main loop — should not reach here.
            unreachable!("PlayUrl must be dispatched via play_url()");
        }
        PlayerCommand::EnqueueNext { url, duration } => {
            match download_and_decode(&url) {
                Ok(source) => {
                    *next_total = duration;
                    *next_queued = true;
                    let tapped = SampleTap::new(source, sample_buffer.clone());
                    player.append(tapped);
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
        PlayerCommand::Seek(pos) => {
            let result = player.try_seek(pos);
            eprintln!("[seek] target={:?} result={:?}", pos, result);
            // Update prev_elapsed so the gapless-transition heuristic isn't
            // confused by the sudden position jump.
            *prev_elapsed = pos;
        }
        PlayerCommand::Quit => {
            // Handled by the 'outer break in player_thread — should not reach here.
            unreachable!("Quit must be handled in the outer command-drain loop");
        }
    }
}

// ── Stream + decode ───────────────────────────────────────────────────────────

fn download_and_decode(url: &str) -> Result<Decoder<BufReader<crate::stream::StreamingReader>>> {
    let reader = open_stream(url)?;
    // with_seekable(true): tells symphonia the stream supports random access;
    //   without this, ReadSeekSource::is_seekable() returns false and symphonia
    //   treats the stream as forward-only, causing all seeks to fail.
    // with_coarse_seek(true): bypasses the time_base requirement for accurate
    //   seeking (unavailable on transcoded MP3 streams); seeks to the nearest
    //   keyframe, which is accurate enough for a music player.
    let decoder = Decoder::builder()
        .with_data(BufReader::new(reader))
        .with_seekable(true)
        .with_coarse_seek(true)
        .build()?;
    Ok(decoder)
}
