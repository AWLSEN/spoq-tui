//! Position types for text selection
//!
//! This module defines types for mapping between screen coordinates (x, y pixels/cells)
//! and content positions (line, column in the text). These are essential for:
//! - Converting mouse click positions to text positions
//! - Determining which characters are within a selection range
//! - Rendering selection highlights at the correct screen positions

use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthChar;

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

// ============================================================================
// Position Mapping Index
// ============================================================================

/// A complete mapping index for a rendered content area.
///
/// This structure holds all the mappings from screen rows to content positions
/// for a rendered message area. It's built during rendering and used during
/// mouse event handling to convert screen clicks to content positions.
///
/// # Architecture
///
/// The mappings are stored in order of screen rows. Each mapping describes
/// what content appears on that screen row. For wrapped lines, multiple
/// screen rows may map to the same content line with different column ranges.
///
/// # Example
///
/// For content "Hello world" wrapped at width 6:
/// - Screen row 0: content line 0, columns 0-6 ("Hello ")
/// - Screen row 1: content line 0, columns 6-11 ("world")
#[derive(Debug, Clone, Default)]
pub struct PositionMappingIndex {
    /// Mappings from screen rows to content positions, sorted by screen_row
    mappings: Vec<ScreenToContentMapping>,
    /// The scroll offset (in visual lines) from the top of content
    scroll_offset: usize,
    /// The content area's left edge (for converting absolute screen x to relative)
    area_x: u16,
    /// The content area's top edge (for converting absolute screen y to relative)
    area_y: u16,
    /// The content area width
    area_width: u16,
    /// The content area height
    area_height: u16,
}

impl PositionMappingIndex {
    /// Create a new empty position mapping index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a position mapping index with the given content area bounds.
    ///
    /// # Arguments
    /// * `area_x` - Left edge of the content area
    /// * `area_y` - Top edge of the content area
    /// * `area_width` - Width of the content area
    /// * `area_height` - Height of the content area
    /// * `scroll_offset` - Current scroll position in visual lines
    pub fn with_area(
        area_x: u16,
        area_y: u16,
        area_width: u16,
        area_height: u16,
        scroll_offset: usize,
    ) -> Self {
        Self {
            mappings: Vec::new(),
            scroll_offset,
            area_x,
            area_y,
            area_width,
            area_height,
        }
    }

    /// Add a mapping for a screen row.
    ///
    /// Mappings should be added in order of screen rows for optimal performance.
    pub fn add_mapping(&mut self, mapping: ScreenToContentMapping) {
        self.mappings.push(mapping);
    }

    /// Add multiple mappings at once.
    pub fn add_mappings(&mut self, mappings: impl IntoIterator<Item = ScreenToContentMapping>) {
        self.mappings.extend(mappings);
    }

    /// Get the number of mappings in the index.
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Get all mappings.
    pub fn mappings(&self) -> &[ScreenToContentMapping] {
        &self.mappings
    }

    /// Get the scroll offset.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Set the scroll offset.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    /// Check if an absolute screen position is within the content area.
    ///
    /// # Arguments
    /// * `screen_pos` - Absolute screen position
    pub fn is_within_area(&self, screen_pos: ScreenPosition) -> bool {
        screen_pos.is_within(self.area_x, self.area_y, self.area_width, self.area_height)
    }

    /// Convert an absolute screen position to a content position.
    ///
    /// This is the main entry point for mouse click handling. It takes an
    /// absolute screen position (from mouse events) and returns the
    /// corresponding content position (line and column in the text).
    ///
    /// # Arguments
    /// * `screen_pos` - Absolute screen position
    ///
    /// # Returns
    /// - `Some(ContentPosition)` if the position maps to content
    /// - `None` if the position is outside the content area or unmapped
    pub fn screen_to_content(&self, screen_pos: ScreenPosition) -> Option<ContentPosition> {
        // Check if within content area
        if !self.is_within_area(screen_pos) {
            return None;
        }

        // Convert to area-relative position
        let relative = screen_pos.offset_from(self.area_x, self.area_y);

        // Account for scroll offset to get the actual screen row in the content
        let content_screen_row = self.scroll_offset + relative.y as usize;

        // Find the mapping for this screen row
        let mapping = self.mappings.iter().find(|m| m.screen_row as usize == content_screen_row)?;

        // Convert screen column to content column
        let content_column = mapping.screen_column_to_content(relative.x)?;

        Some(ContentPosition::new(mapping.content_line, content_column))
    }

