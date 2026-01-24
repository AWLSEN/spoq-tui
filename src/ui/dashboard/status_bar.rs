//! Status bar component for the dashboard
//!
//! Renders a clickable status bar with proportional segments showing the
//! distribution of working, ready-to-test, and idle threads. Supports
//! filtering by clicking on segments.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    Frame,
};

use super::{FilterState, RenderContext};
use crate::ui::interaction::{ClickAction, HitAreaRegistry};

// ============================================================================
// Block Characters
// ============================================================================

/// Filled block character for active segments (working, ready)
const BLOCK_FILLED: char = '\u{2588}'; // Full block

/// Light shade block for idle segment
const BLOCK_LIGHT: char = '\u{2591}'; // Light shade

/// Highlighted block for filtered segment
const BLOCK_HIGHLIGHT: char = '\u{2593}'; // Dark shade

// ============================================================================
// Colors
// ============================================================================

/// Color for working segment (yellow)
const COLOR_WORKING: Color = Color::Yellow;

/// Color for ready-to-test segment (green)
const COLOR_READY: Color = Color::Green;

/// Color for idle segment (gray)
const COLOR_IDLE: Color = Color::DarkGray;

// ============================================================================
// Public API
// ============================================================================

/// Render the status bar with proportional segments and filter state
///
/// # Layout
///
/// Row 0: Proportional progress bar using block characters
/// ```text
/// ████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
/// ```
///
/// Row 1: Labels with counts
/// ```text
/// working 24              ready to test 8                                idle 15
/// ```
///
/// When filtered, shows indicator and clear button:
/// ```text
/// ▶ working 24            ready to test 8                                idle 15   ✕
/// ```
///
/// # Arguments
/// * `frame` - The ratatui frame to render into
/// * `area` - The rectangle area allocated for the status bar (min height: 2)
/// * `ctx` - The render context containing aggregate data and filter state
/// * `registry` - Hit area registry for registering clickable regions
pub fn render(frame: &mut Frame, area: Rect, ctx: &RenderContext, registry: &mut HitAreaRegistry) {
    // Need at least 2 rows for the status bar
    if area.height < 2 || area.width < 10 {
        return;
    }

    // Calculate 84% width (8% margin on each side), centered
    let bar_width = (area.width as f32 * 0.84).round() as u16;
    let left_padding = (area.width - bar_width) / 2;

    let bar_area = Rect {
        x: area.x + left_padding,
        y: area.y,
        width: bar_width,
        height: area.height,
    };

    let buf = frame.buffer_mut();

    // Calculate segment widths
    let widths = calculate_segment_widths(
        ctx.aggregate.working(),
        ctx.aggregate.ready_to_test(),
        ctx.aggregate.idle(),
        bar_area.width,
    );

    // Row 0: Proportional bar
    render_proportional_bar(buf, bar_area, &widths, ctx.filter);

    // Row 1: Labels
    render_labels(buf, bar_area, ctx, &widths);

    // Register hit areas for clickable segments
    register_hit_areas(registry, bar_area, &widths);
}

// ============================================================================
// Segment Width Calculation
// ============================================================================

/// Calculated segment widths for the status bar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentWidths {
    /// Width of the working segment
    pub working: u16,
    /// Width of the ready-to-test segment
    pub ready: u16,
    /// Width of the idle segment
    pub idle: u16,
}

