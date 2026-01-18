//! Editing operations for TextAreaInput.

use super::TextAreaInput;

impl<'a> TextAreaInput<'a> {
    /// Insert a character at the current cursor position.
    /// If hard wrap is enabled and the line exceeds the wrap width,
    /// automatically insert a newline at an appropriate position.
    pub fn insert_char(&mut self, c: char) {
        let (line, col) = self.textarea.cursor();

        // Don't allow inserting inside a token (but OK to insert at the end)
        if self.token_containing_cursor().is_some() {
            return; // silently ignore
        }

        self.textarea.insert_char(c);
        self.update_token_positions(line, col, 1);

        // Check if we need to hard wrap
        if let Some(wrap_width) = self.wrap_width {
            self.maybe_hard_wrap(wrap_width as usize);
        }
    }

    /// Delete the character before the cursor (like Backspace key)
    /// Maps to: `backspace()` -> `delete_char()`
    pub fn backspace(&mut self) {
        // Check if cursor is at or inside a token (backspace at token end = delete entire token)
        if let Some(idx) = self.token_at_or_ending_at_cursor() {
            self.remove_token(idx);
            return;
        }

        // Normal backspace
        let (line, col) = self.textarea.cursor();
        if col > 0 {
            self.textarea.delete_char();
            // Update token positions (1 char deleted)
            self.update_token_positions(line, col, -1);
        } else {
            // At start of line - just do normal backspace (may join lines)
            self.textarea.delete_char();
        }
    }

    /// Delete the character at the current cursor position (like Delete key)
    /// Maps to: `delete_char()` -> `delete_next_char()`
    pub fn delete_char(&mut self) {
        // Check if cursor is at start of a token
        if let Some(idx) = self.token_after_cursor() {
            self.remove_token(idx);
            return;
        }

        // Normal delete
        let (line, col) = self.textarea.cursor();
        self.textarea.delete_next_char();
        // Update token positions
        self.update_token_positions(line, col + 1, -1);
    }

    /// Delete from cursor position backward to the previous word boundary
    /// Maps to: `delete_word_backward()` -> `delete_word()`
    pub fn delete_word_backward(&mut self) {
        self.textarea.delete_word();
    }

    /// Delete from cursor position back to the start of the current line.
    /// Handles paste tokens atomically: removes tokens in deletion range and
    /// shifts remaining token positions.
    /// Maps to: `delete_to_line_start()` -> `delete_line_by_head()`
    pub fn delete_to_line_start(&mut self) {
        let (row, col) = self.textarea.cursor();
        // Remove tokens entirely or partially before cursor on this line
        self.paste_tokens
            .retain(|t| !(t.line == row && t.col_start < col));
        // After deletion, cursor at col 0 - shift remaining tokens on this line
        for token in &mut self.paste_tokens {
            if token.line == row {
                token.col_start = token.col_start.saturating_sub(col);
                token.col_end = token.col_end.saturating_sub(col);
            }
        }
        self.textarea.delete_line_by_head();
    }

    /// Delete from cursor to end of line
    pub fn delete_to_line_end(&mut self) {
        self.textarea.delete_line_by_end();
    }

    /// Delete the entire current line
    pub fn delete_line(&mut self) {
        self.textarea.delete_line_by_head();
        self.textarea.delete_line_by_end();
    }

    /// Insert a newline at the current cursor position
    /// Maps to: `insert_char('\n')` -> `insert_newline()`
    pub fn insert_newline(&mut self) {
        self.textarea.insert_newline();
    }

    /// Check if there are yank (paste) contents available
    pub fn yank_text(&self) -> String {
        self.textarea.yank_text()
    }

    /// Paste yanked text
    pub fn paste(&mut self) {
        self.textarea.paste();
    }

    /// Copy the current selection (or line if no selection)
    pub fn copy(&mut self) {
        self.textarea.copy();
    }

    /// Cut the current selection (or line if no selection)
    pub fn cut(&mut self) {
        self.textarea.cut();
    }

