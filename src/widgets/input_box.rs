use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    symbols::border,
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

    /// Clear all content and reset cursor
    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_position = 0;
        self.scroll_offset = 0;
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
    /// Whether to show dashed border (for streaming state)
    dashed: bool,
}

impl<'a> InputBoxWidget<'a> {
    pub fn new(input_box: &'a InputBox, title: &'a str, focused: bool) -> Self {
        Self {
            input_box,
            title,
            focused,
            dashed: false,
        }
    }

    /// Create an input box widget with dashed border (for streaming state)
    pub fn dashed(input_box: &'a InputBox, title: &'a str, focused: bool) -> Self {
        Self {
            input_box,
            title,
            focused,
            dashed: true,
        }
    }
}

impl Widget for InputBoxWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let show_cursor = self.focused;
        if self.dashed {
            self.render_with_dashed_border(area, buf);
        } else {
            self.render_normal(area, buf, show_cursor);
        }
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

    /// Render with a custom dashed border for streaming state
    fn render_with_dashed_border(&self, area: Rect, buf: &mut Buffer) {
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

        // Draw custom dashed border
        let border_color = if self.focused { Color::Gray } else { Color::DarkGray };
        let border_style = Style::default().fg(border_color);

        // Use custom border set with dashed horizontal lines
        let dashed_border = border::Set {
            top_left: "┌",
            top_right: "┐",
            bottom_left: "└",
            bottom_right: "┘",
            vertical_left: "│",
            vertical_right: "│",
            horizontal_top: "┄",     // Dashed line
            horizontal_bottom: "┄",  // Dashed line
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(dashed_border)
            .border_style(border_style)
            .title(self.title);

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

        // DO NOT render cursor when dashed (streaming state)
        // Cursor is disabled during streaming
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
}