    /// Convert a content position to screen position(s).
    ///
    /// A content position may map to multiple screen positions when text is
    /// wrapped, but this returns the first (primary) screen position.
    ///
    /// # Arguments
    /// * `content_pos` - Content position to convert
    ///
    /// # Returns
    /// - `Some(ScreenPosition)` with absolute screen coordinates
    /// - `None` if the position is not currently visible
    pub fn content_to_screen(&self, content_pos: ContentPosition) -> Option<ScreenPosition> {
        // Find the mapping that contains this content position
        for mapping in &self.mappings {
            if mapping.content_line == content_pos.line && mapping.contains_column(content_pos.column) {
                // Calculate screen column within this row
                let screen_col = (content_pos.column - mapping.content_start_column) as u16;
                let screen_row = mapping.screen_row;

                // Check if this row is visible (accounting for scroll)
                if (screen_row as usize) < self.scroll_offset {
                    continue;
                }

                let visible_row = (screen_row as usize - self.scroll_offset) as u16;
                if visible_row >= self.area_height {
                    continue;
                }

                // Convert to absolute screen position
                return Some(ScreenPosition::new(
                    self.area_x + screen_col,
                    self.area_y + visible_row,
                ));
            }
        }

        None
    }

    /// Find the mapping for a given screen row.
    ///
    /// # Arguments
    /// * `screen_row` - Screen row (relative to content, accounting for scroll)
    pub fn mapping_for_row(&self, screen_row: u16) -> Option<&ScreenToContentMapping> {
        self.mappings.iter().find(|m| m.screen_row == screen_row)
    }

    /// Get all mappings for a given content line.
    ///
    /// Returns multiple mappings if the line is wrapped across screen rows.
    pub fn mappings_for_line(&self, content_line: usize) -> Vec<&ScreenToContentMapping> {
        self.mappings
            .iter()
            .filter(|m| m.content_line == content_line)
            .collect()
    }

    /// Clear all mappings while preserving area settings.
    pub fn clear(&mut self) {
        self.mappings.clear();
    }
}

// ============================================================================
// Unicode Width Utilities
// ============================================================================

/// Calculate the display width of a character.
///
/// This properly handles:
/// - ASCII characters (width 1)
/// - Wide characters like CJK (width 2)
/// - Emojis (typically width 2)
/// - Zero-width characters (width 0)
///
/// # Arguments
/// * `c` - The character to measure
///
/// # Returns
/// The display width in terminal cells
pub fn char_display_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// Calculate the display width of a string.
///
/// This sums the widths of all characters, properly handling Unicode.
///
/// # Arguments
/// * `s` - The string to measure
///
/// # Returns
/// The total display width in terminal cells
pub fn string_display_width(s: &str) -> usize {
    s.chars().map(char_display_width).sum()
}

/// Map a screen column to a character index in a string.
///
/// Given a target screen column, finds which character index in the string
/// corresponds to that visual position. This is essential for click handling
/// with Unicode text.
///
/// # Arguments
/// * `s` - The string to search
/// * `target_screen_col` - The target screen column (0-indexed)
///
/// # Returns
/// The character index, or the string length if past the end.
///
/// # Example
///
/// ```ignore
/// let s = "Hello 世界";  // "Hello " = 6 cells, "世" = 2 cells, "界" = 2 cells
/// assert_eq!(screen_col_to_char_index(s, 0), 0);  // 'H'
/// assert_eq!(screen_col_to_char_index(s, 6), 6);  // '世'
/// assert_eq!(screen_col_to_char_index(s, 7), 6);  // Still '世' (middle of wide char)
/// assert_eq!(screen_col_to_char_index(s, 8), 7);  // '界'
/// ```
pub fn screen_col_to_char_index(s: &str, target_screen_col: usize) -> usize {
    let mut current_col = 0;

    for (char_idx, c) in s.chars().enumerate() {
        let char_width = char_display_width(c);

        // Check if target is within this character's display range
        if current_col + char_width > target_screen_col {
            return char_idx;
        }

        current_col += char_width;
    }

    // Past the end of string - return length
    s.chars().count()
}

