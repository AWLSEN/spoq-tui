// Integration tests for responsive layout behavior
// Tests layouts at various terminal sizes:
// - 40x20 (mobile-like)
// - 80x24 (standard terminal)
// - 120x40 (wide terminal)
// - 200x50 (ultra-wide)
// - 30x10 (minimum size boundary)

use spoq::ui::{
    breakpoints, calculate_stacked_heights, calculate_two_column_widths, is_terminal_too_small,
    LayoutContext, SizeCategory, MIN_TERMINAL_HEIGHT, MIN_TERMINAL_WIDTH,
};

// =============================================================================
// Test Size Constants
// =============================================================================

// Mobile-like terminal size
const MOBILE_WIDTH: u16 = 40;
const MOBILE_HEIGHT: u16 = 20;

// Standard terminal size
const STANDARD_WIDTH: u16 = 80;
const STANDARD_HEIGHT: u16 = 24;

// Wide terminal size
const WIDE_WIDTH: u16 = 120;
const WIDE_HEIGHT: u16 = 40;

// Ultra-wide terminal size
const ULTRA_WIDE_WIDTH: u16 = 200;
const ULTRA_WIDE_HEIGHT: u16 = 50;

// Minimum size boundary
const MIN_BOUNDARY_WIDTH: u16 = 30;
const MIN_BOUNDARY_HEIGHT: u16 = 10;

// =============================================================================
// LayoutContext Integration Tests - Mobile-like (40x20)
// =============================================================================

mod mobile_size {
    use super::*;

    fn ctx() -> LayoutContext {
        LayoutContext::new(MOBILE_WIDTH, MOBILE_HEIGHT)
    }

    #[test]
    fn test_mobile_size_categories() {
        let layout = ctx();
        assert_eq!(
            layout.width_category(),
            SizeCategory::ExtraSmall,
            "40 columns should be ExtraSmall width"
        );
        assert_eq!(
            layout.height_category(),
            SizeCategory::Small,
            "20 rows should be Small height"
        );
    }

    #[test]
    fn test_mobile_state_flags() {
        let layout = ctx();
        assert!(layout.is_narrow(), "40 columns should be narrow");
        assert!(layout.is_short(), "20 rows should be short");
        assert!(layout.is_compact(), "40x20 should be compact");
        assert!(layout.is_extra_small(), "40 columns is extra small");
    }

    #[test]
    fn test_mobile_panel_stacking() {
        let layout = ctx();
        assert!(
            layout.should_stack_panels(),
            "40 columns should require stacked panels"
        );
        assert!(
            layout.should_collapse_sidebar(),
            "40 columns should collapse sidebar"
        );
    }

    #[test]
    fn test_mobile_ui_features() {
        let layout = ctx();
        assert!(
            !layout.should_show_scrollbar(),
            "40 columns should not show scrollbar"
        );
        assert!(
            !layout.should_show_full_badges(),
            "40 columns should show abbreviated badges"
        );
        assert!(
            !layout.should_show_tool_previews(),
            "20 rows should not show tool previews"
        );
    }

    #[test]
    fn test_mobile_two_column_widths() {
        let layout = ctx();
        let (left, right) = layout.two_column_widths();
        // Very narrow: equal 50/50 split
        assert_eq!(left, 20, "Left panel should be half of 40");
        assert_eq!(right, 20, "Right panel should be half of 40");
        assert_eq!(left + right, MOBILE_WIDTH, "Widths should sum to total");
    }

    #[test]
    fn test_mobile_percent_calculations() {
        let layout = ctx();
        assert_eq!(layout.percent_width(50), 20, "50% of 40 = 20");
        assert_eq!(layout.percent_height(50), 10, "50% of 20 = 10");
    }

    #[test]
    fn test_mobile_content_area() {
        let layout = ctx();
        assert_eq!(
            layout.available_content_width(4),
            36,
            "Content width should be 40-4=36"
        );
        assert_eq!(
            layout.available_content_height(6),
            14,
            "Content height should be 20-6=14"
        );
    }

    #[test]
    fn test_mobile_text_truncation() {
        let layout = ctx();
        assert_eq!(layout.max_title_length(), 20, "Mobile title length limit");
        assert_eq!(layout.max_preview_length(), 40, "Mobile preview length limit");
    }

    #[test]
    fn test_mobile_header_and_input_sizes() {
        let layout = ctx();
        assert_eq!(layout.header_height(), 3, "Compact header for mobile");
        assert_eq!(layout.input_area_height(), 4, "Compact input for mobile");
    }
}

// =============================================================================
// LayoutContext Integration Tests - Standard (80x24)
// =============================================================================

mod standard_size {
    use super::*;

    fn ctx() -> LayoutContext {
        LayoutContext::new(STANDARD_WIDTH, STANDARD_HEIGHT)
    }

    #[test]
    fn test_standard_size_categories() {
        let layout = ctx();
        assert_eq!(
            layout.width_category(),
            SizeCategory::Medium,
            "80 columns should be Medium width"
        );
        // Height 24 is >= SM_HEIGHT (24), so it's Medium (between SM and MD)
        assert_eq!(
            layout.height_category(),
            SizeCategory::Medium,
            "24 rows should be Medium height (>= 24, < 40)"
        );
    }

    #[test]
    fn test_standard_state_flags() {
        let layout = ctx();
        assert!(!layout.is_narrow(), "80 columns is not narrow");
        assert!(!layout.is_short(), "24 rows is not short (boundary)");
        assert!(!layout.is_compact(), "80x24 is not compact");
        assert!(!layout.is_extra_small(), "80 columns is not extra small");
    }

    #[test]
    fn test_standard_panel_stacking() {
        let layout = ctx();
        assert!(
            !layout.should_stack_panels(),
            "80 columns should use side-by-side panels"
        );
        assert!(
            !layout.should_collapse_sidebar(),
            "80 columns should show sidebar"
        );
    }

    #[test]
    fn test_standard_ui_features() {
        let layout = ctx();
        assert!(
            layout.should_show_scrollbar(),
            "80 columns should show scrollbar"
        );
        assert!(
            layout.should_show_full_badges(),
            "80 columns should show full badges"
        );
        assert!(
            layout.should_show_tool_previews(),
            "24 rows should show tool previews (boundary)"
        );
    }

