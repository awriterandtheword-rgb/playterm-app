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

    let title = app.library.current_album()
        .map(|a| format!(" {} ", a.name))
        .unwrap_or_else(|| " Tracks ".to_string());

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(SURFACE));

    let album_id = match app.library.current_album() {
        Some(a) => a.id.clone(),
        None => {
            let list = List::new(vec![
                ListItem::new("← Select an album").style(Style::default().fg(TEXT_MUTED)),
            ])
            .block(block);
            frame.render_widget(list, area);
            return;
        }
    };

    match app.library.tracks.get(&album_id) {
        None | Some(LoadingState::NotLoaded) | Some(LoadingState::Loading) => {
            let item = ListItem::new("Loading…").style(Style::default().fg(TEXT_MUTED));
            let list = List::new(vec![item]).block(block);
            frame.render_widget(list, area);
        }
        Some(LoadingState::Error(e)) => {
            let item = ListItem::new(format!("Error: {e}")).style(Style::default().fg(ACCENT));
            let list = List::new(vec![item]).block(block);
            frame.render_widget(list, area);
        }
        Some(LoadingState::Loaded(songs)) => {
            let items: Vec<ListItem> = songs
                .iter()
                .map(|s| {
                    let num = s.track.map(|n| format!("{n:>2}. ")).unwrap_or_default();
                    let dur = s.duration.map(|d| {
                        let m = d / 60;
                        let s = d % 60;
                        format!("  {m}:{s:02}")
                    }).unwrap_or_default();
                    let label = format!("{}{}{}", num, s.title, dur);
                    ListItem::new(label).style(Style::default().fg(TEXT))
                })
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
            state.select(app.library.selected_track);
            frame.render_stateful_widget(list, area, &mut state);
        }
    }
}
