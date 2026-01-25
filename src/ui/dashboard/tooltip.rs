//! Floating tooltip popup component
//!
//! Renders a floating popup that appears above an info icon when hovered.
//! The tooltip displays contextual information about permissions or other
//! actions requiring user attention.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

use crate::view_state::Theme;

// ============================================================================
// Constants
// ============================================================================

/// Maximum width for tooltip content (not including border)
const MAX_CONTENT_WIDTH: u16 = 40;

/// Padding on each side of content
const HORIZONTAL_PADDING: u16 = 1;

/// Border width (1 on each side)
const BORDER_WIDTH: u16 = 2;

/// Height of content area (single line)
const CONTENT_HEIGHT: u16 = 1;

/// Total height including borders
const TOTAL_HEIGHT: u16 = CONTENT_HEIGHT + 2; // 1 top border + 1 content + 1 bottom border

// ============================================================================
// Public API
// ============================================================================

/// Render a floating tooltip popup above the anchor point
///
/// The tooltip is rendered with a border and background, positioned above
/// the anchor point (the info icon location). If the tooltip would go above
/// the top of the screen, it flips to below the anchor.
///
/// # Arguments
/// * `buf` - The buffer to render into
/// * `content` - The tooltip text content
/// * `anchor_x` - X coordinate of the anchor point (info icon)
/// * `anchor_y` - Y coordinate of the anchor point (info icon)
/// * `theme` - Theme colors for styling
pub fn render_tooltip(buf: &mut Buffer, content: &str, anchor_x: u16, anchor_y: u16, theme: &Theme) {
    let terminal_area = buf.area;

    // Calculate tooltip dimensions
    let dimensions = calculate_dimensions(content);

    // Calculate position (above anchor, clamped to terminal bounds)
    let position = calculate_position(
        anchor_x,
        anchor_y,
        dimensions.width,
        dimensions.height,
        terminal_area,
    );

    // Create the tooltip rect
    let tooltip_rect = Rect::new(position.x, position.y, dimensions.width, dimensions.height);

    // Render the tooltip
    render_tooltip_box(buf, tooltip_rect, content, theme);
}

// ============================================================================
// Internal Types
// ============================================================================

/// Calculated dimensions for a tooltip
struct TooltipDimensions {
    width: u16,
    height: u16,
}

/// Calculated position for a tooltip
struct TooltipPosition {
    x: u16,
    y: u16,
}

// ============================================================================
// Calculation Functions
// ============================================================================

/// Calculate tooltip dimensions based on content
///
/// Returns the total width and height of the tooltip including borders.
fn calculate_dimensions(content: &str) -> TooltipDimensions {
    // Content width: char count, clamped to max
    let content_chars: usize = content.chars().count();
    let content_width = (content_chars as u16).min(MAX_CONTENT_WIDTH);

    // Total width: content + padding on each side + border on each side
    let total_width = content_width + (HORIZONTAL_PADDING * 2) + BORDER_WIDTH;

    TooltipDimensions {
        width: total_width,
        height: TOTAL_HEIGHT,
    }
}

/// Calculate tooltip position, ensuring it stays within terminal bounds
///
/// The tooltip is positioned above the anchor point by default (anchor_y - 2 for border + text).
/// If it would go off-screen, it's adjusted to stay visible.
fn calculate_position(
    anchor_x: u16,
    anchor_y: u16,
    tooltip_width: u16,
    tooltip_height: u16,
    terminal_area: Rect,
) -> TooltipPosition {
    // Horizontal positioning: try to center on anchor, clamp to bounds
    let half_width = tooltip_width / 2;
    let ideal_x = anchor_x.saturating_sub(half_width);
    let max_x = terminal_area.x + terminal_area.width.saturating_sub(tooltip_width);
    let x = ideal_x.max(terminal_area.x).min(max_x);

    // Vertical positioning: above anchor by default
    // Check if there's enough room above the anchor for the tooltip
    let can_fit_above = anchor_y >= terminal_area.y + tooltip_height;

    let y = if can_fit_above {
        // Position so bottom of tooltip is 1 row above anchor
        anchor_y - tooltip_height
    } else {
        // Position below anchor (anchor_y + 1 to leave space for the icon row)
        (anchor_y + 1).min(terminal_area.y + terminal_area.height.saturating_sub(tooltip_height))
    };

    TooltipPosition { x, y }
}

// ============================================================================
// Rendering Functions
// ============================================================================

