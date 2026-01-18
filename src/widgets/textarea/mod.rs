//! TextArea wrapper module that adapts tui-textarea's API to match existing InputBox patterns.
//!
//! This module provides a compatibility layer that allows gradual migration from the custom
//! InputBox widget to tui-textarea without breaking existing code. The wrapper exposes methods
//! with the same names as InputBox but internally delegates to tui-textarea.

mod cursor;
mod editing;
mod paste;
mod wrapping;

use paste::PasteToken;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Widget},
};
use tui_textarea::{CursorMove, TextArea};
use unicode_width::UnicodeWidthStr;

/// A wrapper around tui-textarea that provides an API compatible with InputBox.
///
/// This allows existing code using InputBox to migrate to tui-textarea with minimal changes.
/// The wrapper maintains the same method names as InputBox while leveraging tui-textarea's
/// more robust text editing capabilities, including proper multi-line support.
#[derive(Debug, Clone)]
pub struct TextAreaInput<'a> {
    /// The underlying tui-textarea widget
    pub(super) textarea: TextArea<'a>,
    /// Tracked paste tokens for atomic deletion
    pub(super) paste_tokens: Vec<PasteToken>,
    /// Counter for generating unique paste token IDs
    pub(super) paste_counter: u32,
    /// Width for hard wrap (auto-newline). When set, lines are automatically
    /// wrapped by inserting newlines when they exceed this width.
    pub(super) wrap_width: Option<u16>,
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
        textarea.set_line_wrap(true); // Enable soft line wrapping for visual display
                                      // Line numbers are OFF by default in tui-textarea (no need to explicitly remove)

        Self {
            textarea,
            paste_tokens: Vec::new(),
            paste_counter: 0,
            wrap_width: None,
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
        textarea.set_line_wrap(true); // Enable soft line wrapping for visual display
                                      // Line numbers are OFF by default in tui-textarea (no need to explicitly remove)

        // Move cursor to end
        textarea.move_cursor(CursorMove::Bottom);
        textarea.move_cursor(CursorMove::End);
        Self {
            textarea,
            paste_tokens: Vec::new(),
            paste_counter: 0,
            wrap_width: None,
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
    // Content methods
    // =========================================================================

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

    /// Get the number of visual lines considering soft wrapping.
    /// This calculates how many lines the content will occupy when rendered
    /// at the given width (accounting for borders).
    pub fn visual_line_count(&self, available_width: u16) -> usize {
        // Callers already account for borders, use the width directly
        let content_width = available_width as usize;
        if content_width == 0 {
            return self.line_count();
        }

        let mut visual_lines = 0;
        for line in self.textarea.lines() {
            let line_width = line.width();
            if line_width == 0 {
                // Empty line still takes 1 visual line
                visual_lines += 1;
            } else {
                // Calculate how many visual lines this logical line needs
                visual_lines += line_width.div_ceil(content_width);
            }
        }
        visual_lines.max(1)
    }

    /// Set the content of the textarea
    pub fn set_content(&mut self, text: &str) {
        // Clear existing content
        self.textarea.select_all();
        self.textarea.delete_char();
        // Insert new content
        for c in text.chars() {
            if c == '\n' {
                self.textarea.insert_newline();
            } else {
                self.textarea.insert_char(c);
            }
        }
    }

    /// Get the lines as a slice
    pub fn lines(&self) -> &[String] {
        self.textarea.lines()
    }

    /// Get styled content lines for unified scroll rendering.
    pub fn to_content_lines(&self) -> Vec<Line<'static>> {
        self.textarea.to_content_lines()
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
    pub fn render_with_title(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        title: &'a str,
        focused: bool,
    ) {
        self.configure_for_render(title, focused);
        // Use reference rendering (tui-textarea 0.7+ supports rendering &TextArea directly)
        (&self.textarea).render(area, buf);
    }

    /// Render the textarea without any border (raw content only).
    ///
    /// Used when the caller wants to handle border rendering separately,
    /// such as when rendering a chip alongside the input.
    pub fn render_without_border(&mut self, area: Rect, buf: &mut Buffer, focused: bool) {
        // Configure cursor visibility based on focus
        if focused {
            self.textarea
                .set_cursor_style(Style::default().fg(Color::Black).bg(Color::White));
        } else {
            self.textarea.set_cursor_style(Style::default());
        }

        // Clear any existing block (no border)
        self.textarea.set_block(Block::default());

        // Render the textarea
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
    fn test_clear() {
        let mut input = TextAreaInput::new();
        input.insert_char('H');
        input.insert_char('i');
        input.clear();
        assert!(input.is_empty());
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

    #[test]
    fn test_set_content_single_line() {
        let mut input = TextAreaInput::new();
        input.set_content("hello world");
        assert_eq!(input.content(), "hello world");
        assert_eq!(input.line_count(), 1);
    }

    #[test]
    fn test_set_content_multiple_lines() {
        let mut input = TextAreaInput::new();
        input.set_content("line1\nline2\nline3");
        assert_eq!(input.content(), "line1\nline2\nline3");
        assert_eq!(input.line_count(), 3);

        let lines = input.lines();
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "line2");
        assert_eq!(lines[2], "line3");
    }

    #[test]
    fn test_set_content_replaces_existing() {
        let mut input = TextAreaInput::new();
        // Add initial content
        for c in "initial content".chars() {
            input.insert_char(c);
        }
        assert_eq!(input.content(), "initial content");

        // Replace with new content
        input.set_content("new content");
        assert_eq!(input.content(), "new content");
        assert_eq!(input.line_count(), 1);
    }

    #[test]
    fn test_set_content_empty_string() {
        let mut input = TextAreaInput::new();
        for c in "some content".chars() {
            input.insert_char(c);
        }

        input.set_content("");
        assert!(input.is_empty());
    }

    #[test]
    fn test_to_content_lines() {
        let mut input = TextAreaInput::new();
        for c in "hello".chars() {
            input.insert_char(c);
        }
        input.insert_newline();
        for c in "world".chars() {
            input.insert_char(c);
        }

        let lines = input.to_content_lines();
        assert_eq!(lines.len(), 2);

        // Lines should be owned (static lifetime)
        // Just verify we get the expected number of lines
        // The actual styling/rendering is handled by tui-textarea
    }

    #[test]
    fn test_to_content_lines_empty() {
        let input = TextAreaInput::new();
        let lines = input.to_content_lines();
        assert_eq!(lines.len(), 1); // Empty textarea has one line
    }

    #[test]
    fn test_to_content_lines_single_line() {
        let mut input = TextAreaInput::new();
        for c in "single line".chars() {
            input.insert_char(c);
        }

        let lines = input.to_content_lines();
        assert_eq!(lines.len(), 1);
    }
}
