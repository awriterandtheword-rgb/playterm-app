use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};

use crate::app::App;
use super::{ACCENT, BG, BORDER, BORDER_ACTIVE, SURFACE, TEXT, TEXT_MUTED};

pub fn render(app: &App, frame: &mut Frame, area: Rect, is_active: bool) {
    let border_color = if is_active { BORDER_ACTIVE } else { BORDER };
    let title_color = if is_active { ACCENT } else { TEXT_MUTED };

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
        .style(Style::default().bg(SURFACE));

    if app.queue.songs.is_empty() {
        let item = ListItem::new("Queue is empty — press 'a' to add tracks")
            .style(Style::default().fg(TEXT_MUTED));
        let list = List::new(vec![item]).block(block);
        frame.render_widget(list, area);
        return;
    }

    let cursor = app.queue.cursor;
    let items: Vec<ListItem> = app.queue.songs.iter().enumerate().map(|(i, s)| {
        let marker = if i == cursor { "▶ " } else { "  " };
        let artist = s.artist.as_deref().unwrap_or("");
        let dur = s.duration.map(|d| {
            let m = d / 60;
            let sec = d % 60;
            format!("  {m}:{sec:02}")
        }).unwrap_or_default();
        let label = format!("{}{} — {}{}", marker, s.title, artist, dur);
        let style = if i == cursor {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT)
        };
        ListItem::new(label).style(style)
    }).collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(ACCENT).fg(BG).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(SURFACE));

    let mut state = ListState::default().with_offset(app.queue.scroll);
    state.select(Some(app.queue.cursor));
    frame.render_stateful_widget(list, area, &mut state);
}