/// Map a character index to a screen column in a string.
///
/// Given a character index, calculates the starting screen column for that character.
///
/// # Arguments
/// * `s` - The string
/// * `char_index` - The character index (0-indexed)
///
/// # Returns
/// The screen column where the character starts
pub fn char_index_to_screen_col(s: &str, char_index: usize) -> usize {
    s.chars()
        .take(char_index)
        .map(char_display_width)
        .sum()
}

/// Split a string into segments that fit within a given screen width.
///
/// This performs word-aware wrapping, trying to break at word boundaries
/// when possible. Returns tuples of (start_char_index, end_char_index) for
/// each segment.
///
/// # Arguments
/// * `s` - The string to wrap
/// * `max_width` - Maximum screen width for each segment
///
/// # Returns
/// Vector of (start_index, end_index) tuples for each wrapped segment.
/// The indices are character indices (not byte indices).
pub fn wrap_string_to_width(s: &str, max_width: usize) -> Vec<(usize, usize)> {
    if max_width == 0 {
        return vec![(0, s.chars().count())];
    }

    let chars: Vec<char> = s.chars().collect();
    let total_chars = chars.len();

    if total_chars == 0 {
        return vec![(0, 0)];
    }

    let mut segments = Vec::new();
    let mut segment_start = 0;
    let mut current_width = 0;
    let mut last_break_point = None; // Character index of last word break opportunity

    for (i, &c) in chars.iter().enumerate() {
        let char_width = char_display_width(c);

        // Track word break opportunities (spaces)
        if c == ' ' {
            last_break_point = Some(i + 1); // Break after the space
        }

        // Check if adding this character would exceed width
        if current_width + char_width > max_width && current_width > 0 {
            // Try to break at a word boundary
            let break_at = if let Some(bp) = last_break_point {
                if bp > segment_start {
                    bp
                } else {
                    i // No good break point, break at current position
                }
            } else {
                i // No break point found
            };

            // Ensure we make progress (at least one character per segment)
            let actual_break = if break_at <= segment_start {
                (segment_start + 1).min(total_chars)
            } else {
                break_at
            };

            segments.push((segment_start, actual_break));
            segment_start = actual_break;
            last_break_point = None;

            // Skip leading spaces at start of new segment
            while segment_start < total_chars && chars[segment_start] == ' ' {
                segment_start += 1;
            }

            // Recalculate width from new segment start to current position
            current_width = 0;
            for &ch in &chars[segment_start..=i.min(total_chars - 1)] {
                current_width += char_display_width(ch);
            }
        } else {
            current_width += char_width;
        }
    }

    // Add final segment if there's remaining content
    if segment_start < total_chars {
        segments.push((segment_start, total_chars));
    }

    // Ensure we have at least one segment
    if segments.is_empty() {
        segments.push((0, total_chars));
    }

    segments
}

/// Build mappings for a content line that may be wrapped.
///
/// Given a content line, its line number, and viewport width, creates
/// the screen-to-content mappings for all wrapped segments.
///
/// # Arguments
/// * `line_content` - The text content of the line
/// * `content_line` - The line number in the content
/// * `start_screen_row` - The starting screen row for this line
/// * `viewport_width` - The viewport width for wrapping
///
/// # Returns
/// Tuple of (mappings, next_screen_row)
pub fn build_line_mappings(
    line_content: &str,
    content_line: usize,
    start_screen_row: u16,
    viewport_width: u16,
) -> (Vec<ScreenToContentMapping>, u16) {
    let segments = wrap_string_to_width(line_content, viewport_width as usize);
    let mut mappings = Vec::with_capacity(segments.len());
    let mut screen_row = start_screen_row;

    for (start_char, end_char) in segments {
        mappings.push(ScreenToContentMapping::new(
            screen_row,
            content_line,
            start_char,
            end_char,
        ));
        screen_row = screen_row.saturating_add(1);
    }

    (mappings, screen_row)
}

