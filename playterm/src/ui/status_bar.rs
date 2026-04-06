use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;

// ── Public render ─────────────────────────────────────────────────────────────

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let t = &app.theme;

    let line = if app.search_mode.active {
        Line::from(vec![
            Span::styled("/ ", Style::default().fg(app.accent())),
            Span::styled(app.search_mode.query.as_str(), Style::default().fg(t.foreground)),
            Span::styled("_", Style::default().fg(app.accent())),
            Span::raw("   "),
            Span::styled("Enter", Style::default().fg(t.dimmed)),
            Span::raw(" "),
            Span::styled("Confirm", Style::default().fg(app.accent())),
            Span::styled("  │  ", Style::default().fg(t.dimmed)),
            Span::styled("Esc", Style::default().fg(t.dimmed)),
            Span::raw(" "),
            Span::styled("Cancel", Style::default().fg(app.accent())),
        ])
    } else if let Some((msg, _)) = &app.status_flash {
        // Flash message: left-aligned, truncated to the bar width (centred long
        // strings overflow and corrupt the TUI layout).
        let w = area.width as usize;
        let shown = fit_status_bar_text(msg, w);
        Line::from(vec![Span::styled(
            shown,
            Style::default().fg(app.accent()),
        )])
    } else {
        let host = app.config.subsonic_url
            .trim_start_matches("http://")
            .trim_start_matches("https://");

        let hint = "i — help";
        let host_w = 2 + host.len(); // "● " + host
        let gap = (area.width as usize).saturating_sub(host_w + hint.len());

        Line::from(vec![
            Span::styled("● ", Style::default().fg(app.accent())),
            Span::styled(host.to_string(), Style::default().fg(t.dimmed)),
            Span::raw(" ".repeat(gap)),
            Span::styled(hint, Style::default().fg(t.dimmed)),
        ])
    };

    let para = Paragraph::new(line).style(Style::default().bg(t.background));
    frame.render_widget(para, area);
}

/// Truncate `s` to at most `max_cols` Unicode scalars (status bar is one row).
fn fit_status_bar_text(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    let n = s.chars().count();
    if n <= max_cols {
        return s.to_string();
    }
    if max_cols <= 1 {
        return "…".to_string();
    }
    s.chars().take(max_cols - 1).collect::<String>() + "…"
}
