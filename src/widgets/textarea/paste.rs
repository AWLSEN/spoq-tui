//! Paste token functionality for atomic paste summarization.
//!
//! Paste tokens are created when users paste text that exceeds certain thresholds.
//! They display as placeholders like `[Pasted #1 ~5 lines]` but expand to their
//! full content on submit. Tokens behave atomically - backspace/delete removes
//! the entire token, and the cursor cannot be positioned inside them.

use super::TextAreaInput;

/// A paste token that behaves as an atomic unit.
///
/// Paste tokens are created when users paste text that exceeds certain thresholds.
/// They display as placeholders like `[Pasted #1 ~5 lines]` but expand to their
/// full content on submit. Tokens behave atomically - backspace/delete removes
/// the entire token, and the cursor cannot be positioned inside them.
#[derive(Debug, Clone)]
pub(in crate::widgets) struct PasteToken {
    /// Line number where this token is located
    pub(super) line: usize,
    /// Starting column (inclusive)
    pub(super) col_start: usize,
    /// Ending column (exclusive) - token occupies [col_start, col_end)
    pub(super) col_end: usize,
    /// The actual pasted content this token represents
    pub(super) content: String,
}

impl<'a> TextAreaInput<'a> {
    /// Find token where cursor is inside or at end (for backspace - at end should delete token)
    pub(super) fn token_at_or_ending_at_cursor(&self) -> Option<usize> {
        let (row, col) = self.textarea.cursor();
        self.paste_tokens
            .iter()
            .position(|t| t.line == row && col > t.col_start && col <= t.col_end)
    }

    /// Find token where cursor is strictly inside (for insert - at end should allow insertion)
    pub(super) fn token_containing_cursor(&self) -> Option<usize> {
        let (row, col) = self.textarea.cursor();
        self.paste_tokens
            .iter()
            .position(|t| t.line == row && col > t.col_start && col < t.col_end)
    }

    /// Find token immediately after cursor (for delete key)
    pub(super) fn token_after_cursor(&self) -> Option<usize> {
        let (row, col) = self.textarea.cursor();
        self.paste_tokens
            .iter()
            .position(|t| t.line == row && col == t.col_start)
    }

    /// Update all token positions after an edit at (line, col) with delta chars
    pub(super) fn update_token_positions(
        &mut self,
        edit_line: usize,
        edit_col: usize,
        delta: isize,
    ) {
        for token in &mut self.paste_tokens {
            if token.line == edit_line && token.col_start >= edit_col {
                token.col_start = (token.col_start as isize + delta) as usize;
                token.col_end = (token.col_end as isize + delta) as usize;
            }
        }
    }

    /// Remove token by index and delete its text from textarea
    pub(super) fn remove_token(&mut self, idx: usize) {
        use tui_textarea::CursorMove;

        let token = self.paste_tokens.remove(idx);
        let token_len = token.col_end - token.col_start;

        // Move cursor to token start
        self.textarea.move_cursor(CursorMove::Top);
        self.textarea.move_cursor(CursorMove::Head);
        for _ in 0..token.line {
            self.textarea.move_cursor(CursorMove::Down);
        }
        for _ in 0..token.col_start {
            self.textarea.move_cursor(CursorMove::Forward);
        }

        // Select the token's text range and delete
        self.textarea.start_selection();
        for _ in 0..token_len {
            self.textarea.move_cursor(CursorMove::Forward);
        }
        self.textarea.delete_char(); // deletes selection
        self.textarea.cancel_selection();

        // Update positions of tokens after this one
        self.update_token_positions(token.line, token.col_start, -(token_len as isize));
    }

    /// Insert a paste token at current cursor position.
    ///
    /// This creates a placeholder like `[Pasted #1 ~5 lines]` that represents the
    /// pasted content. The token is tracked for atomic deletion and will be expanded
    /// to its full content when `content_expanded()` is called.
    ///
    /// Returns the token ID for reference.
    pub fn insert_paste_token(&mut self, content: String) -> u32 {
        self.paste_counter += 1;
        let id = self.paste_counter;
        let line_count = content.lines().count().max(1);
        let display = format!("[Pasted #{} ~{} lines]", id, line_count);
        let display_len = display.chars().count();

        let (line, col_start) = self.textarea.cursor();

        // Insert the display text (bypass our insert_char to avoid token check)
        for ch in display.chars() {
            self.textarea.insert_char(ch);
        }

        // Update positions of any tokens after this insertion point
        self.update_token_positions(line, col_start, display_len as isize);

        // Track this token
        self.paste_tokens.push(PasteToken {
            line,
            col_start,
            col_end: col_start + display_len,
            content,
        });

        id
    }

