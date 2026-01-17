//! Position types for text selection
//!
//! This module defines types for mapping between screen coordinates (x, y pixels/cells)
//! and content positions (line, column in the text). These are essential for:
//! - Converting mouse click positions to text positions
//! - Determining which characters are within a selection range
//! - Rendering selection highlights at the correct screen positions

use serde::{Deserialize, Serialize};

/// Screen position in terminal cells (column, row)
///
/// Represents a position on the screen in terminal cell coordinates.
/// The origin (0, 0) is at the top-left corner of the terminal.
///
/// # Fields
/// - `x`: Column position (0-indexed, left to right)
/// - `y`: Row position (0-indexed, top to bottom)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ScreenPosition {
    /// Column position (0-indexed from left edge)
    pub x: u16,
    /// Row position (0-indexed from top edge)
    pub y: u16,
}

impl ScreenPosition {
    /// Create a new screen position
    ///
    /// # Arguments
    /// * `x` - Column position (0-indexed)
    /// * `y` - Row position (0-indexed)
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    /// Check if this position is within a rectangular area
    ///
    /// # Arguments
    /// * `area_x` - Left edge of the area (inclusive)
    /// * `area_y` - Top edge of the area (inclusive)
    /// * `area_width` - Width of the area
    /// * `area_height` - Height of the area
    pub fn is_within(&self, area_x: u16, area_y: u16, area_width: u16, area_height: u16) -> bool {
        self.x >= area_x
            && self.x < area_x.saturating_add(area_width)
            && self.y >= area_y
            && self.y < area_y.saturating_add(area_height)
    }

    /// Calculate the offset from an area's origin
    ///
    /// Returns the position relative to the top-left corner of the given area.
    /// Useful for converting absolute screen positions to widget-relative positions.
    ///
    /// # Arguments
    /// * `area_x` - Left edge of the area
    /// * `area_y` - Top edge of the area
    pub fn offset_from(&self, area_x: u16, area_y: u16) -> ScreenPosition {
        ScreenPosition {
            x: self.x.saturating_sub(area_x),
            y: self.y.saturating_sub(area_y),
        }
    }
}

/// Content position in text (line number, column/character offset)
///
/// Represents a position within the text content, independent of how it's
/// rendered on screen. This is used to track selection anchors and ranges
/// within the actual text data.
///
/// # Fields
/// - `line`: Line number (0-indexed from the start of content)
/// - `column`: Character offset within the line (0-indexed, grapheme-aware)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ContentPosition {
    /// Line number (0-indexed)
    pub line: usize,
    /// Character/grapheme offset within the line (0-indexed)
    pub column: usize,
}

impl ContentPosition {
    /// Create a new content position
    ///
    /// # Arguments
    /// * `line` - Line number (0-indexed)
    /// * `column` - Character offset within the line (0-indexed)
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Create a position at the start of a line
    ///
    /// # Arguments
    /// * `line` - Line number (0-indexed)
    pub fn line_start(line: usize) -> Self {
        Self { line, column: 0 }
    }

    /// Check if this position comes before another in document order
    ///
    /// Document order is: lines first, then columns within the same line.
    ///
    /// # Arguments
    /// * `other` - The position to compare against
    pub fn is_before(&self, other: &ContentPosition) -> bool {
        self.line < other.line || (self.line == other.line && self.column < other.column)
    }

    /// Check if this position comes after another in document order
    ///
    /// # Arguments
    /// * `other` - The position to compare against
    pub fn is_after(&self, other: &ContentPosition) -> bool {
        self.line > other.line || (self.line == other.line && self.column > other.column)
    }

    /// Get the minimum (earlier) of two positions
    ///
    /// Returns the position that comes first in document order.
    pub fn min(self, other: ContentPosition) -> ContentPosition {
        if self.is_before(&other) {
            self
        } else {
            other
        }
    }

    /// Get the maximum (later) of two positions
    ///
    /// Returns the position that comes last in document order.
    pub fn max(self, other: ContentPosition) -> ContentPosition {
        if self.is_after(&other) {
            self
        } else {
            other
        }
    }
}

impl PartialOrd for ContentPosition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ContentPosition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.line.cmp(&other.line) {
            std::cmp::Ordering::Equal => self.column.cmp(&other.column),
            ordering => ordering,
        }
    }
}

/// Represents a mapping between a screen row and its content
///
/// This is used to map rendered lines back to their source content positions.
/// Essential for converting mouse clicks to text positions when content
/// is wrapped or transformed during rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenToContentMapping {
    /// Screen row (relative to the content area, not absolute screen position)
    pub screen_row: u16,
    /// Source line in the content
    pub content_line: usize,
    /// Start column in the content line (for wrapped lines)
    pub content_start_column: usize,
    /// End column in the content line (exclusive, for wrapped lines)
    pub content_end_column: usize,
}

