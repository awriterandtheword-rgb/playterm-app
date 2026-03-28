use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use crate::app::App;
use super::{queue, BORDER, SURFACE, TEXT_MUTED};

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let cols = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);

    render_art_placeholder(frame, cols[0]);
    queue::render(app, frame, cols[1], true);
}

fn render_art_placeholder(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Album Art ")
        .title_style(Style::default().fg(TEXT_MUTED).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(SURFACE));
    frame.render_widget(block, area);
}