    /// Get content with all tokens expanded to their actual content.
    ///
    /// This is what should be used when submitting the input - it replaces
    /// all placeholder tokens with their original pasted content.
    pub fn content_expanded(&self) -> String {
        let raw = self.textarea.lines().join("\n");
        let mut result = raw.clone();

        // Replace tokens in reverse order (so positions stay valid)
        let mut sorted_tokens: Vec<_> = self.paste_tokens.iter().collect();
        sorted_tokens.sort_by(|a, b| b.line.cmp(&a.line).then(b.col_start.cmp(&a.col_start)));

        for token in sorted_tokens {
            // Find this token's display text in result
            let lines: Vec<&str> = result.lines().collect();
            if token.line < lines.len() {
                let line = lines[token.line];
                if token.col_end <= line.chars().count() {
                    // Build new line with token replaced
                    let before: String = line.chars().take(token.col_start).collect();
                    let after: String = line.chars().skip(token.col_end).collect();
                    let new_line = format!("{}{}{}", before, token.content, after);

                    // Reconstruct result with new line
                    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
                    new_lines[token.line] = new_line;
                    result = new_lines.join("\n");
                }
            }
        }

        result
    }

    /// Clear all paste tokens (call after submit).
    ///
    /// This resets the token tracking state. Call this after successfully
    /// submitting input to prepare for the next input session.
    pub fn clear_paste_tokens(&mut self) {
        self.paste_tokens.clear();
        self.paste_counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::super::TextAreaInput;

    #[test]
    fn test_insert_paste_token_creates_placeholder() {
        let mut input = TextAreaInput::new();
        input.insert_paste_token("line1\nline2\nline3\nline4".to_string());
        let raw = input.content();
        assert!(raw.contains("[Pasted #1 ~4 lines]"));
    }

    #[test]
    fn test_insert_paste_token_increments_id() {
        let mut input = TextAreaInput::new();
        input.insert_paste_token("a".to_string());
        input.insert_paste_token("b".to_string());
        let raw = input.content();
        assert!(raw.contains("#1"));
        assert!(raw.contains("#2"));
    }

    #[test]
    fn test_content_expanded_replaces_token() {
        let mut input = TextAreaInput::new();
        input.insert_char('X');
        input.insert_paste_token("ACTUAL CONTENT".to_string());
        input.insert_char('Y');

        let expanded = input.content_expanded();
        assert!(expanded.contains("ACTUAL CONTENT"));
        assert!(!expanded.contains("[Pasted"));
        assert!(expanded.starts_with('X'));
        assert!(expanded.ends_with('Y'));
    }

    #[test]
    fn test_backspace_deletes_entire_token() {
        let mut input = TextAreaInput::new();
        input.insert_paste_token("content".to_string());
        // Cursor is now at end of token
        input.backspace();
        assert!(input.is_empty());
        assert!(input.paste_tokens.is_empty());
    }

    #[test]
    fn test_delete_at_token_start_deletes_entire_token() {
        let mut input = TextAreaInput::new();
        input.insert_paste_token("content".to_string());
        input.move_cursor_home(); // Move to start of token
        input.delete_char();
        assert!(input.is_empty());
    }

    #[test]
    fn test_manual_typing_not_treated_as_token() {
        let mut input = TextAreaInput::new();
        for ch in "[Pasted #1 ~5 lines]".chars() {
            input.insert_char(ch);
        }
        // This is NOT a token - no tokens tracked
        assert!(input.paste_tokens.is_empty());
        // Backspace deletes one char, not the whole thing
        input.backspace();
        assert!(input.content().ends_with("lines"));
    }

    #[test]
    fn test_clear_removes_all_tokens() {
        let mut input = TextAreaInput::new();
        input.insert_paste_token("a".to_string());
        input.insert_paste_token("b".to_string());
        input.clear();
        assert!(input.is_empty());
        assert!(input.paste_tokens.is_empty());
        assert_eq!(input.paste_counter, 0);
    }

    #[test]
    fn test_multiple_tokens_expand_correctly() {
        let mut input = TextAreaInput::new();
        input.insert_paste_token("FIRST".to_string());
        input.insert_char(' ');
        input.insert_paste_token("SECOND".to_string());

        let expanded = input.content_expanded();
        assert_eq!(expanded, "FIRST SECOND");
    }
}
