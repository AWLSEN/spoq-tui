//! Responsive Layout System
//!
//! Provides a comprehensive `LayoutContext` that encapsulates terminal dimensions
//! and provides fluid sizing calculations for responsive UI rendering.
//!
//! The `LayoutContext` is passed to all render functions to enable proportional
//! sizing based on the current terminal dimensions.

// ============================================================================
// Screen Size Breakpoints
// ============================================================================

/// Terminal width breakpoints for responsive layouts
pub mod breakpoints {
    /// Extra small terminal (< 60 columns)
    pub const XS_WIDTH: u16 = 60;
    /// Small terminal (< 80 columns)
    pub const SM_WIDTH: u16 = 80;
    /// Medium terminal (< 120 columns)
    pub const MD_WIDTH: u16 = 120;
    /// Large terminal (< 160 columns)
    #[allow(dead_code)]
    pub const LG_WIDTH: u16 = 160;

    /// Extra small terminal height (< 16 rows)
    pub const XS_HEIGHT: u16 = 16;
    /// Small terminal height (< 24 rows)
    pub const SM_HEIGHT: u16 = 24;
    /// Medium terminal height (< 40 rows)
    pub const MD_HEIGHT: u16 = 40;
}

/// Size category for responsive design decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeCategory {
    /// Extra small (< 60 cols or < 16 rows)
    ExtraSmall,
    /// Small (< 80 cols or < 24 rows)
    Small,
    /// Medium (< 120 cols or < 40 rows)
    Medium,
    /// Large (>= 120 cols and >= 40 rows)
    Large,
}

// ============================================================================
// Layout Context
// ============================================================================

/// Layout context holding terminal dimensions for responsive calculations.
///
/// This struct is the core of the responsive layout system. It encapsulates
/// terminal dimensions and provides methods for calculating proportional sizes,
/// determining layout modes, and making responsive design decisions.
///
/// # Example
///
/// ```ignore
/// let ctx = LayoutContext::new(120, 40);
///
/// // Calculate proportional widths
/// let sidebar_width = ctx.percent_width(30);
/// let content_width = ctx.available_content_width(4);
///
/// // Make layout decisions
/// if ctx.should_stack_panels() {
///     // Use vertical stacking for narrow terminals
/// } else {
///     // Use horizontal side-by-side layout
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct LayoutContext {
    /// Terminal width in columns
    pub width: u16,
    /// Terminal height in rows
    pub height: u16,
}

