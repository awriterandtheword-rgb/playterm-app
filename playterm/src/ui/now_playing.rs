use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph};

use crate::app::App;
use super::{ACCENT, BG, SURFACE, TEXT, TEXT_MUTED};

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // track title + artist
            Constraint::Length(1), // transport controls
            Constraint::Length(1), // progress gauge
        ])
        .split(area);

    render_track_info(app, frame, rows[0]);
    render_controls(app, frame, rows[1]);
    render_progress(app, frame, rows[2]);
}

fn render_track_info(app: &App, frame: &mut Frame, area: Rect) {
    let line = if let Some(song) = &app.playback.current_song {
        let artist = song.artist.as_deref().unwrap_or("Unknown Artist");
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                song.title.as_str(),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ·  ", Style::default().fg(TEXT_MUTED)),
            Span::styled(artist, Style::default().fg(TEXT_MUTED)),
        ])
    } else {
        Line::from(Span::styled("  Not playing", Style::default().fg(TEXT_MUTED)))
    };
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(SURFACE)),
        area,
    );
}

fn render_controls(app: &App, frame: &mut Frame, area: Rect) {
    let play_icon = if app.playback.paused || app.playback.current_song.is_none() {
        "▶"
    } else {
        "⏸"
    };
    let play_style = if app.playback.current_song.is_some() {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(TEXT_MUTED)
    };

    let line = Line::from(vec![
        Span::styled("⇄", Style::default().fg(TEXT_MUTED)),
        Span::styled("   ⏮   ", Style::default().fg(TEXT_MUTED)),
        Span::styled(play_icon, play_style),
        Span::styled("   ⏭   ", Style::default().fg(TEXT_MUTED)),
        Span::styled("↻", Style::default().fg(TEXT_MUTED)),
    ]);

    frame.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .style(Style::default().bg(SURFACE)),
        area,
    );
}

fn render_progress(app: &App, frame: &mut Frame, area: Rect) {
    let ratio = match (&app.playback.current_song, app.playback.total) {
        (Some(_), Some(total)) if !total.is_zero() => {
            (app.playback.elapsed.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };

    let elapsed = app.playback.elapsed.as_secs();
    let label = match app.playback.total {
        Some(t) => {
            let ts = t.as_secs();
            format!(
                " {}:{:02} / {}:{:02} ",
                elapsed / 60,
                elapsed % 60,
                ts / 60,
                ts % 60
            )
        }
        None => format!(" {}:{:02} ", elapsed / 60, elapsed % 60),
    };

    let gauge = Gauge::default()
        .style(Style::default().bg(SURFACE))
        .gauge_style(Style::default().bg(ACCENT).fg(BG))
        .label(label)
        .ratio(ratio);
    frame.render_widget(gauge, area);
}
