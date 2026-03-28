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

    let block = Block::default()
        .title(" Artists ")
        .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(t.surface));

    match &app.library.artists {
        LoadingState::NotLoaded | LoadingState::Loading => {
            let item = ListItem::new("Loading…").style(Style::default().fg(t.dimmed));
            let list = List::new(vec![item]).block(block).style(Style::default().bg(t.background));
            frame.render_widget(list, area);
        }
        LoadingState::Error(e) => {
            let item = ListItem::new(format!("Error: {e}")).style(Style::default().fg(t.accent));
            let list = List::new(vec![item]).block(block).style(Style::default().bg(t.background));
            frame.render_widget(list, area);
        }
        LoadingState::Loaded(artists) => {
            // Build (original_index, name) pairs, filtered when search is active.
            let visible: Vec<(usize, &str)> = if let Some(q) = &app.search_filter {
                artists.iter().enumerate()
                    .filter(|(_, a)| a.name.to_lowercase().contains(q.as_str()))
                    .map(|(i, a)| (i, a.name.as_str()))
                    .collect()
            } else {
                artists.iter().enumerate().map(|(i, a)| (i, a.name.as_str())).collect()
            };

            let items: Vec<ListItem> = if visible.is_empty() {
                vec![ListItem::new("No matches").style(Style::default().fg(t.dimmed))]
            } else {
                visible.iter()
                    .map(|(_, name)| ListItem::new(*name).style(Style::default().fg(t.foreground)))
                    .collect()
            };

            // Find where the currently selected artist sits in the visible list.
            let sel = app.library.selected_artist
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
