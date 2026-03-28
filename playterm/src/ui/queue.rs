use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};

use crate::app::App;

pub fn render(app: &App, frame: &mut Frame, area: Rect, is_active: bool) {
    let t = &app.theme;
    let border_color = if is_active { t.border_active } else { t.border };
    let title_color  = if is_active { t.accent }        else { t.dimmed };

    let count = app.queue.songs.len();
    let title = if count == 0 {
        " Queue ".to_string()
    } else {
        format!(" Queue ({count}) ")
    };

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(t.surface));

    if app.queue.songs.is_empty() {
        let item = ListItem::new("Queue is empty — press 'a' to add tracks")
            .style(Style::default().fg(t.dimmed));
        let list = List::new(vec![item]).block(block);
        frame.render_widget(list, area);
        return;
    }

    let items: Vec<ListItem> = app.queue.songs.iter().enumerate().map(|(i, s)| {
        // Track number: 4 chars right-aligned ("  1.")
        let num = s.track
            .map(|n| format!("{n:>3}."))
            .unwrap_or_else(|| "    ".to_string());

        // Title: 40 chars, left-aligned, truncated with …
        let title_col = format!("{:<40}", trunc(&s.title, 40));

        // Artist: 25 chars, left-aligned, truncated with …
        let artist_col = format!("{:<25}", trunc(s.artist.as_deref().unwrap_or(""), 25));

        // Duration: 5 chars right-aligned (" 3:04")
        let dur = s.duration
            .map(|d| format!("{:>5}", format!("{}:{:02}", d / 60, d % 60)))
            .unwrap_or_else(|| "     ".to_string());

        let label = format!("{}  {}  {}  {}", num, title_col, artist_col, dur);

        let style = if i == app.queue.cursor {
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.foreground)
        };
        ListItem::new(label).style(style)
    }).collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().fg(t.accent).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(t.surface));

    let mut state = ListState::default().with_offset(app.queue.scroll);
    state.select(Some(app.queue.cursor));
    frame.render_stateful_widget(list, area, &mut state);
}

/// Truncate `s` to at most `max` Unicode characters, appending `…` if cut.
fn trunc(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let mut result = String::with_capacity(max);
    let mut count = 0;
    for ch in chars.by_ref() {
        if count >= max - 1 {
            // Check if there are more characters coming.
            if chars.next().is_some() {
                result.push('…');
            } else {
                result.push(ch);
            }
            return result;
        }
        result.push(ch);
        count += 1;
    }
    result
}
