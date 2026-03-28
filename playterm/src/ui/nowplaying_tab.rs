use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use super::queue;

use crate::app::App;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let cols = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);

    render_art_placeholder(app, frame, cols[0]);
    queue::render(app, frame, cols[1], true);
}

fn render_art_placeholder(app: &App, frame: &mut Frame, area: Rect) {
    let t = &app.theme;
    let block = Block::default()
        .title(" Album Art ")
        .title_style(Style::default().fg(t.dimmed).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(t.border))
        .style(Style::default().bg(t.surface));
    frame.render_widget(block, area);
}