impl ScreenToContentMapping {
    /// Create a new screen-to-content mapping
    ///
    /// # Arguments
    /// * `screen_row` - The screen row (0-indexed, relative to content area)
    /// * `content_line` - The source line number
    /// * `content_start_column` - Start column offset (for wrapped lines)
    /// * `content_end_column` - End column offset (exclusive)
    pub fn new(
        screen_row: u16,
        content_line: usize,
        content_start_column: usize,
        content_end_column: usize,
    ) -> Self {
        Self {
            screen_row,
            content_line,
            content_start_column,
            content_end_column,
        }
    }

    /// Convert a screen column to a content column
    ///
    /// Given a column position on screen, returns the corresponding column
    /// in the content. Returns None if the screen column is beyond the
    /// content for this line segment.
    ///
    /// # Arguments
    /// * `screen_column` - Column position on screen (0-indexed)
    pub fn screen_column_to_content(&self, screen_column: u16) -> Option<usize> {
        let content_column = self.content_start_column + screen_column as usize;
        if content_column < self.content_end_column {
            Some(content_column)
        } else {
            // Return the end of the line segment
            Some(self.content_end_column.saturating_sub(1).max(self.content_start_column))
        }
    }

    /// Check if a content column falls within this line segment
    ///
    /// # Arguments
    /// * `column` - Content column to check
    pub fn contains_column(&self, column: usize) -> bool {
        column >= self.content_start_column && column < self.content_end_column
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============= ScreenPosition Tests =============

    #[test]
    fn test_screen_position_new() {
        let pos = ScreenPosition::new(10, 20);
        assert_eq!(pos.x, 10);
        assert_eq!(pos.y, 20);
    }

    #[test]
    fn test_screen_position_default() {
        let pos = ScreenPosition::default();
        assert_eq!(pos.x, 0);
        assert_eq!(pos.y, 0);
    }

    #[test]
    fn test_screen_position_is_within() {
        let pos = ScreenPosition::new(5, 5);

        // Within the area
        assert!(pos.is_within(0, 0, 10, 10));
        assert!(pos.is_within(5, 5, 1, 1)); // Exactly at the corner

        // Outside the area
        assert!(!pos.is_within(6, 0, 10, 10)); // Left of area
        assert!(!pos.is_within(0, 6, 10, 10)); // Above area
        assert!(!pos.is_within(0, 0, 5, 10)); // Right of area
        assert!(!pos.is_within(0, 0, 10, 5)); // Below area
    }

    #[test]
    fn test_screen_position_is_within_edge_cases() {
        let pos = ScreenPosition::new(0, 0);

        // At origin
        assert!(pos.is_within(0, 0, 1, 1));
        assert!(!pos.is_within(0, 0, 0, 0)); // Zero-size area
    }

    #[test]
    fn test_screen_position_offset_from() {
        let pos = ScreenPosition::new(10, 20);
        let offset = pos.offset_from(5, 10);
        assert_eq!(offset.x, 5);
        assert_eq!(offset.y, 10);
    }

    #[test]
    fn test_screen_position_offset_from_underflow() {
        let pos = ScreenPosition::new(5, 5);
        let offset = pos.offset_from(10, 10);
        assert_eq!(offset.x, 0); // Saturates to 0
        assert_eq!(offset.y, 0);
    }

    #[test]
    fn test_screen_position_equality() {
        let pos1 = ScreenPosition::new(5, 10);
        let pos2 = ScreenPosition::new(5, 10);
        let pos3 = ScreenPosition::new(5, 11);

        assert_eq!(pos1, pos2);
        assert_ne!(pos1, pos3);
    }

    #[test]
    fn test_screen_position_clone() {
        let pos1 = ScreenPosition::new(5, 10);
        let pos2 = pos1;
        assert_eq!(pos1, pos2);
    }

    #[test]
    fn test_screen_position_serialization() {
        let pos = ScreenPosition::new(10, 20);
        let json = serde_json::to_string(&pos).expect("Failed to serialize");
        let deserialized: ScreenPosition = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(pos, deserialized);
    }

    // ============= ContentPosition Tests =============

    #[test]
    fn test_content_position_new() {
        let pos = ContentPosition::new(5, 10);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.column, 10);
    }

