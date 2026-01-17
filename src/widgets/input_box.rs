use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Widget},
};

/// A text input widget with cursor handling and scrolling support.
///
/// Features:
/// - Basic text editing (insert, delete, backspace)
/// - Cursor movement (left/right)
/// - Horizontal scrolling when text exceeds widget width
/// - Dark theme styling with gray border and white cursor
#[derive(Debug, Clone, Default)]
pub struct InputBox {
    /// The text content of the input box
    content: String,
    /// Current cursor position (character index)
    cursor_position: usize,
    /// Horizontal scroll offset for when text exceeds widget width
    scroll_offset: usize,
}

impl InputBox {
    /// Create a new empty InputBox
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
        }
    }

    /// Convert character index to byte index
    fn char_to_byte_index(&self, char_idx: usize) -> usize {
        self.content
            .char_indices()
            .nth(char_idx)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or(self.content.len())
    }

    /// Insert a character at the current cursor position
    pub fn insert_char(&mut self, c: char) {
        let byte_idx = self.char_to_byte_index(self.cursor_position);
        self.content.insert(byte_idx, c);
        self.cursor_position += 1;
    }

    /// Delete the character at the current cursor position (like Delete key)
    pub fn delete_char(&mut self) {
        if self.cursor_position < self.content.chars().count() {
            let byte_idx = self.char_to_byte_index(self.cursor_position);
            self.content.remove(byte_idx);
        }
    }

    /// Delete the character before the cursor (like Backspace key)
    pub fn backspace(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            let byte_idx = self.char_to_byte_index(self.cursor_position);
            self.content.remove(byte_idx);
        }
    }

    /// Move cursor one position to the left
    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    /// Move cursor one position to the right
    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.content.chars().count() {
            self.cursor_position += 1;
        }
    }

    /// Move cursor to the beginning of the text
    pub fn move_cursor_home(&mut self) {
        self.cursor_position = 0;
    }

    /// Move cursor to the end of the text
    pub fn move_cursor_end(&mut self) {
        self.cursor_position = self.content.chars().count();
    }

    /// Check if a character is a word character (alphanumeric or underscore)
    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// Move cursor one word to the left
    ///
    /// Word boundary logic:
    /// 1. Skip any whitespace/non-word characters moving left
    /// 2. Then skip contiguous word characters (alphanumeric + underscore)
    /// 3. Stop at the beginning of the word
    pub fn move_cursor_word_left(&mut self) {
        if self.cursor_position == 0 {
            return;
        }

        // Collect characters before cursor position
        let chars: Vec<char> = self.content.chars().collect();

        // Start from position just before cursor
        let mut pos = self.cursor_position;

        // Phase 1: Skip any non-word characters (whitespace, punctuation)
        while pos > 0 && !Self::is_word_char(chars[pos - 1]) {
            pos -= 1;
        }

        // Phase 2: Skip word characters to find start of word
        while pos > 0 && Self::is_word_char(chars[pos - 1]) {
            pos -= 1;
        }

        self.cursor_position = pos;
    }

    /// Move cursor one word to the right
    ///
    /// Word boundary logic:
    /// 1. Skip any whitespace/non-word characters moving right
    /// 2. Then skip contiguous word characters (alphanumeric + underscore)
    /// 3. Stop at the end of the word
    pub fn move_cursor_word_right(&mut self) {
        let chars: Vec<char> = self.content.chars().collect();
        let len = chars.len();

        if self.cursor_position >= len {
            return;
        }

        let mut pos = self.cursor_position;

        // Phase 1: Skip any non-word characters (whitespace, punctuation)
        while pos < len && !Self::is_word_char(chars[pos]) {
            pos += 1;
        }

        // Phase 2: Skip word characters to find end of word
        while pos < len && Self::is_word_char(chars[pos]) {
            pos += 1;
        }

        self.cursor_position = pos;
    }

    /// Delete from cursor position backward to the previous word boundary
    ///
    /// Uses the same word boundary logic as `move_cursor_word_left`:
    /// 1. Skip any whitespace/non-word characters moving left
    /// 2. Then skip contiguous word characters (alphanumeric + underscore)
    /// 3. Delete all characters from that position to the cursor
    ///
    /// # Examples
    ///
    /// - From `"hello wor|ld"` -> `"hello |ld"` (deletes "wor")
    /// - From `"hello |world"` -> `"|world"` (deletes "hello ")
    pub fn delete_word_backward(&mut self) {
        if self.cursor_position == 0 {
            return;
        }

        // Collect characters to work with
        let chars: Vec<char> = self.content.chars().collect();

        // Find the word boundary using the same logic as move_cursor_word_left
        let mut target_pos = self.cursor_position;

        // Phase 1: Skip any non-word characters (whitespace, punctuation)
        while target_pos > 0 && !Self::is_word_char(chars[target_pos - 1]) {
            target_pos -= 1;
        }

        // Phase 2: Skip word characters to find start of word
        while target_pos > 0 && Self::is_word_char(chars[target_pos - 1]) {
            target_pos -= 1;
        }

        // Delete characters from target_pos to cursor_position
        // We need to work with byte indices for string manipulation
        let start_byte = self.char_to_byte_index(target_pos);
        let end_byte = self.char_to_byte_index(self.cursor_position);

        // Remove the range of characters
        self.content.replace_range(start_byte..end_byte, "");

        // Update cursor position
        self.cursor_position = target_pos;
    }

    /// Clear all content and reset cursor
    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_position = 0;
        self.scroll_offset = 0;
    }

    /// Delete from cursor position back to the start of the current line.
    ///
    /// In multi-line content, finds the previous newline character and deletes
    /// from there to the cursor. If on the first line (no preceding newline),
    /// deletes from the start of content to the cursor.
    ///
    /// # Examples
    ///
    /// - From `"line1\nli|ne2"` -> `"line1\n|ne2"` (deletes "li")
    /// - From `"hel|lo"` -> `"|lo"` (deletes "hel", single line)
    /// - From `"|hello"` -> `"|hello"` (cursor at start, nothing deleted)
    pub fn delete_to_line_start(&mut self) {
        if self.cursor_position == 0 {
            return;
        }

        // Find the position of the last newline before cursor, if any
        // We look at characters from index 0 to cursor_position - 1
        let mut line_start_pos = 0;
        for (char_idx, c) in self.content.chars().enumerate() {
            if char_idx >= self.cursor_position {
                break;
            }
            if c == '\n' {
                // The line starts after this newline
                line_start_pos = char_idx + 1;
            }
        }

        // If cursor is already at line start, nothing to delete
        if self.cursor_position == line_start_pos {
            return;
        }

        // Delete from line_start_pos to cursor_position
        let start_byte = self.char_to_byte_index(line_start_pos);
        let end_byte = self.char_to_byte_index(self.cursor_position);

        // Remove the range from the string
        self.content.replace_range(start_byte..end_byte, "");

        // Update cursor position to line start
        self.cursor_position = line_start_pos;
    }

    /// Check if the input box is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Get the current content of the input box
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the number of lines in the content (for dynamic height calculation)
    pub fn line_count(&self) -> usize {
        if self.content.is_empty() {
            1
        } else {
            self.content.lines().count().max(1)
        }
    }

    /// Render the input box with the given title
    pub fn render_with_title(&self, area: Rect, buf: &mut Buffer, title: &str, focused: bool) {
        // Calculate inner area (accounting for border)
        let inner_width = area.width.saturating_sub(2);

        // Create a mutable copy to update scroll
        let mut scroll_offset = self.scroll_offset;

        // Update scroll offset calculation
        if inner_width > 0 {
            if self.cursor_position < scroll_offset {
                scroll_offset = self.cursor_position;
            }
            if self.cursor_position >= scroll_offset + inner_width as usize {
                scroll_offset = self.cursor_position - inner_width as usize + 1;
            }
        }

        // Draw border with dark theme colors
        let border_color = if focused { Color::Gray } else { Color::DarkGray };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title);

        // Render the block
        block.render(area, buf);

        // Calculate inner area for text (use available height for multi-line)
        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: inner_width,
            height: area.height.saturating_sub(2),
        };
        let center_y = inner_area.y + inner_area.height.saturating_sub(1) / 2;

        if inner_area.width == 0 || inner_area.height == 0 {
            return;
        }

        // Get the visible portion of text
        let visible_text: String = self
            .content
            .chars()
            .skip(scroll_offset)
            .take(inner_width as usize)
            .collect();

        // Render the text
        let text_style = Style::default().fg(Color::White);
        for (i, c) in visible_text.chars().enumerate() {
            if i < inner_width as usize {
                buf.set_string(
                    inner_area.x + i as u16,
                    center_y,
                    c.to_string(),
                    text_style,
                );
            }
        }

        // Render the cursor if focused
        if focused {
            let cursor_x = (self.cursor_position - scroll_offset) as u16;
            if cursor_x < inner_width {
                let cursor_char = self
                    .content
                    .chars()
                    .nth(self.cursor_position)
                    .unwrap_or(' ');

                // White cursor block for dark theme
                let cursor_style = Style::default()
                    .fg(Color::Black)
                    .bg(Color::White);

                buf.set_string(
                    inner_area.x + cursor_x,
                    center_y,
                    cursor_char.to_string(),
                    cursor_style,
                );
            }
        }
    }
}