/// Build a complete position mapping index from lines of content.
///
/// This is a convenience function that creates mappings for multiple content lines.
///
/// # Arguments
/// * `lines` - Iterator of (line_index, line_content) pairs
/// * `viewport_width` - The viewport width for wrapping
/// * `area_x` - Content area left edge
/// * `area_y` - Content area top edge
/// * `area_width` - Content area width
/// * `area_height` - Content area height
/// * `scroll_offset` - Current scroll position
///
/// # Returns
/// A PositionMappingIndex with all mappings
pub fn build_position_index<'a>(
    lines: impl Iterator<Item = (usize, &'a str)>,
    viewport_width: u16,
    area_x: u16,
    area_y: u16,
    area_width: u16,
    area_height: u16,
    scroll_offset: usize,
) -> PositionMappingIndex {
    let mut index = PositionMappingIndex::with_area(
        area_x,
        area_y,
        area_width,
        area_height,
        scroll_offset,
    );

    let mut screen_row: u16 = 0;
    for (line_idx, line_content) in lines {
        let (mappings, next_row) = build_line_mappings(
            line_content,
            line_idx,
            screen_row,
            viewport_width,
        );
        index.add_mappings(mappings);
        screen_row = next_row;
    }

    index
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

    // ============= PositionMappingIndex Tests =============

    #[test]
    fn test_position_mapping_index_new() {
        let index = PositionMappingIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_position_mapping_index_with_area() {
        let index = PositionMappingIndex::with_area(10, 5, 80, 30, 0);
        assert!(index.is_empty());
        assert_eq!(index.scroll_offset(), 0);
    }

    #[test]
    fn test_position_mapping_index_add_mapping() {
        let mut index = PositionMappingIndex::with_area(0, 0, 80, 24, 0);
        index.add_mapping(ScreenToContentMapping::new(0, 0, 0, 40));
        index.add_mapping(ScreenToContentMapping::new(1, 0, 40, 80));

        assert_eq!(index.len(), 2);
        assert!(!index.is_empty());
    }

    #[test]
    fn test_position_mapping_index_is_within_area() {
        let index = PositionMappingIndex::with_area(10, 5, 80, 30, 0);

        // Inside area
        assert!(index.is_within_area(ScreenPosition::new(10, 5)));
        assert!(index.is_within_area(ScreenPosition::new(50, 20)));
        assert!(index.is_within_area(ScreenPosition::new(89, 34)));

        // Outside area
        assert!(!index.is_within_area(ScreenPosition::new(9, 5))); // Left of area
        assert!(!index.is_within_area(ScreenPosition::new(10, 4))); // Above area
        assert!(!index.is_within_area(ScreenPosition::new(90, 5))); // Right of area
        assert!(!index.is_within_area(ScreenPosition::new(10, 35))); // Below area
    }

    #[test]
    fn test_position_mapping_index_screen_to_content_basic() {
        let mut index = PositionMappingIndex::with_area(0, 0, 80, 24, 0);
        index.add_mapping(ScreenToContentMapping::new(0, 0, 0, 80));
        index.add_mapping(ScreenToContentMapping::new(1, 1, 0, 50));

        // First line
        let pos = index.screen_to_content(ScreenPosition::new(10, 0));
        assert_eq!(pos, Some(ContentPosition::new(0, 10)));

        // Second line
        let pos = index.screen_to_content(ScreenPosition::new(20, 1));
        assert_eq!(pos, Some(ContentPosition::new(1, 20)));
    }

    #[test]
    fn test_position_mapping_index_screen_to_content_with_offset() {
        let mut index = PositionMappingIndex::with_area(10, 5, 80, 24, 0);
        index.add_mapping(ScreenToContentMapping::new(0, 0, 0, 80));

        // Click at absolute (15, 5) = relative (5, 0) in area
        let pos = index.screen_to_content(ScreenPosition::new(15, 5));
        assert_eq!(pos, Some(ContentPosition::new(0, 5)));
    }

    #[test]
    fn test_position_mapping_index_screen_to_content_with_scroll() {
        let mut index = PositionMappingIndex::with_area(0, 0, 80, 24, 5);
        index.add_mapping(ScreenToContentMapping::new(5, 3, 0, 80));
        index.add_mapping(ScreenToContentMapping::new(6, 4, 0, 80));

        // With scroll_offset=5, screen row 0 maps to content screen row 5
        let pos = index.screen_to_content(ScreenPosition::new(10, 0));
        assert_eq!(pos, Some(ContentPosition::new(3, 10)));

        // Screen row 1 maps to content screen row 6
        let pos = index.screen_to_content(ScreenPosition::new(20, 1));
        assert_eq!(pos, Some(ContentPosition::new(4, 20)));
    }

    #[test]
    fn test_position_mapping_index_screen_to_content_outside() {
        let index = PositionMappingIndex::with_area(10, 5, 80, 24, 0);

        // Outside content area
        let pos = index.screen_to_content(ScreenPosition::new(5, 5));
        assert_eq!(pos, None);
    }

    #[test]
    fn test_position_mapping_index_content_to_screen_basic() {
        let mut index = PositionMappingIndex::with_area(10, 5, 80, 24, 0);
        index.add_mapping(ScreenToContentMapping::new(0, 0, 0, 80));
        index.add_mapping(ScreenToContentMapping::new(1, 1, 0, 50));

        // Content (0, 15) -> screen (10+15, 5+0) = (25, 5)
        let pos = index.content_to_screen(ContentPosition::new(0, 15));
        assert_eq!(pos, Some(ScreenPosition::new(25, 5)));

        // Content (1, 30) -> screen (10+30, 5+1) = (40, 6)
        let pos = index.content_to_screen(ContentPosition::new(1, 30));
        assert_eq!(pos, Some(ScreenPosition::new(40, 6)));
    }

    #[test]
    fn test_position_mapping_index_wrapped_line() {
        let mut index = PositionMappingIndex::with_area(0, 0, 40, 24, 0);
        // Line 0 wraps: first 40 chars on row 0, next 40 on row 1
        index.add_mapping(ScreenToContentMapping::new(0, 0, 0, 40));
        index.add_mapping(ScreenToContentMapping::new(1, 0, 40, 80));

        // Click on screen row 0, column 10 -> content line 0, column 10
        let pos = index.screen_to_content(ScreenPosition::new(10, 0));
        assert_eq!(pos, Some(ContentPosition::new(0, 10)));

        // Click on screen row 1, column 10 -> content line 0, column 50
        let pos = index.screen_to_content(ScreenPosition::new(10, 1));
        assert_eq!(pos, Some(ContentPosition::new(0, 50)));
    }

    #[test]
    fn test_position_mapping_index_mappings_for_line() {
        let mut index = PositionMappingIndex::new();
        index.add_mapping(ScreenToContentMapping::new(0, 0, 0, 40));
        index.add_mapping(ScreenToContentMapping::new(1, 0, 40, 80));
        index.add_mapping(ScreenToContentMapping::new(2, 1, 0, 50));

        let line0_mappings = index.mappings_for_line(0);
        assert_eq!(line0_mappings.len(), 2);

        let line1_mappings = index.mappings_for_line(1);
        assert_eq!(line1_mappings.len(), 1);

        let line2_mappings = index.mappings_for_line(2);
        assert_eq!(line2_mappings.len(), 0);
    }

    #[test]
    fn test_position_mapping_index_clear() {
        let mut index = PositionMappingIndex::with_area(10, 5, 80, 24, 0);
        index.add_mapping(ScreenToContentMapping::new(0, 0, 0, 80));
        assert_eq!(index.len(), 1);

        index.clear();
        assert!(index.is_empty());
    }

    // ============= Unicode Width Utility Tests =============

    #[test]
    fn test_char_display_width_ascii() {
        assert_eq!(char_display_width('a'), 1);
        assert_eq!(char_display_width('Z'), 1);
        assert_eq!(char_display_width('0'), 1);
        assert_eq!(char_display_width(' '), 1);
    }

    #[test]
    fn test_char_display_width_wide() {
        // CJK characters are typically 2 cells wide
        assert_eq!(char_display_width('世'), 2);
        assert_eq!(char_display_width('界'), 2);
        assert_eq!(char_display_width('日'), 2);
    }

    #[test]
    fn test_string_display_width_ascii() {
        assert_eq!(string_display_width("Hello"), 5);
        assert_eq!(string_display_width(""), 0);
        assert_eq!(string_display_width("abc def"), 7);
    }

    #[test]
    fn test_string_display_width_mixed() {
        // "Hello " = 6, "世界" = 4 (2+2)
        assert_eq!(string_display_width("Hello 世界"), 10);
    }

    #[test]
    fn test_screen_col_to_char_index_ascii() {
        let s = "Hello";
        assert_eq!(screen_col_to_char_index(s, 0), 0);
        assert_eq!(screen_col_to_char_index(s, 1), 1);
        assert_eq!(screen_col_to_char_index(s, 4), 4);
        assert_eq!(screen_col_to_char_index(s, 5), 5); // Past end
        assert_eq!(screen_col_to_char_index(s, 10), 5); // Way past end
    }

    #[test]
    fn test_screen_col_to_char_index_wide_chars() {
        let s = "A世界B";  // A=1, 世=2, 界=2, B=1 -> total 6 cells
        assert_eq!(screen_col_to_char_index(s, 0), 0);  // 'A' at col 0
        assert_eq!(screen_col_to_char_index(s, 1), 1);  // '世' starts at col 1
        assert_eq!(screen_col_to_char_index(s, 2), 1);  // Still '世' (middle of wide char)
        assert_eq!(screen_col_to_char_index(s, 3), 2);  // '界' starts at col 3
        assert_eq!(screen_col_to_char_index(s, 4), 2);  // Still '界'
        assert_eq!(screen_col_to_char_index(s, 5), 3);  // 'B' at col 5
        assert_eq!(screen_col_to_char_index(s, 6), 4);  // Past end
    }

    #[test]
    fn test_char_index_to_screen_col_ascii() {
        let s = "Hello";
        assert_eq!(char_index_to_screen_col(s, 0), 0);
        assert_eq!(char_index_to_screen_col(s, 1), 1);
        assert_eq!(char_index_to_screen_col(s, 5), 5);
    }

    #[test]
    fn test_char_index_to_screen_col_wide_chars() {
        let s = "A世界B";
        assert_eq!(char_index_to_screen_col(s, 0), 0);  // 'A'
        assert_eq!(char_index_to_screen_col(s, 1), 1);  // '世'
        assert_eq!(char_index_to_screen_col(s, 2), 3);  // '界' (after 1+2)
        assert_eq!(char_index_to_screen_col(s, 3), 5);  // 'B' (after 1+2+2)
    }

    #[test]
    fn test_wrap_string_to_width_no_wrap() {
        let s = "Hello";
        let segments = wrap_string_to_width(s, 10);
        assert_eq!(segments, vec![(0, 5)]);
    }

    #[test]
    fn test_wrap_string_to_width_single_wrap() {
        let s = "Hello World";
        let segments = wrap_string_to_width(s, 8);
        // Should break after "Hello " (6 chars) at word boundary
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], (0, 6));  // "Hello "
        assert_eq!(segments[1], (6, 11)); // "World"
    }

    #[test]
    fn test_wrap_string_to_width_empty() {
        let s = "";
        let segments = wrap_string_to_width(s, 10);
        assert_eq!(segments, vec![(0, 0)]);
    }

    #[test]
    fn test_wrap_string_to_width_zero_width() {
        let s = "Hello";
        let segments = wrap_string_to_width(s, 0);
        assert_eq!(segments, vec![(0, 5)]);
    }

    #[test]
    fn test_wrap_string_to_width_exact_fit() {
        let s = "Hello";
        let segments = wrap_string_to_width(s, 5);
        assert_eq!(segments, vec![(0, 5)]);
    }

    #[test]
    fn test_wrap_string_to_width_wide_chars() {
        let s = "A世界"; // 1 + 2 + 2 = 5 display width, 3 chars
        let segments = wrap_string_to_width(s, 3);
        // Should break after 'A世' (3 display width)
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn test_build_line_mappings_no_wrap() {
        let (mappings, next_row) = build_line_mappings("Hello", 0, 0, 80);
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].screen_row, 0);
        assert_eq!(mappings[0].content_line, 0);
        assert_eq!(mappings[0].content_start_column, 0);
        assert_eq!(mappings[0].content_end_column, 5);
        assert_eq!(next_row, 1);
    }

    #[test]
    fn test_build_line_mappings_with_wrap() {
        let (mappings, next_row) = build_line_mappings("Hello World Test", 0, 5, 8);
        assert!(mappings.len() >= 2);
        assert_eq!(mappings[0].screen_row, 5);
        assert_eq!(mappings[0].content_line, 0);
        assert_eq!(next_row, 5 + mappings.len() as u16);
    }

    #[test]
    fn test_build_position_index() {
        let lines = vec![(0, "Hello"), (1, "World")];
        let index = build_position_index(
            lines.into_iter(),
            80,
            0, 0, 80, 24,
            0,
        );

        assert_eq!(index.len(), 2);

        // Verify we can map positions
        let pos = index.screen_to_content(ScreenPosition::new(2, 0));
        assert_eq!(pos, Some(ContentPosition::new(0, 2)));

        let pos = index.screen_to_content(ScreenPosition::new(3, 1));
        assert_eq!(pos, Some(ContentPosition::new(1, 3)));
    }
}
