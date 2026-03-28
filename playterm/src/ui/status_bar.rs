use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use super::{ACCENT, TEXT_MUTED, BG};

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let line = if app.search_mode.active {
        Line::from(vec![
            Span::styled("/ ", Style::default().fg(ACCENT)),
            Span::styled(app.search_mode.query.as_str(), Style::default().fg(super::TEXT)),
            Span::styled("_", Style::default().fg(ACCENT)),
            Span::styled("   Enter confirm  Esc cancel", Style::default().fg(TEXT_MUTED)),
        ])
    } else {
        let host = app.config.subsonic_url
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        Line::from(vec![
            Span::styled("● ", Style::default().fg(ACCENT)),
            Span::styled(host, Style::default().fg(TEXT_MUTED)),
            Span::styled(
                "  h/l columns  j/k scroll  Tab switch  / search  a add  A add all  D clear  p play  n next  N prev  q quit",
                Style::default().fg(TEXT_MUTED),
            ),
        ])
    };

    let para = Paragraph::new(line).style(Style::default().bg(BG));
    frame.render_widget(para, area);
}
