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
/// - Cyberpunk styling with cyan border and magenta cursor
#[derive(Debug, Clone, Default)]
pub struct InputBox {
    /// The text content of the input box
    content: String,
    /// Current cursor position (character index)
    cursor_position: usize,
    /// Scroll offset for horizontal scrolling
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

    /// Insert a character at the current cursor position
    pub fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor_position, c);
        self.cursor_position += 1;
    }

    /// Delete the character at the current cursor position (like Delete key)
    pub fn delete_char(&mut self) {
        if self.cursor_position < self.content.len() {
            self.content.remove(self.cursor_position);
        }
    }

    /// Delete the character before the cursor (like Backspace key)
    pub fn backspace(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.content.remove(self.cursor_position);
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
        if self.cursor_position < self.content.len() {
            self.cursor_position += 1;
        }
    }

    /// Move cursor to the beginning of the text
    pub fn move_cursor_home(&mut self) {
        self.cursor_position = 0;
    }

    /// Move cursor to the end of the text
    pub fn move_cursor_end(&mut self) {
        self.cursor_position = self.content.len();
    }

    /// Get the current text content
    pub fn get_content(&self) -> &str {
        &self.content
    }

    /// Get the current cursor position
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Set the text content and reset cursor to end
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.cursor_position = self.content.len();
        self.scroll_offset = 0;
    }

    /// Clear all content and reset cursor
    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_position = 0;
        self.scroll_offset = 0;
    }

    /// Check if the input box is empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Get the length of the content
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Update scroll offset to ensure cursor is visible within the given width
    fn update_scroll(&mut self, visible_width: usize) {
        if visible_width == 0 {
            return;
        }

        // If cursor is before the visible area, scroll left
        if self.cursor_position < self.scroll_offset {
            self.scroll_offset = self.cursor_position;
        }

        // If cursor is after the visible area, scroll right
        // Leave one character space for the cursor block
        if self.cursor_position >= self.scroll_offset + visible_width {
            self.scroll_offset = self.cursor_position - visible_width + 1;
        }
    }

    /// Render the input box with the given title
    pub fn render_with_title(&self, area: Rect, buf: &mut Buffer, title: &str, focused: bool) {
        // Calculate inner area (accounting for border)
        let inner_width = if area.width > 2 { area.width - 2 } else { 0 };

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

        // Draw border with cyberpunk cyan color
        let border_color = if focused { Color::Cyan } else { Color::DarkGray };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title);

        // Render the block
        block.render(area, buf);

        // Calculate inner area for text
        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: inner_width,
            height: if area.height > 2 { 1 } else { 0 },
        };

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
                    inner_area.y,
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

                // Magenta cursor block for cyberpunk look
                let cursor_style = Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta);

                buf.set_string(
                    inner_area.x + cursor_x,
                    inner_area.y,
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
        self.input_box.render_with_title(area, buf, self.title, self.focused);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_input_box() {
        let input = InputBox::new();
        assert!(input.is_empty());
        assert_eq!(input.cursor_position(), 0);
        assert_eq!(input.get_content(), "");
    }

    #[test]
    fn test_insert_char() {
        let mut input = InputBox::new();
        input.insert_char('H');
        input.insert_char('i');
        assert_eq!(input.get_content(), "Hi");
        assert_eq!(input.cursor_position(), 2);
    }

    #[test]
    fn test_backspace() {
        let mut input = InputBox::new();
        input.insert_char('H');
        input.insert_char('i');
        input.backspace();
        assert_eq!(input.get_content(), "H");
        assert_eq!(input.cursor_position(), 1);
    }

    #[test]
    fn test_delete_char() {
        let mut input = InputBox::new();
        input.insert_char('H');
        input.insert_char('i');
        input.move_cursor_left();
        input.delete_char();
        assert_eq!(input.get_content(), "H");
        assert_eq!(input.cursor_position(), 1);
    }

    #[test]
    fn test_cursor_movement() {
        let mut input = InputBox::new();
        input.set_content("Hello".to_string());
        assert_eq!(input.cursor_position(), 5);

        input.move_cursor_left();
        assert_eq!(input.cursor_position(), 4);

        input.move_cursor_home();
        assert_eq!(input.cursor_position(), 0);

        input.move_cursor_right();
        assert_eq!(input.cursor_position(), 1);

        input.move_cursor_end();
        assert_eq!(input.cursor_position(), 5);
    }

    #[test]
    fn test_cursor_bounds() {
        let mut input = InputBox::new();
        input.insert_char('X');

        // Cursor should not go below 0
        input.move_cursor_home();
        input.move_cursor_left();
        assert_eq!(input.cursor_position(), 0);

        // Cursor should not go beyond content length
        input.move_cursor_end();
        input.move_cursor_right();
        assert_eq!(input.cursor_position(), 1);
    }

    #[test]
    fn test_clear() {
        let mut input = InputBox::new();
        input.set_content("Hello World".to_string());
        input.clear();
        assert!(input.is_empty());
        assert_eq!(input.cursor_position(), 0);
    }

    #[test]
    fn test_insert_at_cursor() {
        let mut input = InputBox::new();
        input.set_content("Hllo".to_string());
        input.move_cursor_home();
        input.move_cursor_right(); // cursor at position 1
        input.insert_char('e');
        assert_eq!(input.get_content(), "Hello");
    }
}