    #[test]
    fn test_content_position_default() {
        let pos = ContentPosition::default();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_content_position_line_start() {
        let pos = ContentPosition::line_start(5);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_content_position_is_before() {
        let pos1 = ContentPosition::new(0, 5);
        let pos2 = ContentPosition::new(0, 10);
        let pos3 = ContentPosition::new(1, 0);

        // Same line, different column
        assert!(pos1.is_before(&pos2));
        assert!(!pos2.is_before(&pos1));

        // Different lines
        assert!(pos1.is_before(&pos3));
        assert!(pos2.is_before(&pos3));
        assert!(!pos3.is_before(&pos1));

        // Same position
        assert!(!pos1.is_before(&pos1));
    }

    #[test]
    fn test_content_position_is_after() {
        let pos1 = ContentPosition::new(0, 5);
        let pos2 = ContentPosition::new(0, 10);
        let pos3 = ContentPosition::new(1, 0);

        // Same line, different column
        assert!(pos2.is_after(&pos1));
        assert!(!pos1.is_after(&pos2));

        // Different lines
        assert!(pos3.is_after(&pos1));
        assert!(pos3.is_after(&pos2));
        assert!(!pos1.is_after(&pos3));

        // Same position
        assert!(!pos1.is_after(&pos1));
    }

    #[test]
    fn test_content_position_min() {
        let pos1 = ContentPosition::new(1, 5);
        let pos2 = ContentPosition::new(2, 0);

        assert_eq!(pos1.min(pos2), pos1);
        assert_eq!(pos2.min(pos1), pos1);
        assert_eq!(pos1.min(pos1), pos1);
    }

    #[test]
    fn test_content_position_max() {
        let pos1 = ContentPosition::new(1, 5);
        let pos2 = ContentPosition::new(2, 0);

        assert_eq!(pos1.max(pos2), pos2);
        assert_eq!(pos2.max(pos1), pos2);
        assert_eq!(pos2.max(pos2), pos2);
    }

    #[test]
    fn test_content_position_ordering() {
        let pos1 = ContentPosition::new(0, 0);
        let pos2 = ContentPosition::new(0, 5);
        let pos3 = ContentPosition::new(1, 0);
        let pos4 = ContentPosition::new(1, 10);

        // Test Ord implementation
        assert!(pos1 < pos2);
        assert!(pos2 < pos3);
        assert!(pos3 < pos4);
        assert!(pos1 < pos4);

        // Test equality
        let pos1_copy = ContentPosition::new(0, 0);
        assert!(pos1 == pos1_copy);
        assert!(!(pos1 < pos1_copy));
        assert!(!(pos1 > pos1_copy));
    }

    #[test]
    fn test_content_position_serialization() {
        let pos = ContentPosition::new(10, 20);
        let json = serde_json::to_string(&pos).expect("Failed to serialize");
        let deserialized: ContentPosition = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(pos, deserialized);
    }

    // ============= ScreenToContentMapping Tests =============

    #[test]
    fn test_mapping_new() {
        let mapping = ScreenToContentMapping::new(5, 10, 0, 80);
        assert_eq!(mapping.screen_row, 5);
        assert_eq!(mapping.content_line, 10);
        assert_eq!(mapping.content_start_column, 0);
        assert_eq!(mapping.content_end_column, 80);
    }

    #[test]
    fn test_mapping_screen_column_to_content() {
        // Full line mapping (no wrapping)
        let mapping = ScreenToContentMapping::new(0, 0, 0, 80);
        assert_eq!(mapping.screen_column_to_content(0), Some(0));
        assert_eq!(mapping.screen_column_to_content(40), Some(40));
        assert_eq!(mapping.screen_column_to_content(79), Some(79));
        assert_eq!(mapping.screen_column_to_content(100), Some(79)); // Beyond end, clamp to end

        // Wrapped line segment (second part of a wrapped line)
        let wrapped = ScreenToContentMapping::new(1, 0, 80, 120);
        assert_eq!(wrapped.screen_column_to_content(0), Some(80));
        assert_eq!(wrapped.screen_column_to_content(10), Some(90));
        assert_eq!(wrapped.screen_column_to_content(39), Some(119));
        assert_eq!(wrapped.screen_column_to_content(50), Some(119)); // Beyond end
    }

    #[test]
    fn test_mapping_screen_column_to_content_edge_cases() {
        // Single character line
        let single = ScreenToContentMapping::new(0, 0, 0, 1);
        assert_eq!(single.screen_column_to_content(0), Some(0));
        assert_eq!(single.screen_column_to_content(1), Some(0)); // Beyond end

        // Empty segment (shouldn't happen in practice but handle gracefully)
        let empty = ScreenToContentMapping::new(0, 0, 0, 0);
        assert_eq!(empty.screen_column_to_content(0), Some(0)); // Returns start
    }

    #[test]
    fn test_mapping_contains_column() {
        let mapping = ScreenToContentMapping::new(0, 0, 10, 20);

        assert!(!mapping.contains_column(9)); // Before start
        assert!(mapping.contains_column(10)); // At start
        assert!(mapping.contains_column(15)); // Middle
        assert!(mapping.contains_column(19)); // Last valid
        assert!(!mapping.contains_column(20)); // At end (exclusive)
        assert!(!mapping.contains_column(25)); // After end
    }

    #[test]
    fn test_mapping_equality() {
        let m1 = ScreenToContentMapping::new(0, 1, 0, 80);
        let m2 = ScreenToContentMapping::new(0, 1, 0, 80);
        let m3 = ScreenToContentMapping::new(1, 1, 0, 80);

        assert_eq!(m1, m2);
        assert_ne!(m1, m3);
    }
}
