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
/// `bands` must be a slice of 0.0–1.0 normalised band amplitudes (typically 32
/// elements).  Each bar occupies 1 column; bars are distributed evenly across
/// the full `area` width using floating-point stepping so the rightmost bar
/// always lands near the right edge regardless of terminal width.
///
/// The number of rendered bars is `min(bands.len(), 32, area.width / 2)` —
/// bars are never closer together than 2 columns (1 bar + 1 gap minimum).
///
/// Does nothing if all bands are zero (startup / toggled off).
pub fn render_visualizer(f: &mut Frame, area: Rect, bands: &[f32], accent: Color) {
    if area.width == 0 || area.height == 0 || bands.is_empty() {
        return;
    }

    // Skip rendering when the visualizer was just toggled off and reset.
    if bands.iter().all(|&b| b == 0.0) {
        return;
    }

    // How many bars to draw:
    //   • At most bands.len() (can't show more data than we have).
    //   • At most 32 (visual upper bound; above this bars become too thin).
    //   • At most area.width / 2 (guarantees minimum 2-col pitch: 1 bar + 1 gap).
    //   • At least 1 (avoid division by zero below).
    let max_bars = bands.len().min(32);
    let num_bars = max_bars.min((area.width / 2) as usize).max(1);

    // Floating-point step distributes all `num_bars` bars evenly across the
    // full area width, so the last bar sits near the right edge even when the
    // area is wider than num_bars * 2 columns.
    let bar_step_f = area.width as f32 / num_bars as f32;

    for i in 0..num_bars {
        // Map bar index → band value.
        let band_idx = i * bands.len() / num_bars;
        let band_val = bands[band_idx].clamp(0.0, 1.0);

        // Column position — clamped so we never write outside area.
        let col_x = area.x + (i as f32 * bar_step_f) as u16;
        if col_x >= area.x + area.width {
            break;
        }
        let bar_rect = Rect::new(col_x, area.y, 1, area.height);

        // Total height in units of 1/8 of a row.
        let total_units = (band_val * area.height as f32 * 8.0) as usize;
        let full_rows = (total_units / 8).min(area.height as usize);
        let partial_idx = total_units % 8;

        // Build lines top→bottom: empty rows, optional partial block, full-block rows.
        let has_partial = partial_idx > 0 && full_rows < area.height as usize;
        let top_empty =
            area.height as usize - full_rows - if has_partial { 1 } else { 0 };

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
