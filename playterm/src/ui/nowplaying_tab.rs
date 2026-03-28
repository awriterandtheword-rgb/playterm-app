use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph};

use crate::app::App;
use super::{queue, ACCENT, BG, BORDER, SURFACE, TEXT, TEXT_MUTED};

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    // Vertical split: [art+queue area | progress strip (3 lines)]
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // art + queue
            Constraint::Length(3), // progress strip
        ])
        .split(area);

    render_top(app, frame, rows[0]);
    render_progress(app, frame, rows[1]);
}

fn render_top(app: &App, frame: &mut Frame, area: Rect) {
    // Horizontal split: [album art 50% | queue 50%]
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    render_art_placeholder(frame, cols[0]);
    queue::render(app, frame, cols[1], true);
}

fn render_art_placeholder(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Album Art ")
        .title_style(Style::default().fg(TEXT_MUTED))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE));
    frame.render_widget(block, area);
}

fn render_progress(app: &App, frame: &mut Frame, area: Rect) {
    // 3-line strip: gauge | track info | key hints
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // gauge
            Constraint::Length(1), // track info
            Constraint::Length(1), // hints
        ])
        .split(area);

    // ── Gauge ──────────────────────────────────────────────────────────────────
    let ratio = match (&app.playback.current_song, app.playback.total) {
        (Some(_), Some(total)) if !total.is_zero() => {
            (app.playback.elapsed.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };

    let elapsed = app.playback.elapsed.as_secs();
    let time_label = match app.playback.total {
        Some(t) => {
            let ts = t.as_secs();
            format!(" {}:{:02} / {}:{:02} ", elapsed / 60, elapsed % 60, ts / 60, ts % 60)
        }
        None => format!(" {}:{:02} ", elapsed / 60, elapsed % 60),
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(ACCENT).bg(BORDER))
        .label(time_label)
        .ratio(ratio)
        .style(Style::default().bg(BG));
    frame.render_widget(gauge, rows[0]);

    // ── Track info ─────────────────────────────────────────────────────────────
    let info_line = if let Some(song) = &app.playback.current_song {
        let pause_icon = if app.playback.paused { "⏸ " } else { "▶ " };
        let artist = song.artist.as_deref().unwrap_or("Unknown Artist");
        let album = song.album.as_deref().unwrap_or("");
        Line::from(vec![
            Span::styled(pause_icon, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(song.title.as_str(), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled("  ·  ", Style::default().fg(TEXT_MUTED)),
            Span::styled(artist, Style::default().fg(TEXT)),
            Span::styled("  ·  ", Style::default().fg(TEXT_MUTED)),
            Span::styled(album, Style::default().fg(TEXT_MUTED)),
        ])
    } else {
        Line::from(Span::styled("No track playing", Style::default().fg(TEXT_MUTED)))
    };
    frame.render_widget(Paragraph::new(info_line).style(Style::default().bg(BG)), rows[1]);

    // ── Hints ──────────────────────────────────────────────────────────────────
    let hints = Line::from(Span::styled(
        "Tab browser  j/k scroll  p play  n next  N prev  q quit",
        Style::default().fg(TEXT_MUTED),
    ));
    frame.render_widget(Paragraph::new(hints).style(Style::default().bg(BG)), rows[2]);
}