/// Render the tooltip box with border, background, and content
fn render_tooltip_box(buf: &mut Buffer, rect: Rect, content: &str, theme: &Theme) {
    // Don't render if rect is too small or outside buffer
    if rect.width < 3 || rect.height < 3 {
        return;
    }

    // Check buffer bounds
    let buf_area = buf.area;
    if rect.x >= buf_area.x + buf_area.width || rect.y >= buf_area.y + buf_area.height {
        return;
    }

    // Styles
    let border_style = Style::default().fg(theme.border);
    let bg_style = Style::default().bg(Color::Black);
    let text_style = Style::default().fg(theme.accent).bg(Color::Black);

    // Fill background
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            if x < buf_area.x + buf_area.width && y < buf_area.y + buf_area.height {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style);
                }
            }
        }
    }

    // Draw border
    draw_border(buf, rect, border_style, buf_area);

    // Draw content (centered in the content area)
    let content_x = rect.x + HORIZONTAL_PADDING + 1; // +1 for left border
    let content_y = rect.y + 1; // +1 for top border
    let max_chars = (rect.width.saturating_sub(BORDER_WIDTH + HORIZONTAL_PADDING * 2)) as usize;

    let truncated_content = if content.chars().count() > max_chars {
        let chars: String = content.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", chars)
    } else {
        content.to_string()
    };

    // Render content characters
    for (i, ch) in truncated_content.chars().enumerate() {
        let x = content_x + i as u16;
        if x < buf_area.x + buf_area.width && content_y < buf_area.y + buf_area.height {
            if let Some(cell) = buf.cell_mut((x, content_y)) {
                cell.set_char(ch).set_style(text_style);
            }
        }
    }
}

