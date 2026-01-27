//! Dashboard header component
//!
//! Renders the header with system stats (left) and SPOQ logo (right).

use ratatui::{buffer::Buffer, layout::Rect, style::Style, Frame};

use super::RenderContext;

// ============================================================================
// Logo Constants
// ============================================================================

/// SPOQ logo top line (15 chars wide)
const LOGO_TOP: &str = "\u{2584}\u{2584}\u{2584} \u{2584}\u{2584}\u{2584} \u{2584}\u{2584}\u{2584} \u{2584}\u{2584}\u{2584}";

/// SPOQ logo bottom line (15 chars wide)
const LOGO_BOT: &str = "\u{2580}\u{2580}\u{2588} \u{2588}\u{2580}\u{2580} \u{2588}\u{2584}\u{2588} \u{2588}\u{2584}\u{2588}";

/// Logo width in characters
const LOGO_WIDTH: u16 = 15;

// ============================================================================
// Public API
// ============================================================================

/// Render the dashboard header
///
/// # Layout
/// ```text
/// [left_stats] [spacer] [logo]
/// ```
///
/// - Left section (x=2): Connection status, CPU bar, RAM usage
/// - Right: SPOQ logo (2 rows, 15 chars wide)
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The rectangle area allocated for the header
/// * `ctx` - The render context containing system stats and aggregate data
pub fn render(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    if area.height < 2 || area.width < 20 {
        return;
    }

    let buf = frame.buffer_mut();

    // Render left section (system stats)
    render_left_section(buf, area, ctx);

    // Render logo (top-right)
    render_logo(buf, area, ctx);
}

// ============================================================================
// Section Renderers
// ============================================================================

/// Render the left section with connection status dot
fn render_left_section(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let x = area.x + 2;
    // Vertically center the single-line text within the header area
    let y = area.y + (area.height.saturating_sub(1)) / 2;

    // Connection status circle
    let (conn_char, conn_color) = if ctx.system_stats.connected {
        ('●', ctx.theme.success) // Filled circle, green
    } else {
        ('○', ctx.theme.error) // Empty circle, red
    };

    // Render circle at x position
    buf[(x, y)]
        .set_char(conn_char)
        .set_style(Style::default().fg(conn_color));
}

/// Render the SPOQ logo (top-right aligned)
fn render_logo(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    // Right-align the logo with 2 char padding from edge
    let logo_x = (area.x + area.width).saturating_sub(LOGO_WIDTH + 2);

    // Vertically center the 2-row logo within the header area
    let logo_y = area.y + (area.height.saturating_sub(2)) / 2;

    // Row 0: LOGO_TOP
    let y0 = logo_y;
    for (offset, ch) in LOGO_TOP.chars().enumerate() {
        let pos_x = logo_x + offset as u16;
        if pos_x < area.x + area.width && y0 < area.y + area.height {
            buf[(pos_x, y0)]
                .set_char(ch)
                .set_style(Style::default().fg(ctx.theme.accent));
        }
    }

    // Row 1: LOGO_BOT
    let y1 = logo_y + 1;
    if y1 < area.y + area.height {
        for (offset, ch) in LOGO_BOT.chars().enumerate() {
            let pos_x = logo_x + offset as u16;
            if pos_x < area.x + area.width {
                buf[(pos_x, y1)]
                    .set_char(ch)
                    .set_style(Style::default().fg(ctx.theme.accent));
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    // Connection status dot rendering is tested via integration tests
}
