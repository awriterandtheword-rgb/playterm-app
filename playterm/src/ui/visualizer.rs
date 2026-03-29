//! Spectrum visualizer bar chart widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Unicode block characters for sub-row precision: ▁▂▃▄▅▆▇█
const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Render a spectrum visualizer into `area`.
///
/// `bands` must be a slice of 0.0–1.0 normalised band amplitudes.
/// Each bar is 1 column wide with a 1-column gap, so up to `area.width / 2`
/// bars are drawn, clamped to 8..=32.
///
/// Does nothing if all bands are zero (startup / turned off).
pub fn render_visualizer(f: &mut Frame, area: Rect, bands: &[f32], accent: Color) {
    if area.width == 0 || area.height == 0 || bands.is_empty() {
        return;
    }

    // Skip rendering when the visualizer was just toggled off and reset.
    if bands.iter().all(|&b| b == 0.0) {
        return;
    }

    let num_bars = ((area.width / 2) as usize)
        .min(32)
        .max(8)
        .min(bands.len());

    for i in 0..num_bars {
        // Map bar index to the corresponding band (bands always has 32 elements).
        let band_idx = i * bands.len() / num_bars;
        let band_val = bands[band_idx].clamp(0.0, 1.0);

        // Total height in units of 1/8 of a row.
        let total_units = (band_val * area.height as f32 * 8.0) as usize;
        let full_rows = (total_units / 8).min(area.height as usize);
        let partial_idx = total_units % 8;

        let col_x = area.x + (i * 2) as u16;
        if col_x >= area.x + area.width {
            break;
        }
        let bar_rect = Rect::new(col_x, area.y, 1, area.height);

        // Build lines from top to bottom.
        // Layout (top→bottom): empty rows, optional partial block, full-block rows.
        let has_partial = partial_idx > 0 && full_rows < area.height as usize;
        let top_empty = area.height as usize - full_rows - if has_partial { 1 } else { 0 };

        let mut lines: Vec<Line> = Vec::with_capacity(area.height as usize);

        for _ in 0..top_empty {
            lines.push(Line::from(" "));
        }
        if has_partial {
            let ch = BLOCKS[partial_idx - 1].to_string();
            lines.push(Line::from(Span::styled(ch, Style::default().fg(accent))));
        }
        for _ in 0..full_rows {
            lines.push(Line::from(Span::styled("█", Style::default().fg(accent))));
        }

        f.render_widget(Paragraph::new(lines), bar_rect);
    }
}