/// Draw box border using box-drawing characters
fn draw_border(buf: &mut Buffer, rect: Rect, style: Style, buf_area: Rect) {
    let x1 = rect.x;
    let x2 = rect.x + rect.width.saturating_sub(1);
    let y1 = rect.y;
    let y2 = rect.y + rect.height.saturating_sub(1);

    // Helper to set cell if in bounds
    let set_cell = |buf: &mut Buffer, x: u16, y: u16, ch: char| {
        if x < buf_area.x + buf_area.width && y < buf_area.y + buf_area.height {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch).set_style(style);
            }
        }
    };

    // Corners
    set_cell(buf, x1, y1, '\u{250C}'); // Top-left
    set_cell(buf, x2, y1, '\u{2510}'); // Top-right
    set_cell(buf, x1, y2, '\u{2514}'); // Bottom-left
    set_cell(buf, x2, y2, '\u{2518}'); // Bottom-right

    // Horizontal lines
    for x in (x1 + 1)..x2 {
        if x < buf_area.x + buf_area.width {
            if let Some(cell) = buf.cell_mut((x, y1)) {
                cell.set_char('\u{2500}').set_style(style); // Top
            }
            if let Some(cell) = buf.cell_mut((x, y2)) {
                cell.set_char('\u{2500}').set_style(style); // Bottom
            }
        }
    }

    // Vertical lines
    for y in (y1 + 1)..y2 {
        if y < buf_area.y + buf_area.height {
            if let Some(cell) = buf.cell_mut((x1, y)) {
                cell.set_char('\u{2502}').set_style(style); // Left
            }
            if let Some(cell) = buf.cell_mut((x2, y)) {
                cell.set_char('\u{2502}').set_style(style); // Right
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_theme() -> Theme {
        Theme::default()
    }

    // -------------------- Dimension Calculation Tests --------------------

    #[test]
    fn test_calculate_dimensions_short_content() {
        let content = "Hello";
        let dims = calculate_dimensions(content);

        // 5 chars + 2 padding + 2 border = 9 width
        assert_eq!(dims.width, 5 + 2 + 2);
        assert_eq!(dims.height, TOTAL_HEIGHT);
    }

    #[test]
    fn test_calculate_dimensions_max_width() {
        let content = "This is a very long tooltip content that exceeds the maximum allowed width";
        let dims = calculate_dimensions(content);

        // Should be clamped to MAX_CONTENT_WIDTH + padding + border
        assert_eq!(dims.width, MAX_CONTENT_WIDTH + 2 + 2);
        assert_eq!(dims.height, TOTAL_HEIGHT);
    }

    #[test]
    fn test_calculate_dimensions_empty_content() {
        let content = "";
        let dims = calculate_dimensions(content);

        // 0 chars + 2 padding + 2 border = 4 width
        assert_eq!(dims.width, 0 + 2 + 2);
        assert_eq!(dims.height, TOTAL_HEIGHT);
    }

    #[test]
    fn test_calculate_dimensions_exact_max() {
        // Create content exactly at max width
        let content: String = "x".repeat(MAX_CONTENT_WIDTH as usize);
        let dims = calculate_dimensions(&content);

        assert_eq!(dims.width, MAX_CONTENT_WIDTH + 2 + 2);
    }

    // -------------------- Position Calculation Tests --------------------

    #[test]
    fn test_calculate_position_above_anchor() {
        let terminal = Rect::new(0, 0, 100, 50);
        let pos = calculate_position(50, 20, 20, 3, terminal);

        // Should be above anchor (20 - 3 = 17)
        assert_eq!(pos.y, 17);
        // Should be horizontally centered (50 - 10 = 40)
        assert_eq!(pos.x, 40);
    }

    #[test]
    fn test_calculate_position_clamp_top() {
        let terminal = Rect::new(0, 0, 100, 50);
        // Anchor near top of screen
        let pos = calculate_position(50, 2, 20, 3, terminal);

        // Should flip to below anchor since it would go above terminal
        assert_eq!(pos.y, 3); // anchor_y + 1
    }

    #[test]
    fn test_calculate_position_clamp_left() {
        let terminal = Rect::new(0, 0, 100, 50);
        // Anchor near left edge
        let pos = calculate_position(5, 20, 20, 3, terminal);

        // Should be clamped to left edge (0)
        assert_eq!(pos.x, 0);
    }

    #[test]
    fn test_calculate_position_clamp_right() {
        let terminal = Rect::new(0, 0, 100, 50);
        // Anchor near right edge
        let pos = calculate_position(95, 20, 20, 3, terminal);

        // Should be clamped so tooltip stays in bounds (100 - 20 = 80)
        assert_eq!(pos.x, 80);
    }

    #[test]
    fn test_calculate_position_with_offset_terminal() {
        // Terminal area that doesn't start at origin
        let terminal = Rect::new(10, 5, 80, 40);
        let pos = calculate_position(50, 25, 20, 3, terminal);

        // Should respect terminal bounds
        assert!(pos.x >= terminal.x);
        assert!(pos.x + 20 <= terminal.x + terminal.width);
    }

    // -------------------- Full Render Tests --------------------

    #[test]
    fn test_render_tooltip_basic() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        render_tooltip(&mut buffer, "Test tooltip", 40, 12, &theme);

        // Verify tooltip was rendered (check for border characters)
        // Find the top-left corner character
        let mut found_corner = false;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "\u{250C}" {
                        found_corner = true;
                        break;
                    }
                }
            }
        }
        assert!(found_corner, "Tooltip border corner not found");
    }

    #[test]
    fn test_render_tooltip_content_visible() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        render_tooltip(&mut buffer, "Hello", 40, 12, &theme);

        // Verify content characters are present
        let mut found_h = false;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "H" {
                        found_h = true;
                        break;
                    }
                }
            }
        }
        assert!(found_h, "Tooltip content 'H' not found");
    }

    #[test]
    fn test_render_tooltip_truncation() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        let long_content = "This is a very long tooltip that should be truncated with ellipsis at the end";

        render_tooltip(&mut buffer, long_content, 40, 12, &theme);

        // Verify ellipsis is present (content was truncated)
        let mut found_ellipsis = false;
        for y in 0..24 {
            for x in 0..78 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "." {
                        // Check if this might be part of "..."
                        if let (Some(c1), Some(c2)) = (buffer.cell((x + 1, y)), buffer.cell((x + 2, y))) {
                            if c1.symbol() == "." && c2.symbol() == "." {
                                found_ellipsis = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
        assert!(found_ellipsis, "Truncation ellipsis not found");
    }

    #[test]
    fn test_render_tooltip_near_top_edge() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        // Anchor at y=1, tooltip should appear below
        render_tooltip(&mut buffer, "Test", 40, 1, &theme);

        // Should still render (below the anchor)
        let mut found_corner = false;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "\u{250C}" {
                        found_corner = true;
                        // Verify it's below anchor (y > 1)
                        assert!(y > 1, "Tooltip should be below anchor when near top");
                        break;
                    }
                }
            }
        }
        assert!(found_corner, "Tooltip border not found");
    }

    #[test]
    fn test_render_tooltip_near_left_edge() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        // Anchor at x=2
        render_tooltip(&mut buffer, "Test tooltip content", 2, 12, &theme);

        // Should still render (clamped to left edge)
        // Check that left border is at x=0 (clamped)
        let mut found_at_edge = false;
        for y in 0..24 {
            if let Some(cell) = buffer.cell((0, y)) {
                let sym = cell.symbol();
                if sym == "\u{250C}" || sym == "\u{2502}" || sym == "\u{2514}" {
                    found_at_edge = true;
                    break;
                }
            }
        }
        assert!(found_at_edge, "Tooltip should be clamped to left edge");
    }

    #[test]
    fn test_render_tooltip_near_right_edge() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        // Anchor at x=78 (near right edge)
        render_tooltip(&mut buffer, "Test tooltip", 78, 12, &theme);

        // Should still render (clamped to right edge)
        // Find the right border and verify it's within bounds
        let mut found_right_border = false;
        for y in 0..24 {
            // Check for right corner at column 79 (last column)
            if let Some(cell) = buffer.cell((79, y)) {
                let sym = cell.symbol();
                if sym == "\u{2510}" || sym == "\u{2502}" || sym == "\u{2518}" {
                    found_right_border = true;
                    break;
                }
            }
        }
        assert!(found_right_border, "Tooltip right border should be at terminal edge");
    }

    // -------------------- Edge Case Tests --------------------

    #[test]
    fn test_render_tooltip_empty_content() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        render_tooltip(&mut buffer, "", 40, 12, &theme);

        // Should still render the box (just empty)
        let mut found_corner = false;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "\u{250C}" {
                        found_corner = true;
                        break;
                    }
                }
            }
        }
        assert!(found_corner, "Empty tooltip should still render border");
    }

    #[test]
    fn test_render_tooltip_unicode_content() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        render_tooltip(&mut buffer, "Permission: \u{2713}", 40, 12, &theme);

        // Should render unicode characters
        let mut found_check = false;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "\u{2713}" {
                        found_check = true;
                        break;
                    }
                }
            }
        }
        assert!(found_check, "Unicode checkmark should be rendered");
    }

    #[test]
    fn test_calculate_position_small_terminal() {
        // Very small terminal
        let terminal = Rect::new(0, 0, 20, 5);
        let pos = calculate_position(10, 2, 15, 3, terminal);

        // Should fit within bounds
        assert!(pos.x + 15 <= terminal.width);
        assert!(pos.y + 3 <= terminal.height);
    }

    #[test]
    fn test_render_tooltip_positioned_above_anchor() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        // Anchor at y=15, tooltip should appear above it
        render_tooltip(&mut buffer, "Above", 40, 15, &theme);

        // Find the top-left corner and verify it's above anchor
        let mut corner_y = None;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "\u{250C}" {
                        corner_y = Some(y);
                        break;
                    }
                }
            }
            if corner_y.is_some() {
                break;
            }
        }

        assert!(corner_y.is_some(), "Tooltip corner not found");
        let tooltip_y = corner_y.unwrap();
        assert!(tooltip_y < 15, "Tooltip should be above anchor (y=15), but found at y={}", tooltip_y);
    }

    #[test]
    fn test_render_tooltip_border_characters() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        render_tooltip(&mut buffer, "Test", 40, 12, &theme);

        // Find the tooltip position by locating the top-left corner
        let mut corner_x = None;
        let mut corner_y = None;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "\u{250C}" {
                        corner_x = Some(x);
                        corner_y = Some(y);
                        break;
                    }
                }
            }
            if corner_x.is_some() {
                break;
            }
        }

        assert!(corner_x.is_some() && corner_y.is_some(), "Top-left corner not found");

        let x = corner_x.unwrap();
        let y = corner_y.unwrap();

        // Verify all four corners are present
        assert_eq!(buffer.cell((x, y)).unwrap().symbol(), "\u{250C}", "Top-left corner missing");

        // Find the dimensions to check other corners
        let dims = calculate_dimensions("Test");
        let x2 = x + dims.width - 1;
        let y2 = y + dims.height - 1;

        assert_eq!(buffer.cell((x2, y)).unwrap().symbol(), "\u{2510}", "Top-right corner missing");
        assert_eq!(buffer.cell((x, y2)).unwrap().symbol(), "\u{2514}", "Bottom-left corner missing");
        assert_eq!(buffer.cell((x2, y2)).unwrap().symbol(), "\u{2518}", "Bottom-right corner missing");
    }

    #[test]
    fn test_render_tooltip_horizontal_lines() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        render_tooltip(&mut buffer, "Test", 40, 12, &theme);

        // Find the tooltip position
        let mut corner_x = None;
        let mut corner_y = None;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "\u{250C}" {
                        corner_x = Some(x);
                        corner_y = Some(y);
                        break;
                    }
                }
            }
            if corner_x.is_some() {
                break;
            }
        }

        let x = corner_x.unwrap();
        let y = corner_y.unwrap();

        // Check that horizontal lines are present (between corners on top row)
        assert_eq!(buffer.cell((x + 1, y)).unwrap().symbol(), "\u{2500}", "Top horizontal line missing");
    }

    #[test]
    fn test_render_tooltip_vertical_lines() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = create_test_theme();

        render_tooltip(&mut buffer, "Test", 40, 12, &theme);

        // Find the tooltip position
        let mut corner_x = None;
        let mut corner_y = None;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y)) {
                    if cell.symbol() == "\u{250C}" {
                        corner_x = Some(x);
                        corner_y = Some(y);
                        break;
                    }
                }
            }
            if corner_x.is_some() {
                break;
            }
        }

        let x = corner_x.unwrap();
        let y = corner_y.unwrap();

        // Check that vertical line is present (between corners on left side)
        assert_eq!(buffer.cell((x, y + 1)).unwrap().symbol(), "\u{2502}", "Left vertical line missing");
    }
}
