use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::queue;
use super::visualizer::render_visualizer;

use crate::app::App;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let (art_col, queue_col) = super::layout::now_playing_split_center(area);

    render_art_placeholder(app, frame, art_col);

    if app.visualizer_visible {
        // Split the queue column: top 75% = queue, bottom 25% = visualizer pane.
        let rows = Layout::vertical([
            Constraint::Percentage(75),
            Constraint::Percentage(25),
        ])
        .split(queue_col);
        queue::render(app, frame, rows[0], true);
        render_visualizer_pane(app, frame, rows[1]);
    } else if app.lyrics_visible {
        // Split the queue column: top 75% = queue, bottom 25% = lyrics pane.
        let rows = Layout::vertical([
            Constraint::Percentage(75),
            Constraint::Percentage(25),
        ])
        .split(queue_col);
        queue::render(app, frame, rows[0], true);
        render_lyrics_pane(app, frame, rows[1]);
    } else {
        queue::render(app, frame, queue_col, true);
    }
}

fn render_art_placeholder(app: &App, frame: &mut Frame, area: Rect) {
    let t = &app.theme;
    let block = Block::default()
        .title(" Album Art ")
        .title_style(Style::default().fg(t.dimmed).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(t.border))
        .style(Style::default().bg(t.surface));
    frame.render_widget(block, area);
}

// ── Visualizer pane ───────────────────────────────────────────────────────────

fn render_visualizer_pane(app: &App, frame: &mut Frame, area: Rect) {
    let t = &app.theme;
    let accent = app.accent();

    let block = Block::default()
        .title(" Visualizer ")
        .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(t.surface));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    render_visualizer(frame, inner, &app.spectrum_bands, accent);
}

// ── Lyrics pane ───────────────────────────────────────────────────────────────

fn render_lyrics_pane(app: &App, frame: &mut Frame, area: Rect) {
    let t = &app.theme;
    let accent = app.accent();

    let block = Block::default()
        .title(" Lyrics ")
        .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(t.surface));

    // Inner area for text (inside borders).
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let inner_h = inner.height as usize;
    let inner_w = inner.width as usize;

    // Determine what to show.
    let current_song_id = app.playback.current_song.as_ref().map(|s| s.id.as_str());

    // Check cache match.
    let cache_match = current_song_id.and_then(|sid| {
        app.lyrics_cache.as_ref().and_then(|(cached_id, lines)| {
            if cached_id.as_str() == sid { Some(lines.as_slice()) } else { None }
        })
    });

    match cache_match {
        None if app.lyrics_loading => {
            // Fetch in flight.
            render_centered_msg(frame, inner, "Loading…", t.dimmed);
        }
        None => {
            // No song playing, or cache not yet populated (and not loading).
            render_centered_msg(frame, inner, "Loading…", t.dimmed);
        }
        Some(lines) if lines.is_empty() => {
            render_centered_msg(frame, inner, "No lyrics available", t.dimmed);
        }
        Some(lines) => {
            let is_synced = lines.iter().any(|l| l.time.is_some());
            if is_synced {
                render_synced(app, frame, inner, lines, inner_h, inner_w, accent);
            } else {
                render_unsynced(app, frame, inner, lines, inner_h, inner_w);
            }
        }
    }
}

/// Render a single centered message line.
fn render_centered_msg(
    frame: &mut Frame,
    area: Rect,
    msg: &'static str,
    color: ratatui::style::Color,
) {
    let para = Paragraph::new(msg)
        .style(Style::default().fg(color))
        .alignment(Alignment::Center);
    frame.render_widget(para, area);
}

// ── Synced lyrics ─────────────────────────────────────────────────────────────

fn render_synced(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    lines: &[playterm_subsonic::LyricLine],
    inner_h: usize,
    _inner_w: usize,
    accent: ratatui::style::Color,
) {
    let t = &app.theme;
    let elapsed = app.playback.elapsed;

    // Find the index of the last line whose timestamp ≤ elapsed.
    let current_idx: Option<usize> = lines.iter().enumerate()
        .filter(|(_, l)| l.time.map(|ts| ts <= elapsed).unwrap_or(false))
        .map(|(i, _)| i)
        .last();

    // Auto-scroll to keep current line vertically centred.
    let scroll: usize = current_idx
        .map(|ci| ci.saturating_sub(inner_h / 2))
        .unwrap_or(0);

    let display: Vec<Line> = lines.iter().enumerate()
        .skip(scroll)
        .take(inner_h)
        .map(|(i, l)| {
            let style = match current_idx {
                Some(ci) if i == ci => {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                }
                Some(ci) if i < ci => Style::default().fg(t.dimmed),
                _ => Style::default().fg(t.foreground),
            };
            Line::from(Span::styled(l.text.as_str(), style))
        })
        .collect();

    let para = Paragraph::new(display)
        .style(Style::default().bg(t.surface))
        .alignment(Alignment::Center);
    frame.render_widget(para, area);
}

// ── Unsynced lyrics ───────────────────────────────────────────────────────────

fn render_unsynced(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    lines: &[playterm_subsonic::LyricLine],
    inner_h: usize,
    inner_w: usize,
) {
    let t = &app.theme;

    // Word-wrap all lyric lines into display rows.
    let wrapped: Vec<String> = lines.iter()
        .flat_map(|l| wrap_text(&l.text, inner_w))
        .collect();

    let scroll = app.lyrics_scroll.min(wrapped.len().saturating_sub(1));

    let display: Vec<Line> = wrapped.iter()
        .skip(scroll)
        .take(inner_h)
        .map(|row| Line::from(Span::styled(row.as_str(), Style::default().fg(t.foreground))))
        .collect();

    let para = Paragraph::new(display)
        .style(Style::default().bg(t.surface));
    frame.render_widget(para, area);
}

/// Word-wrap `text` to at most `width` visible characters per line.
/// Returns at least one element (empty string for empty input).
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    if chars.len() <= width {
        return vec![chars.iter().collect()];
    }

    let mut lines = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + width).min(chars.len());
        // If we haven't reached the end, try to break on a space.
        let break_at = if end < chars.len() {
            chars[start..end]
                .iter()
                .rposition(|&c| c == ' ')
                .map(|i| start + i)
                .unwrap_or(end)
        } else {
            end
        };
        lines.push(chars[start..break_at].iter().collect());
        start = break_at;
        // Skip the space we broke on.
        while start < chars.len() && chars[start] == ' ' {
            start += 1;
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
