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

/// Render the left section with CPU bar and RAM usage
fn render_left_section(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let mut x = area.x + 2;
    // Vertically center the single-line text within the header area
    let y = area.y + (area.height.saturating_sub(1)) / 2;

    // Connection status circle (far left, before CPU)
    let (conn_char, conn_color) = if ctx.system_stats.connected {
        ('●', ctx.theme.success) // Filled circle, green
    } else {
        ('○', ctx.theme.error) // Empty circle, red
    };

    // Render circle at x position
    buf[(x, y)]
        .set_char(conn_char)
        .set_style(Style::default().fg(conn_color));

    // Add space after circle, then continue with CPU
    // Shift all subsequent rendering by 2 positions (circle + space)
    x += 2; // Now CPU starts after "● "

    // Format: "cpu ▓▓▓░░  4.2/8g"
    // CPU label and bar
    let cpu_bar = render_cpu_bar(ctx.system_stats.cpu_percent);
    let cpu_text = format!("cpu {}  ", cpu_bar);

    let mut offset = 0u16;
    for ch in cpu_text.chars() {
        let pos_x = x + offset;
        if pos_x < area.x + area.width {
            buf[(pos_x, y)]
                .set_char(ch)
                .set_style(Style::default().fg(ctx.theme.dim));
        }
        offset += 1;
    }

    // RAM usage: "X.X/Yg"
    let ram_text = format!(
        "{:.1}/{}g",
        ctx.system_stats.ram_used_gb, ctx.system_stats.ram_total_gb as u32
    );
    for ch in ram_text.chars() {
        let pos_x = x + offset;
        if pos_x < area.x + area.width {
            buf[(pos_x, y)]
                .set_char(ch)
                .set_style(Style::default().fg(ctx.theme.dim));
        }
        offset += 1;
    }
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
// Helper Functions
// ============================================================================

/// Render a CPU usage bar with 5 segments
///
/// Each segment represents 20% CPU usage:
/// - 0-20%: 1 filled segment
/// - 20-40%: 2 filled segments
/// - etc.
///
/// # Arguments
/// * `cpu_percent` - CPU usage percentage (0-100)
///
/// # Returns
/// A string with 5 characters of filled (\u{2593}) and empty (\u{2591}) segments
///
/// # Example
/// ```
/// use spoq::ui::dashboard::header::render_cpu_bar;
/// let bar = render_cpu_bar(45.0);
/// assert_eq!(bar, "\u{2593}\u{2593}\u{2593}\u{2591}\u{2591}");
/// ```
pub fn render_cpu_bar(cpu_percent: f32) -> String {
    // 5 segments: each represents 20%
    let filled = (cpu_percent / 20.0).ceil().max(0.0) as usize;
    let filled = filled.min(5);
    let empty = 5 - filled;
    format!("{}{}", "\u{2593}".repeat(filled), "\u{2591}".repeat(empty))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- render_cpu_bar Tests --------------------

    #[test]
    fn test_cpu_bar_zero() {
        let bar = render_cpu_bar(0.0);
        assert_eq!(bar, "\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}");
    }

    #[test]
    fn test_cpu_bar_low() {
        // 15% -> ceil(15/20) = 1 filled
        let bar = render_cpu_bar(15.0);
        assert_eq!(bar, "\u{2593}\u{2591}\u{2591}\u{2591}\u{2591}");
    }

    #[test]
    fn test_cpu_bar_mid() {
        // 45% -> ceil(45/20) = 3 filled
        let bar = render_cpu_bar(45.0);
        assert_eq!(bar, "\u{2593}\u{2593}\u{2593}\u{2591}\u{2591}");
    }

    #[test]
    fn test_cpu_bar_high() {
        // 80% -> ceil(80/20) = 4 filled
        let bar = render_cpu_bar(80.0);
        assert_eq!(bar, "\u{2593}\u{2593}\u{2593}\u{2593}\u{2591}");
    }

    #[test]
    fn test_cpu_bar_max() {
        // 100% -> 5 filled
        let bar = render_cpu_bar(100.0);
        assert_eq!(bar, "\u{2593}\u{2593}\u{2593}\u{2593}\u{2593}");
    }

    #[test]
    fn test_cpu_bar_over_max() {
        // Over 100% should still cap at 5 filled
        let bar = render_cpu_bar(150.0);
        assert_eq!(bar, "\u{2593}\u{2593}\u{2593}\u{2593}\u{2593}");
    }

    #[test]
    fn test_cpu_bar_negative() {
        // Negative should result in 0 filled (ceil of negative is 0 or negative, min capped)
        let bar = render_cpu_bar(-10.0);
        // ceil(-0.5) = 0, but since we use usize it might wrap or be 0
        // The bar should be all empty
        assert_eq!(bar.chars().count(), 5);
        assert_eq!(bar, "\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}");
    }

    #[test]
    fn test_cpu_bar_boundary_20() {
        // Exactly 20% -> ceil(20/20) = 1 filled
        let bar = render_cpu_bar(20.0);
        assert_eq!(bar, "\u{2593}\u{2591}\u{2591}\u{2591}\u{2591}");
    }

    #[test]
    fn test_cpu_bar_boundary_40() {
        // Exactly 40% -> ceil(40/20) = 2 filled
        let bar = render_cpu_bar(40.0);
        assert_eq!(bar, "\u{2593}\u{2593}\u{2591}\u{2591}\u{2591}");
    }

    #[test]
    fn test_cpu_bar_boundary_60() {
        // Exactly 60% -> ceil(60/20) = 3 filled
        let bar = render_cpu_bar(60.0);
        assert_eq!(bar, "\u{2593}\u{2593}\u{2593}\u{2591}\u{2591}");
    }

    #[test]
    fn test_cpu_bar_length_is_always_5() {
        // Ensure the bar is always exactly 5 characters
        for pct in [0.0, 25.0, 50.0, 75.0, 100.0, 200.0] {
            let bar = render_cpu_bar(pct);
            assert_eq!(
                bar.chars().count(),
                5,
                "CPU bar should have 5 chars for {}%",
                pct
            );
        }
    }
}
