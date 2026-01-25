//! Tooltip rendering for dashboard hover interactions.
//!
//! Displays contextual information when hovering over info icons.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::view_state::Theme;

/// Render a tooltip at the specified anchor position.
///
/// The tooltip appears as a small bordered box with the content text.
/// It positions itself relative to the anchor point to avoid clipping.
///
/// # Arguments
/// * `buf` - The ratatui buffer to render into
/// * `content` - The tooltip text content
/// * `anchor_x` - X coordinate of the anchor point (typically the info icon)
/// * `anchor_y` - Y coordinate of the anchor point
/// * `theme` - The current theme for styling
pub fn render_tooltip(buf: &mut Buffer, content: &str, anchor_x: u16, anchor_y: u16, theme: &Theme) {
    // Calculate tooltip dimensions
    let content_width = content.len() as u16;
    let tooltip_width = (content_width + 4).min(60); // Padding + max width
    let tooltip_height = 3; // Border + content + border

    // Position tooltip to the right of anchor, or left if too close to edge
    let buf_width = buf.area.width;
    let buf_height = buf.area.height;

    let x = if anchor_x + tooltip_width + 2 < buf_width {
        anchor_x + 2 // To the right
    } else {
        anchor_x.saturating_sub(tooltip_width + 2) // To the left
    };

    let y = if anchor_y + tooltip_height < buf_height {
        anchor_y // Below anchor
    } else {
        anchor_y.saturating_sub(tooltip_height) // Above anchor
    };

    // Ensure tooltip fits within buffer bounds
    let x = x.min(buf_width.saturating_sub(tooltip_width));
    let y = y.min(buf_height.saturating_sub(tooltip_height));

    let tooltip_rect = Rect::new(x, y, tooltip_width, tooltip_height);

    // Create tooltip widget
    let tooltip_style = Style::default()
        .fg(theme.dim)
        .bg(theme.border);

    let border_style = Style::default()
        .fg(theme.accent)
        .bg(theme.border);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(tooltip_style);

    let paragraph = Paragraph::new(Line::from(content))
        .block(block)
        .style(tooltip_style);

    paragraph.render(tooltip_rect, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn test_render_tooltip_basic() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = Theme::default();

        render_tooltip(&mut buffer, "Test tooltip", 10, 5, &theme);

        // Verify tooltip was rendered (basic smoke test)
        // The tooltip should appear somewhere around (10, 5)
        let cell = buffer.cell((12, 6)).unwrap(); // Inside expected tooltip area
        assert_ne!(cell.symbol(), " "); // Should have some content
    }

    #[test]
    fn test_render_tooltip_near_right_edge() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 20));
        let theme = Theme::default();

        // Anchor near right edge - tooltip should flip to the left
        render_tooltip(&mut buffer, "Edge tooltip", 35, 10, &theme);

        // Tooltip should still fit within bounds
        // This is a basic smoke test - detailed positioning tested manually
    }

    #[test]
    fn test_render_tooltip_near_bottom_edge() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 15));
        let theme = Theme::default();

        // Anchor near bottom edge - tooltip should flip upward
        render_tooltip(&mut buffer, "Bottom tooltip", 20, 13, &theme);

        // Tooltip should still fit within bounds
    }
}