impl LayoutContext {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a new layout context with the given dimensions.
    ///
    /// # Arguments
    /// * `width` - Terminal width in columns
    /// * `height` - Terminal height in rows
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }


    // ========================================================================
    // Percentage-Based Calculations
    // ========================================================================

    /// Calculate a width as a percentage of terminal width.
    ///
    /// # Arguments
    /// * `percentage` - Value between 0 and 100
    ///
    /// # Returns
    /// The calculated width in columns, minimum 1
    ///
    /// # Example
    /// ```ignore
    /// let ctx = LayoutContext::new(100, 40);
    /// assert_eq!(ctx.percent_width(50), 50);
    /// assert_eq!(ctx.percent_width(30), 30);
    /// ```
    pub fn percent_width(&self, percentage: u16) -> u16 {
        ((self.width as u32 * percentage as u32) / 100).max(1) as u16
    }

    /// Calculate a height as a percentage of terminal height.
    ///
    /// # Arguments
    /// * `percentage` - Value between 0 and 100
    ///
    /// # Returns
    /// The calculated height in rows, minimum 1
    pub fn percent_height(&self, percentage: u16) -> u16 {
        ((self.height as u32 * percentage as u32) / 100).max(1) as u16
    }

    /// Calculate proportional width with min/max bounds.
    ///
    /// This is useful when you want a percentage-based width but need to
    /// ensure it stays within reasonable bounds.
    ///
    /// # Arguments
    /// * `percentage` - Base percentage (0-100)
    /// * `min` - Minimum width
    /// * `max` - Maximum width
    ///
    /// # Example
    /// ```ignore
    /// let ctx = LayoutContext::new(200, 40);
    /// // 30% of 200 = 60, but clamped to max of 50
    /// assert_eq!(ctx.bounded_width(30, 20, 50), 50);
    /// ```
    pub fn bounded_width(&self, percentage: u16, min: u16, max: u16) -> u16 {
        self.percent_width(percentage).clamp(min, max)
    }

    /// Calculate proportional height with min/max bounds.
    ///
    /// # Arguments
    /// * `percentage` - Base percentage (0-100)
    /// * `min` - Minimum height
    /// * `max` - Maximum height
    pub fn bounded_height(&self, percentage: u16, min: u16, max: u16) -> u16 {
        self.percent_height(percentage).clamp(min, max)
    }

    // ========================================================================
    // Content Area Calculations
    // ========================================================================

    /// Get available content width after accounting for borders and margins.
    ///
    /// # Arguments
    /// * `border_width` - Total horizontal space used by borders/margins.
    ///   For example, 4 for left+right borders with padding.
    pub fn available_content_width(&self, border_width: u16) -> u16 {
        self.width.saturating_sub(border_width)
    }

    /// Get available content height after accounting for header/footer/chrome.
    ///
    /// # Arguments
    /// * `chrome_height` - Total vertical space used by header/footer/borders
    pub fn available_content_height(&self, chrome_height: u16) -> u16 {
        self.height.saturating_sub(chrome_height)
    }

    /// Calculate the text wrap width for content areas.
    ///
    /// This accounts for borders, margins, and optionally an indent level.
    ///
    /// # Arguments
    /// * `indent_level` - Number of indentation levels (each level = 2 spaces)
    pub fn text_wrap_width(&self, indent_level: u16) -> u16 {
        let border_margin = 4; // 2 for borders + 2 for padding
        let indent = indent_level * 2;
        self.width.saturating_sub(border_margin + indent)
    }

    // ========================================================================
    // Size Category Detection
    // ========================================================================

    /// Get the width size category.
    pub fn width_category(&self) -> SizeCategory {
        if self.width < breakpoints::XS_WIDTH {
            SizeCategory::ExtraSmall
        } else if self.width < breakpoints::SM_WIDTH {
            SizeCategory::Small
        } else if self.width < breakpoints::MD_WIDTH {
            SizeCategory::Medium
        } else {
            SizeCategory::Large
        }
    }

    /// Get the height size category.
    pub fn height_category(&self) -> SizeCategory {
        if self.height < breakpoints::XS_HEIGHT {
            SizeCategory::ExtraSmall
        } else if self.height < breakpoints::SM_HEIGHT {
            SizeCategory::Small
        } else if self.height < breakpoints::MD_HEIGHT {
            SizeCategory::Medium
        } else {
            SizeCategory::Large
        }
    }

    /// Check if the terminal is in a "narrow" state (less than 80 columns).
    pub fn is_narrow(&self) -> bool {
        self.width < breakpoints::SM_WIDTH
    }

    /// Check if the terminal is in a "short" state (less than 24 rows).
    pub fn is_short(&self) -> bool {
        self.height < breakpoints::SM_HEIGHT
    }

    /// Check if the terminal is in a "compact" state (narrow or short).
    ///
    /// Compact state indicates that UI elements should be condensed.
    pub fn is_compact(&self) -> bool {
        self.is_narrow() || self.is_short()
    }

    /// Check if the terminal is extra small (very constrained space).
    pub fn is_extra_small(&self) -> bool {
        self.width < breakpoints::XS_WIDTH || self.height < breakpoints::XS_HEIGHT
    }

    // ========================================================================
    // Layout Mode Decisions
    // ========================================================================

    /// Determine if panels should be stacked vertically instead of side-by-side.
    ///
    /// Returns `true` when the terminal is too narrow for a comfortable
    /// side-by-side panel layout (< 80 columns).
    pub fn should_stack_panels(&self) -> bool {
        self.width < breakpoints::SM_WIDTH
    }

    /// Determine if the sidebar should be collapsed/hidden.
    ///
    /// Returns `true` when the terminal is very narrow (< 60 columns).
    pub fn should_collapse_sidebar(&self) -> bool {
        self.width < breakpoints::XS_WIDTH
    }

    /// Determine if scrollbars should be shown.
    ///
    /// Returns `true` when there's enough space for scrollbar UI chrome.
    pub fn should_show_scrollbar(&self) -> bool {
        self.width >= breakpoints::XS_WIDTH
    }

    /// Determine if status badges should be shown in full or abbreviated.
    ///
    /// Returns `true` when there's enough space for full badge text.
    pub fn should_show_full_badges(&self) -> bool {
        self.width >= breakpoints::SM_WIDTH
    }

    /// Determine if tool result previews should be shown inline.
    ///
    /// Returns `true` when there's enough vertical space for previews.
    pub fn should_show_tool_previews(&self) -> bool {
        self.height >= breakpoints::SM_HEIGHT
    }

    // ========================================================================
    // Panel Layout Calculations
    // ========================================================================

    /// Calculate responsive panel widths for a two-column layout.
    ///
    /// Returns `(left_width, right_width)` based on terminal width:
    /// - Very narrow (< 60): Equal 50/50 split
    /// - Medium (< 120): 40/60 split
    /// - Wide (>= 120): 35/65 split with max left width of 60
    pub fn two_column_widths(&self) -> (u16, u16) {
        if self.width < breakpoints::XS_WIDTH {
            // Very narrow: equal split
            let half = self.width / 2;
            (half, self.width - half)
        } else if self.width < breakpoints::MD_WIDTH {
            // Medium: 40/60 split
            let left = (self.width * 40) / 100;
            (left, self.width - left)
        } else {
            // Wide: 35/65 split, with max left width
            let left = ((self.width * 35) / 100).min(60);
            (left, self.width - left)
        }
    }

    /// Calculate responsive panel heights for a stacked layout.
    ///
    /// Returns `(top_height, bottom_height)` based on terminal height.
    /// The bottom panel (typically input) is limited to 1/3 of height.
    ///
    /// # Arguments
    /// * `input_rows` - Desired number of rows for the input area
    pub fn stacked_heights(&self, input_rows: u16) -> (u16, u16) {
        let max_bottom = self.height / 3;
        let bottom = input_rows.min(max_bottom);
        let top = self.height.saturating_sub(bottom);
        (top, bottom)
    }

    /// Calculate the optimal header height based on terminal size.
    ///
    /// Returns a smaller header for compact terminals.
    pub fn header_height(&self) -> u16 {
        if self.is_compact() {
            3
        } else {
            9
        }
    }

    /// Calculate the optimal input area height based on terminal size.
    ///
    /// Returns a smaller input area for compact terminals.
    pub fn input_area_height(&self) -> u16 {
        if self.is_compact() {
            4
        } else {
            6
        }
    }

    // ========================================================================
    // Text Truncation Helpers
    // ========================================================================

    /// Calculate the maximum display length for a title/label.
    ///
    /// This is useful for truncating thread titles, file paths, etc.
    pub fn max_title_length(&self) -> usize {
        match self.width_category() {
            SizeCategory::ExtraSmall => 20,
            SizeCategory::Small => 30,
            SizeCategory::Medium => 50,
            SizeCategory::Large => 80,
        }
    }

    /// Calculate the maximum display length for a preview/description.
    pub fn max_preview_length(&self) -> usize {
        match self.width_category() {
            SizeCategory::ExtraSmall => 40,
            SizeCategory::Small => 60,
            SizeCategory::Medium => 100,
            SizeCategory::Large => 150,
        }
    }

    /// Calculate the maximum number of visible list items.
    ///
    /// This helps in virtualized list rendering to determine how many
    /// items to render.
    pub fn max_visible_items(&self) -> usize {
        // Account for header (3 rows) and some padding
        let available = self.height.saturating_sub(5);
        (available / 2).max(1) as usize // Assume 2 rows per item average
    }
}