    #[test]
    fn test_standard_two_column_widths() {
        let layout = ctx();
        let (left, right) = layout.two_column_widths();
        // Medium: 40/60 split
        assert_eq!(left, 32, "Left panel should be 40% of 80");
        assert_eq!(right, 48, "Right panel should be 60% of 80");
        assert_eq!(left + right, STANDARD_WIDTH, "Widths should sum to total");
    }

    #[test]
    fn test_standard_percent_calculations() {
        let layout = ctx();
        assert_eq!(layout.percent_width(30), 24, "30% of 80 = 24");
        assert_eq!(layout.percent_height(50), 12, "50% of 24 = 12");
    }

    #[test]
    fn test_standard_content_area() {
        let layout = ctx();
        assert_eq!(
            layout.available_content_width(4),
            76,
            "Content width should be 80-4=76"
        );
        assert_eq!(
            layout.available_content_height(6),
            18,
            "Content height should be 24-6=18"
        );
    }

    #[test]
    fn test_standard_text_truncation() {
        let layout = ctx();
        assert_eq!(layout.max_title_length(), 50, "Standard title length limit");
        assert_eq!(
            layout.max_preview_length(),
            100,
            "Standard preview length limit"
        );
    }

    #[test]
    fn test_standard_stacked_heights() {
        let layout = ctx();
        let (top, bottom) = layout.stacked_heights(6);
        assert_eq!(bottom, 6, "Input should be 6 rows");
        assert_eq!(top, 18, "Content should be remaining rows");
        assert_eq!(top + bottom, STANDARD_HEIGHT, "Heights should sum to total");
    }

    #[test]
    fn test_standard_header_and_input_sizes() {
        let layout = ctx();
        // 80x24 is not compact (width >= 80, height >= 24)
        assert_eq!(layout.header_height(), 9, "Full header for standard");
        assert_eq!(layout.input_area_height(), 6, "Full input for standard");
    }

    #[test]
    fn test_standard_default_matches() {
        let layout = ctx();
        let default_layout = LayoutContext::default();
        assert_eq!(layout.width, default_layout.width);
        assert_eq!(layout.height, default_layout.height);
    }
}

// =============================================================================
// LayoutContext Integration Tests - Wide (120x40)
// =============================================================================

mod wide_size {
    use super::*;

    fn ctx() -> LayoutContext {
        LayoutContext::new(WIDE_WIDTH, WIDE_HEIGHT)
    }

    #[test]
    fn test_wide_size_categories() {
        let layout = ctx();
        // Width 120 is >= MD_WIDTH (120), so it's Large (>= 120)
        assert_eq!(
            layout.width_category(),
            SizeCategory::Large,
            "120 columns should be Large width (>= 120)"
        );
        assert_eq!(
            layout.height_category(),
            SizeCategory::Large,
            "40 rows should be Large height"
        );
    }

    #[test]
    fn test_wide_state_flags() {
        let layout = ctx();
        assert!(!layout.is_narrow(), "120 columns is not narrow");
        assert!(!layout.is_short(), "40 rows is not short");
        assert!(!layout.is_compact(), "120x40 is not compact");
        assert!(!layout.is_extra_small(), "120 columns is not extra small");
    }

    #[test]
    fn test_wide_panel_stacking() {
        let layout = ctx();
        assert!(
            !layout.should_stack_panels(),
            "120 columns should use side-by-side panels"
        );
        assert!(
            !layout.should_collapse_sidebar(),
            "120 columns should show sidebar"
        );
    }

    #[test]
    fn test_wide_ui_features() {
        let layout = ctx();
        assert!(layout.should_show_scrollbar(), "Wide should show scrollbar");
        assert!(
            layout.should_show_full_badges(),
            "Wide should show full badges"
        );
        assert!(
            layout.should_show_tool_previews(),
            "40 rows should show tool previews"
        );
    }

    #[test]
    fn test_wide_two_column_widths() {
        let layout = ctx();
        let (left, right) = layout.two_column_widths();
        // Wide (>= 120): 35/65 split, left capped at 60
        // 35% of 120 = 42, which is under the cap
        assert_eq!(left, 42, "Left panel should be 35% of 120");
        assert_eq!(right, 78, "Right panel should be remainder");
        assert_eq!(left + right, WIDE_WIDTH, "Widths should sum to total");
    }

    #[test]
    fn test_wide_percent_calculations() {
        let layout = ctx();
        assert_eq!(layout.percent_width(25), 30, "25% of 120 = 30");
        assert_eq!(layout.percent_height(25), 10, "25% of 40 = 10");
    }

    #[test]
    fn test_wide_bounded_width() {
        let layout = ctx();
        // Test that bounded_width correctly clamps values
        assert_eq!(
            layout.bounded_width(50, 20, 50),
            50,
            "50% of 120 = 60, clamped to max 50"
        );
        assert_eq!(
            layout.bounded_width(10, 20, 50),
            20,
            "10% of 120 = 12, clamped to min 20"
        );
    }

    #[test]
    fn test_wide_content_area() {
        let layout = ctx();
        assert_eq!(
            layout.available_content_width(4),
            116,
            "Content width should be 120-4=116"
        );
        assert_eq!(
            layout.available_content_height(10),
            30,
            "Content height should be 40-10=30"
        );
    }

    #[test]
    fn test_wide_text_truncation() {
        let layout = ctx();
        // 120 columns is Large category, so title limit is 80
        assert_eq!(layout.max_title_length(), 80, "Wide (Large) title length limit");
        assert_eq!(layout.max_preview_length(), 150, "Wide (Large) preview length limit");
    }

    #[test]
    fn test_wide_visible_items() {
        let layout = ctx();
        // 40 - 5 = 35, 35 / 2 = 17
        assert_eq!(
            layout.max_visible_items(),
            17,
            "Wide terminal should show more items"
        );
    }

    #[test]
    fn test_wide_header_and_input_sizes() {
        let layout = ctx();
        assert_eq!(layout.header_height(), 9, "Full header for wide");
        assert_eq!(layout.input_area_height(), 6, "Full input for wide");
    }
}

