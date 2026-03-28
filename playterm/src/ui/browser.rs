use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use crate::app::{App, BrowserColumn};
use super::{artists, albums, tracks};

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let cols = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(35),
        Constraint::Percentage(35),
    ])
    .split(area);

    artists::render(app, frame, cols[0], matches!(app.browser_focus, BrowserColumn::Artists));
    albums::render(app, frame, cols[1], matches!(app.browser_focus, BrowserColumn::Albums));
    tracks::render(app, frame, cols[2], matches!(app.browser_focus, BrowserColumn::Tracks));
}
