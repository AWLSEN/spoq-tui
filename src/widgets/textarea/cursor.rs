//! Cursor movement functionality for TextAreaInput.

use super::TextAreaInput;
use tui_textarea::CursorMove;

impl<'a> TextAreaInput<'a> {
    /// Move cursor one position to the left
    /// Maps to: `move_cursor_left()` -> `move_cursor(CursorMove::Back)`
    pub fn move_cursor_left(&mut self) {
        self.textarea.move_cursor(CursorMove::Back);
    }

    /// Move cursor one position to the right
    /// Maps to: `move_cursor_right()` -> `move_cursor(CursorMove::Forward)`
    pub fn move_cursor_right(&mut self) {
        self.textarea.move_cursor(CursorMove::Forward);
    }

    /// Move cursor to the beginning of the current line
    /// Maps to: `move_cursor_home()` -> `move_cursor(CursorMove::Head)`
    pub fn move_cursor_home(&mut self) {
        self.textarea.move_cursor(CursorMove::Head);
    }

    /// Move cursor to the end of the current line
    /// Maps to: `move_cursor_end()` -> `move_cursor(CursorMove::End)`
    pub fn move_cursor_end(&mut self) {
        self.textarea.move_cursor(CursorMove::End);
    }

    /// Move cursor one word to the left
    /// Maps to: `move_cursor_word_left()` -> `move_cursor(CursorMove::WordBack)`
    pub fn move_cursor_word_left(&mut self) {
        self.textarea.move_cursor(CursorMove::WordBack);
    }

    /// Move cursor one word to the right
    /// Maps to: `move_cursor_word_right()` -> `move_cursor(CursorMove::WordForward)`
    pub fn move_cursor_word_right(&mut self) {
        self.textarea.move_cursor(CursorMove::WordForward);
    }

    /// Move cursor up one line
    /// New capability from tui-textarea
    pub fn move_cursor_up(&mut self) {
        self.textarea.move_cursor(CursorMove::Up);
    }

    /// Move cursor down one line
    /// New capability from tui-textarea
    pub fn move_cursor_down(&mut self) {
        self.textarea.move_cursor(CursorMove::Down);
    }

    /// Move cursor to the top of the document
    pub fn move_cursor_top(&mut self) {
        self.textarea.move_cursor(CursorMove::Top);
    }

    /// Move cursor to the bottom of the document
    pub fn move_cursor_bottom(&mut self) {
        self.textarea.move_cursor(CursorMove::Bottom);
    }

    /// Get the current cursor position as (row, col)
    pub fn cursor(&self) -> (usize, usize) {
        self.textarea.cursor()
    }

    /// Set cursor position
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        // Move to start first
        self.textarea.move_cursor(CursorMove::Top);
        self.textarea.move_cursor(CursorMove::Head);
        // Move down to target row
        for _ in 0..row {
            self.textarea.move_cursor(CursorMove::Down);
        }
        // Move right to target column
        for _ in 0..col {
            self.textarea.move_cursor(CursorMove::Forward);
        }
    }

    /// Check if cursor is on the first line
    pub fn is_cursor_on_first_line(&self) -> bool {
        self.textarea.cursor().0 == 0
    }

    /// Check if cursor is on the last line
    pub fn is_cursor_on_last_line(&self) -> bool {
        let (row, _) = self.textarea.cursor();
        let line_count = self.textarea.lines().len();
        row == line_count.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::super::TextAreaInput;

    #[test]
    fn test_cursor_movement() {
        let mut input = TextAreaInput::new();
        input.insert_char('H');
        input.insert_char('i');
        assert_eq!(input.cursor(), (0, 2));

        input.move_cursor_left();
        assert_eq!(input.cursor(), (0, 1));

        input.move_cursor_home();
        assert_eq!(input.cursor(), (0, 0));

        input.move_cursor_right();
        assert_eq!(input.cursor(), (0, 1));

        input.move_cursor_end();
        assert_eq!(input.cursor(), (0, 2));
    }

    #[test]
    fn test_cursor_bounds() {
        let mut input = TextAreaInput::new();
        input.insert_char('X');

        // Cursor should not go below 0
        input.move_cursor_home();
        input.move_cursor_left();
        assert_eq!(input.cursor(), (0, 0));

        // Cursor should not go beyond content length
        input.move_cursor_end();
        input.move_cursor_right();
        assert_eq!(input.cursor(), (0, 1));
    }

    #[test]
    fn test_multiline_cursor_movement() {
        let mut input = TextAreaInput::new();
        // Create "line1\nline2"
        for c in "line1".chars() {
            input.insert_char(c);
        }
        input.insert_newline();
        for c in "line2".chars() {
            input.insert_char(c);
        }

        assert_eq!(input.cursor(), (1, 5)); // End of second line

        // Move up
        input.move_cursor_up();
        assert_eq!(input.cursor(), (0, 5)); // End of first line

        // Move down
        input.move_cursor_down();
        assert_eq!(input.cursor(), (1, 5)); // End of second line
    }

    #[test]
    fn test_word_navigation() {
        let mut input = TextAreaInput::new();
        for c in "hello world".chars() {
            input.insert_char(c);
        }

        // Move word left from end
        input.move_cursor_word_left();
        // Should be at start of "world"
        let (row, col) = input.cursor();
        assert_eq!(row, 0);
        assert!(col <= 6); // At or before "world"
    }

    #[test]
    fn test_cursor_top_bottom() {
        let mut input = TextAreaInput::new();
        for c in "line1".chars() {
            input.insert_char(c);
        }
        input.insert_newline();
        for c in "line2".chars() {
            input.insert_char(c);
        }
        input.insert_newline();
        for c in "line3".chars() {
            input.insert_char(c);
        }

        assert_eq!(input.cursor().0, 2); // On line 3 (0-indexed)

        input.move_cursor_top();
        assert_eq!(input.cursor().0, 0); // On line 1

        input.move_cursor_bottom();
        assert_eq!(input.cursor().0, 2); // Back to line 3
    }

    #[test]
    fn test_is_cursor_on_first_line() {
        let mut input = TextAreaInput::new();
        // Empty input - cursor is on first line
        assert!(input.is_cursor_on_first_line());

        // Add content on first line
        for c in "line1".chars() {
            input.insert_char(c);
        }
        assert!(input.is_cursor_on_first_line());

        // Add second line
        input.insert_newline();
        assert!(!input.is_cursor_on_first_line());

        // Add content to second line
        for c in "line2".chars() {
            input.insert_char(c);
        }
        assert!(!input.is_cursor_on_first_line());

        // Move cursor up to first line
        input.move_cursor_up();
        assert!(input.is_cursor_on_first_line());
    }

    #[test]
    fn test_is_cursor_on_last_line() {
        let mut input = TextAreaInput::new();
        // Empty input - cursor is on last line (first and last are same)
        assert!(input.is_cursor_on_last_line());

        // Add content on first line
        for c in "line1".chars() {
            input.insert_char(c);
        }
        assert!(input.is_cursor_on_last_line());

        // Add second line
        input.insert_newline();
        for c in "line2".chars() {
            input.insert_char(c);
        }
        assert!(input.is_cursor_on_last_line());

        // Move cursor up to first line
        input.move_cursor_up();
        assert!(!input.is_cursor_on_last_line());

        // Move cursor down to last line
        input.move_cursor_down();
        assert!(input.is_cursor_on_last_line());
    }
}
