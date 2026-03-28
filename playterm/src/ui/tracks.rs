use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};

use crate::app::App;
use crate::state::LoadingState;

pub fn render(app: &App, frame: &mut Frame, area: Rect, is_active: bool) {
    let t = &app.theme;
    let border_color = if is_active { t.border_active } else { t.border };
    let title_color  = if is_active { t.accent }        else { t.dimmed };

    let title = app.library.current_album()
        .map(|a| format!(" {} ", a.name))
        .unwrap_or_else(|| " Tracks ".to_string());

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(t.surface));

    let album_id = match app.library.current_album() {
        Some(a) => a.id.clone(),
        None => {
            let list = List::new(vec![
                ListItem::new("← Select an album").style(Style::default().fg(t.dimmed)),
            ])
            .block(block);
            frame.render_widget(list, area);
            return;
        }
    };

    match app.library.tracks.get(&album_id) {
        None | Some(LoadingState::NotLoaded) | Some(LoadingState::Loading) => {
            let item = ListItem::new("Loading…").style(Style::default().fg(t.dimmed));
            let list = List::new(vec![item]).block(block);
            frame.render_widget(list, area);
        }
        Some(LoadingState::Error(e)) => {
            let item = ListItem::new(format!("Error: {e}")).style(Style::default().fg(t.accent));
            let list = List::new(vec![item]).block(block);
            frame.render_widget(list, area);
        }
        Some(LoadingState::Loaded(songs)) => {
            let make_label = |s: &playterm_subsonic::Song| {
                let num = s.track.map(|n| format!("{n:>2}. ")).unwrap_or_default();
                let dur = s.duration.map(|d| {
                    let m = d / 60;
                    let sec = d % 60;
                    format!("  {m}:{sec:02}")
                }).unwrap_or_default();
                format!("{}{}{}", num, s.title, dur)
            };

            let visible: Vec<(usize, String)> = if let Some(q) = &app.search_filter {
                songs.iter().enumerate()
                    .filter(|(_, s)| s.title.to_lowercase().contains(q.as_str()))
                    .map(|(i, s)| (i, make_label(s)))
                    .collect()
            } else {
                songs.iter().enumerate().map(|(i, s)| (i, make_label(s))).collect()
            };

            let items: Vec<ListItem> = if visible.is_empty() {
                vec![ListItem::new("No matches").style(Style::default().fg(t.dimmed))]
            } else {
                visible.iter()
                    .map(|(_, label)| ListItem::new(label.as_str()).style(Style::default().fg(t.foreground)))
                    .collect()
            };

            let sel = app.library.selected_track
                .and_then(|s| visible.iter().position(|(i, _)| *i == s));

            let list = List::new(items)
                .block(block)
                .highlight_style(
                    Style::default()
                        .bg(t.accent)
                        .fg(t.background)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ")
                .style(Style::default().bg(t.surface));

            let mut state = ListState::default();
            state.select(sel);
            frame.render_stateful_widget(list, area, &mut state);
        }
    }
}
