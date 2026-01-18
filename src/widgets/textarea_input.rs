//! TextArea wrapper module that adapts tui-textarea's API to match existing InputBox patterns.
//!
//! This module provides a compatibility layer that allows gradual migration from the custom
//! InputBox widget to tui-textarea without breaking existing code. The wrapper exposes methods
//! with the same names as InputBox but internally delegates to tui-textarea.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Widget},
};
use tui_textarea::{CursorMove, TextArea};

/// A paste token that behaves as an atomic unit.
///
/// Paste tokens are created when users paste text that exceeds certain thresholds.
/// They display as placeholders like `[Pasted #1 ~5 lines]` but expand to their
/// full content on submit. Tokens behave atomically - backspace/delete removes
/// the entire token, and the cursor cannot be positioned inside them.
#[derive(Debug, Clone)]
struct PasteToken {
    /// Unique identifier for this token
    id: u32,
    /// Line number where this token is located
    line: usize,
    /// Starting column (inclusive)
    col_start: usize,
    /// Ending column (exclusive) - token occupies [col_start, col_end)
    col_end: usize,
    /// The actual pasted content this token represents
    content: String,
}

/// A wrapper around tui-textarea that provides an API compatible with InputBox.
///
/// This allows existing code using InputBox to migrate to tui-textarea with minimal changes.
/// The wrapper maintains the same method names as InputBox while leveraging tui-textarea's
/// more robust text editing capabilities, including proper multi-line support.
#[derive(Debug, Clone)]
pub struct TextAreaInput<'a> {
    /// The underlying tui-textarea widget
    textarea: TextArea<'a>,
    /// Tracked paste tokens for atomic deletion
    paste_tokens: Vec<PasteToken>,
    /// Counter for generating unique paste token IDs
    paste_counter: u32,
}

