use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;

// ── Top-level: 3-column Spotify-style bar ────────────────────────────────────

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // track info
            Constraint::Percentage(40), // transport controls
            Constraint::Percentage(30), // inline progress
        ])
        .split(area);

    render_track_info(app, frame, cols[0]);
    render_controls(app, frame, cols[1]);
    render_progress(app, frame, cols[2]);
}

// ── Left 30%: track title (accent/bold) + artist (muted) + quality tag ───────

fn render_track_info(app: &App, frame: &mut Frame, area: Rect) {
    let t = &app.theme;
    let lines: Vec<Line> = if let Some(song) = &app.playback.current_song {
        let artist = song.artist.as_deref().unwrap_or("Unknown Artist");
        // Quality tag: codec name for lossless, bitrate string otherwise.
        let quality = format_quality(song);
        let mut rows = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    song.title.as_str(),
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(artist, Style::default().fg(t.dimmed)),
            ]),
        ];
        if let Some(q) = quality {
            rows.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(q, Style::default().fg(t.dimmed)),
            ]));
        } else {
            rows.push(Line::from(""));
        }
        rows
    } else {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("Not playing", Style::default().fg(t.dimmed)),
            ]),
            Line::from(""),
            Line::from(""),
        ]
    };
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(t.surface)),
        area,
    );
}

/// Format the audio quality label for the now-playing bar.
/// Returns `None` when no quality info is available.
fn format_quality(song: &playterm_subsonic::Song) -> Option<String> {
    let lossless = song.suffix.as_deref()
        .map(|s| matches!(s.to_lowercase().as_str(), "flac" | "wav" | "alac" | "ape" | "aiff"))
        .unwrap_or(false);

    if lossless {
        let fmt = song.suffix.as_deref().unwrap_or("").to_uppercase();
        return Some(fmt);
    }
    song.bit_rate.map(|br| format!("{}kbps", br))
}

// ── Center 40%: transport controls, centered ─────────────────────────────────
//
// Target: ⇄      ⏮      ( ⏸ )      ⏭      ↻
//         4-6 spaces between each symbol; play/pause bracketed + accent.

fn render_controls(app: &App, frame: &mut Frame, area: Rect) {
    let t = &app.theme;
    let (play_label, play_style) = if app.playback.current_song.is_none() {
        ("▶", Style::default().fg(t.dimmed))
    } else if app.playback.paused {
        ("( ▶ )", Style::default().fg(t.accent).add_modifier(Modifier::BOLD))
    } else {
        ("( ⏸ )", Style::default().fg(t.accent).add_modifier(Modifier::BOLD))
    };

    let sep = Style::default().fg(t.dimmed);
    let controls = Line::from(vec![
        Span::styled("⇄", sep),
        Span::raw("      "),
        Span::styled("⏮", sep),
        Span::raw("      "),
        Span::styled(play_label, play_style),
        Span::raw("      "),
        Span::styled("⏭", sep),
        Span::raw("      "),
        Span::styled("↻", sep),
    ]);

    // Place controls on row 1 of 4 (1 blank row above for visual centering).
    let lines: Vec<Line> = vec![
        Line::from(""),
        controls,
        Line::from(""),
        Line::from(""),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(Style::default().bg(t.surface)),
        area,
    );
}

// ── Right 30%: inline progress "elapsed  ████░░░░  total" ────────────────────
//
// No Gauge widget — bar is built as a string of █ (ACCENT) and ░ (TEXT_MUTED)
// sized to fit the column width.  Placed on row 2 of 4.

fn render_progress(app: &App, frame: &mut Frame, area: Rect) {
    let t = &app.theme;
    let (elapsed_str, total_str, ratio) = if app.playback.current_song.is_some() {
        let e = app.playback.elapsed.as_secs();
        let elapsed_str = format!("{}:{:02}", e / 60, e % 60);
        let (total_str, ratio) = match app.playback.total {
            Some(tot) => {
                let ts = tot.as_secs();
                let r = if ts > 0 { (e as f64 / ts as f64).clamp(0.0, 1.0) } else { 0.0 };
                (format!("{}:{:02}", ts / 60, ts % 60), r)
            }
            None => ("--:--".to_string(), 0.0),
        };
        (elapsed_str, total_str, ratio)
    } else {
        ("0:00".to_string(), "0:00".to_string(), 0.0)
    };

    // Bar width: column width minus elapsed, total, and two 2-space gaps.
    let col_w = area.width as usize;
    let bar_w = col_w.saturating_sub(elapsed_str.len() + total_str.len() + 4);
    let filled = ((ratio * bar_w as f64) as usize).min(bar_w);
    let empty = bar_w - filled;

    let progress = Line::from(vec![
        Span::styled(elapsed_str, Style::default().fg(t.dimmed)),
        Span::raw("  "),
        Span::styled("█".repeat(filled), Style::default().fg(t.accent)),
        Span::styled("░".repeat(empty), Style::default().fg(t.dimmed)),
        Span::raw("  "),
        Span::styled(total_str, Style::default().fg(t.dimmed)),
    ]);

    // Row 0: empty, Row 1: empty, Row 2: progress, Row 3: empty.
    let lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(""),
        progress,
        Line::from(""),
    ];

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(t.surface)),
        area,
    );
}
