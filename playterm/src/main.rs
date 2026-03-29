mod action;
mod app;
mod color;
mod config;
mod keybinds;
mod persist;
mod state;
mod theme;
mod ui;

use std::io;
use std::process;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use action::{Action, Direction};
use app::{App, BrowserColumn, Tab};
use config::Config;
use keybinds::Keybinds;

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
    stdout.execute(EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app).await;

    // Clear any Kitty images before leaving the alternate screen.
    if app.kitty_supported {
        let _ = ui::kitty_art::clear_image();
    }

    // Restore terminal regardless of errors.
    disable_raw_mode()?;
    terminal.backend_mut().execute(DisableMouseCapture)?;
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

        // Advance colour transition before drawing.
        app.tick_accent_transition();

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
                        // Same album, same rect — image is in terminal store
                        // (placement was cleared on tab-away).  Redisplay instantly.
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
                // Switched away from NowPlaying — remove the visible Kitty
                // placement so it doesn't float above the browser columns.
                // clear_image() uses a=d,d=A which removes the on-screen
                // placement only; the image data stays in the terminal's store,
                // so display_image() can redisplay it instantly on tab-back.
                if art_displayed {
                    let _ = ui::kitty_art::clear_image();
                    art_displayed = false;
                }
            }
        }
        last_tab = app.active_tab;

        // Poll for events. During colour transitions redraw at 33 ms for
        // smooth animation; otherwise 50 ms keeps the progress bar responsive.
        let poll_ms = if app.accent_transition_active() { 33 } else { 50 };
        if event::poll(Duration::from_millis(poll_ms))? {
            match event::read()? {
                Event::Key(key) => {
                    // Only process key-press events; ignore release/repeat to avoid
                    // double-firing on terminals that send all event kinds (e.g. Kitty).
                    if key.kind == KeyEventKind::Press {
                        let action = if app.search_mode.active {
                            map_search_key(key.code)
                        } else {
                            map_key(key.code, key.modifiers, app.active_tab, &app.keybinds)
                        };
                        app.dispatch(action);
                    }
                }
                Event::Mouse(mouse) => {
                    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                        let sz = terminal.size()?;
                        let area = ratatui::layout::Rect::new(0, 0, sz.width, sz.height);
                        handle_mouse_click(mouse.column, mouse.row, app, area);
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

fn map_key(code: KeyCode, modifiers: KeyModifiers, active_tab: Tab, kb: &Keybinds) -> Action {
    // ── Always-on / non-configurable ─────────────────────────────────────────
    // g / G: jump to top/bottom — not exposed in config
    if code == KeyCode::Char('g') && modifiers.is_empty() { return Action::Navigate(Direction::Top);    }
    if code == KeyCode::Char('G') && modifiers.is_empty() { return Action::Navigate(Direction::Bottom); }
    // Enter / Esc — not configurable
    if code == KeyCode::Enter { return Action::Select; }
    if code == KeyCode::Esc   { return Action::Back;   }
    // Space is always an alias for play_pause
    if code == KeyCode::Char(' ') { return Action::PlayPause; }
    // '=' is always a secondary alias for volume_up (easy to hit with +)
    if code == KeyCode::Char('=') { return Action::VolumeUp; }
    // 't' toggles dynamic accent colour
    if code == KeyCode::Char('t') && modifiers.is_empty() { return Action::ToggleDynamicTheme; }
    // 'L' toggles lyrics overlay (NowPlaying tab only)
    if code == KeyCode::Char('L') { return Action::ToggleLyrics; }
    // Up/Down arrows are always secondary scroll aliases
    if code == KeyCode::Up   { return Action::Navigate(Direction::Up);   }
    if code == KeyCode::Down { return Action::Navigate(Direction::Down); }

    // ── Configurable keybinds ─────────────────────────────────────────────────
    if kb.quit.matches(code, modifiers)       { return Action::Quit;       }
    if kb.tab_switch.matches(code, modifiers) { return Action::SwitchTab;  }

    // seek_forward / seek_backward are tab-aware: they also act as column
    // navigation in the Browser tab so Right/Left keep working there.
    if kb.seek_forward.matches(code, modifiers) {
        return match active_tab {
            Tab::NowPlaying => Action::SeekForward,
            Tab::Browser    => Action::FocusRight,
        };
    }
    if kb.seek_backward.matches(code, modifiers) {
        return match active_tab {
            Tab::NowPlaying => Action::SeekBackward,
            Tab::Browser    => Action::FocusLeft,
        };
    }

    if kb.column_left.matches(code, modifiers)  { return Action::FocusLeft;  }
    if kb.column_right.matches(code, modifiers) { return Action::FocusRight; }
    if kb.scroll_up.matches(code, modifiers)    { return Action::Navigate(Direction::Up);   }
    if kb.scroll_down.matches(code, modifiers)  { return Action::Navigate(Direction::Down); }

    if kb.play_pause.matches(code, modifiers)   { return Action::PlayPause;    }
    if kb.next_track.matches(code, modifiers)   { return Action::NextTrack;    }
    if kb.prev_track.matches(code, modifiers)   { return Action::PrevTrack;    }

    // add_all must be checked before add_track (it's typically a superset key).
    if kb.add_all.matches(code, modifiers)      { return Action::AddAllToQueue; }
    if kb.add_track.matches(code, modifiers)    { return Action::AddToQueue;    }

    if kb.shuffle.matches(code, modifiers)      { return Action::Shuffle;      }
    if kb.unshuffle.matches(code, modifiers)    { return Action::Unshuffle;    }
    if kb.clear_queue.matches(code, modifiers)  { return Action::ClearQueue;   }
    if kb.search.matches(code, modifiers)       { return Action::SearchStart;  }
    if kb.volume_up.matches(code, modifiers)    { return Action::VolumeUp;     }
    if kb.volume_down.matches(code, modifiers)  { return Action::VolumeDown;   }

    Action::None
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

// ── Mouse click handler ───────────────────────────────────────────────────────

fn handle_mouse_click(x: u16, y: u16, app: &mut App, terminal_size: ratatui::layout::Rect) {
    use ratatui::layout::{Constraint, Layout};
    use state::LoadingState;

    let (center, now_playing) = match app.active_tab {
        Tab::Browser => {
            let areas = ui::layout::build_browser(terminal_size);
            (areas.center, areas.now_playing)
        }
        Tab::NowPlaying => {
            let areas = ui::layout::build_nowplaying(terminal_size);
            (areas.center, areas.now_playing)
        }
    };

    // ── Now-playing bar: [30% info | 40% controls | 30% progress] ────────────
    let np_cols = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(40),
        Constraint::Percentage(30),
    ])
    .split(now_playing);

    let controls_area = np_cols[1];
    let progress_area = np_cols[2];

    // Controls: row 1 of the now-playing bar (0-indexed).
    if y == now_playing.y + 1
        && x >= controls_area.x
        && x < controls_area.x + controls_area.width
    {
        // Divide controls into three equal zones: prev | play-pause | next.
        let rel_x = x - controls_area.x;
        let third = controls_area.width / 3;
        if rel_x < third {
            app.dispatch(Action::PrevTrack);
        } else if rel_x < 2 * third {
            app.dispatch(Action::PlayPause);
        } else {
            app.dispatch(Action::NextTrack);
        }
        return;
    }

    // Progress bar: row 2 of the now-playing bar.
    if y == now_playing.y + 2
        && x >= progress_area.x
        && x < progress_area.x + progress_area.width
        && app.playback.current_song.is_some()
    {
        if let Some(total) = app.playback.total {
            let e = app.playback.elapsed.as_secs();
            let ts = total.as_secs();
            let elapsed_str_len = format!("{}:{:02}", e / 60, e % 60).len() as u16;
            let total_str_len = format!("{}:{:02}", ts / 60, ts % 60).len() as u16;
            let bar_start = progress_area.x + elapsed_str_len + 2;
            let bar_end = (progress_area.x + progress_area.width)
                .saturating_sub(total_str_len + 2);

            if x >= bar_start && bar_end > bar_start {
                let bar_w = (bar_end - bar_start) as f64;
                let ratio = (x - bar_start) as f64 / bar_w;
                let seek_secs = (ratio * ts as f64) as u64;
                app.dispatch(Action::SeekTo(std::time::Duration::from_secs(seek_secs)));
            }
        }
        return;
    }

    // ── Center area ───────────────────────────────────────────────────────────
    if y < center.y || y >= center.y + center.height {
        return;
    }

    match app.active_tab {
        Tab::Browser => {
            // 3 columns: [30% artists | 35% albums | 35% tracks]
            let browser_cols = Layout::horizontal([
                Constraint::Percentage(30),
                Constraint::Percentage(35),
                Constraint::Percentage(35),
            ])
            .split(center);

            let col_idx = if x < browser_cols[1].x {
                0usize
            } else if x < browser_cols[2].x {
                1
            } else {
                2
            };

            let col_area = browser_cols[col_idx];
            // Ignore clicks on the border rows.
            if y <= col_area.y || y >= col_area.y + col_area.height - 1 {
                return;
            }

            let visible_row = (y - col_area.y - 1) as usize;
            let visible_height = col_area.height.saturating_sub(2) as usize;

            // Switch focus to the clicked column.
            app.browser_focus = match col_idx {
                0 => BrowserColumn::Artists,
                1 => BrowserColumn::Albums,
                _ => BrowserColumn::Tracks,
            };

            match col_idx {
                0 => {
                    // Artists: compute ratatui's auto-scroll offset and map click.
                    let orig_idx: Option<usize> = {
                        if let LoadingState::Loaded(artists) = &app.library.artists {
                            let visible: Vec<usize> = if let Some(q) = &app.search_filter {
                                artists.iter().enumerate()
                                    .filter(|(_, a)| a.name.to_lowercase().contains(q.as_str()))
                                    .map(|(i, _)| i)
                                    .collect()
                            } else {
                                (0..artists.len()).collect()
                            };
                            let sel_pos = app.library.selected_artist
                                .and_then(|s| visible.iter().position(|&i| i == s))
                                .unwrap_or(0);
                            // ratatui scrolls to keep selection visible from below:
                            // scroll = max(0, sel_pos - (visible_height - 1))
                            let scroll = sel_pos.saturating_sub(visible_height.saturating_sub(1));
                            let clicked = scroll + visible_row;
                            visible.get(clicked).copied()
                        } else {
                            None
                        }
                    };
                    if let Some(idx) = orig_idx {
                        app.click_browser_artist(idx);
                    }
                }
                1 => {
                    let orig_idx: Option<usize> = {
                        let artist_id = match app.library.current_artist() {
                            Some(a) => a.id.clone(),
                            None => return,
                        };
                        if let Some(LoadingState::Loaded(albums)) =
                            app.library.albums.get(&artist_id)
                        {
                            let visible: Vec<usize> = if let Some(q) = &app.search_filter {
                                albums.iter().enumerate()
                                    .filter(|(_, a)| a.name.to_lowercase().contains(q.as_str()))
                                    .map(|(i, _)| i)
                                    .collect()
                            } else {
                                (0..albums.len()).collect()
                            };
                            let sel_pos = app.library.selected_album
                                .and_then(|s| visible.iter().position(|&i| i == s))
                                .unwrap_or(0);
                            let scroll = sel_pos.saturating_sub(visible_height.saturating_sub(1));
                            let clicked = scroll + visible_row;
                            visible.get(clicked).copied()
                        } else {
                            None
                        }
                    };
                    if let Some(idx) = orig_idx {
                        app.click_browser_album(idx);
                    }
                }
                _ => {
                    let orig_idx: Option<usize> = {
                        let album_id = match app.library.current_album() {
                            Some(a) => a.id.clone(),
                            None => return,
                        };
                        if let Some(LoadingState::Loaded(songs)) =
                            app.library.tracks.get(&album_id)
                        {
                            let visible: Vec<usize> = if let Some(q) = &app.search_filter {
                                songs.iter().enumerate()
                                    .filter(|(_, s)| s.title.to_lowercase().contains(q.as_str()))
                                    .map(|(i, _)| i)
                                    .collect()
                            } else {
                                (0..songs.len()).collect()
                            };
                            let sel_pos = app.library.selected_track
                                .and_then(|s| visible.iter().position(|&i| i == s))
                                .unwrap_or(0);
                            let scroll = sel_pos.saturating_sub(visible_height.saturating_sub(1));
                            let clicked = scroll + visible_row;
                            visible.get(clicked).copied()
                        } else {
                            None
                        }
                    };
                    if let Some(idx) = orig_idx {
                        app.click_browser_track(idx);
                    }
                }
            }
        }
        Tab::NowPlaying => {
            // NowPlaying center: [50% art | 50% queue]
            let np_center = Layout::horizontal([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(center);

            let queue_area = np_center[1];
            if x < queue_area.x || x >= queue_area.x + queue_area.width {
                return;
            }
            // Ignore border rows.
            if y <= queue_area.y || y >= queue_area.y + queue_area.height - 1 {
                return;
            }
            let visible_row = (y - queue_area.y - 1) as usize;
            let clicked_idx = app.queue.scroll + visible_row;
            app.set_queue_cursor(clicked_idx);
        }
    }
}