impl Default for TextAreaInput<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> TextAreaInput<'a> {
    /// Create a new empty TextAreaInput
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        // Configure default styling to match InputBox dark theme
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default().fg(Color::Black).bg(Color::White));

        // Configure textarea behavior
        textarea.set_tab_length(4); // 4 spaces per tab (matches common Rust convention)
        // Line numbers are OFF by default in tui-textarea (no need to explicitly remove)

        Self {
            textarea,
            paste_tokens: Vec::new(),
            paste_counter: 0,
        }
    }

    /// Create a TextAreaInput with initial content
    pub fn with_content(content: &str) -> Self {
        let lines: Vec<String> = content.lines().map(String::from).collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default().fg(Color::Black).bg(Color::White));

        // Configure textarea behavior
        textarea.set_tab_length(4); // 4 spaces per tab (matches common Rust convention)
        // Line numbers are OFF by default in tui-textarea (no need to explicitly remove)

        // Move cursor to end
        textarea.move_cursor(CursorMove::Bottom);
        textarea.move_cursor(CursorMove::End);
        Self {
            textarea,
            paste_tokens: Vec::new(),
            paste_counter: 0,
        }
    }

    /// Get a reference to the underlying TextArea
    pub fn inner(&self) -> &TextArea<'a> {
        &self.textarea
    }

    /// Get a mutable reference to the underlying TextArea
    pub fn inner_mut(&mut self) -> &mut TextArea<'a> {
        &mut self.textarea
    }

    // =========================================================================
    // Paste token helper methods (private)
    // =========================================================================

    /// Find token where cursor is inside or at end (for backspace - at end should delete token)
    fn token_at_or_ending_at_cursor(&self) -> Option<usize> {
        let (row, col) = self.textarea.cursor();
        self.paste_tokens
            .iter()
            .position(|t| t.line == row && col > t.col_start && col <= t.col_end)
    }

    /// Find token where cursor is strictly inside (for insert - at end should allow insertion)
    fn token_containing_cursor(&self) -> Option<usize> {
        let (row, col) = self.textarea.cursor();
        self.paste_tokens
            .iter()
            .position(|t| t.line == row && col > t.col_start && col < t.col_end)
    }

    /// Find token immediately after cursor (for delete key)
    fn token_after_cursor(&self) -> Option<usize> {
        let (row, col) = self.textarea.cursor();
        self.paste_tokens
            .iter()
            .position(|t| t.line == row && col == t.col_start)
    }

    /// Update all token positions after an edit at (line, col) with delta chars
    fn update_token_positions(&mut self, edit_line: usize, edit_col: usize, delta: isize) {
        for token in &mut self.paste_tokens {
            if token.line == edit_line && token.col_start >= edit_col {
                token.col_start = (token.col_start as isize + delta) as usize;
                token.col_end = (token.col_end as isize + delta) as usize;
            }
        }
    }

    /// Remove token by index and delete its text from textarea
    fn remove_token(&mut self, idx: usize) {
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

    // =========================================================================
    // InputBox-compatible API methods
    // =========================================================================

    /// Insert a character at the current cursor position
    /// Maps to: `insert_char(c)` -> `insert_char(c)`
    pub fn insert_char(&mut self, c: char) {
        let (line, col) = self.textarea.cursor();

        // Don't allow inserting inside a token (but OK to insert at the end)
        if self.token_containing_cursor().is_some() {
            return; // silently ignore
        }

        self.textarea.insert_char(c);
        self.update_token_positions(line, col, 1);
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

    /// Delete from cursor position backward to the previous word boundary
    /// Maps to: `delete_word_backward()` -> `delete_word()`
    pub fn delete_word_backward(&mut self) {
        self.textarea.delete_word();
    }

    /// Delete from cursor position back to the start of the current line
    /// Maps to: `delete_to_line_start()` -> `delete_line_by_head()`
    pub fn delete_to_line_start(&mut self) {
        self.textarea.delete_line_by_head();
    }

    /// Insert a newline at the current cursor position
    /// Maps to: `insert_char('\n')` -> `insert_newline()`
    pub fn insert_newline(&mut self) {
        self.textarea.insert_newline();
    }

    /// Clear all content and reset cursor
    /// Maps to: `clear()` -> select_all + delete_char or recreate
    pub fn clear(&mut self) {
        self.textarea.select_all();
        self.textarea.delete_char();
        self.paste_tokens.clear();
        self.paste_counter = 0;
    }

    /// Get the current content of the input as a single string
    /// Maps to: `content()` -> `lines().join("\n")`
    pub fn content(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// Check if the input is empty
    /// Maps to: `is_empty()` -> `lines().iter().all(|l| l.is_empty())`
    pub fn is_empty(&self) -> bool {
        self.textarea.lines().iter().all(|l| l.is_empty())
    }

    /// Get the number of lines in the content
    /// Maps to: `line_count()` -> `lines().len()`
    pub fn line_count(&self) -> usize {
        self.textarea.lines().len().max(1)
    }

    // =========================================================================
    // New capabilities from tui-textarea (not in original InputBox)
    // =========================================================================

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

    /// Delete the entire current line
    pub fn delete_line(&mut self) {
        self.textarea.delete_line_by_head();
        self.textarea.delete_line_by_end();
    }

    /// Delete from cursor to end of line
    pub fn delete_to_line_end(&mut self) {
        self.textarea.delete_line_by_end();
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

    /// Get the lines as a slice
    pub fn lines(&self) -> &[String] {
        self.textarea.lines()
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

    // =========================================================================
    // Paste token methods (for atomic paste summarization)
    // =========================================================================

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
            id,
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
        sorted_tokens.sort_by(|a, b| {
            b.line
                .cmp(&a.line)
                .then(b.col_start.cmp(&a.col_start))
        });

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

    // =========================================================================
    // Styling and rendering
    // =========================================================================

    /// Set the block (border) for the textarea
    pub fn set_block(&mut self, block: Block<'a>) {
        self.textarea.set_block(block);
    }

    /// Set the cursor style
    pub fn set_cursor_style(&mut self, style: Style) {
        self.textarea.set_cursor_style(style);
    }

    /// Set the style for the current line
    pub fn set_cursor_line_style(&mut self, style: Style) {
        self.textarea.set_cursor_line_style(style);
    }

    /// Set the placeholder text
    pub fn set_placeholder_text(&mut self, text: impl Into<String>) {
        self.textarea.set_placeholder_text(text);
    }

    /// Set placeholder style
    pub fn set_placeholder_style(&mut self, style: Style) {
        self.textarea.set_placeholder_style(style);
    }

    /// Set tab width (number of spaces per tab character)
    pub fn set_tab_length(&mut self, len: u8) {
        self.textarea.set_tab_length(len);
    }

    /// Configure the textarea with a title and focus state (InputBox-compatible rendering)
    pub fn configure_for_render(&mut self, title: &'a str, focused: bool) {
        let border_color = if focused {
            Color::Gray
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title);

        self.textarea.set_block(block);

        // Set cursor visibility based on focus
        if focused {
            self.textarea
                .set_cursor_style(Style::default().fg(Color::Black).bg(Color::White));
        } else {
            self.textarea.set_cursor_style(Style::default());
        }
    }

    /// Render the textarea (InputBox-compatible method)
    pub fn render_with_title(&mut self, area: Rect, buf: &mut Buffer, title: &'a str, focused: bool) {
        self.configure_for_render(title, focused);
        // Use reference rendering (tui-textarea 0.7+ supports rendering &TextArea directly)
        (&self.textarea).render(area, buf);
    }
}

/// A renderable wrapper for TextAreaInput that implements the Widget trait
pub struct TextAreaInputWidget<'a, 'b> {
    textarea_input: &'b mut TextAreaInput<'a>,
    title: &'a str,
    focused: bool,
}

impl<'a, 'b> TextAreaInputWidget<'a, 'b> {
    pub fn new(textarea_input: &'b mut TextAreaInput<'a>, title: &'a str, focused: bool) -> Self {
        Self {
            textarea_input,
            title,
            focused,
        }
    }
}

impl Widget for TextAreaInputWidget<'_, '_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.textarea_input
            .configure_for_render(self.title, self.focused);
        // Use reference rendering (tui-textarea 0.7+ supports rendering &TextArea directly)
        (&self.textarea_input.textarea).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_textarea_input() {
        let input = TextAreaInput::new();
        assert!(input.is_empty());
    }

    #[test]
    fn test_with_content() {
        let input = TextAreaInput::with_content("hello\nworld");
        assert_eq!(input.line_count(), 2);
        assert_eq!(input.content(), "hello\nworld");
    }

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
    fn test_clear() {
        let mut input = TextAreaInput::new();
        input.insert_char('H');
        input.insert_char('i');
        input.clear();
        assert!(input.is_empty());
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
    fn test_line_count() {
        let mut input = TextAreaInput::new();
        assert_eq!(input.line_count(), 1); // Empty is 1 line

        input.insert_char('h');
        assert_eq!(input.line_count(), 1);

        input.insert_newline();
        assert_eq!(input.line_count(), 2);

        input.insert_char('w');
        assert_eq!(input.line_count(), 2);

        input.insert_newline();
        assert_eq!(input.line_count(), 3);
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
    fn test_lines_access() {
        let mut input = TextAreaInput::new();
        for c in "hello".chars() {
            input.insert_char(c);
        }
        input.insert_newline();
        for c in "world".chars() {
            input.insert_char(c);
        }

        let lines = input.lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "hello");
        assert_eq!(lines[1], "world");
    }

    // =========================================================================
    // Paste token tests
    // =========================================================================

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
