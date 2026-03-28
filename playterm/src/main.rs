mod action;
mod app;
mod config;
mod persist;
mod state;
mod ui;

use std::io;
use std::process;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use action::{Action, Direction};
use app::App;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        process::exit(1);
    });
    let mut app = App::new(config)?;

    // Detect Kitty graphics support before entering raw mode / alternate screen.
    app.kitty_supported = ui::kitty_art::detect_kitty_support();

    // Restore previous session state (selections, queue) before first render.
    if let Err(e) = persist::restore_state(&mut app) {
        eprintln!("warn: could not restore state: {e}");
    }

    // Begin fetching artists immediately.
    app.fetch_artists();

    // Set up terminal.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app).await;

    // Clear any Kitty images before leaving the alternate screen.
    if app.kitty_supported {
        let _ = ui::kitty_art::clear_image();
    }

    // Restore terminal regardless of errors.
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    // `last_rendered_art` — the (cover_id, rect) of the last full image
    // transmission.  Kept across tab switches so we can detect whether a
    // re-transmit is actually needed.
    //
    // `art_displayed` — whether the image is currently visible on screen.
    // Set to false when switching away (ratatui overwrites those cells) but we
    // deliberately do NOT clear the image from the terminal's store, so we can
    // redisplay it instantly with `a=p,i=1` when switching back.
    let mut last_rendered_art: Option<(String, Rect)> = None;
    let mut art_displayed = false;
    let mut last_tab = app.active_tab;

    loop {
        // Drain library updates from background tokio tasks.
        while let Ok(update) = app.library_rx.try_recv() {
            app.apply_library_update(update);
        }
        // Drain player events from the audio thread.
        while let Ok(event) = app.player_rx.try_recv() {
            app.handle_player_event(event);
        }

        terminal.draw(|f| ui::render(app, f))?;

        // ── Kitty album art (rendered after ratatui so it sits above text) ──────
        if app.kitty_supported {
            if app.active_tab == app::Tab::NowPlaying {
                if let Some((cover_id, bytes)) = &app.art_cache {
                    let sz = terminal.size()?;
                    let art_rect = ui::layout::art_rect(Rect::new(0, 0, sz.width, sz.height));
                    let stored_matches = last_rendered_art
                        .as_ref()
                        .map(|(id, r)| id == cover_id && r == &art_rect)
                        .unwrap_or(false);

                    if stored_matches && art_displayed {
                        // Image is already visible — nothing to do.
                    } else if stored_matches && !art_displayed {
                        // Same album, same rect — image is in terminal store but
                        // ratatui overwrote those cells.  Redisplay instantly.
                        match ui::kitty_art::display_image(art_rect) {
                            Ok(()) => art_displayed = true,
                            Err(e) => eprintln!("kitty display: {e}"),
                        }
                    } else {
                        // Album changed, first display, or terminal was resized —
                        // full re-encode and re-transmit.
                        match ui::kitty_art::render_image(bytes, art_rect) {
                            Ok(()) => {
                                last_rendered_art = Some((cover_id.clone(), art_rect));
                                art_displayed = true;
                            }
                            Err(e) => eprintln!("kitty render: {e}"),
                        }
                    }
                } else if art_displayed {
                    // In NowPlaying tab but no art — clear any stale image.
                    let _ = ui::kitty_art::clear_image();
                    last_rendered_art = None;
                    art_displayed = false;
                }
            } else if last_tab == app::Tab::NowPlaying {
                // Switched away from NowPlaying.  Do NOT call clear_image —
                // the ratatui redraw already overwrote those cells, and we want
                // the terminal to keep the image in its store so we can
                // redisplay it instantly when switching back.
                art_displayed = false;
            }
        }
        last_tab = app.active_tab;

        // Poll for events (50 ms timeout keeps progress bar responsive).
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    // Only process key-press events; ignore release/repeat to avoid
                    // double-firing on terminals that send all event kinds (e.g. Kitty).
                    if key.kind == KeyEventKind::Press {
                        let action = if app.search_mode.active {
                            map_search_key(key.code)
                        } else {
                            map_key(key.code, key.modifiers)
                        };
                        app.dispatch(action);
                    }
                }
                Event::Resize(_, _) => {
                    // Terminal resized — clear any displayed image and reset
                    // stored state so the art is re-encoded at the new size.
                    // last_rendered_art rect will no longer match the new
                    // art_rect, so the full render path is taken on next frame.
                    if app.kitty_supported && art_displayed {
                        let _ = ui::kitty_art::clear_image();
                        art_displayed = false;
                        last_rendered_art = None;
                    }
                }
                _ => {}
            }
        }

        // Drain once more so any triggered playback reflects on next frame.
        while let Ok(event) = app.player_rx.try_recv() {
            app.handle_player_event(event);
        }

        if app.should_quit {
            break;
        }
    }
    // Persist UI state on clean quit.
    if let Err(e) = persist::save_state(app) {
        eprintln!("warn: could not save state: {e}");
    }
    Ok(())
}

fn map_key(code: KeyCode, modifiers: KeyModifiers) -> Action {
    match code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Tab => Action::SwitchTab,
        KeyCode::Char('h') | KeyCode::Left => Action::FocusLeft,
        KeyCode::Char('l') | KeyCode::Right => Action::FocusRight,
        KeyCode::Char('j') | KeyCode::Down => Action::Navigate(Direction::Down),
        KeyCode::Char('k') | KeyCode::Up => Action::Navigate(Direction::Up),
        KeyCode::Char('g') => Action::Navigate(Direction::Top),
        KeyCode::Char('G') => Action::Navigate(Direction::Bottom),
        KeyCode::Enter => Action::Select,
        KeyCode::Esc => Action::Back,
        KeyCode::Char('a') if modifiers.contains(KeyModifiers::SHIFT) => Action::AddAllToQueue,
        KeyCode::Char('a') => Action::AddToQueue,
        KeyCode::Char('A') => Action::AddAllToQueue,
        KeyCode::Char('p') => Action::PlayPause,
        KeyCode::Char('n') => Action::NextTrack,
        KeyCode::Char('N') => Action::PrevTrack,
        KeyCode::Char('+') | KeyCode::Char('=') => Action::VolumeUp,
        KeyCode::Char('-') => Action::VolumeDown,
        KeyCode::Char('D') => Action::ClearQueue,
        KeyCode::Char(' ') => Action::PlayPause,
        KeyCode::Char('x') => Action::Shuffle,
        KeyCode::Char('/') => Action::SearchStart,
        _ => Action::None,
    }
}

fn map_search_key(code: KeyCode) -> Action {
    match code {
        KeyCode::Esc => Action::SearchCancel,
        KeyCode::Enter => Action::SearchConfirm,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Char(ch) => Action::SearchInput(ch),
        _ => Action::None,
    }
}