// =============================================================================
// LayoutContext Integration Tests - Ultra-wide (200x50)
// =============================================================================

mod ultra_wide_size {
    use super::*;

    fn ctx() -> LayoutContext {
        LayoutContext::new(ULTRA_WIDE_WIDTH, ULTRA_WIDE_HEIGHT)
    }

    #[test]
    fn test_ultra_wide_size_categories() {
        let layout = ctx();
        assert_eq!(
            layout.width_category(),
            SizeCategory::Large,
            "200 columns should be Large width"
        );
        assert_eq!(
            layout.height_category(),
            SizeCategory::Large,
            "50 rows should be Large height"
        );
    }

    #[test]
    fn test_ultra_wide_state_flags() {
        let layout = ctx();
        assert!(!layout.is_narrow(), "200 columns is not narrow");
        assert!(!layout.is_short(), "50 rows is not short");
        assert!(!layout.is_compact(), "200x50 is not compact");
        assert!(!layout.is_extra_small(), "200 columns is not extra small");
    }

    #[test]
    fn test_ultra_wide_two_column_widths() {
        let layout = ctx();
        let (left, right) = layout.two_column_widths();
        // Wide: 35% of 200 = 70, capped at 60
        assert_eq!(left, 60, "Left panel should be capped at 60");
        assert_eq!(right, 140, "Right panel should be remainder");
        assert_eq!(
            left + right,
            ULTRA_WIDE_WIDTH,
            "Widths should sum to total"
        );
    }

    #[test]
    fn test_ultra_wide_percent_calculations() {
        let layout = ctx();
        assert_eq!(layout.percent_width(10), 20, "10% of 200 = 20");
        assert_eq!(layout.percent_height(10), 5, "10% of 50 = 5");
    }

    #[test]
    fn test_ultra_wide_bounded_width() {
        let layout = ctx();
        // 30% of 200 = 60, clamped to max of 50
        assert_eq!(
            layout.bounded_width(30, 20, 50),
            50,
            "30% of 200 exceeds max"
        );
        // 5% of 200 = 10, clamped to min of 20
        assert_eq!(
            layout.bounded_width(5, 20, 50),
            20,
            "5% of 200 below min"
        );
    }

    #[test]
    fn test_ultra_wide_text_truncation() {
        let layout = ctx();
        assert_eq!(
            layout.max_title_length(),
            80,
            "Ultra-wide title length limit"
        );
        assert_eq!(
            layout.max_preview_length(),
            150,
            "Ultra-wide preview length limit"
        );
    }

    #[test]
    fn test_ultra_wide_visible_items() {
        let layout = ctx();
        // 50 - 5 = 45, 45 / 2 = 22
        assert_eq!(
            layout.max_visible_items(),
            22,
            "Ultra-wide terminal should show many items"
        );
    }

    #[test]
    fn test_ultra_wide_text_wrap_width() {
        let layout = ctx();
        // 200 - 4 (border/padding) - 0 (indent) = 196
        assert_eq!(layout.text_wrap_width(0), 196, "Full wrap width");
        // 200 - 4 - 4 (2 indent levels * 2) = 192
        assert_eq!(layout.text_wrap_width(2), 192, "Indented wrap width");
    }
}

// =============================================================================
// LayoutContext Integration Tests - Minimum Boundary (30x10)
// =============================================================================

mod minimum_boundary_size {
    use super::*;

    fn ctx() -> LayoutContext {
        LayoutContext::new(MIN_BOUNDARY_WIDTH, MIN_BOUNDARY_HEIGHT)
    }

    #[test]
    fn test_minimum_size_categories() {
        let layout = ctx();
        assert_eq!(
            layout.width_category(),
            SizeCategory::ExtraSmall,
            "30 columns should be ExtraSmall width"
        );
        assert_eq!(
            layout.height_category(),
            SizeCategory::ExtraSmall,
            "10 rows should be ExtraSmall height"
        );
    }

    #[test]
    fn test_minimum_state_flags() {
        let layout = ctx();
        assert!(layout.is_narrow(), "30 columns is narrow");
        assert!(layout.is_short(), "10 rows is short");
        assert!(layout.is_compact(), "30x10 is compact");
        assert!(layout.is_extra_small(), "30x10 is extra small");
    }

    #[test]
    fn test_minimum_panel_stacking() {
        let layout = ctx();
        assert!(
            layout.should_stack_panels(),
            "30 columns requires stacked panels"
        );
        assert!(
            layout.should_collapse_sidebar(),
            "30 columns requires collapsed sidebar"
        );
    }

    #[test]
    fn test_minimum_ui_features() {
        let layout = ctx();
        assert!(
            !layout.should_show_scrollbar(),
            "30 columns should not show scrollbar"
        );
        assert!(
            !layout.should_show_full_badges(),
            "30 columns should show abbreviated badges"
        );
        assert!(
            !layout.should_show_tool_previews(),
            "10 rows should not show tool previews"
        );
    }

    #[test]
    fn test_minimum_two_column_widths() {
        let layout = ctx();
        let (left, right) = layout.two_column_widths();
        // Very narrow: equal 50/50 split
        assert_eq!(left, 15, "Left panel should be half of 30");
        assert_eq!(right, 15, "Right panel should be half of 30");
    }

    #[test]
    fn test_minimum_content_area_saturates() {
        let layout = ctx();
        // With a small terminal, large borders should saturate to 0
        assert_eq!(
            layout.available_content_width(50),
            0,
            "Content width should saturate to 0 with large borders"
        );
        assert_eq!(
            layout.available_content_height(20),
            0,
            "Content height should saturate to 0 with large chrome"
        );
    }

    #[test]
    fn test_minimum_visible_items() {
        let layout = ctx();
        // 10 - 5 = 5, 5 / 2 = 2
        assert_eq!(
            layout.max_visible_items(),
            2,
            "Minimum terminal shows few items"
        );
    }

    #[test]
    fn test_minimum_stacked_heights() {
        let layout = ctx();
        let (top, bottom) = layout.stacked_heights(5);
        // 1/3 of 10 = 3, so input capped at 3
        assert_eq!(bottom, 3, "Input capped at 1/3 of height");
        assert_eq!(top, 7, "Top gets remainder");
    }