    /// Undo the last edit
    pub fn undo(&mut self) -> bool {
        self.textarea.undo()
    }

    /// Redo the last undone edit
    pub fn redo(&mut self) -> bool {
        self.textarea.redo()
    }

    /// Start or update selection
    pub fn start_selection(&mut self) {
        self.textarea.start_selection();
    }

    /// Cancel selection
    pub fn cancel_selection(&mut self) {
        self.textarea.cancel_selection();
    }

    /// Select all text
    pub fn select_all(&mut self) {
        self.textarea.select_all();
    }
}

#[cfg(test)]
mod tests {
    use super::super::TextAreaInput;

    #[test]
    fn test_insert_char() {
        let mut input = TextAreaInput::new();
        input.insert_char('H');
        input.insert_char('i');
        assert_eq!(input.content(), "Hi");
    }

    #[test]
    fn test_backspace() {
        let mut input = TextAreaInput::new();
        input.insert_char('H');
        input.insert_char('i');
        input.backspace();
        assert_eq!(input.content(), "H");
    }

    #[test]
    fn test_delete_char() {
        let mut input = TextAreaInput::new();
        input.insert_char('H');
        input.insert_char('i');
        input.move_cursor_left();
        input.delete_char();
        assert_eq!(input.content(), "H");
    }

    #[test]
    fn test_delete_word_backward() {
        let mut input = TextAreaInput::new();
        for c in "hello world".chars() {
            input.insert_char(c);
        }

        input.delete_word_backward();
        // "world" should be deleted
        let content = input.content();
        assert!(content.starts_with("hello"));
    }

    #[test]
    fn test_delete_to_line_start() {
        let mut input = TextAreaInput::new();
        for c in "hello".chars() {
            input.insert_char(c);
        }

        input.delete_to_line_start();
        assert!(input.is_empty() || input.content().is_empty());
    }

    #[test]
    fn test_delete_to_line_start_cleans_up_tokens() {
        let mut input = TextAreaInput::new();
        // Insert first token
        input.insert_paste_token("BEFORE".to_string());
        input.insert_char(' ');
        // Insert second token
        input.insert_paste_token("AFTER".to_string());
        // Cursor is after second token - content looks like: "[Pasted #1 ~1 lines] [Pasted #2 ~1 lines]"

        // Move cursor to middle (between tokens)
        for _ in 0..20 {
            input.move_cursor_left();
        }

        // delete_to_line_start should remove first token
        input.delete_to_line_start();
        let expanded = input.content_expanded();
        // First token gone, second should still expand correctly
        assert!(!expanded.contains("BEFORE"));
        // Second token should still be there and expand
        assert!(expanded.contains("AFTER"));
    }

    #[test]
    fn test_delete_to_line_start_shifts_remaining_token_positions() {
        let mut input = TextAreaInput::new();
        // Add some text before the token
        for c in "prefix ".chars() {
            input.insert_char(c);
        }
        // Insert a token
        input.insert_paste_token("TOKEN_CONTENT".to_string());

        // Move to end of prefix (before token)
        input.move_cursor_home();
        for _ in 0..7 {
            input.move_cursor_right();
        }

        // Delete to line start - removes "prefix ", token should shift to col 0
        input.delete_to_line_start();
        let expanded = input.content_expanded();
        // Token should still expand correctly from new position
        assert!(expanded.contains("TOKEN_CONTENT"));
    }

    #[test]
    fn test_insert_newline() {
        let mut input = TextAreaInput::new();
        input.insert_char('H');
        input.insert_newline();
        input.insert_char('i');
        assert_eq!(input.line_count(), 2);
        assert_eq!(input.content(), "H\ni");
    }

    #[test]
    fn test_undo_redo() {
        let mut input = TextAreaInput::new();
        input.insert_char('H');
        input.insert_char('i');
        assert_eq!(input.content(), "Hi");

        // Undo should work
        let undone = input.undo();
        assert!(undone);
        // Content should be changed (exact behavior depends on tui-textarea)
    }
}
