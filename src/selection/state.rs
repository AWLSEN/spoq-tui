//! Selection state management
//!
//! This module contains the core state structures for text selection:
//! - SelectionMode: Character, word, or line selection granularity
//! - SelectionAnchor: The fixed starting point of a selection
//! - SelectionRange: A normalized range from start to end
//! - SelectionState: The complete selection state with active/inactive tracking

use serde::{Deserialize, Serialize};

use super::position::ContentPosition;

/// Selection granularity mode
///
/// Determines how text is selected when the user drags:
/// - Character: Individual characters (default)
/// - Word: Whole words (typically triggered by double-click)
/// - Line: Entire lines (typically triggered by triple-click)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SelectionMode {
    /// Select individual characters (default behavior)
    #[default]
    Character,
    /// Select whole words (double-click to initiate)
    Word,
    /// Select entire lines (triple-click to initiate)
    Line,
}

/// The anchor point where a selection started
///
/// When the user initiates a selection (mouse down), this captures
/// the starting position. As they drag, the current position moves
/// but the anchor remains fixed.
///
/// # Fields
/// - `position`: The content position where the selection started
/// - `mode`: The selection mode (character/word/line) in effect
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionAnchor {
    /// Position in content where the selection started
    pub position: ContentPosition,
    /// Selection mode (granularity)
    pub mode: SelectionMode,
}

impl SelectionAnchor {
    /// Create a new selection anchor
    ///
    /// # Arguments
    /// * `position` - The content position where selection starts
    /// * `mode` - The selection mode (character, word, or line)
    pub fn new(position: ContentPosition, mode: SelectionMode) -> Self {
        Self { position, mode }
    }

    /// Create a character-mode anchor at the given position
    ///
    /// # Arguments
    /// * `position` - The content position
    pub fn character(position: ContentPosition) -> Self {
        Self::new(position, SelectionMode::Character)
    }

    /// Create a word-mode anchor at the given position
    ///
    /// # Arguments
    /// * `position` - The content position (will select the word containing this position)
    pub fn word(position: ContentPosition) -> Self {
        Self::new(position, SelectionMode::Word)
    }

    /// Create a line-mode anchor at the given position
    ///
    /// # Arguments
    /// * `position` - The content position (will select the entire line)
    pub fn line(position: ContentPosition) -> Self {
        Self::new(position, SelectionMode::Line)
    }
}

/// A normalized selection range from start to end
///
/// Unlike the anchor+current model used during active selection,
/// SelectionRange always has `start` before or equal to `end` in
/// document order. This makes it easier to work with for rendering
/// and text extraction.
///
/// # Fields
/// - `start`: The earlier position in the selection
/// - `end`: The later position in the selection (may equal start for a cursor)
/// - `mode`: The selection mode that was used to create this range
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionRange {
    /// Start position (always <= end in document order)
    pub start: ContentPosition,
    /// End position (always >= start in document order)
    pub end: ContentPosition,
    /// The selection mode used
    pub mode: SelectionMode,
}

impl SelectionRange {
    /// Create a new selection range
    ///
    /// The start and end positions will be normalized so that
    /// `start` is always before or equal to `end`.
    ///
    /// # Arguments
    /// * `pos1` - First position
    /// * `pos2` - Second position
    /// * `mode` - Selection mode
    pub fn new(pos1: ContentPosition, pos2: ContentPosition, mode: SelectionMode) -> Self {
        let (start, end) = if pos1 <= pos2 {
            (pos1, pos2)
        } else {
            (pos2, pos1)
        };
        Self { start, end, mode }
    }

    /// Create a range from an anchor and current position
    ///
    /// # Arguments
    /// * `anchor` - The selection anchor
    /// * `current` - The current cursor position
    pub fn from_anchor(anchor: SelectionAnchor, current: ContentPosition) -> Self {
        Self::new(anchor.position, current, anchor.mode)
    }

    /// Create a cursor (zero-width selection) at the given position
    ///
    /// # Arguments
    /// * `position` - The cursor position
    pub fn cursor(position: ContentPosition) -> Self {
        Self {
            start: position,
            end: position,
            mode: SelectionMode::Character,
        }
    }

    /// Check if this is a cursor (zero-width selection)
    ///
    /// Returns true if start equals end (no text is selected).
    pub fn is_cursor(&self) -> bool {
        self.start == self.end
    }

    /// Check if this range is empty (zero-width)
    ///
    /// Alias for `is_cursor()`.
    pub fn is_empty(&self) -> bool {
        self.is_cursor()
    }

