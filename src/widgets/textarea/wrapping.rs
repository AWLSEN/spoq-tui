//! Line wrapping functionality for TextAreaInput.

use super::TextAreaInput;
use unicode_width::UnicodeWidthStr;

impl<'a> TextAreaInput<'a> {
    /// Set the wrap width for hard wrapping. When set, inserting characters
    /// will automatically insert newlines when the line exceeds this width.
    /// Pass None to disable hard wrapping.
    pub fn set_wrap_width(&mut self, width: Option<u16>) {
        self.wrap_width = width;
    }

    /// Get the current wrap width
    pub fn wrap_width(&self) -> Option<u16> {
        self.wrap_width
    }

    /// Check if the current line exceeds the wrap width and insert a newline if needed.
    /// Wraps at word boundaries when possible for cleaner text.
    pub(super) fn maybe_hard_wrap(&mut self, wrap_width: usize) {
        use tui_textarea::CursorMove;

        if wrap_width == 0 {
            return;
        }

        let (row, col) = self.textarea.cursor();

        // Clone the current line to avoid borrow checker issues
        let current_line = {
            let lines = self.textarea.lines();
            if row >= lines.len() {
                return;
            }
            lines[row].clone()
        };

        let line_width = current_line.width();

        // Only wrap if line exceeds the wrap width
        if line_width <= wrap_width {
            return;
        }

        // Find the best position to wrap (at a word boundary if possible)
        let wrap_pos = self.find_wrap_position(&current_line, wrap_width);

        if wrap_pos > 0 && wrap_pos < current_line.len() {
            // Save current cursor column
            let cursor_col = col;

            // Skip any spaces at the wrap position (calculate before mutating)
            let chars: Vec<char> = current_line.chars().collect();
            let mut skip_spaces = 0;
            let mut pos = wrap_pos;
            while pos < chars.len() && chars[pos] == ' ' {
                skip_spaces += 1;
                pos += 1;
            }

            // Move cursor to the wrap position
            self.textarea.move_cursor(CursorMove::Head);
            for _ in 0..wrap_pos {
                self.textarea.move_cursor(CursorMove::Forward);
            }

            // Delete the spaces and insert newline
            for _ in 0..skip_spaces {
                self.textarea.delete_next_char();
            }
            self.textarea.insert_newline();

            // Restore cursor position on the new line
            if cursor_col > wrap_pos {
                // Cursor was after the wrap point, move to new line
                let new_col = cursor_col - wrap_pos - skip_spaces;
                self.textarea.move_cursor(CursorMove::Head);
                for _ in 0..new_col {
                    self.textarea.move_cursor(CursorMove::Forward);
                }
            } else {
                // Cursor was before wrap point, stay on current line
                self.textarea.move_cursor(CursorMove::Up);
                self.textarea.move_cursor(CursorMove::Head);
                for _ in 0..cursor_col {
                    self.textarea.move_cursor(CursorMove::Forward);
                }
            }
        }
    }

    /// Find the best position to wrap a line, preferring word boundaries.
    pub(super) fn find_wrap_position(&self, line: &str, wrap_width: usize) -> usize {
        let chars: Vec<char> = line.chars().collect();
        let mut current_width = 0;
        let mut last_space_idx = None;

        for (idx, ch) in chars.iter().enumerate() {
            let ch_width = ch.to_string().width();
            current_width += ch_width;

            if *ch == ' ' {
                last_space_idx = Some(idx);
            }

            if current_width > wrap_width {
                // We've exceeded the wrap width
                // Prefer to wrap at the last space if it's not too far back
                if let Some(space_idx) = last_space_idx {
                    // Only use space if it's within reasonable distance (at least half the width)
                    if space_idx > wrap_width / 3 {
                        return space_idx;
                    }
                }
                // No good space found, wrap at current position
                return idx;
            }
        }

        // Line doesn't need wrapping
        line.len()
    }
}

#[cfg(test)]
mod tests {
    use super::super::TextAreaInput;

    #[test]
    fn test_hard_wrap_disabled_by_default() {
        let input = TextAreaInput::new();
        assert!(input.wrap_width().is_none());
    }

    #[test]
    fn test_set_wrap_width() {
        let mut input = TextAreaInput::new();
        input.set_wrap_width(Some(20));
        assert_eq!(input.wrap_width(), Some(20));

        input.set_wrap_width(None);
        assert!(input.wrap_width().is_none());
    }

    #[test]
    fn test_hard_wrap_inserts_newline_at_width() {
        let mut input = TextAreaInput::new();
        input.set_wrap_width(Some(10)); // Wrap at 10 characters

        // Type "hello world" - should wrap after "hello" at width 10
        for c in "hello world".chars() {
            input.insert_char(c);
        }

        // Should have 2 lines now
        assert_eq!(input.line_count(), 2);
    }

    #[test]
    fn test_hard_wrap_at_word_boundary() {
        let mut input = TextAreaInput::new();
        input.set_wrap_width(Some(15)); // Wrap at 15 characters

        // Type "hello beautiful world"
        for c in "hello beautiful world".chars() {
            input.insert_char(c);
        }

        // Should wrap at word boundaries
        assert!(input.line_count() >= 2);
    }

    #[test]
    fn test_hard_wrap_cursor_position_after_wrap() {
        let mut input = TextAreaInput::new();
        input.set_wrap_width(Some(10));

        // Type text that will wrap
        for c in "hello world".chars() {
            input.insert_char(c);
        }

        // Cursor should be at end of new line
        let (row, _col) = input.cursor();
        assert_eq!(row, 1); // Should be on second line
    }

    #[test]
    fn test_hard_wrap_no_wrap_when_disabled() {
        let mut input = TextAreaInput::new();
        // Don't set wrap_width

        // Type long text
        for c in "this is a very long line that should not wrap automatically".chars() {
            input.insert_char(c);
        }

        // Should still be on one line
        assert_eq!(input.line_count(), 1);
    }

    #[test]
    fn test_hard_wrap_allows_navigation_with_arrows() {
        let mut input = TextAreaInput::new();
        input.set_wrap_width(Some(10));

        // Type text that wraps
        for c in "hello world here".chars() {
            input.insert_char(c);
        }

        // Multiple lines should exist
        assert!(input.line_count() >= 2);

        // Should be able to navigate up
        let initial_row = input.cursor().0;
        input.move_cursor_up();
        let new_row = input.cursor().0;

        // If we were on line > 0, we should have moved up
        if initial_row > 0 {
            assert!(new_row < initial_row);
        }
    }
}
