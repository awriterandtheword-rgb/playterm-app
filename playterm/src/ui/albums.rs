use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};

use crate::app::App;
use crate::state::LoadingState;
use playterm_subsonic::Album;

pub fn render(app: &App, frame: &mut Frame, area: Rect, is_active: bool) {
    let t = &app.theme;
    let border_color = if is_active { t.border_active } else { t.border };
    let title_color  = if is_active { t.accent }        else { t.dimmed };

    let block = Block::default()
        .title(" Albums ")
        .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(t.surface));

    let artist_id = match app.library.current_artist() {
        Some(a) => a.id.clone(),
        None => {
            let list = List::new(vec![
                ListItem::new("← Select an artist").style(Style::default().fg(t.dimmed)),
            ])
            .block(block);
            frame.render_widget(list, area);
            return;
        }
    };

    match app.library.albums.get(&artist_id) {
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
        Some(LoadingState::Loaded(albums)) => {
            let make_label = |a: &Album| match a.year {
                Some(y) => format!("{} ({})", a.name, y),
                None => a.name.clone(),
            };

            let visible: Vec<(usize, String)> = if let Some(q) = &app.search_filter {
                albums.iter().enumerate()
                    .filter(|(_, a)| a.name.to_lowercase().contains(q.as_str()))
                    .map(|(i, a)| (i, make_label(a)))
                    .collect()
            } else {
                albums.iter().enumerate().map(|(i, a)| (i, make_label(a))).collect()
            };

            let items: Vec<ListItem> = if visible.is_empty() {
                vec![ListItem::new("No matches").style(Style::default().fg(t.dimmed))]
            } else {
                visible.iter()
                    .map(|(_, label)| ListItem::new(label.as_str()).style(Style::default().fg(t.foreground)))
                    .collect()
            };

            let sel = app.library.selected_album
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
