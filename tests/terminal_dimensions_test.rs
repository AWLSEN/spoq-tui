// Integration tests for terminal dimension tracking

use spoq::app::App;
use spoq::ui::{
    calculate_stacked_heights, calculate_two_column_widths, LayoutContext,
};

// =============================================================================
// App Terminal Dimension Tests
// =============================================================================

#[test]
fn test_terminal_dimensions_default_values() {
    let app = App::new().expect("Failed to create app");

    // Default dimensions should be 80x24 (standard terminal size)
    assert_eq!(app.terminal_width(), 80, "Default terminal width should be 80");
    assert_eq!(app.terminal_height(), 24, "Default terminal height should be 24");
}

#[test]
fn test_update_terminal_dimensions() {
    let mut app = App::new().expect("Failed to create app");

    app.update_terminal_dimensions(120, 40);

    assert_eq!(app.terminal_width(), 120, "Terminal width should be updated");
    assert_eq!(app.terminal_height(), 40, "Terminal height should be updated");
}

#[test]
fn test_update_terminal_dimensions_small_values() {
    let mut app = App::new().expect("Failed to create app");

    app.update_terminal_dimensions(40, 10);

    assert_eq!(app.terminal_width(), 40, "Should handle small width");
    assert_eq!(app.terminal_height(), 10, "Should handle small height");
}

#[test]
fn test_update_terminal_dimensions_large_values() {
    let mut app = App::new().expect("Failed to create app");

    app.update_terminal_dimensions(400, 100);

    assert_eq!(app.terminal_width(), 400, "Should handle large width");
    assert_eq!(app.terminal_height(), 100, "Should handle large height");
}

#[test]
fn test_content_width_calculation() {
    let mut app = App::new().expect("Failed to create app");

    app.update_terminal_dimensions(100, 30);

    // Content width should subtract 4 (2 for each side border)
    assert_eq!(app.content_width(), 96, "Content width should account for borders");
}

#[test]
fn test_content_width_small_terminal() {
    let mut app = App::new().expect("Failed to create app");

    app.update_terminal_dimensions(10, 10);

    // Should handle small terminals gracefully with saturating_sub
    assert_eq!(app.content_width(), 6, "Content width should not go negative");
}

#[test]
fn test_content_width_very_small_terminal() {
    let mut app = App::new().expect("Failed to create app");

    app.update_terminal_dimensions(3, 3);

    // saturating_sub prevents underflow
    assert_eq!(app.content_width(), 0, "Content width should be 0 for tiny terminal");
}

#[test]
fn test_content_height_calculation() {
    let mut app = App::new().expect("Failed to create app");

    app.update_terminal_dimensions(100, 30);

    // Content height should subtract 6 (header, footer, borders)
    assert_eq!(app.content_height(), 24, "Content height should account for chrome");
}

#[test]
fn test_content_height_small_terminal() {
    let mut app = App::new().expect("Failed to create app");

    app.update_terminal_dimensions(80, 8);

    // Should handle small terminals gracefully
    assert_eq!(app.content_height(), 2, "Content height should not go negative");
}

#[test]
fn test_content_height_very_small_terminal() {
    let mut app = App::new().expect("Failed to create app");

    app.update_terminal_dimensions(80, 4);

    // saturating_sub prevents underflow
    assert_eq!(app.content_height(), 0, "Content height should be 0 for tiny terminal");
}

// =============================================================================
// LayoutContext Tests
// =============================================================================

#[test]
fn test_layout_context_new() {
    let ctx = LayoutContext::new(100, 50);

    assert_eq!(ctx.width, 100);
    assert_eq!(ctx.height, 50);
}

#[test]
fn test_layout_context_default() {
    let ctx = LayoutContext::default();

    assert_eq!(ctx.width, 80, "Default width should be 80");
    assert_eq!(ctx.height, 24, "Default height should be 24");
}

#[test]
fn test_layout_context_from_rect() {
    use ratatui::layout::Rect;

    let rect = Rect::new(10, 20, 100, 50);
    let ctx = LayoutContext::from_rect(rect);

    assert_eq!(ctx.width, 100, "Should take width from rect");
    assert_eq!(ctx.height, 50, "Should take height from rect");
}

