use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;

// ── Key-legend helpers ────────────────────────────────────────────────────────

/// Build the spans for a row of keybind groups, truncating to `max_width`.
/// `binds` is `&[(key_label, action_label)]` with owned key strings.
/// Format: " key Action │ key Action │ …"
fn build_legend(
    binds: &[(String, &'static str)],
    max_width: u16,
    key_style:    ratatui::style::Style,
    action_style: ratatui::style::Style,
    sep_style:    ratatui::style::Style,
) -> Vec<Span<'static>> {
    let sep = " │ ";
    let n = binds.len();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used: u16 = 1; // leading space

    spans.push(Span::raw(" "));

    for (i, (key, action)) in binds.iter().enumerate() {
        let is_last = i + 1 == n;
        let chunk_w = (key.len() + 1 + action.len()) as u16
            + if is_last { 0 } else { sep.len() as u16 };

        if used + chunk_w > max_width {
            break;
        }

        // key is an owned String — Span::styled accepts String → Cow::Owned<'static>.
        spans.push(Span::styled(key.clone(), key_style));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*action, action_style));

        if !is_last {
            used += chunk_w;
            spans.push(Span::styled(sep, sep_style));
        }
    }

    spans
}

// ── Public render ─────────────────────────────────────────────────────────────

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let t  = &app.theme;
    let kb = &app.keybinds;

    let line = if app.search_mode.active {
        Line::from(vec![
            Span::styled("/ ", Style::default().fg(t.accent)),
            Span::styled(app.search_mode.query.as_str(), Style::default().fg(t.foreground)),
            Span::styled("_", Style::default().fg(t.accent)),
            Span::raw("   "),
            Span::styled("Enter", Style::default().fg(t.dimmed)),
            Span::raw(" "),
            Span::styled("Confirm", Style::default().fg(t.accent)),
            Span::styled("  │  ", Style::default().fg(t.dimmed)),
            Span::styled("Esc", Style::default().fg(t.dimmed)),
            Span::raw(" "),
            Span::styled("Cancel", Style::default().fg(t.accent)),
        ])
    } else {
        let host = app.config.subsonic_url
            .trim_start_matches("http://")
            .trim_start_matches("https://");

        // Build key labels from the actual configured keybinds.
        let col_nav = format!("{}/{}", kb.column_left.display(), kb.column_right.display());
        let scroll  = format!("{}/{}", kb.scroll_up.display(), kb.scroll_down.display());
        let binds: Vec<(String, &'static str)> = vec![
            (col_nav,                    "Columns"),
            (scroll,                     "Scroll"),
            (kb.tab_switch.display(),    "Switch"),
            (kb.search.display(),        "Search"),
            (kb.add_track.display(),     "Add"),
            (kb.add_all.display(),       "Add All"),
            (kb.clear_queue.display(),   "Clear"),
            (kb.play_pause.display(),    "Play"),
            (kb.next_track.display(),    "Next"),
            (kb.prev_track.display(),    "Prev"),
            (kb.quit.display(),          "Quit"),
        ];

        let host_span_w = (2 + host.len()) as u16; // "● " + host
        let legend_w = area.width.saturating_sub(host_span_w);

        let key_style    = Style::default().fg(t.dimmed);
        let action_style = Style::default().fg(t.accent);
        let sep_style    = Style::default().fg(t.dimmed);

        let mut spans: Vec<Span> = vec![
            Span::styled("● ", Style::default().fg(t.accent)),
            Span::styled(host.to_string(), Style::default().fg(t.dimmed)),
        ];
        spans.extend(build_legend(&binds, legend_w, key_style, action_style, sep_style));

        Line::from(spans)
    };

    let para = Paragraph::new(line).style(Style::default().bg(t.background));
    frame.render_widget(para, area);
}
