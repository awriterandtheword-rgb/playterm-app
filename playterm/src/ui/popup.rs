//! Floating keybind reference popup.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

/// Width reserved for the key column (padded with spaces to align descriptions).
const KEY_COL_W: usize = 12;

fn sections() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        ("Navigation", vec![
            ("j / k",   "Scroll up / down"),
            ("h / l",   "Previous / next column (Browser)"),
            ("Tab",     "Switch tab"),
            ("/",       "Search current column"),
            ("Enter",   "Select / expand"),
        ]),
        ("Playback", vec![
            ("p / Space", "Play / pause"),
            ("n / N",     "Next / previous track"),
            ("x / Z",     "Shuffle / unshuffle"),
            ("\u{2190} / \u{2192}", "Seek \u{b1}10s"),
        ]),
        ("Queue", vec![
            ("a",       "Add track to queue"),
            ("A",       "On artist / album add to queue"),
            ("D",       "Clear queue"),
        ]),
        ("Volume & Display", vec![
            ("+ / -", "Volume up / down"),
            ("t",     "Toggle dynamic theme"),
            ("L",     "Toggle lyrics"),
        ]),
        ("App", vec![
            ("i", "Toggle this help"),
            ("q", "Quit (or close help)"),
        ]),
    ]
}

/// Render the keybind help popup centered over the current frame.
///
/// Call this last in the render pass so it layers on top of all other widgets.
pub fn render_help(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let t = &app.theme;

    let accent = app.accent();
    let fg     = t.foreground;
    let dim    = t.dimmed;
    let bg     = t.background;

    // ── Content ───────────────────────────────────────────────────────────────

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (si, (header, entries)) in sections().into_iter().enumerate() {
        if si > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            header,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
        for (key, desc) in entries {
            // Pad the key string to KEY_COL_W so descriptions align.
            let key_padded = format!("{:<width$}", key, width = KEY_COL_W);
            lines.push(Line::from(vec![
                Span::styled(key_padded, Style::default().fg(fg)),
                Span::styled(desc, Style::default().fg(dim)),
            ]));
        }
    }

    // ── Sizing & positioning ──────────────────────────────────────────────────

    let popup_w = (area.width * 60 / 100).max(40);
    let content_h = lines.len() as u16 + 2; // +2 for top/bottom border
    let max_h     = (area.height * 80 / 100).max(10);
    let popup_h   = content_h.min(max_h);

    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup_area = Rect::new(x, y, popup_w, popup_h);

    // ── Render ────────────────────────────────────────────────────────────────

    // Clear obscures content behind the popup.
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(" Keybinds ", Style::default().fg(accent)))
        .style(Style::default().bg(bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let para = Paragraph::new(Text::from(lines))
        .style(Style::default().bg(bg));
    frame.render_widget(para, inner);
}