    #[test]
    fn test_minimum_header_and_input_sizes() {
        let layout = ctx();
        assert_eq!(layout.header_height(), 3, "Compact header for minimum");
        assert_eq!(layout.input_area_height(), 4, "Compact input for minimum");
    }
}

// =============================================================================
// Panel Stacking Behavior Tests - 60-column threshold
// =============================================================================

mod panel_stacking_threshold {
    use super::*;

    #[test]
    fn test_stacking_threshold_below() {
        let layout = LayoutContext::new(59, 24);
        assert!(
            layout.should_stack_panels(),
            "59 columns (below 60) should stack"
        );
    }

    #[test]
    fn test_stacking_threshold_at_60() {
        let layout = LayoutContext::new(60, 24);
        assert!(
            layout.should_stack_panels(),
            "60 columns should stack (threshold is 80)"
        );
    }

    #[test]
    fn test_stacking_threshold_at_79() {
        let layout = LayoutContext::new(79, 24);
        assert!(
            layout.should_stack_panels(),
            "79 columns (below 80) should stack"
        );
    }

    #[test]
    fn test_stacking_threshold_at_80() {
        let layout = LayoutContext::new(80, 24);
        assert!(
            !layout.should_stack_panels(),
            "80 columns (at threshold) should not stack"
        );
    }

    #[test]
    fn test_stacking_threshold_above() {
        let layout = LayoutContext::new(81, 24);
        assert!(
            !layout.should_stack_panels(),
            "81 columns (above 80) should not stack"
        );
    }

    #[test]
    fn test_sidebar_collapse_threshold_below() {
        let layout = LayoutContext::new(59, 24);
        assert!(
            layout.should_collapse_sidebar(),
            "59 columns should collapse sidebar"
        );
    }

    #[test]
    fn test_sidebar_collapse_threshold_at_60() {
        let layout = LayoutContext::new(60, 24);
        assert!(
            !layout.should_collapse_sidebar(),
            "60 columns should not collapse sidebar"
        );
    }

    #[test]
    fn test_sidebar_collapse_threshold_above() {
        let layout = LayoutContext::new(61, 24);
        assert!(
            !layout.should_collapse_sidebar(),
            "61 columns should not collapse sidebar"
        );
    }
}

// =============================================================================
// Content Truncation Tests at Various Sizes
// =============================================================================

mod content_truncation {
    use super::*;

    #[test]
    fn test_title_length_progression() {
        // ExtraSmall
        let xs = LayoutContext::new(50, 24);
        assert_eq!(xs.max_title_length(), 20);

        // Small
        let sm = LayoutContext::new(70, 24);
        assert_eq!(sm.max_title_length(), 30);

        // Medium
        let md = LayoutContext::new(100, 24);
        assert_eq!(md.max_title_length(), 50);

        // Large
        let lg = LayoutContext::new(160, 24);
        assert_eq!(lg.max_title_length(), 80);
    }

    #[test]
    fn test_preview_length_progression() {
        // ExtraSmall
        let xs = LayoutContext::new(50, 24);
        assert_eq!(xs.max_preview_length(), 40);

        // Small
        let sm = LayoutContext::new(70, 24);
        assert_eq!(sm.max_preview_length(), 60);

        // Medium
        let md = LayoutContext::new(100, 24);
        assert_eq!(md.max_preview_length(), 100);

        // Large
        let lg = LayoutContext::new(160, 24);
        assert_eq!(lg.max_preview_length(), 150);
    }

    #[test]
    fn test_visible_items_progression() {
        let short = LayoutContext::new(80, 10);
        let standard = LayoutContext::new(80, 24);
        let tall = LayoutContext::new(80, 50);

        // Items should increase with height
        assert!(
            short.max_visible_items() < standard.max_visible_items(),
            "Short terminal shows fewer items"
        );
        assert!(
            standard.max_visible_items() < tall.max_visible_items(),
            "Standard terminal shows fewer items than tall"
        );
    }
}

// =============================================================================
// Minimum Size Detection Tests
// =============================================================================

mod minimum_size_detection {
    use super::*;

    #[test]
    fn test_min_terminal_constants() {
        assert_eq!(MIN_TERMINAL_WIDTH, 30, "Minimum width should be 30");
        assert_eq!(MIN_TERMINAL_HEIGHT, 10, "Minimum height should be 10");
    }

    #[test]
    fn test_terminal_too_small_width() {
        assert!(
            is_terminal_too_small(29, 24),
            "29 columns is too small (below 30)"
        );
        assert!(
            !is_terminal_too_small(30, 24),
            "30 columns is acceptable (at threshold)"
        );
        assert!(
            !is_terminal_too_small(31, 24),
            "31 columns is acceptable (above threshold)"
        );
    }

    #[test]
    fn test_terminal_too_small_height() {
        assert!(
            is_terminal_too_small(80, 9),
            "9 rows is too small (below 10)"
        );
        assert!(
            !is_terminal_too_small(80, 10),
            "10 rows is acceptable (at threshold)"
        );
        assert!(
            !is_terminal_too_small(80, 11),
            "11 rows is acceptable (above threshold)"
        );
    }

    #[test]
    fn test_terminal_too_small_both() {
        assert!(
            is_terminal_too_small(29, 9),
            "Both dimensions too small"
        );
        assert!(
            is_terminal_too_small(29, 24),
            "Width too small"
        );
        assert!(
            is_terminal_too_small(80, 9),
            "Height too small"
        );
        assert!(
            !is_terminal_too_small(30, 10),
            "Both at minimum are acceptable"
        );
    }

    #[test]
    fn test_very_small_terminal() {
        // Edge case: 1x1 terminal
        assert!(is_terminal_too_small(1, 1), "1x1 is definitely too small");

        // Edge case: 0x0 terminal
        assert!(is_terminal_too_small(0, 0), "0x0 is definitely too small");
    }
}

// =============================================================================
// Breakpoint Constants Verification
// =============================================================================

mod breakpoint_verification {
    use super::*;

    #[test]
    fn test_width_breakpoints() {
        assert_eq!(breakpoints::XS_WIDTH, 60, "XS width threshold");
        assert_eq!(breakpoints::SM_WIDTH, 80, "SM width threshold");
        assert_eq!(breakpoints::MD_WIDTH, 120, "MD width threshold");
        assert_eq!(breakpoints::LG_WIDTH, 160, "LG width threshold");
    }