/// Calculate proportional widths for each segment
///
/// Ensures the total width equals `total_width` even with rounding.
/// If all counts are zero, distributes space evenly.
///
/// # Arguments
/// * `working` - Count of working threads
/// * `ready` - Count of ready-to-test threads
/// * `idle` - Count of idle threads
/// * `total_width` - Total width available for the bar
///
/// # Returns
/// `SegmentWidths` with calculated widths that sum to `total_width`
pub fn calculate_segment_widths(working: u32, ready: u32, idle: u32, total_width: u16) -> SegmentWidths {
    let total = working + ready + idle;

    if total == 0 {
        // No threads - distribute evenly (or show all as idle)
        return SegmentWidths {
            working: 0,
            ready: 0,
            idle: total_width,
        };
    }

    let total_f = total as f32;
    let width_f = total_width as f32;

    // Calculate proportional widths using floor to avoid overflowing total
    let working_width = ((working as f32 / total_f) * width_f).floor() as u16;
    let ready_width = ((ready as f32 / total_f) * width_f).floor() as u16;

    // Idle gets the remainder to ensure exact total (handles rounding)
    let idle_width = total_width.saturating_sub(working_width + ready_width);

    SegmentWidths {
        working: working_width,
        ready: ready_width,
        idle: idle_width,
    }
}

// ============================================================================
// Bar Rendering
// ============================================================================

/// Render the proportional bar on row 0
fn render_proportional_bar(buf: &mut Buffer, area: Rect, widths: &SegmentWidths, filter: Option<FilterState>) {
    let y = area.y;
    let mut x = area.x;

    // Determine which segment is highlighted (filtered)
    let working_highlighted = matches!(filter, Some(FilterState::Working));
    let ready_highlighted = matches!(filter, Some(FilterState::ReadyToTest));
    let idle_highlighted = matches!(filter, Some(FilterState::Idle));

    // Working segment (filled, yellow)
    let working_char = if working_highlighted {
        BLOCK_HIGHLIGHT
    } else {
        BLOCK_FILLED
    };
    let working_style = Style::default().fg(COLOR_WORKING);
    for _ in 0..widths.working {
        if x < area.x + area.width {
            buf[(x, y)].set_char(working_char).set_style(working_style);
            x += 1;
        }
    }

    // Ready segment (filled, green)
    let ready_char = if ready_highlighted {
        BLOCK_HIGHLIGHT
    } else {
        BLOCK_FILLED
    };
    let ready_style = Style::default().fg(COLOR_READY);
    for _ in 0..widths.ready {
        if x < area.x + area.width {
            buf[(x, y)].set_char(ready_char).set_style(ready_style);
            x += 1;
        }
    }

    // Idle segment (light shade, gray)
    let idle_char = if idle_highlighted {
        BLOCK_HIGHLIGHT
    } else {
        BLOCK_LIGHT
    };
    let idle_style = Style::default().fg(COLOR_IDLE);
    for _ in 0..widths.idle {
        if x < area.x + area.width {
            buf[(x, y)].set_char(idle_char).set_style(idle_style);
            x += 1;
        }
    }
}

// ============================================================================
// Label Rendering
// ============================================================================

/// Render the labels on row 1
fn render_labels(buf: &mut Buffer, area: Rect, ctx: &RenderContext, widths: &SegmentWidths) {
    let y = area.y + 1;
    let filter = ctx.filter;

    // Build label texts
    let working_count = ctx.aggregate.working();
    let ready_count = ctx.aggregate.ready_to_test();
    let idle_count = ctx.aggregate.idle();

    // Determine filter indicators
    let working_prefix = if matches!(filter, Some(FilterState::Working)) {
        "\u{25B6} " // ▶
    } else {
        ""
    };
    let ready_prefix = if matches!(filter, Some(FilterState::ReadyToTest)) {
        "\u{25B6} " // ▶
    } else {
        ""
    };
    let idle_prefix = if matches!(filter, Some(FilterState::Idle)) {
        "\u{25B6} " // ▶
    } else {
        ""
    };

    // Format labels
    let working_label = format!("{}working {}", working_prefix, working_count);
    let ready_label = format!("{}ready to test {}", ready_prefix, ready_count);
    let idle_label = format!("{}idle {}", idle_prefix, idle_count);

    // Calculate positions - labels start at the beginning of each segment
    let working_x = area.x;
    let ready_x = area.x + widths.working;
    let idle_x = area.x + widths.working + widths.ready;

    // Render working label
    let working_style = Style::default().fg(COLOR_WORKING);
    render_text_at(buf, working_x, y, &working_label, working_style, area);

    // Render ready label
    let ready_style = Style::default().fg(COLOR_READY);
    render_text_at(buf, ready_x, y, &ready_label, ready_style, area);

    // Render idle label (right-align within idle segment if space permits)
    let idle_style = Style::default().fg(COLOR_IDLE);
    let idle_label_len = idle_label.chars().count() as u16;

    // Position idle label - if segment is wide enough, right-align; otherwise left-align
    let idle_label_x = if widths.idle > idle_label_len + 4 {
        // Right align within idle segment, leave room for clear button
        (area.x + area.width).saturating_sub(idle_label_len + 4)
    } else {
        idle_x
    };
    render_text_at(buf, idle_label_x, y, &idle_label, idle_style, area);

    // Render clear button if filtered
    if filter.is_some() {
        let clear_x = (area.x + area.width).saturating_sub(2);
        let clear_style = Style::default().fg(ctx.theme.error);
        if clear_x < area.x + area.width {
            buf[(clear_x, y)]
                .set_char('\u{2715}') // ✕
                .set_style(clear_style);
        }
    }
}