impl Default for LayoutContext {
    /// Returns a default layout context with standard 80x24 terminal size.
    fn default() -> Self {
        Self {
            width: 80,
            height: 24,
        }
    }
}

// ============================================================================
// Legacy Helper Functions (for backwards compatibility)
// ============================================================================

/// Calculate responsive panel widths for a two-column layout.
///
/// Returns `(left_width, right_width)` based on terminal width.
/// The left panel gets more space on wider terminals.
///
/// **Deprecated**: Use `LayoutContext::two_column_widths()` instead.
pub fn calculate_two_column_widths(total_width: u16) -> (u16, u16) {
    let ctx = LayoutContext::new(total_width, 24);
    ctx.two_column_widths()
}

/// Calculate responsive panel heights for a stacked layout.
///
/// Returns `(top_height, bottom_height)` based on terminal height.
///
/// **Deprecated**: Use `LayoutContext::stacked_heights()` instead.
pub fn calculate_stacked_heights(total_height: u16, input_rows: u16) -> (u16, u16) {
    let ctx = LayoutContext::new(80, total_height);
    ctx.stacked_heights(input_rows)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Constructor Tests
    // ========================================================================

    #[test]
    fn test_new_layout_context() {
        let ctx = LayoutContext::new(120, 40);
        assert_eq!(ctx.width, 120);
        assert_eq!(ctx.height, 40);
    }

    #[test]
    fn test_default() {
        let ctx = LayoutContext::default();
        assert_eq!(ctx.width, 80);
        assert_eq!(ctx.height, 24);
    }

    // ========================================================================
    // Percentage Calculation Tests
    // ========================================================================

    #[test]
    fn test_percent_width() {
        let ctx = LayoutContext::new(100, 40);
        assert_eq!(ctx.percent_width(50), 50);
        assert_eq!(ctx.percent_width(30), 30);
        assert_eq!(ctx.percent_width(100), 100);
        assert_eq!(ctx.percent_width(0), 1); // Minimum of 1
    }

    #[test]
    fn test_percent_height() {
        let ctx = LayoutContext::new(100, 50);
        assert_eq!(ctx.percent_height(50), 25);
        assert_eq!(ctx.percent_height(20), 10);
        assert_eq!(ctx.percent_height(100), 50);
    }

    #[test]
    fn test_bounded_width() {
        let ctx = LayoutContext::new(200, 40);
        // 30% of 200 = 60, clamped to max of 50
        assert_eq!(ctx.bounded_width(30, 20, 50), 50);
        // 10% of 200 = 20, clamped to min of 25
        assert_eq!(ctx.bounded_width(10, 25, 50), 25);
        // 20% of 200 = 40, within bounds
        assert_eq!(ctx.bounded_width(20, 20, 50), 40);
    }

    #[test]
    fn test_bounded_height() {
        let ctx = LayoutContext::new(100, 100);
        assert_eq!(ctx.bounded_height(30, 10, 20), 20);
        assert_eq!(ctx.bounded_height(5, 10, 20), 10);
        assert_eq!(ctx.bounded_height(15, 10, 20), 15);
    }

    // ========================================================================
    // Content Area Tests
    // ========================================================================

    #[test]
    fn test_available_content_width() {
        let ctx = LayoutContext::new(100, 40);
        assert_eq!(ctx.available_content_width(4), 96);
        assert_eq!(ctx.available_content_width(10), 90);
    }

    #[test]
    fn test_available_content_width_saturates() {
        let ctx = LayoutContext::new(10, 40);
        assert_eq!(ctx.available_content_width(20), 0);
    }

    #[test]
    fn test_available_content_height() {
        let ctx = LayoutContext::new(100, 40);
        assert_eq!(ctx.available_content_height(10), 30);
    }

    #[test]
    fn test_text_wrap_width() {
        let ctx = LayoutContext::new(80, 24);
        // 80 - 4 (border/padding) - 0 (indent) = 76
        assert_eq!(ctx.text_wrap_width(0), 76);
        // 80 - 4 - 4 (2 indent levels * 2) = 72
        assert_eq!(ctx.text_wrap_width(2), 72);
    }

    // ========================================================================
    // Size Category Tests
    // ========================================================================

    #[test]
    fn test_width_category() {
        assert_eq!(
            LayoutContext::new(50, 24).width_category(),
            SizeCategory::ExtraSmall
        );
        assert_eq!(
            LayoutContext::new(70, 24).width_category(),
            SizeCategory::Small
        );
        assert_eq!(
            LayoutContext::new(100, 24).width_category(),
            SizeCategory::Medium
        );
        assert_eq!(
            LayoutContext::new(160, 24).width_category(),
            SizeCategory::Large
        );
    }

    #[test]
    fn test_height_category() {
        assert_eq!(
            LayoutContext::new(80, 10).height_category(),
            SizeCategory::ExtraSmall
        );
        assert_eq!(
            LayoutContext::new(80, 20).height_category(),
            SizeCategory::Small
        );
        assert_eq!(
            LayoutContext::new(80, 35).height_category(),
            SizeCategory::Medium
        );
        assert_eq!(
            LayoutContext::new(80, 50).height_category(),
            SizeCategory::Large
        );
    }

    #[test]
    fn test_is_narrow() {
        assert!(LayoutContext::new(60, 24).is_narrow());
        assert!(LayoutContext::new(79, 24).is_narrow());
        assert!(!LayoutContext::new(80, 24).is_narrow());
        assert!(!LayoutContext::new(120, 24).is_narrow());
    }

    #[test]
    fn test_is_short() {
        assert!(LayoutContext::new(80, 16).is_short());
        assert!(LayoutContext::new(80, 23).is_short());
        assert!(!LayoutContext::new(80, 24).is_short());
        assert!(!LayoutContext::new(80, 40).is_short());
    }

    #[test]
    fn test_is_compact() {
        // Narrow
        assert!(LayoutContext::new(60, 40).is_compact());
        // Short
        assert!(LayoutContext::new(120, 16).is_compact());
        // Both narrow and short
        assert!(LayoutContext::new(60, 16).is_compact());
        // Neither
        assert!(!LayoutContext::new(120, 40).is_compact());
    }

    #[test]
    fn test_is_extra_small() {
        assert!(LayoutContext::new(50, 40).is_extra_small());
        assert!(LayoutContext::new(100, 10).is_extra_small());
        assert!(!LayoutContext::new(80, 24).is_extra_small());
    }

    // ========================================================================
    // Layout Mode Decision Tests
    // ========================================================================

    #[test]
    fn test_should_stack_panels() {
        assert!(LayoutContext::new(60, 24).should_stack_panels());
        assert!(LayoutContext::new(79, 24).should_stack_panels());
        assert!(!LayoutContext::new(80, 24).should_stack_panels());
        assert!(!LayoutContext::new(120, 24).should_stack_panels());
    }

    #[test]
    fn test_should_collapse_sidebar() {
        assert!(LayoutContext::new(50, 24).should_collapse_sidebar());
        assert!(LayoutContext::new(59, 24).should_collapse_sidebar());
        assert!(!LayoutContext::new(60, 24).should_collapse_sidebar());
        assert!(!LayoutContext::new(80, 24).should_collapse_sidebar());
    }

    #[test]
    fn test_should_show_scrollbar() {
        assert!(!LayoutContext::new(50, 24).should_show_scrollbar());
        assert!(LayoutContext::new(60, 24).should_show_scrollbar());
        assert!(LayoutContext::new(80, 24).should_show_scrollbar());
    }

    #[test]
    fn test_should_show_full_badges() {
        assert!(!LayoutContext::new(60, 24).should_show_full_badges());
        assert!(LayoutContext::new(80, 24).should_show_full_badges());
        assert!(LayoutContext::new(120, 24).should_show_full_badges());
    }

    #[test]
    fn test_should_show_tool_previews() {
        assert!(!LayoutContext::new(80, 16).should_show_tool_previews());
        assert!(LayoutContext::new(80, 24).should_show_tool_previews());
        assert!(LayoutContext::new(80, 40).should_show_tool_previews());
    }

    // ========================================================================
    // Panel Layout Tests
    // ========================================================================

    #[test]
    fn test_two_column_widths_narrow() {
        let ctx = LayoutContext::new(50, 24);
        let (left, right) = ctx.two_column_widths();
        assert_eq!(left, 25);
        assert_eq!(right, 25);
    }

    #[test]
    fn test_two_column_widths_medium() {
        let ctx = LayoutContext::new(100, 24);
        let (left, right) = ctx.two_column_widths();
        assert_eq!(left, 40);
        assert_eq!(right, 60);
    }

    #[test]
    fn test_two_column_widths_wide() {
        let ctx = LayoutContext::new(200, 24);
        let (left, right) = ctx.two_column_widths();
        assert_eq!(left, 60); // Capped at 60
        assert_eq!(right, 140);
    }

    #[test]
    fn test_stacked_heights() {
        let ctx = LayoutContext::new(80, 30);
        let (top, bottom) = ctx.stacked_heights(6);
        assert_eq!(top, 24);
        assert_eq!(bottom, 6);
    }

    #[test]
    fn test_stacked_heights_large_input() {
        let ctx = LayoutContext::new(80, 30);
        // Input rows of 20 should be capped at 1/3 of 30 = 10
        let (top, bottom) = ctx.stacked_heights(20);
        assert_eq!(top, 20);
        assert_eq!(bottom, 10);
    }

    #[test]
    fn test_header_height() {
        // Compact terminal
        assert_eq!(LayoutContext::new(60, 40).header_height(), 3);
        assert_eq!(LayoutContext::new(100, 16).header_height(), 3);
        // Normal terminal
        assert_eq!(LayoutContext::new(100, 40).header_height(), 9);
    }

    #[test]
    fn test_input_area_height() {
        // Compact terminal
        assert_eq!(LayoutContext::new(60, 40).input_area_height(), 4);
        // Normal terminal
        assert_eq!(LayoutContext::new(100, 40).input_area_height(), 6);
    }

    // ========================================================================
    // Text Truncation Tests
    // ========================================================================

    #[test]
    fn test_max_title_length() {
        assert_eq!(LayoutContext::new(50, 24).max_title_length(), 20);
        assert_eq!(LayoutContext::new(70, 24).max_title_length(), 30);
        assert_eq!(LayoutContext::new(100, 24).max_title_length(), 50);
        assert_eq!(LayoutContext::new(160, 24).max_title_length(), 80);
    }

    #[test]
    fn test_max_preview_length() {
        assert_eq!(LayoutContext::new(50, 24).max_preview_length(), 40);
        assert_eq!(LayoutContext::new(70, 24).max_preview_length(), 60);
        assert_eq!(LayoutContext::new(100, 24).max_preview_length(), 100);
        assert_eq!(LayoutContext::new(160, 24).max_preview_length(), 150);
    }

    #[test]
    fn test_max_visible_items() {
        // 24 - 5 = 19, 19 / 2 = 9
        assert_eq!(LayoutContext::new(80, 24).max_visible_items(), 9);
        // 40 - 5 = 35, 35 / 2 = 17
        assert_eq!(LayoutContext::new(80, 40).max_visible_items(), 17);
        // Very short: 10 - 5 = 5, 5 / 2 = 2
        assert_eq!(LayoutContext::new(80, 10).max_visible_items(), 2);
        // Minimum of 1
        assert_eq!(LayoutContext::new(80, 5).max_visible_items(), 1);
    }

    // ========================================================================
    // Legacy Function Tests
    // ========================================================================

    #[test]
    fn test_calculate_two_column_widths_narrow() {
        let (left, right) = calculate_two_column_widths(50);
        assert_eq!(left, 25);
        assert_eq!(right, 25);
    }

    #[test]
    fn test_calculate_two_column_widths_medium() {
        let (left, right) = calculate_two_column_widths(100);
        assert_eq!(left, 40);
        assert_eq!(right, 60);
    }

    #[test]
    fn test_calculate_two_column_widths_wide() {
        let (left, right) = calculate_two_column_widths(200);
        assert_eq!(left, 60);
        assert_eq!(right, 140);
    }

    #[test]
    fn test_calculate_stacked_heights() {
        let (top, bottom) = calculate_stacked_heights(30, 6);
        assert_eq!(top, 24);
        assert_eq!(bottom, 6);
    }

    #[test]
    fn test_calculate_stacked_heights_capped() {
        let (top, bottom) = calculate_stacked_heights(30, 20);
        assert_eq!(top, 20);
        assert_eq!(bottom, 10);
    }
}