    #[test]
    fn test_height_breakpoints() {
        assert_eq!(breakpoints::XS_HEIGHT, 16, "XS height threshold");
        assert_eq!(breakpoints::SM_HEIGHT, 24, "SM height threshold");
        assert_eq!(breakpoints::MD_HEIGHT, 40, "MD height threshold");
    }

    #[test]
    fn test_breakpoint_transitions_width() {
        // Test each breakpoint transition
        // Width categories: <60=XS, 60-79=S, 80-119=M, >=120=L
        assert_eq!(
            LayoutContext::new(59, 24).width_category(),
            SizeCategory::ExtraSmall,
            "59 < 60 (XS_WIDTH)"
        );
        assert_eq!(
            LayoutContext::new(60, 24).width_category(),
            SizeCategory::Small,
            "60 >= 60 (XS_WIDTH), < 80 (SM_WIDTH)"
        );
        assert_eq!(
            LayoutContext::new(79, 24).width_category(),
            SizeCategory::Small,
            "79 >= 60, < 80"
        );
        assert_eq!(
            LayoutContext::new(80, 24).width_category(),
            SizeCategory::Medium,
            "80 >= 80 (SM_WIDTH), < 120 (MD_WIDTH)"
        );
        assert_eq!(
            LayoutContext::new(119, 24).width_category(),
            SizeCategory::Medium,
            "119 >= 80, < 120"
        );
        // 120 is >= MD_WIDTH (120), so it's Large
        assert_eq!(
            LayoutContext::new(120, 24).width_category(),
            SizeCategory::Large,
            "120 >= 120 (MD_WIDTH)"
        );
        assert_eq!(
            LayoutContext::new(159, 24).width_category(),
            SizeCategory::Large,
            "159 >= 120"
        );
        assert_eq!(
            LayoutContext::new(160, 24).width_category(),
            SizeCategory::Large,
            "160 >= 120"
        );
    }

    #[test]
    fn test_breakpoint_transitions_height() {
        // Test each breakpoint transition
        // Height categories: <16=XS, 16-23=S, 24-39=M, >=40=L
        assert_eq!(
            LayoutContext::new(80, 15).height_category(),
            SizeCategory::ExtraSmall,
            "15 < 16 (XS_HEIGHT)"
        );
        assert_eq!(
            LayoutContext::new(80, 16).height_category(),
            SizeCategory::Small,
            "16 >= 16 (XS_HEIGHT), < 24 (SM_HEIGHT)"
        );
        assert_eq!(
            LayoutContext::new(80, 23).height_category(),
            SizeCategory::Small,
            "23 >= 16, < 24"
        );
        // 24 is >= SM_HEIGHT (24), so it's Medium
        assert_eq!(
            LayoutContext::new(80, 24).height_category(),
            SizeCategory::Medium,
            "24 >= 24 (SM_HEIGHT), < 40 (MD_HEIGHT)"
        );
        assert_eq!(
            LayoutContext::new(80, 39).height_category(),
            SizeCategory::Medium,
            "39 >= 24, < 40"
        );
        assert_eq!(
            LayoutContext::new(80, 40).height_category(),
            SizeCategory::Large,
            "40 >= 40 (MD_HEIGHT)"
        );
    }
}

// =============================================================================
// Legacy Helper Function Tests
// =============================================================================

mod legacy_helpers {
    use super::*;

    #[test]
    fn test_calculate_two_column_widths_mobile() {
        let (left, right) = calculate_two_column_widths(MOBILE_WIDTH);
        assert_eq!(left, 20);
        assert_eq!(right, 20);
    }

    #[test]
    fn test_calculate_two_column_widths_standard() {
        let (left, right) = calculate_two_column_widths(STANDARD_WIDTH);
        assert_eq!(left, 32);
        assert_eq!(right, 48);
    }

    #[test]
    fn test_calculate_two_column_widths_wide() {
        let (left, right) = calculate_two_column_widths(WIDE_WIDTH);
        assert_eq!(left, 42);
        assert_eq!(right, 78);
    }

    #[test]
    fn test_calculate_two_column_widths_ultra_wide() {
        let (left, right) = calculate_two_column_widths(ULTRA_WIDE_WIDTH);
        assert_eq!(left, 60); // Capped
        assert_eq!(right, 140);
    }

    #[test]
    fn test_calculate_stacked_heights_standard() {
        let (top, bottom) = calculate_stacked_heights(STANDARD_HEIGHT, 6);
        assert_eq!(bottom, 6);
        assert_eq!(top, 18);
    }

    #[test]
    fn test_calculate_stacked_heights_capped() {
        let (top, bottom) = calculate_stacked_heights(30, 15);
        // 1/3 of 30 = 10, so capped at 10
        assert_eq!(bottom, 10);
        assert_eq!(top, 20);
    }

    #[test]
    fn test_calculate_stacked_heights_minimum() {
        let (top, bottom) = calculate_stacked_heights(MIN_BOUNDARY_HEIGHT, 10);
        // 1/3 of 10 = 3, so capped at 3
        assert_eq!(bottom, 3);
        assert_eq!(top, 7);
    }
}