/// Helper to render text at a position, respecting area bounds
fn render_text_at(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style, area: Rect) {
    for (offset, ch) in text.chars().enumerate() {
        let pos_x = x + offset as u16;
        if pos_x < area.x + area.width {
            buf[(pos_x, y)].set_char(ch).set_style(style);
        }
    }
}

// ============================================================================
// Hit Area Registration
// ============================================================================

/// Register clickable hit areas for the status bar segments
fn register_hit_areas(registry: &mut HitAreaRegistry, area: Rect, widths: &SegmentWidths) {
    // Hit areas span both rows (height: 2)
    let hit_height = 2.min(area.height);

    // Hover style for interactive feedback
    let hover_style = Style::default().fg(Color::White);

    // Working segment hit area
    if widths.working > 0 {
        let working_rect = Rect::new(area.x, area.y, widths.working, hit_height);
        registry.register(working_rect, ClickAction::FilterWorking, Some(hover_style));
    }

    // Ready segment hit area
    if widths.ready > 0 {
        let ready_rect = Rect::new(
            area.x + widths.working,
            area.y,
            widths.ready,
            hit_height,
        );
        registry.register(ready_rect, ClickAction::FilterReadyToTest, Some(hover_style));
    }

    // Idle segment hit area
    if widths.idle > 0 {
        let idle_rect = Rect::new(
            area.x + widths.working + widths.ready,
            area.y,
            widths.idle,
            hit_height,
        );
        registry.register(idle_rect, ClickAction::FilterIdle, Some(hover_style));
    }

    // Clear filter button hit area (positioned at far right of row 1)
    // We register this regardless of filter state - click handler will ignore if not filtered
    let clear_rect = Rect::new(
        (area.x + area.width).saturating_sub(3),
        area.y + 1,
        3,
        1,
    );
    registry.register(clear_rect, ClickAction::ClearFilter, Some(hover_style));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- calculate_segment_widths Tests --------------------

    #[test]
    fn test_calculate_segment_widths_basic() {
        // Simple case: 24 working, 8 ready, 15 idle = 47 total
        // With 80 width:
        // working: 24/47 * 80 = 40.85 -> 41
        // ready: 8/47 * 80 = 13.62 -> 14
        // idle: 80 - 41 - 14 = 25
        let widths = calculate_segment_widths(24, 8, 15, 80);
        assert_eq!(widths.working + widths.ready + widths.idle, 80);
    }

    #[test]
    fn test_calculate_segment_widths_zero_total() {
        // No threads - all idle
        let widths = calculate_segment_widths(0, 0, 0, 100);
        assert_eq!(widths.working, 0);
        assert_eq!(widths.ready, 0);
        assert_eq!(widths.idle, 100);
    }

    #[test]
    fn test_calculate_segment_widths_only_working() {
        let widths = calculate_segment_widths(10, 0, 0, 50);
        assert_eq!(widths.working, 50);
        assert_eq!(widths.ready, 0);
        assert_eq!(widths.idle, 0);
    }

    #[test]
    fn test_calculate_segment_widths_only_ready() {
        let widths = calculate_segment_widths(0, 10, 0, 50);
        assert_eq!(widths.working, 0);
        assert_eq!(widths.ready, 50);
        assert_eq!(widths.idle, 0);
    }

    #[test]
    fn test_calculate_segment_widths_only_idle() {
        let widths = calculate_segment_widths(0, 0, 10, 50);
        assert_eq!(widths.working, 0);
        assert_eq!(widths.ready, 0);
        assert_eq!(widths.idle, 50);
    }

    #[test]
    fn test_calculate_segment_widths_equal_distribution() {
        // Equal counts should give roughly equal widths
        let widths = calculate_segment_widths(10, 10, 10, 30);
        assert_eq!(widths.working, 10);
        assert_eq!(widths.ready, 10);
        assert_eq!(widths.idle, 10);
    }

    #[test]
    fn test_calculate_segment_widths_sum_equals_total() {
        // Test various combinations to ensure sum always equals total
        for total_width in [50, 80, 100, 120] {
            for working in [0, 5, 10, 20] {
                for ready in [0, 3, 8, 15] {
                    for idle in [0, 7, 12, 25] {
                        let widths = calculate_segment_widths(working, ready, idle, total_width);
                        assert_eq!(
                            widths.working + widths.ready + widths.idle,
                            total_width,
                            "Failed for w={}, r={}, i={}, total={}",
                            working,
                            ready,
                            idle,
                            total_width
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_calculate_segment_widths_narrow_width() {
        // Very narrow width
        let widths = calculate_segment_widths(10, 10, 10, 6);
        assert_eq!(widths.working + widths.ready + widths.idle, 6);
    }

    #[test]
    fn test_calculate_segment_widths_wide_width() {
        // Very wide width
        let widths = calculate_segment_widths(1, 1, 1, 300);
        assert_eq!(widths.working + widths.ready + widths.idle, 300);
        // Each should be roughly 100
        assert!(widths.working >= 99 && widths.working <= 101);
        assert!(widths.ready >= 99 && widths.ready <= 101);
    }

    #[test]
    fn test_calculate_segment_widths_small_counts() {
        // Small counts with larger width
        let widths = calculate_segment_widths(1, 2, 3, 60);
        // 1/6 * 60 = 10, 2/6 * 60 = 20, 3/6 * 60 = 30
        assert_eq!(widths.working + widths.ready + widths.idle, 60);
    }

    // -------------------- SegmentWidths Tests --------------------

    #[test]
    fn test_segment_widths_struct() {
        let widths = SegmentWidths {
            working: 40,
            ready: 20,
            idle: 40,
        };
        assert_eq!(widths.working, 40);
        assert_eq!(widths.ready, 20);
        assert_eq!(widths.idle, 40);
    }

    #[test]
    fn test_segment_widths_clone() {
        let widths = SegmentWidths {
            working: 10,
            ready: 20,
            idle: 30,
        };
        let cloned = widths;
        assert_eq!(widths, cloned);
    }

    // -------------------- Label Formatting Tests --------------------

    #[test]
    fn test_label_formatting() {
        // Test that labels format correctly
        let working_label = format!("working {}", 24);
        assert_eq!(working_label, "working 24");

        let ready_label = format!("ready to test {}", 8);
        assert_eq!(ready_label, "ready to test 8");

        let idle_label = format!("idle {}", 15);
        assert_eq!(idle_label, "idle 15");
    }

    #[test]
    fn test_label_with_prefix() {
        let prefix = "\u{25B6} ";
        let working_label = format!("{}working {}", prefix, 24);
        assert_eq!(working_label, "\u{25B6} working 24");
    }
}