/// A renderable wrapper for InputBox that implements the Widget trait
pub struct InputBoxWidget<'a> {
    input_box: &'a InputBox,
    title: &'a str,
    focused: bool,
}

impl<'a> InputBoxWidget<'a> {
    pub fn new(input_box: &'a InputBox, title: &'a str, focused: bool) -> Self {
        Self {
            input_box,
            title,
            focused,
        }
    }
}

impl Widget for InputBoxWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let show_cursor = self.focused;
        self.render_normal(area, buf, show_cursor);
    }
}

impl InputBoxWidget<'_> {
    /// Render with normal border and optional cursor
    fn render_normal(&self, area: Rect, buf: &mut Buffer, show_cursor: bool) {
        // Calculate inner area (accounting for border)
        let inner_width = area.width.saturating_sub(2);

        // Create a mutable copy to update scroll
        let mut scroll_offset = self.input_box.scroll_offset;

        // Update scroll offset calculation
        if inner_width > 0 {
            if self.input_box.cursor_position < scroll_offset {
                scroll_offset = self.input_box.cursor_position;
            }
            if self.input_box.cursor_position >= scroll_offset + inner_width as usize {
                scroll_offset = self.input_box.cursor_position - inner_width as usize + 1;
            }
        }

        // Draw border with dark theme colors
        let border_color = if self.focused { Color::Gray } else { Color::DarkGray };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(self.title);

        // Render the block
        block.render(area, buf);

        // Calculate inner area for text
        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: inner_width,
            height: area.height.saturating_sub(2),
        };
        let center_y = inner_area.y + inner_area.height.saturating_sub(1) / 2;

        if inner_area.width == 0 || inner_area.height == 0 {
            return;
        }

        // Get the visible portion of text
        let visible_text: String = self.input_box
            .content
            .chars()
            .skip(scroll_offset)
            .take(inner_width as usize)
            .collect();

        // Render the text
        let text_style = Style::default().fg(Color::White);
        for (i, c) in visible_text.chars().enumerate() {
            if i < inner_width as usize {
                buf.set_string(
                    inner_area.x + i as u16,
                    center_y,
                    c.to_string(),
                    text_style,
                );
            }
        }

        // Render the cursor
        if show_cursor {
            let cursor_x = (self.input_box.cursor_position - scroll_offset) as u16;
            if cursor_x < inner_width {
                let cursor_char = self.input_box
                    .content
                    .chars()
                    .nth(self.input_box.cursor_position)
                    .unwrap_or(' ');

                // White cursor block for dark theme
                let cursor_style = Style::default()
                    .fg(Color::Black)
                    .bg(Color::White);

                buf.set_string(
                    inner_area.x + cursor_x,
                    center_y,
                    cursor_char.to_string(),
                    cursor_style,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_input_box() {
        let input = InputBox::new();
        assert!(input.is_empty());
    }

    #[test]
    fn test_insert_char() {
        let mut input = InputBox::new();
        input.insert_char('H');
        input.insert_char('i');
        assert_eq!(input.content, "Hi");
    }

    #[test]
    fn test_backspace() {
        let mut input = InputBox::new();
        input.insert_char('H');
        input.insert_char('i');
        input.backspace();
        assert_eq!(input.content, "H");
    }

    #[test]
    fn test_delete_char() {
        let mut input = InputBox::new();
        input.insert_char('H');
        input.insert_char('i');
        input.move_cursor_left();
        input.delete_char();
        assert_eq!(input.content, "H");
    }

    #[test]
    fn test_cursor_movement() {
        let mut input = InputBox::new();
        input.insert_char('H');
        input.insert_char('i');
        assert_eq!(input.cursor_position, 2);

        input.move_cursor_left();
        assert_eq!(input.cursor_position, 1);

        input.move_cursor_home();
        assert_eq!(input.cursor_position, 0);

        input.move_cursor_right();
        assert_eq!(input.cursor_position, 1);

        input.move_cursor_end();
        assert_eq!(input.cursor_position, 2);
    }

    #[test]
    fn test_cursor_bounds() {
        let mut input = InputBox::new();
        input.insert_char('X');

        // Cursor should not go below 0
        input.move_cursor_home();
        input.move_cursor_left();
        assert_eq!(input.cursor_position, 0);

        // Cursor should not go beyond content length
        input.move_cursor_end();
        input.move_cursor_right();
        assert_eq!(input.cursor_position, 1);
    }

    #[test]
    fn test_clear() {
        let mut input = InputBox::new();
        input.insert_char('H');
        input.insert_char('i');
        input.clear();
        assert!(input.is_empty());
    }

    #[test]
    fn test_move_cursor_word_left_basic() {
        let mut input = InputBox::new();
        // "hello world|"
        for c in "hello world".chars() {
            input.insert_char(c);
        }
        assert_eq!(input.cursor_position, 11);

        // Should move to start of "world"
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 6); // "hello |world"
    }

    #[test]
    fn test_move_cursor_word_left_multiple_spaces() {
        let mut input = InputBox::new();
        // "hello   world|"
        for c in "hello   world".chars() {
            input.insert_char(c);
        }

        // Should skip spaces and move to start of "world"
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 8); // "hello   |world"
    }

    #[test]
    fn test_move_cursor_word_left_punctuation() {
        let mut input = InputBox::new();
        // "hello, world|"
        for c in "hello, world".chars() {
            input.insert_char(c);
        }

        // Should skip punctuation and move to start of "world"
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 7); // "hello, |world"
    }

    #[test]
    fn test_move_cursor_word_left_at_start() {
        let mut input = InputBox::new();
        for c in "hello".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();

        // Should not move if already at start
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_move_cursor_word_right_basic() {
        let mut input = InputBox::new();
        // "|hello world"
        for c in "hello world".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();
        assert_eq!(input.cursor_position, 0);

        // Should move to end of "hello"
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 5); // "hello| world"
    }

    #[test]
    fn test_move_cursor_word_right_multiple_spaces() {
        let mut input = InputBox::new();
        // "|hello   world"
        for c in "hello   world".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();

        // Should move to end of "hello"
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 5); // "hello|   world"
    }

    #[test]
    fn test_move_cursor_word_right_at_end() {
        let mut input = InputBox::new();
        for c in "hello".chars() {
            input.insert_char(c);
        }

        // Should not move if already at end
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 5);
    }

    #[test]
    fn test_delete_word_backward_basic() {
        let mut input = InputBox::new();
        // "hello world|"
        for c in "hello world".chars() {
            input.insert_char(c);
        }

        // Should delete "world"
        input.delete_word_backward();
        assert_eq!(input.content, "hello ");
        assert_eq!(input.cursor_position, 6);
    }

    #[test]
    fn test_delete_word_backward_with_spaces() {
        let mut input = InputBox::new();
        // "hello   world|"
        for c in "hello   world".chars() {
            input.insert_char(c);
        }

        // Should delete "world" and spaces
        input.delete_word_backward();
        assert_eq!(input.content, "hello   ");
        assert_eq!(input.cursor_position, 8);
    }

    #[test]
    fn test_delete_word_backward_middle_of_word() {
        let mut input = InputBox::new();
        // "hello wor|ld"
        for c in "hello world".chars() {
            input.insert_char(c);
        }
        input.move_cursor_left();
        input.move_cursor_left();

        // Should delete "wor"
        input.delete_word_backward();
        assert_eq!(input.content, "hello ld");
        assert_eq!(input.cursor_position, 6);
    }

    #[test]
    fn test_delete_word_backward_at_start() {
        let mut input = InputBox::new();
        for c in "hello".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();

        // Should not delete if at start
        input.delete_word_backward();
        assert_eq!(input.content, "hello");
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_delete_to_line_start_basic() {
        let mut input = InputBox::new();
        // "hel|lo"
        for c in "hello".chars() {
            input.insert_char(c);
        }
        // Move cursor to position 3
        input.move_cursor_home();
        input.move_cursor_right();
        input.move_cursor_right();
        input.move_cursor_right();

        // Should delete "hel"
        input.delete_to_line_start();
        assert_eq!(input.content, "lo");
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_delete_to_line_start_multiline() {
        let mut input = InputBox::new();
        // "line1\nli|ne2"
        for c in "line1\nline2".chars() {
            input.insert_char(c);
        }
        // Position cursor at "li|ne2" (position 8)
        input.move_cursor_home();
        for _ in 0..8 {
            input.move_cursor_right();
        }

        // Should delete "li" from second line
        input.delete_to_line_start();
        assert_eq!(input.content, "line1\nne2");
        assert_eq!(input.cursor_position, 6); // At start of line after newline
    }

    #[test]
    fn test_delete_to_line_start_at_line_start() {
        let mut input = InputBox::new();
        for c in "hello".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();

        // Should not delete if already at line start
        input.delete_to_line_start();
        assert_eq!(input.content, "hello");
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_delete_to_line_start_after_newline() {
        let mut input = InputBox::new();
        // "line1\n|line2"
        for c in "line1\nline2".chars() {
            input.insert_char(c);
        }
        // Position cursor right after newline (position 6)
        input.move_cursor_home();
        for _ in 0..6 {
            input.move_cursor_right();
        }

        // Should not delete if at start of line
        input.delete_to_line_start();
        assert_eq!(input.content, "line1\nline2");
        assert_eq!(input.cursor_position, 6);
    }

    // Multi-line input tests (for Round 1)
    #[test]
    fn test_insert_newline_single_char() {
        let mut input = InputBox::new();
        input.insert_char('\n');
        assert_eq!(input.content, "\n");
        assert_eq!(input.cursor_position, 1);
    }

    #[test]
    fn test_insert_newline_in_middle_of_text() {
        let mut input = InputBox::new();
        // "hello|world"
        for c in "helloworld".chars() {
            input.insert_char(c);
        }
        // Move cursor to middle
        input.move_cursor_home();
        for _ in 0..5 {
            input.move_cursor_right();
        }

        // Insert newline
        input.insert_char('\n');
        assert_eq!(input.content, "hello\nworld");
        assert_eq!(input.cursor_position, 6);
    }

    #[test]
    fn test_multiline_line_count() {
        let mut input = InputBox::new();
        assert_eq!(input.line_count(), 1); // Empty is 1 line

        input.insert_char('h');
        assert_eq!(input.line_count(), 1);

        // "h\n" - lines() returns 1 (trailing newline doesn't create a new line)
        input.insert_char('\n');
        assert_eq!(input.line_count(), 1);

        // "h\nw" - lines() returns 2
        input.insert_char('w');
        assert_eq!(input.line_count(), 2);

        // "h\nw\n" - lines() returns 2 (trailing newline doesn't create a new line)
        input.insert_char('\n');
        assert_eq!(input.line_count(), 2);

        // "h\nw\nx" - lines() returns 3
        input.insert_char('x');
        assert_eq!(input.line_count(), 3);
    }

    #[test]
    fn test_backspace_across_newline() {
        let mut input = InputBox::new();
        // "hello\n|world"
        for c in "hello\nworld".chars() {
            input.insert_char(c);
        }
        // Move cursor to after newline (position 6)
        input.move_cursor_home();
        for _ in 0..6 {
            input.move_cursor_right();
        }

        // Backspace should delete the newline
        input.backspace();
        assert_eq!(input.content, "helloworld");
        assert_eq!(input.cursor_position, 5);
    }

    #[test]
    fn test_delete_char_at_newline() {
        let mut input = InputBox::new();
        // "hello|\nworld"
        for c in "hello\nworld".chars() {
            input.insert_char(c);
        }
        // Move cursor to before newline (position 5)
        input.move_cursor_home();
        for _ in 0..5 {
            input.move_cursor_right();
        }

        // Delete should remove the newline
        input.delete_char();
        assert_eq!(input.content, "helloworld");
        assert_eq!(input.cursor_position, 5);
    }

    #[test]
    fn test_cursor_navigation_across_newlines() {
        let mut input = InputBox::new();
        // "line1\nline2\nline3"
        for c in "line1\nline2\nline3".chars() {
            input.insert_char(c);
        }
        assert_eq!(input.cursor_position, 17);

        // Move to start
        input.move_cursor_home();
        assert_eq!(input.cursor_position, 0);

        // Move across newlines
        for _ in 0..6 {
            input.move_cursor_right();
        }
        assert_eq!(input.cursor_position, 6); // At 'l' in "line2"

        // Content at position 5 should be newline
        let chars: Vec<char> = input.content.chars().collect();
        assert_eq!(chars[5], '\n');
    }

    // ==========================================================================
    // Phase 5: Additional edge case tests for macOS text editing shortcuts
    // ==========================================================================

    // Word navigation: mid-word scenarios
    #[test]
    fn test_move_cursor_word_left_mid_word() {
        let mut input = InputBox::new();
        // "hello wor|ld"
        for c in "hello world".chars() {
            input.insert_char(c);
        }
        // Move cursor to mid-word position (after "wor")
        input.move_cursor_left();
        input.move_cursor_left();
        assert_eq!(input.cursor_position, 9); // "hello wor|ld"

        // Should move to start of "world"
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 6); // "hello |world"
    }

    #[test]
    fn test_move_cursor_word_right_mid_word() {
        let mut input = InputBox::new();
        // "hel|lo world"
        for c in "hello world".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();
        input.move_cursor_right();
        input.move_cursor_right();
        input.move_cursor_right();
        assert_eq!(input.cursor_position, 3); // "hel|lo world"

        // Should move to end of "hello"
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 5); // "hello| world"
    }

    #[test]
    fn test_move_cursor_word_right_punctuation() {
        let mut input = InputBox::new();
        // "|hello, world"
        for c in "hello, world".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();

        // Should move to end of "hello"
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 5); // "hello|, world"

        // Next word move should skip punctuation and space, then move through "world"
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 12); // "hello, world|"
    }

    #[test]
    fn test_move_cursor_word_left_empty_content() {
        let mut input = InputBox::new();
        // Empty content
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_move_cursor_word_right_empty_content() {
        let mut input = InputBox::new();
        // Empty content
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_move_cursor_word_consecutive_punctuation() {
        let mut input = InputBox::new();
        // "hello... world|"
        for c in "hello... world".chars() {
            input.insert_char(c);
        }

        // Move word left should skip to start of "world"
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 9); // "hello... |world"

        // Another word left should skip punctuation and move to start of "hello"
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 0); // "|hello... world"
    }

    #[test]
    fn test_move_cursor_word_with_underscore() {
        let mut input = InputBox::new();
        // "hello_world test|"
        for c in "hello_world test".chars() {
            input.insert_char(c);
        }

        // Move word left should move to start of "test"
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 12); // "hello_world |test"

        // Another word left should treat "hello_world" as one word
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 0); // "|hello_world test"
    }

    #[test]
    fn test_move_cursor_word_right_with_underscore() {
        let mut input = InputBox::new();
        // "|hello_world test"
        for c in "hello_world test".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();

        // Should treat "hello_world" as one word
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 11); // "hello_world| test"
    }

    #[test]
    fn test_move_cursor_word_left_only_spaces() {
        let mut input = InputBox::new();
        // "   |"
        for c in "   ".chars() {
            input.insert_char(c);
        }

        // Move word left should move to start
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_move_cursor_word_right_only_spaces() {
        let mut input = InputBox::new();
        // "|   "
        for c in "   ".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();

        // Move word right should move to end (skipping spaces)
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 3);
    }

    // Word deletion: additional edge cases
    #[test]
    fn test_delete_word_backward_with_punctuation() {
        let mut input = InputBox::new();
        // "hello, world|"
        for c in "hello, world".chars() {
            input.insert_char(c);
        }

        // Delete "world"
        input.delete_word_backward();
        assert_eq!(input.content, "hello, ");
        assert_eq!(input.cursor_position, 7);

        // Delete skips non-word chars (space, comma) then deletes word "hello"
        input.delete_word_backward();
        assert_eq!(input.content, "");
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_delete_word_backward_empty_content() {
        let mut input = InputBox::new();
        // Empty content
        input.delete_word_backward();
        assert_eq!(input.content, "");
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_delete_word_backward_with_underscore() {
        let mut input = InputBox::new();
        // "hello_world|"
        for c in "hello_world".chars() {
            input.insert_char(c);
        }

        // Should delete entire "hello_world" as one word
        input.delete_word_backward();
        assert_eq!(input.content, "");
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_delete_word_backward_consecutive_punctuation() {
        let mut input = InputBox::new();
        // "hello...|"
        for c in "hello...".chars() {
            input.insert_char(c);
        }

        // delete_word_backward first skips non-word chars (the "...") then deletes word chars ("hello")
        input.delete_word_backward();
        assert_eq!(input.content, "");
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_delete_word_backward_only_spaces() {
        let mut input = InputBox::new();
        // "hello   |"
        for c in "hello   ".chars() {
            input.insert_char(c);
        }

        // Should delete spaces and then "hello"
        input.delete_word_backward();
        assert_eq!(input.content, "");
        assert_eq!(input.cursor_position, 0);
    }

    // Line deletion: additional edge cases
    #[test]
    fn test_delete_to_line_start_empty_content() {
        let mut input = InputBox::new();
        // Empty content
        input.delete_to_line_start();
        assert_eq!(input.content, "");
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_delete_to_line_start_entire_first_line() {
        let mut input = InputBox::new();
        // "hello|"
        for c in "hello".chars() {
            input.insert_char(c);
        }

        // Should delete entire line
        input.delete_to_line_start();
        assert_eq!(input.content, "");
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_delete_to_line_start_multiline_entire_second_line() {
        let mut input = InputBox::new();
        // "line1\nline2|"
        for c in "line1\nline2".chars() {
            input.insert_char(c);
        }

        // Should delete entire second line content (but not the newline)
        input.delete_to_line_start();
        assert_eq!(input.content, "line1\n");
        assert_eq!(input.cursor_position, 6);
    }

    #[test]
    fn test_delete_to_line_start_multiple_newlines() {
        let mut input = InputBox::new();
        // "line1\n\nline3|"
        for c in "line1\n\nline3".chars() {
            input.insert_char(c);
        }

        // Should delete "line3" but not the empty line
        input.delete_to_line_start();
        assert_eq!(input.content, "line1\n\n");
        assert_eq!(input.cursor_position, 7);
    }

    #[test]
    fn test_delete_to_line_start_cursor_at_newline() {
        let mut input = InputBox::new();
        // "hello\n|world"
        for c in "hello\nworld".chars() {
            input.insert_char(c);
        }
        // Position cursor right after newline (position 6)
        input.move_cursor_home();
        for _ in 0..6 {
            input.move_cursor_right();
        }

        // Cursor is at line start (right after newline), nothing to delete
        input.delete_to_line_start();
        assert_eq!(input.content, "hello\nworld");
        assert_eq!(input.cursor_position, 6);
    }

    // Unicode handling tests
    #[test]
    fn test_word_navigation_with_unicode() {
        let mut input = InputBox::new();
        // "hello \u{1F600} world|" - 14 chars: h(0)e(1)l(2)l(3)o(4) (5)\u{1F600}(6) (7)w(8)o(9)r(10)l(11)d(12)
        for c in "hello \u{1F600} world".chars() {
            input.insert_char(c);
        }
        // Verify character count
        assert_eq!(input.content.chars().count(), 13);

        // Move word left should handle emoji as non-word char
        // Should move to start of "world" which is position 8
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 8); // "hello \u{1F600} |world"
    }

    #[test]
    fn test_delete_word_backward_with_unicode() {
        let mut input = InputBox::new();
        // "hello world|"
        for c in "hello \u{1F600} world".chars() {
            input.insert_char(c);
        }

        // Delete "world"
        input.delete_word_backward();
        assert_eq!(input.content, "hello \u{1F600} ");
    }

    #[test]
    fn test_delete_to_line_start_with_unicode() {
        let mut input = InputBox::new();
        // "hello\nworld|"
        for c in "\u{1F600}line1\n\u{1F601}line2".chars() {
            input.insert_char(c);
        }

        // Delete to line start
        input.delete_to_line_start();
        assert_eq!(input.content, "\u{1F600}line1\n");
    }

    // Boundary tests for word boundaries
    #[test]
    fn test_move_cursor_word_left_single_word() {
        let mut input = InputBox::new();
        // "hello|"
        for c in "hello".chars() {
            input.insert_char(c);
        }

        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_move_cursor_word_right_single_word() {
        let mut input = InputBox::new();
        // "|hello"
        for c in "hello".chars() {
            input.insert_char(c);
        }
        input.move_cursor_home();

        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 5);
    }

    #[test]
    fn test_move_cursor_word_left_at_word_boundary() {
        let mut input = InputBox::new();
        // "hello |world"
        for c in "hello world".chars() {
            input.insert_char(c);
        }
        // Position cursor at word boundary (position 6, start of "world")
        input.move_cursor_home();
        for _ in 0..6 {
            input.move_cursor_right();
        }

        // Should move to start of "hello"
        input.move_cursor_word_left();
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn test_move_cursor_word_right_at_word_boundary() {
        let mut input = InputBox::new();
        // "hello| world"
        for c in "hello world".chars() {
            input.insert_char(c);
        }
        // Position cursor at word boundary (position 5, end of "hello")
        input.move_cursor_home();
        for _ in 0..5 {
            input.move_cursor_right();
        }

        // Should skip space and move to end of "world"
        input.move_cursor_word_right();
        assert_eq!(input.cursor_position, 11);
    }

    #[test]
    fn test_delete_word_backward_at_word_start() {
        let mut input = InputBox::new();
        // "hello |world"
        for c in "hello world".chars() {
            input.insert_char(c);
        }
        // Position cursor at word boundary (position 6, start of "world")
        input.move_cursor_home();
        for _ in 0..6 {
            input.move_cursor_right();
        }

        // Should delete "hello "
        input.delete_word_backward();
        assert_eq!(input.content, "world");
        assert_eq!(input.cursor_position, 0);
    }
}