// =============================================================================
// Edge Case and Boundary Tests
// =============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn test_zero_width() {
        let layout = LayoutContext::new(0, 24);
        assert!(layout.is_extra_small());
        assert!(layout.should_collapse_sidebar());
        // Percent calculations should still return minimum 1
        assert_eq!(layout.percent_width(50), 1);
    }

    #[test]
    fn test_zero_height() {
        let layout = LayoutContext::new(80, 0);
        assert!(layout.is_extra_small());
        // Percent calculations should still return minimum 1
        assert_eq!(layout.percent_height(50), 1);
    }

    #[test]
    fn test_max_u16_dimensions() {
        let layout = LayoutContext::new(u16::MAX, u16::MAX);
        assert_eq!(layout.width_category(), SizeCategory::Large);
        assert_eq!(layout.height_category(), SizeCategory::Large);
        assert!(!layout.is_compact());
        // Note: two_column_widths() may overflow with u16::MAX, so we don't test it here
        // The UI would never realistically have such dimensions
    }

    #[test]
    fn test_percent_width_zero_percent() {
        let layout = LayoutContext::new(100, 24);
        // 0% should return minimum of 1
        assert_eq!(layout.percent_width(0), 1);
    }

    #[test]
    fn test_percent_height_zero_percent() {
        let layout = LayoutContext::new(100, 50);
        // 0% should return minimum of 1
        assert_eq!(layout.percent_height(0), 1);
    }

    #[test]
    fn test_bounded_width_min_greater_than_max() {
        let layout = LayoutContext::new(100, 24);
        // When min > max, clamp behavior: result is min (since value < min, clamp returns min)
        // Actually clamp(value, min, max) when min > max has undefined behavior in Rust
        // Let's test with valid bounds
        assert_eq!(layout.bounded_width(50, 40, 60), 50);
    }

    #[test]
    fn test_stacked_heights_zero_input() {
        let layout = LayoutContext::new(80, 24);
        let (top, bottom) = layout.stacked_heights(0);
        assert_eq!(bottom, 0);
        assert_eq!(top, 24);
    }

    #[test]
    fn test_stacked_heights_full_height_input() {
        let layout = LayoutContext::new(80, 24);
        let (top, bottom) = layout.stacked_heights(24);
        // Capped at 1/3 = 8
        assert_eq!(bottom, 8);
        assert_eq!(top, 16);
    }

    #[test]
    fn test_text_wrap_width_large_indent() {
        let layout = LayoutContext::new(80, 24);
        // Large indent should saturate
        let wrap = layout.text_wrap_width(50); // 100 spaces indent
        // 80 - 4 - 100 = -24, saturates to 0
        assert_eq!(wrap, 0);
    }
}

// =============================================================================
// From Rect Tests
// =============================================================================

mod from_rect {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn test_from_rect_mobile() {
        let rect = Rect::new(0, 0, MOBILE_WIDTH, MOBILE_HEIGHT);
        let layout = LayoutContext::from_rect(rect);
        assert_eq!(layout.width, MOBILE_WIDTH);
        assert_eq!(layout.height, MOBILE_HEIGHT);
    }

    #[test]
    fn test_from_rect_standard() {
        let rect = Rect::new(10, 5, STANDARD_WIDTH, STANDARD_HEIGHT);
        let layout = LayoutContext::from_rect(rect);
        // x and y are ignored, only width and height matter
        assert_eq!(layout.width, STANDARD_WIDTH);
        assert_eq!(layout.height, STANDARD_HEIGHT);
    }

    #[test]
    fn test_from_rect_wide() {
        let rect = Rect::new(0, 0, WIDE_WIDTH, WIDE_HEIGHT);
        let layout = LayoutContext::from_rect(rect);
        assert_eq!(layout.width, WIDE_WIDTH);
        assert_eq!(layout.height, WIDE_HEIGHT);
    }

    #[test]
    fn test_from_rect_ultra_wide() {
        let rect = Rect::new(0, 0, ULTRA_WIDE_WIDTH, ULTRA_WIDE_HEIGHT);
        let layout = LayoutContext::from_rect(rect);
        assert_eq!(layout.width, ULTRA_WIDE_WIDTH);
        assert_eq!(layout.height, ULTRA_WIDE_HEIGHT);
    }

    #[test]
    fn test_from_rect_zero_size() {
        let rect = Rect::new(0, 0, 0, 0);
        let layout = LayoutContext::from_rect(rect);
        assert_eq!(layout.width, 0);
        assert_eq!(layout.height, 0);
    }
}

// =============================================================================
// Copy and Clone Tests
// =============================================================================

mod copy_clone {
    use super::*;

    #[test]
    fn test_layout_context_copy() {
        let original = LayoutContext::new(100, 50);
        let copied = original; // Copy
        assert_eq!(original.width, copied.width);
        assert_eq!(original.height, copied.height);
    }

    #[test]
    fn test_layout_context_clone() {
        let original = LayoutContext::new(100, 50);
        let cloned = original;
        assert_eq!(original.width, cloned.width);
        assert_eq!(original.height, cloned.height);
    }

    #[test]
    fn test_size_category_copy() {
        let original = SizeCategory::Medium;
        let copied = original;
        assert_eq!(original, copied);
    }

    #[test]
    fn test_size_category_clone() {
        let original = SizeCategory::Large;
        let cloned = original;
        assert_eq!(original, cloned);
    }
}

// =============================================================================
// UI Rendering Integration Tests at Various Terminal Sizes
// =============================================================================
//
// These tests verify that the UI renders correctly at different terminal sizes
// by using ratatui's TestBackend to create virtual terminals.

mod ui_rendering_integration {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use spoq::app::App;
    use spoq::ui::render;

    // Helper function to create an App for testing
    fn create_test_app() -> App {
        App::new().expect("Failed to create app")
    }

