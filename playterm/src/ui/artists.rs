use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};

use crate::app::App;
use crate::state::LoadingState;
use super::{ACCENT, BG, BORDER, BORDER_ACTIVE, SURFACE, TEXT, TEXT_MUTED};

pub fn render(app: &App, frame: &mut Frame, area: Rect, is_active: bool) {
    let border_color = if is_active { BORDER_ACTIVE } else { BORDER };
    let title_color = if is_active { ACCENT } else { TEXT_MUTED };

    let block = Block::default()
        .title(" Artists ")
        .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(SURFACE));

    match &app.library.artists {
        LoadingState::NotLoaded | LoadingState::Loading => {
            let item = ListItem::new("Loading…").style(Style::default().fg(TEXT_MUTED));
            let list = List::new(vec![item]).block(block).style(Style::default().bg(BG));
            frame.render_widget(list, area);
        }
        LoadingState::Error(e) => {
            let item = ListItem::new(format!("Error: {e}")).style(Style::default().fg(ACCENT));
            let list = List::new(vec![item]).block(block).style(Style::default().bg(BG));
            frame.render_widget(list, area);
        }
        LoadingState::Loaded(artists) => {
            let items: Vec<ListItem> = artists
                .iter()
                .map(|a| ListItem::new(a.name.as_str()).style(Style::default().fg(TEXT)))
                .collect();

            let list = List::new(items)
                .block(block)
                .highlight_style(
                    Style::default()
                        .bg(ACCENT)
                        .fg(BG)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ")
                .style(Style::default().bg(SURFACE));

            let mut state = ListState::default();
            state.select(app.library.selected_artist);
            frame.render_stateful_widget(list, area, &mut state);
        }
    }
}