    /// Check if a position is within this selection range
    ///
    /// # Arguments
    /// * `position` - The position to check
    pub fn contains(&self, position: ContentPosition) -> bool {
        position >= self.start && position <= self.end
    }

    /// Check if a line has any selected content
    ///
    /// # Arguments
    /// * `line` - The line number to check
    pub fn intersects_line(&self, line: usize) -> bool {
        line >= self.start.line && line <= self.end.line
    }

    /// Get the selected column range for a specific line
    ///
    /// Returns None if the line is not part of the selection.
    /// Returns Some((start_col, end_col)) for the selected portion.
    ///
    /// # Arguments
    /// * `line` - The line number
    /// * `line_length` - The length of the line (for clamping)
    pub fn columns_for_line(&self, line: usize, line_length: usize) -> Option<(usize, usize)> {
        if !self.intersects_line(line) {
            return None;
        }

        let start_col = if line == self.start.line {
            self.start.column
        } else {
            0
        };

        let end_col = if line == self.end.line {
            self.end.column.min(line_length)
        } else {
            line_length
        };

        if start_col <= end_col {
            Some((start_col, end_col))
        } else {
            None
        }
    }
}

/// Complete selection state for a content area
///
/// Tracks whether a selection is active (user is currently dragging),
/// the anchor position, the current cursor position, and provides
/// methods for selection manipulation.
///
/// # Usage
///
/// ```ignore
/// // Start a selection on mouse down
/// state.start_selection(position, SelectionMode::Character);
///
/// // Update on mouse drag
/// state.update_selection(new_position);
///
/// // Finish on mouse up
/// state.finish_selection();
///
/// // Get the selected range for rendering/copying
/// if let Some(range) = state.get_range() {
///     // Use the range
/// }
///
/// // Clear selection on click elsewhere
/// state.clear();
/// ```
#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    /// Whether the user is currently making a selection (mouse down, dragging)
    pub is_selecting: bool,
    /// The anchor point where selection started (set on mouse down)
    anchor: Option<SelectionAnchor>,
    /// Current cursor position (updated on mouse move during selection)
    current: Option<ContentPosition>,
    /// Cached selection range (for efficiency in rendering)
    cached_range: Option<SelectionRange>,
}

impl SelectionState {
    /// Create a new empty selection state
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there is an active selection (non-empty range)
    pub fn has_selection(&self) -> bool {
        self.get_range().is_some_and(|r| !r.is_empty())
    }

    /// Check if a selection is currently being made (mouse down, dragging)
    pub fn is_active(&self) -> bool {
        self.is_selecting
    }

    /// Start a new selection at the given position
    ///
    /// Called on mouse down. This sets the anchor and prepares for dragging.
    ///
    /// # Arguments
    /// * `position` - Content position where selection starts
    /// * `mode` - Selection mode (character, word, line)
    pub fn start_selection(&mut self, position: ContentPosition, mode: SelectionMode) {
        self.anchor = Some(SelectionAnchor::new(position, mode));
        self.current = Some(position);
        self.is_selecting = true;
        self.update_cache();
    }

    /// Update the current selection endpoint
    ///
    /// Called during mouse drag. Updates the current position while
    /// keeping the anchor fixed.
    ///
    /// # Arguments
    /// * `position` - New current position
    pub fn update_selection(&mut self, position: ContentPosition) {
        if self.is_selecting {
            self.current = Some(position);
            self.update_cache();
        }
    }

    /// Finish the current selection
    ///
    /// Called on mouse up. Stops the active selection but preserves
    /// the selected range.
    pub fn finish_selection(&mut self) {
        self.is_selecting = false;
    }

    /// Clear the selection entirely
    ///
    /// Removes both the anchor and current position, clearing any
    /// selected text.
    pub fn clear(&mut self) {
        self.anchor = None;
        self.current = None;
        self.is_selecting = false;
        self.cached_range = None;
    }

    /// Get the current selection range
    ///
    /// Returns None if there is no selection.
    pub fn get_range(&self) -> Option<SelectionRange> {
        self.cached_range
    }

    /// Get the anchor position
    pub fn anchor(&self) -> Option<&SelectionAnchor> {
        self.anchor.as_ref()
    }

    /// Get the current cursor position
    pub fn current(&self) -> Option<ContentPosition> {
        self.current
    }

    /// Get the selection mode
    pub fn mode(&self) -> SelectionMode {
        self.anchor
            .as_ref()
            .map(|a| a.mode)
            .unwrap_or(SelectionMode::Character)
    }