#[test]
fn test_percent_width() {
    let ctx = LayoutContext::new(100, 50);

    assert_eq!(ctx.percent_width(50), 50, "50% of 100 should be 50");
    assert_eq!(ctx.percent_width(25), 25, "25% of 100 should be 25");
    assert_eq!(ctx.percent_width(100), 100, "100% of 100 should be 100");
}

#[test]
fn test_percent_width_rounds_down() {
    let ctx = LayoutContext::new(100, 50);

    // 33% of 100 = 33 (integer division)
    assert_eq!(ctx.percent_width(33), 33);
}

#[test]
fn test_percent_width_minimum_one() {
    let ctx = LayoutContext::new(10, 10);

    // 1% of 10 = 0.1, should round up to minimum of 1
    assert_eq!(ctx.percent_width(1), 1, "Minimum width should be 1");
}

#[test]
fn test_percent_height() {
    let ctx = LayoutContext::new(100, 50);

    assert_eq!(ctx.percent_height(50), 25, "50% of 50 should be 25");
    assert_eq!(ctx.percent_height(100), 50, "100% of 50 should be 50");
}

#[test]
fn test_percent_height_minimum_one() {
    let ctx = LayoutContext::new(10, 10);

    // 1% of 10 = 0.1, should round up to minimum of 1
    assert_eq!(ctx.percent_height(1), 1, "Minimum height should be 1");
}

#[test]
fn test_bounded_width() {
    let ctx = LayoutContext::new(200, 50);

    // 50% of 200 = 100, bounded between 20 and 80
    assert_eq!(ctx.bounded_width(50, 20, 80), 80, "Should clamp to max");

    // 10% of 200 = 20, bounded between 30 and 80
    assert_eq!(ctx.bounded_width(10, 30, 80), 30, "Should clamp to min");

    // 30% of 200 = 60, bounded between 20 and 80
    assert_eq!(ctx.bounded_width(30, 20, 80), 60, "Should not clamp when in bounds");
}

#[test]
fn test_bounded_height() {
    let ctx = LayoutContext::new(100, 100);

    // 50% of 100 = 50, bounded between 10 and 40
    assert_eq!(ctx.bounded_height(50, 10, 40), 40, "Should clamp to max");

    // 5% of 100 = 5, bounded between 10 and 40
    assert_eq!(ctx.bounded_height(5, 10, 40), 10, "Should clamp to min");
}

#[test]
fn test_is_narrow() {
    let wide_ctx = LayoutContext::new(100, 24);
    let narrow_ctx = LayoutContext::new(60, 24);
    let boundary_ctx = LayoutContext::new(80, 24);

    assert!(!wide_ctx.is_narrow(), "100 columns is not narrow");
    assert!(narrow_ctx.is_narrow(), "60 columns is narrow");
    assert!(!boundary_ctx.is_narrow(), "80 columns is the boundary (not narrow)");
}

#[test]
fn test_is_short() {
    let tall_ctx = LayoutContext::new(80, 40);
    let short_ctx = LayoutContext::new(80, 20);
    let boundary_ctx = LayoutContext::new(80, 24);

    assert!(!tall_ctx.is_short(), "40 rows is not short");
    assert!(short_ctx.is_short(), "20 rows is short");
    assert!(!boundary_ctx.is_short(), "24 rows is the boundary (not short)");
}

#[test]
fn test_is_compact() {
    let normal_ctx = LayoutContext::new(100, 40);
    let narrow_only = LayoutContext::new(60, 40);
    let short_only = LayoutContext::new(100, 20);
    let both_compact = LayoutContext::new(60, 20);

    assert!(!normal_ctx.is_compact(), "Normal size is not compact");
    assert!(narrow_only.is_compact(), "Narrow terminal is compact");
    assert!(short_only.is_compact(), "Short terminal is compact");
    assert!(both_compact.is_compact(), "Narrow and short is compact");
}

#[test]
fn test_content_width_with_border() {
    let ctx = LayoutContext::new(100, 50);

    assert_eq!(ctx.available_content_width(4), 96, "Should subtract border width");
    assert_eq!(ctx.available_content_width(10), 90, "Should subtract custom border width");
}