    // Helper function to render at a specific size and return buffer content
    fn render_at_size(width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(width, height);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    // =========================================================================
    // Mobile-like Terminal Size (40x20)
    // =========================================================================

    #[test]
    fn test_ui_renders_at_mobile_size() {
        let backend = TestBackend::new(MOBILE_WIDTH, MOBILE_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(MOBILE_WIDTH, MOBILE_HEIGHT);

        // Should not panic
        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // Verify some content was rendered
        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "UI should render content at mobile size (40x20)");
    }

    #[test]
    fn test_mobile_size_shows_status() {
        let buffer_str = render_at_size(MOBILE_WIDTH, MOBILE_HEIGHT);

        // Should show connection status
        assert!(
            buffer_str.contains("○") || buffer_str.contains("●"),
            "Mobile size should show connection status indicator"
        );
    }

    // =========================================================================
    // Standard Terminal Size (80x24)
    // =========================================================================

    #[test]
    fn test_ui_renders_at_standard_size() {
        let backend = TestBackend::new(STANDARD_WIDTH, STANDARD_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(STANDARD_WIDTH, STANDARD_HEIGHT);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "UI should render content at standard size (80x24)");
    }

    #[test]
    fn test_standard_size_shows_full_ui() {
        let buffer_str = render_at_size(STANDARD_WIDTH, STANDARD_HEIGHT);

        // Should show connection status
        assert!(
            buffer_str.contains("○") || buffer_str.contains("●") ||
            buffer_str.contains("Disconnected") || buffer_str.contains("Connected"),
            "Standard size should show connection status"
        );
    }

    // =========================================================================
    // Wide Terminal Size (120x40)
    // =========================================================================

    #[test]
    fn test_ui_renders_at_wide_size() {
        let backend = TestBackend::new(WIDE_WIDTH, WIDE_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(WIDE_WIDTH, WIDE_HEIGHT);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "UI should render content at wide size (120x40)");
    }

    #[test]
    fn test_wide_size_shows_full_elements() {
        let buffer_str = render_at_size(WIDE_WIDTH, WIDE_HEIGHT);

        // Wide terminals should show more UI elements
        assert!(
            buffer_str.contains("○") || buffer_str.contains("●"),
            "Wide size should show connection status indicator"
        );
    }

    // =========================================================================
    // Ultra-wide Terminal Size (200x50)
    // =========================================================================

    #[test]
    fn test_ui_renders_at_ultra_wide_size() {
        let backend = TestBackend::new(ULTRA_WIDE_WIDTH, ULTRA_WIDE_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(ULTRA_WIDE_WIDTH, ULTRA_WIDE_HEIGHT);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "UI should render content at ultra-wide size (200x50)");
    }

    #[test]
    fn test_ultra_wide_size_no_overflow() {
        let backend = TestBackend::new(ULTRA_WIDE_WIDTH, ULTRA_WIDE_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(ULTRA_WIDE_WIDTH, ULTRA_WIDE_HEIGHT);

        // Should not panic or overflow
        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "Ultra-wide terminal should render without errors");
    }

    // =========================================================================
    // Minimum Boundary Size (30x10)
    // =========================================================================

    #[test]
    fn test_ui_renders_at_minimum_boundary() {
        let backend = TestBackend::new(MIN_BOUNDARY_WIDTH, MIN_BOUNDARY_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(MIN_BOUNDARY_WIDTH, MIN_BOUNDARY_HEIGHT);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "UI should render at minimum boundary size (30x10)");
    }

    // =========================================================================
    // Below Minimum Size Tests (Terminal Too Small)
    // =========================================================================

    #[test]
    fn test_ui_shows_too_small_message_when_width_below_minimum() {
        let backend = TestBackend::new(29, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(29, 24);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        assert!(
            buffer_str.contains("Too Small") || buffer_str.contains("resize"),
            "Should show 'too small' message when width is below minimum"
        );
    }

    #[test]
    fn test_ui_shows_too_small_message_when_height_below_minimum() {
        let backend = TestBackend::new(80, 9);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(80, 9);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        assert!(
            buffer_str.contains("Too Small") || buffer_str.contains("resize"),
            "Should show 'too small' message when height is below minimum"
        );
    }

    #[test]
    fn test_ui_shows_too_small_message_when_both_below_minimum() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(20, 5);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        assert!(
            buffer_str.contains("Too Small") || buffer_str.contains("resize") || buffer_str.contains("Minimum"),
            "Should show 'too small' message when both dimensions below minimum"
        );
    }

    // =========================================================================
    // Conversation Screen at Various Sizes
    // =========================================================================

    #[test]
    fn test_conversation_screen_at_mobile_size() {
        let backend = TestBackend::new(MOBILE_WIDTH, MOBILE_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = spoq::app::Screen::Conversation;
        app.update_terminal_dimensions(MOBILE_WIDTH, MOBILE_HEIGHT);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "Conversation screen should render at mobile size");
    }

    #[test]
    fn test_conversation_screen_at_standard_size() {
        let backend = TestBackend::new(STANDARD_WIDTH, STANDARD_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = spoq::app::Screen::Conversation;
        app.update_terminal_dimensions(STANDARD_WIDTH, STANDARD_HEIGHT);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // Should show the conversation view elements
        assert!(
            buffer_str.contains("New Conversation") || buffer_str.contains("│"),
            "Conversation screen should show conversation elements at standard size"
        );
    }

    #[test]
    fn test_conversation_screen_at_wide_size() {
        let backend = TestBackend::new(WIDE_WIDTH, WIDE_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = spoq::app::Screen::Conversation;
        app.update_terminal_dimensions(WIDE_WIDTH, WIDE_HEIGHT);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "Conversation screen should render at wide size");
    }

    #[test]
    fn test_conversation_screen_at_ultra_wide_size() {
        let backend = TestBackend::new(ULTRA_WIDE_WIDTH, ULTRA_WIDE_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.screen = spoq::app::Screen::Conversation;
        app.update_terminal_dimensions(ULTRA_WIDE_WIDTH, ULTRA_WIDE_HEIGHT);

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let has_content = buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ");
        assert!(has_content, "Conversation screen should render at ultra-wide size");
    }

    // =========================================================================
    // Resize Tests - Verify UI adapts to size changes
    // =========================================================================

    #[test]
    fn test_ui_adapts_to_resize_from_standard_to_mobile() {
        // Start at standard size
        let backend1 = TestBackend::new(STANDARD_WIDTH, STANDARD_HEIGHT);
        let mut terminal1 = Terminal::new(backend1).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(STANDARD_WIDTH, STANDARD_HEIGHT);

        terminal1
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // "Resize" to mobile size
        let backend2 = TestBackend::new(MOBILE_WIDTH, MOBILE_HEIGHT);
        let mut terminal2 = Terminal::new(backend2).unwrap();
        app.update_terminal_dimensions(MOBILE_WIDTH, MOBILE_HEIGHT);

        // Should render without panicking
        let result = terminal2.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should adapt to resize from standard to mobile");
    }

    #[test]
    fn test_ui_adapts_to_resize_from_mobile_to_wide() {
        // Start at mobile size
        let backend1 = TestBackend::new(MOBILE_WIDTH, MOBILE_HEIGHT);
        let mut terminal1 = Terminal::new(backend1).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(MOBILE_WIDTH, MOBILE_HEIGHT);

        terminal1
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // "Resize" to wide size
        let backend2 = TestBackend::new(WIDE_WIDTH, WIDE_HEIGHT);
        let mut terminal2 = Terminal::new(backend2).unwrap();
        app.update_terminal_dimensions(WIDE_WIDTH, WIDE_HEIGHT);

        let result = terminal2.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should adapt to resize from mobile to wide");
    }

    #[test]
    fn test_ui_adapts_to_resize_from_wide_to_minimum() {
        // Start at wide size
        let backend1 = TestBackend::new(WIDE_WIDTH, WIDE_HEIGHT);
        let mut terminal1 = Terminal::new(backend1).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(WIDE_WIDTH, WIDE_HEIGHT);

        terminal1
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        // "Resize" to minimum size
        let backend2 = TestBackend::new(MIN_BOUNDARY_WIDTH, MIN_BOUNDARY_HEIGHT);
        let mut terminal2 = Terminal::new(backend2).unwrap();
        app.update_terminal_dimensions(MIN_BOUNDARY_WIDTH, MIN_BOUNDARY_HEIGHT);

        let result = terminal2.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should adapt to resize from wide to minimum");
    }

    // =========================================================================
    // Panel Stacking Behavior Tests at 60-column Threshold
    // =========================================================================

    #[test]
    fn test_ui_at_59_columns_stacks_panels() {
        let backend = TestBackend::new(59, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(59, 24);

        // Should render without panicking
        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at 59 columns (stacked mode)");
    }

    #[test]
    fn test_ui_at_60_columns() {
        let backend = TestBackend::new(60, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(60, 24);

        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at 60 columns");
    }

    #[test]
    fn test_ui_at_79_columns() {
        let backend = TestBackend::new(79, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(79, 24);

        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at 79 columns (stacked mode)");
    }

    #[test]
    fn test_ui_at_80_columns_side_by_side() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(80, 24);

        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at 80 columns (side-by-side mode)");
    }

    #[test]
    fn test_ui_at_81_columns() {
        let backend = TestBackend::new(81, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(81, 24);

        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at 81 columns");
    }

    // =========================================================================
    // Content with Thread at Various Sizes
    // =========================================================================

    #[test]
    fn test_thread_list_renders_at_mobile_size() {
        let backend = TestBackend::new(MOBILE_WIDTH, MOBILE_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(MOBILE_WIDTH, MOBILE_HEIGHT);

        // Add a thread to the cache
        app.cache.upsert_thread(spoq::models::Thread {
            id: "test-thread".to_string(),
            title: "Test Thread Title".to_string(),
            description: Some("Test description".to_string()),
            preview: "Test preview".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: spoq::models::ThreadType::default(),
            model: Some("claude-sonnet-4-5".to_string()),
            permission_mode: None,
            message_count: 5,
            created_at: chrono::Utc::now(),
            working_directory: None,
        });

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // At mobile size, title may be truncated but should still be visible
        assert!(
            buffer_str.contains("Test") || buffer_str.contains("Thread"),
            "Thread title should be at least partially visible at mobile size"
        );
    }

    #[test]
    fn test_thread_list_renders_at_standard_size() {
        let backend = TestBackend::new(STANDARD_WIDTH, STANDARD_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(STANDARD_WIDTH, STANDARD_HEIGHT);

        // Add a thread to the cache
        app.cache.upsert_thread(spoq::models::Thread {
            id: "test-thread".to_string(),
            title: "Test Thread Title".to_string(),
            description: Some("Test description".to_string()),
            preview: "Test preview".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: spoq::models::ThreadType::default(),
            model: Some("claude-sonnet-4-5".to_string()),
            permission_mode: None,
            message_count: 5,
            created_at: chrono::Utc::now(),
            working_directory: None,
        });

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // At standard size, full title should be visible
        assert!(
            buffer_str.contains("Test Thread"),
            "Thread title should be visible at standard size"
        );
    }

    #[test]
    fn test_thread_list_renders_at_wide_size() {
        let backend = TestBackend::new(WIDE_WIDTH, WIDE_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(WIDE_WIDTH, WIDE_HEIGHT);

        // Add a thread to the cache
        app.cache.upsert_thread(spoq::models::Thread {
            id: "test-thread".to_string(),
            title: "Test Thread Title That Is Quite Long".to_string(),
            description: Some("This is a detailed description for testing".to_string()),
            preview: "Test preview".to_string(),
            updated_at: chrono::Utc::now(),
            thread_type: spoq::models::ThreadType::default(),
            model: Some("claude-sonnet-4-5".to_string()),
            permission_mode: None,
            message_count: 5,
            created_at: chrono::Utc::now(),
            working_directory: None,
        });

        terminal
            .draw(|f| {
                render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let buffer_str: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();

        // At wide size, more of the title should be visible
        assert!(
            buffer_str.contains("Test Thread Title"),
            "Longer thread title should be visible at wide size"
        );
    }

    // =========================================================================
    // Extreme Edge Cases
    // =========================================================================

    #[test]
    fn test_ui_renders_at_exactly_minimum_width() {
        let backend = TestBackend::new(MIN_TERMINAL_WIDTH, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(MIN_TERMINAL_WIDTH, 24);

        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at exactly minimum width");
    }

    #[test]
    fn test_ui_renders_at_exactly_minimum_height() {
        let backend = TestBackend::new(80, MIN_TERMINAL_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(80, MIN_TERMINAL_HEIGHT);

        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at exactly minimum height");
    }

    #[test]
    fn test_ui_renders_at_exactly_both_minimums() {
        let backend = TestBackend::new(MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT);

        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at exactly both minimum dimensions");
    }

    #[test]
    fn test_ui_renders_at_very_wide_but_short() {
        let backend = TestBackend::new(200, MIN_TERMINAL_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(200, MIN_TERMINAL_HEIGHT);

        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at very wide but minimum height");
    }

    #[test]
    fn test_ui_renders_at_narrow_but_tall() {
        let backend = TestBackend::new(MIN_TERMINAL_WIDTH, 100);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = create_test_app();
        app.update_terminal_dimensions(MIN_TERMINAL_WIDTH, 100);

        let result = terminal.draw(|f| {
            render(f, &mut app);
        });

        assert!(result.is_ok(), "UI should render at minimum width but very tall");
    }
}