    /// Set a specific range programmatically
    ///
    /// Useful for "select all" or restoring a saved selection.
    ///
    /// # Arguments
    /// * `range` - The selection range to set
    pub fn set_range(&mut self, range: SelectionRange) {
        self.anchor = Some(SelectionAnchor::new(range.start, range.mode));
        self.current = Some(range.end);
        self.is_selecting = false;
        self.cached_range = Some(range);
    }

    /// Extend the current selection to a new position
    ///
    /// If there's no existing selection, starts a new one.
    /// If there is a selection, extends it to include the new position.
    ///
    /// # Arguments
    /// * `position` - Position to extend to
    pub fn extend_to(&mut self, position: ContentPosition) {
        if self.anchor.is_some() {
            self.current = Some(position);
            self.update_cache();
        } else {
            // No existing selection, start a new one
            self.start_selection(position, SelectionMode::Character);
        }
    }

    /// Update the cached range from anchor and current
    fn update_cache(&mut self) {
        self.cached_range = match (&self.anchor, &self.current) {
            (Some(anchor), Some(current)) => Some(SelectionRange::from_anchor(*anchor, *current)),
            _ => None,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============= SelectionMode Tests =============

    #[test]
    fn test_selection_mode_default() {
        assert_eq!(SelectionMode::default(), SelectionMode::Character);
    }

    #[test]
    fn test_selection_mode_equality() {
        assert_eq!(SelectionMode::Character, SelectionMode::Character);
        assert_eq!(SelectionMode::Word, SelectionMode::Word);
        assert_eq!(SelectionMode::Line, SelectionMode::Line);
        assert_ne!(SelectionMode::Character, SelectionMode::Word);
    }

    #[test]
    fn test_selection_mode_serialization() {
        let mode = SelectionMode::Word;
        let json = serde_json::to_string(&mode).expect("Failed to serialize");
        let deserialized: SelectionMode = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(mode, deserialized);
    }

    // ============= SelectionAnchor Tests =============

    #[test]
    fn test_selection_anchor_new() {
        let pos = ContentPosition::new(5, 10);
        let anchor = SelectionAnchor::new(pos, SelectionMode::Word);

        assert_eq!(anchor.position, pos);
        assert_eq!(anchor.mode, SelectionMode::Word);
    }

    #[test]
    fn test_selection_anchor_character() {
        let pos = ContentPosition::new(1, 2);
        let anchor = SelectionAnchor::character(pos);

        assert_eq!(anchor.position, pos);
        assert_eq!(anchor.mode, SelectionMode::Character);
    }

    #[test]
    fn test_selection_anchor_word() {
        let pos = ContentPosition::new(3, 4);
        let anchor = SelectionAnchor::word(pos);

        assert_eq!(anchor.position, pos);
        assert_eq!(anchor.mode, SelectionMode::Word);
    }

    #[test]
    fn test_selection_anchor_line() {
        let pos = ContentPosition::new(5, 0);
        let anchor = SelectionAnchor::line(pos);

        assert_eq!(anchor.position, pos);
        assert_eq!(anchor.mode, SelectionMode::Line);
    }

    #[test]
    fn test_selection_anchor_equality() {
        let pos = ContentPosition::new(1, 1);
        let a1 = SelectionAnchor::character(pos);
        let a2 = SelectionAnchor::character(pos);
        let a3 = SelectionAnchor::word(pos);

        assert_eq!(a1, a2);
        assert_ne!(a1, a3); // Different mode
    }

    // ============= SelectionRange Tests =============

    #[test]
    fn test_selection_range_new_normalizes() {
        let pos1 = ContentPosition::new(5, 10);
        let pos2 = ContentPosition::new(2, 5);

        // Even though pos1 > pos2, start should be pos2
        let range = SelectionRange::new(pos1, pos2, SelectionMode::Character);

        assert_eq!(range.start, pos2);
        assert_eq!(range.end, pos1);
    }

    #[test]
    fn test_selection_range_new_already_ordered() {
        let pos1 = ContentPosition::new(2, 5);
        let pos2 = ContentPosition::new(5, 10);

        let range = SelectionRange::new(pos1, pos2, SelectionMode::Character);

        assert_eq!(range.start, pos1);
        assert_eq!(range.end, pos2);
    }

    #[test]
    fn test_selection_range_from_anchor() {
        let anchor = SelectionAnchor::word(ContentPosition::new(1, 5));
        let current = ContentPosition::new(3, 10);

        let range = SelectionRange::from_anchor(anchor, current);

        assert_eq!(range.start, ContentPosition::new(1, 5));
        assert_eq!(range.end, ContentPosition::new(3, 10));
        assert_eq!(range.mode, SelectionMode::Word);
    }

    #[test]
    fn test_selection_range_cursor() {
        let pos = ContentPosition::new(5, 10);
        let range = SelectionRange::cursor(pos);

        assert_eq!(range.start, pos);
        assert_eq!(range.end, pos);
        assert!(range.is_cursor());
        assert!(range.is_empty());
    }

    #[test]
    fn test_selection_range_not_empty() {
        let range = SelectionRange::new(
            ContentPosition::new(0, 0),
            ContentPosition::new(0, 5),
            SelectionMode::Character,
        );

        assert!(!range.is_cursor());
        assert!(!range.is_empty());
    }

    #[test]
    fn test_selection_range_contains() {
        let range = SelectionRange::new(
            ContentPosition::new(1, 5),
            ContentPosition::new(3, 10),
            SelectionMode::Character,
        );

        // Within range
        assert!(range.contains(ContentPosition::new(2, 0)));
        assert!(range.contains(ContentPosition::new(1, 5))); // Start
        assert!(range.contains(ContentPosition::new(3, 10))); // End

        // Outside range
        assert!(!range.contains(ContentPosition::new(0, 0)));
        assert!(!range.contains(ContentPosition::new(1, 4)));
        assert!(!range.contains(ContentPosition::new(3, 11)));
        assert!(!range.contains(ContentPosition::new(4, 0)));
    }

    #[test]
    fn test_selection_range_intersects_line() {
        let range = SelectionRange::new(
            ContentPosition::new(2, 5),
            ContentPosition::new(5, 10),
            SelectionMode::Character,
        );

        assert!(!range.intersects_line(0));
        assert!(!range.intersects_line(1));
        assert!(range.intersects_line(2)); // Start line
        assert!(range.intersects_line(3)); // Middle
        assert!(range.intersects_line(4)); // Middle
        assert!(range.intersects_line(5)); // End line
        assert!(!range.intersects_line(6));
    }

    #[test]
    fn test_selection_range_columns_for_line() {
        let range = SelectionRange::new(
            ContentPosition::new(2, 5),
            ContentPosition::new(4, 10),
            SelectionMode::Character,
        );

        // Line before selection
        assert_eq!(range.columns_for_line(0, 80), None);
        assert_eq!(range.columns_for_line(1, 80), None);

        // Start line: column 5 to end of line
        assert_eq!(range.columns_for_line(2, 80), Some((5, 80)));
        assert_eq!(range.columns_for_line(2, 50), Some((5, 50))); // Shorter line

        // Middle line: entire line
        assert_eq!(range.columns_for_line(3, 80), Some((0, 80)));

        // End line: start to column 10
        assert_eq!(range.columns_for_line(4, 80), Some((0, 10)));
        assert_eq!(range.columns_for_line(4, 5), Some((0, 5))); // Line shorter than selection

        // Line after selection
        assert_eq!(range.columns_for_line(5, 80), None);
    }

    #[test]
    fn test_selection_range_columns_for_single_line() {
        let range = SelectionRange::new(
            ContentPosition::new(2, 5),
            ContentPosition::new(2, 15),
            SelectionMode::Character,
        );

        // Same line for start and end
        assert_eq!(range.columns_for_line(2, 80), Some((5, 15)));
        assert_eq!(range.columns_for_line(2, 10), Some((5, 10))); // Line shorter
    }

    // ============= SelectionState Tests =============

    #[test]
    fn test_selection_state_new() {
        let state = SelectionState::new();

        assert!(!state.is_selecting);
        assert!(!state.has_selection());
        assert!(!state.is_active());
        assert!(state.anchor().is_none());
        assert!(state.current().is_none());
        assert!(state.get_range().is_none());
    }

    #[test]
    fn test_selection_state_default() {
        let state = SelectionState::default();
        assert!(!state.has_selection());
    }

    #[test]
    fn test_selection_state_start_selection() {
        let mut state = SelectionState::new();
        let pos = ContentPosition::new(5, 10);

        state.start_selection(pos, SelectionMode::Character);

        assert!(state.is_selecting);
        assert!(state.is_active());
        assert!(state.anchor().is_some());
        assert_eq!(state.anchor().unwrap().position, pos);
        assert_eq!(state.current(), Some(pos));
        assert_eq!(state.mode(), SelectionMode::Character);

        // Should have a range, but it's a cursor (empty)
        let range = state.get_range();
        assert!(range.is_some());
        assert!(range.unwrap().is_empty());
    }

    #[test]
    fn test_selection_state_update_selection() {
        let mut state = SelectionState::new();
        let start = ContentPosition::new(1, 0);
        let end = ContentPosition::new(3, 20);

        state.start_selection(start, SelectionMode::Character);
        state.update_selection(end);

        assert!(state.is_selecting);
        assert_eq!(state.current(), Some(end));

        let range = state.get_range().unwrap();
        assert_eq!(range.start, start);
        assert_eq!(range.end, end);
        assert!(!range.is_empty());
    }

    #[test]
    fn test_selection_state_update_without_start() {
        let mut state = SelectionState::new();
        let pos = ContentPosition::new(5, 10);

        // Update without starting should do nothing
        state.update_selection(pos);

        assert!(!state.is_selecting);
        assert!(state.current().is_none());
    }

    #[test]
    fn test_selection_state_finish_selection() {
        let mut state = SelectionState::new();
        let start = ContentPosition::new(1, 0);
        let end = ContentPosition::new(2, 10);

        state.start_selection(start, SelectionMode::Character);
        state.update_selection(end);
        state.finish_selection();

        assert!(!state.is_selecting);
        assert!(!state.is_active());
        assert!(state.has_selection()); // Selection is preserved
        assert!(state.get_range().is_some());
    }

    #[test]
    fn test_selection_state_clear() {
        let mut state = SelectionState::new();
        let pos = ContentPosition::new(1, 0);

        state.start_selection(pos, SelectionMode::Word);
        state.update_selection(ContentPosition::new(2, 0));

        state.clear();

        assert!(!state.is_selecting);
        assert!(!state.has_selection());
        assert!(state.anchor().is_none());
        assert!(state.current().is_none());
        assert!(state.get_range().is_none());
    }

    #[test]
    fn test_selection_state_set_range() {
        let mut state = SelectionState::new();
        let range = SelectionRange::new(
            ContentPosition::new(0, 0),
            ContentPosition::new(5, 10),
            SelectionMode::Line,
        );

        state.set_range(range);

        assert!(!state.is_selecting); // Not actively selecting
        assert!(state.has_selection());
        assert_eq!(state.get_range(), Some(range));
        assert_eq!(state.mode(), SelectionMode::Line);
    }

    #[test]
    fn test_selection_state_extend_to() {
        let mut state = SelectionState::new();
        let start = ContentPosition::new(1, 5);
        let mid = ContentPosition::new(2, 10);
        let end = ContentPosition::new(3, 15);

        // Start selection
        state.start_selection(start, SelectionMode::Character);
        state.update_selection(mid);
        state.finish_selection();

        // Extend to new position
        state.extend_to(end);

        let range = state.get_range().unwrap();
        assert_eq!(range.start, start);
        assert_eq!(range.end, end);
    }

    #[test]
    fn test_selection_state_extend_to_no_selection() {
        let mut state = SelectionState::new();
        let pos = ContentPosition::new(5, 10);

        // Extend with no existing selection starts a new one
        state.extend_to(pos);

        assert!(state.is_selecting);
        assert_eq!(state.current(), Some(pos));
    }

    #[test]
    fn test_selection_state_has_selection_empty() {
        let mut state = SelectionState::new();
        let pos = ContentPosition::new(1, 5);

        // Start selection at a point (cursor)
        state.start_selection(pos, SelectionMode::Character);
        state.finish_selection();

        // Cursor is not considered a "selection" for has_selection
        assert!(!state.has_selection());
    }

    #[test]
    fn test_selection_state_backward_selection() {
        let mut state = SelectionState::new();
        let start = ContentPosition::new(5, 10);
        let end = ContentPosition::new(2, 5);

        state.start_selection(start, SelectionMode::Character);
        state.update_selection(end);

        // Range should be normalized
        let range = state.get_range().unwrap();
        assert_eq!(range.start, end);
        assert_eq!(range.end, start);
    }

    #[test]
    fn test_selection_state_mode_preserved() {
        let mut state = SelectionState::new();
        let pos = ContentPosition::new(0, 0);

        state.start_selection(pos, SelectionMode::Line);
        assert_eq!(state.mode(), SelectionMode::Line);

        state.clear();
        assert_eq!(state.mode(), SelectionMode::Character); // Default when no anchor
    }
}
