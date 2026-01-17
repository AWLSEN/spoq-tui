//! Text selection and clipboard support
//!
//! This module provides infrastructure for selecting text in the TUI
//! and copying it to the system clipboard. It includes:
//!
//! - Position types for mapping screen coordinates to content positions
//! - Selection state management (anchor, current position, mode)
//! - Selection range calculation and normalization
//!
//! # Architecture
//!
//! Text selection in a TUI requires mapping between two coordinate systems:
//!
//! 1. **Screen coordinates**: Terminal cells (column, row) where the mouse is clicked
//! 2. **Content coordinates**: Position in the text content (line, character offset)
//!
//! The `position` module provides types for both, plus mappings between them.
//! The `state` module provides the selection state machine.
//!
//! # Selection Modes
//!
//! Three selection granularities are supported:
//!
//! - **Character**: Select individual characters (single click + drag)
//! - **Word**: Select whole words (double-click to initiate)
//! - **Line**: Select entire lines (triple-click to initiate)
//!
//! # Usage
//!
//! ```ignore
//! use spoq::selection::{SelectionState, SelectionMode, ContentPosition};
//!
//! let mut state = SelectionState::new();
//!
//! // On mouse down
//! state.start_selection(ContentPosition::new(0, 5), SelectionMode::Character);
//!
//! // On mouse drag
//! state.update_selection(ContentPosition::new(2, 10));
//!
//! // On mouse up
//! state.finish_selection();
//!
//! // Get selected text range for copying
//! if let Some(range) = state.get_range() {
//!     // Extract text from content using range.start and range.end
//! }
//! ```

pub mod multi_click;
pub mod position;
pub mod state;

// Re-export commonly used types at the module level
pub use multi_click::ClickDetector;
pub use position::{
    ContentPosition, PositionMappingIndex, ScreenPosition, ScreenToContentMapping,
    // Unicode width utilities
    build_line_mappings, build_position_index, char_display_width, char_index_to_screen_col,
    screen_col_to_char_index, string_display_width, wrap_string_to_width,
};
pub use state::{SelectionAnchor, SelectionMode, SelectionRange, SelectionState};

#[cfg(test)]
mod tests {
    use super::*;

    /// Integration test: Complete selection workflow
    #[test]
    fn test_selection_workflow() {
        let mut state = SelectionState::new();

        // Initial state
        assert!(!state.has_selection());
        assert!(!state.is_active());

        // Mouse down - start selection
        let start_pos = ContentPosition::new(2, 5);
        state.start_selection(start_pos, SelectionMode::Character);

        assert!(state.is_active());
        assert!(!state.has_selection()); // Cursor, not a selection

        // Mouse drag - extend selection
        let mid_pos = ContentPosition::new(3, 15);
        state.update_selection(mid_pos);

        assert!(state.is_active());
        assert!(state.has_selection());

        let range = state.get_range().unwrap();
        assert_eq!(range.start, start_pos);
        assert_eq!(range.end, mid_pos);

        // Continue dragging
        let end_pos = ContentPosition::new(5, 20);
        state.update_selection(end_pos);

        let range = state.get_range().unwrap();
        assert_eq!(range.end, end_pos);

        // Mouse up - finish selection
        state.finish_selection();

        assert!(!state.is_active());
        assert!(state.has_selection()); // Selection preserved

        // Click elsewhere - clear selection
        state.clear();

        assert!(!state.has_selection());
    }

    /// Integration test: Double-click word selection
    #[test]
    fn test_word_selection_mode() {
        let mut state = SelectionState::new();

        // Double-click initiates word selection
        let pos = ContentPosition::new(1, 10);
        state.start_selection(pos, SelectionMode::Word);

        assert_eq!(state.mode(), SelectionMode::Word);

        // Drag extends word selection
        state.update_selection(ContentPosition::new(1, 25));
        state.finish_selection();

        let range = state.get_range().unwrap();
        assert_eq!(range.mode, SelectionMode::Word);
    }

    /// Integration test: Screen to content position mapping
    #[test]
    fn test_screen_to_content_mapping() {
        // Simulate a wrapped line: "Hello world" that wraps at column 5
        let mappings = vec![
            ScreenToContentMapping::new(0, 0, 0, 6),  // "Hello " on first screen line
            ScreenToContentMapping::new(1, 0, 6, 11), // "world" on second screen line
        ];

        // Click on 'w' in "world" (screen row 1, column 0)
        let screen_pos = ScreenPosition::new(0, 1);

        // Find the mapping for this screen row
        let mapping = mappings.iter().find(|m| m.screen_row == screen_pos.y);
        assert!(mapping.is_some());

        let mapping = mapping.unwrap();
        let content_col = mapping.screen_column_to_content(screen_pos.x);

        assert_eq!(content_col, Some(6)); // 'w' is at column 6 in the content
    }

    /// Integration test: Check if screen position is in content area
    #[test]
    fn test_screen_position_in_content_area() {
        let pos = ScreenPosition::new(50, 20);

        // Content area starts at (10, 5) with size 80x30
        assert!(pos.is_within(10, 5, 80, 30));

        // Convert to content-area-relative position
        let relative = pos.offset_from(10, 5);
        assert_eq!(relative.x, 40);
        assert_eq!(relative.y, 15);
    }

    /// Integration test: Selection range line intersection
    #[test]
    fn test_multi_line_selection() {
        let range = SelectionRange::new(
            ContentPosition::new(5, 10),  // Line 5, column 10
            ContentPosition::new(8, 20),  // Line 8, column 20
            SelectionMode::Character,
        );

        // Check each line in the selection
        assert!(!range.intersects_line(4)); // Before selection
        assert!(range.intersects_line(5));  // Start line
        assert!(range.intersects_line(6));  // Middle line
        assert!(range.intersects_line(7));  // Middle line
        assert!(range.intersects_line(8));  // End line
        assert!(!range.intersects_line(9)); // After selection

        // Get column ranges for each line
        assert_eq!(range.columns_for_line(5, 80), Some((10, 80))); // Start to end of line
        assert_eq!(range.columns_for_line(6, 80), Some((0, 80)));  // Full line
        assert_eq!(range.columns_for_line(7, 80), Some((0, 80)));  // Full line
        assert_eq!(range.columns_for_line(8, 80), Some((0, 20)));  // Start to end column
    }

    /// Test that positions can be compared and sorted
    #[test]
    fn test_content_position_ordering() {
        let positions = vec![
            ContentPosition::new(2, 10),
            ContentPosition::new(1, 5),
            ContentPosition::new(2, 5),
            ContentPosition::new(1, 10),
        ];

        let mut sorted = positions.clone();
        sorted.sort();

        assert_eq!(sorted[0], ContentPosition::new(1, 5));
        assert_eq!(sorted[1], ContentPosition::new(1, 10));
        assert_eq!(sorted[2], ContentPosition::new(2, 5));
        assert_eq!(sorted[3], ContentPosition::new(2, 10));
    }
}
