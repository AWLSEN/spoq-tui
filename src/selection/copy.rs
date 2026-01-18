//! Clipboard copy functionality
//!
//! This module provides utilities for copying selected text to the system clipboard.
//! It handles text extraction from content based on selection ranges and uses
//! the arboard crate for cross-platform clipboard access.

use arboard::Clipboard;

use super::state::SelectionRange;

/// Error type for clipboard operations
#[derive(Debug)]
pub enum ClipboardError {
    /// Failed to access the clipboard
    ClipboardAccess(String),
    /// No text to copy (empty selection)
    EmptySelection,
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardError::ClipboardAccess(msg) => write!(f, "Clipboard error: {}", msg),
            ClipboardError::EmptySelection => write!(f, "No text selected"),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// Extract selected text from content lines based on selection range.
///
/// This function handles multi-line selections by joining lines with newlines,
/// and properly extracts the correct columns from start and end lines.
///
/// # Arguments
/// * `lines` - All content lines (as strings)
/// * `selection` - The selection range to extract
///
/// # Returns
/// The extracted text, or None if the selection is invalid or empty
pub fn extract_selected_text(lines: &[&str], selection: &SelectionRange) -> Option<String> {
    if selection.is_empty() || lines.is_empty() {
        return None;
    }

    let start_line = selection.start.line;
    let end_line = selection.end.line;

    if start_line >= lines.len() {
        return None;
    }

    let mut result = String::new();

    for (line_idx, line) in lines.iter().enumerate() {
        if !selection.intersects_line(line_idx) {
            continue;
        }

        let chars: Vec<char> = line.chars().collect();
        let line_len = chars.len();

        if let Some((start_col, end_col)) = selection.columns_for_line(line_idx, line_len) {
            // Extract the selected portion of this line
            let start = start_col.min(line_len);
            let end = end_col.min(line_len);

            if start < end {
                let selected: String = chars[start..end].iter().collect();
                result.push_str(&selected);
            }

            // Add newline between lines (but not after the last line)
            if line_idx < end_line && line_idx < lines.len() - 1 {
                result.push('\n');
            }
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Copy text to the system clipboard.
///
/// # Arguments
/// * `text` - The text to copy
///
/// # Returns
/// Ok(()) on success, Err on failure
pub fn copy_to_clipboard(text: &str) -> Result<(), ClipboardError> {
    if text.is_empty() {
        return Err(ClipboardError::EmptySelection);
    }

    let mut clipboard = Clipboard::new()
        .map_err(|e| ClipboardError::ClipboardAccess(e.to_string()))?;

    clipboard
        .set_text(text)
        .map_err(|e| ClipboardError::ClipboardAccess(e.to_string()))?;

    Ok(())
}

/// Copy selected text from content lines to the clipboard.
///
/// This is a convenience function that combines text extraction and clipboard copy.
///
/// # Arguments
/// * `lines` - All content lines
/// * `selection` - The selection range
///
/// # Returns
/// Ok(()) on success, Err if extraction fails or clipboard access fails
pub fn copy_selection_to_clipboard(lines: &[&str], selection: &SelectionRange) -> Result<(), ClipboardError> {
    let text = extract_selected_text(lines, selection)
        .ok_or(ClipboardError::EmptySelection)?;

    copy_to_clipboard(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::{ContentPosition, SelectionMode};

    #[test]
    fn test_extract_single_line() {
        let lines = vec!["Hello World"];
        let selection = SelectionRange::new(
            ContentPosition::new(0, 0),
            ContentPosition::new(0, 5),
            SelectionMode::Character,
        );

        let result = extract_selected_text(&lines, &selection);
        assert_eq!(result, Some("Hello".to_string()));
    }

    #[test]
    fn test_extract_partial_single_line() {
        let lines = vec!["Hello World"];
        let selection = SelectionRange::new(
            ContentPosition::new(0, 6),
            ContentPosition::new(0, 11),
            SelectionMode::Character,
        );

        let result = extract_selected_text(&lines, &selection);
        assert_eq!(result, Some("World".to_string()));
    }

    #[test]
    fn test_extract_multiple_lines() {
        let lines = vec!["Hello", "World", "Test"];
        let selection = SelectionRange::new(
            ContentPosition::new(0, 2),
            ContentPosition::new(2, 2),
            SelectionMode::Character,
        );

        let result = extract_selected_text(&lines, &selection);
        assert_eq!(result, Some("llo\nWorld\nTe".to_string()));
    }

    #[test]
    fn test_extract_full_line() {
        let lines = vec!["Hello", "World"];
        let selection = SelectionRange::new(
            ContentPosition::new(0, 0),
            ContentPosition::new(1, 5),
            SelectionMode::Character,
        );

        let result = extract_selected_text(&lines, &selection);
        assert_eq!(result, Some("Hello\nWorld".to_string()));
    }

    #[test]
    fn test_extract_empty_selection() {
        let lines = vec!["Hello"];
        let selection = SelectionRange::cursor(ContentPosition::new(0, 0));

        let result = extract_selected_text(&lines, &selection);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_out_of_bounds() {
        let lines = vec!["Hello"];
        let selection = SelectionRange::new(
            ContentPosition::new(5, 0),  // Line 5 doesn't exist
            ContentPosition::new(5, 5),
            SelectionMode::Character,
        );

        let result = extract_selected_text(&lines, &selection);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_column_out_of_bounds() {
        let lines = vec!["Hi"];  // Only 2 characters
        let selection = SelectionRange::new(
            ContentPosition::new(0, 0),
            ContentPosition::new(0, 100),  // Way past end
            SelectionMode::Character,
        );

        let result = extract_selected_text(&lines, &selection);
        assert_eq!(result, Some("Hi".to_string()));  // Should clamp to line length
    }

    #[test]
    fn test_clipboard_error_display() {
        let err = ClipboardError::EmptySelection;
        assert_eq!(err.to_string(), "No text selected");

        let err = ClipboardError::ClipboardAccess("test error".to_string());
        assert_eq!(err.to_string(), "Clipboard error: test error");
    }
}