#[test]
fn test_content_width_handles_overflow() {
    let ctx = LayoutContext::new(10, 10);

    // Border larger than width - saturating_sub prevents underflow
    assert_eq!(ctx.available_content_width(20), 0, "Should not go negative");
}

#[test]
fn test_content_height_with_chrome() {
    let ctx = LayoutContext::new(100, 50);

    assert_eq!(ctx.available_content_height(6), 44, "Should subtract chrome height");
    assert_eq!(ctx.available_content_height(10), 40, "Should subtract custom chrome height");
}

#[test]
fn test_content_height_handles_overflow() {
    let ctx = LayoutContext::new(100, 10);

    // Chrome larger than height - saturating_sub prevents underflow
    assert_eq!(ctx.available_content_height(20), 0, "Should not go negative");
}

// =============================================================================
// Layout Helper Function Tests
// =============================================================================

#[test]
fn test_calculate_two_column_widths_narrow() {
    // Very narrow terminal (< 60)
    let (left, right) = calculate_two_column_widths(50);

    assert_eq!(left, 25, "Left should be half for narrow terminal");
    assert_eq!(right, 25, "Right should be half for narrow terminal");
    assert_eq!(left + right, 50, "Widths should sum to total");
}

#[test]
fn test_calculate_two_column_widths_medium() {
    // Medium terminal (60-119)
    let (left, right) = calculate_two_column_widths(100);

    assert_eq!(left, 40, "Left should be 40% for medium terminal");
    assert_eq!(right, 60, "Right should be 60% for medium terminal");
    assert_eq!(left + right, 100, "Widths should sum to total");
}

#[test]
fn test_calculate_two_column_widths_wide() {
    // Wide terminal (>= 120)
    let (left, right) = calculate_two_column_widths(200);

    // 35% of 200 = 70, but capped at 60
    assert_eq!(left, 60, "Left should be capped at 60 for wide terminal");
    assert_eq!(right, 140, "Right should be remainder for wide terminal");
    assert_eq!(left + right, 200, "Widths should sum to total");
}

#[test]
fn test_calculate_two_column_widths_boundary_60() {
    let (left, right) = calculate_two_column_widths(60);

    // At boundary (60), should use medium split (40/60)
    assert_eq!(left, 24, "40% of 60 = 24");
    assert_eq!(right, 36, "60% of 60 = 36");
}

#[test]
fn test_calculate_two_column_widths_boundary_120() {
    let (left, right) = calculate_two_column_widths(120);

    // At boundary (120), should use wide split
    // 35% of 120 = 42, within max of 60
    assert_eq!(left, 42, "35% of 120 = 42");
    assert_eq!(right, 78, "Remainder should be 78");
}

#[test]
fn test_calculate_stacked_heights() {
    let (top, bottom) = calculate_stacked_heights(30, 5);

    assert_eq!(bottom, 5, "Bottom should be the requested input rows");
    assert_eq!(top, 25, "Top should be remainder");
    assert_eq!(top + bottom, 30, "Heights should sum to total");
}

#[test]
fn test_calculate_stacked_heights_caps_input() {
    // Input area shouldn't exceed 1/3 of height
    let (top, bottom) = calculate_stacked_heights(30, 20);

    assert_eq!(bottom, 10, "Bottom should be capped at 1/3 of height");
    assert_eq!(top, 20, "Top should be remainder after cap");
}

#[test]
fn test_calculate_stacked_heights_small_terminal() {
    let (top, bottom) = calculate_stacked_heights(12, 5);

    // 1/3 of 12 = 4, so cap at 4
    assert_eq!(bottom, 4, "Bottom should be capped at 1/3 of height");
    assert_eq!(top, 8, "Top should be remainder");
}

#[test]
fn test_layout_context_copy() {
    let ctx = LayoutContext::new(100, 50);
    let copy = ctx; // Should implement Copy

    assert_eq!(ctx.width, copy.width);
    assert_eq!(ctx.height, copy.height);
}

#[test]
fn test_layout_context_clone() {
    let ctx = LayoutContext::new(100, 50);
    let cloned = ctx.clone();

    assert_eq!(ctx.width, cloned.width);
    assert_eq!(ctx.height, cloned.height);
}
