//! Question card overlay component for dashboard rendering
//!
//! This module renders question card content - both Question mode (with option buttons)
//! and FreeForm mode (with text input). Used by the overlay module to render
//! expanded thread cards that need user input.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Frame,
};

use crate::ui::interaction::{ClickAction, HitAreaRegistry};

// ============================================================================
// Public API
// ============================================================================

/// Renders question card content - handles both Question and FreeForm modes
///
/// # Arguments
///
/// * `frame` - The ratatui frame to render into
/// * `area` - Inner card area (inside border)
/// * `thread_id` - The thread ID for registering click actions
/// * `title` - Thread title
/// * `repo` - Repository name
/// * `question` - The question text to display
/// * `options` - Available options for the question
/// * `input` - None = Question mode, Some((text, cursor_pos)) = FreeForm mode
/// * `registry` - Hit area registry for mouse interaction
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    thread_id: &str,
    title: &str,
    repo: &str,
    question: &str,
    options: &[String],
    input: Option<(&str, usize)>,
    registry: &mut HitAreaRegistry,
) {
    match input {
        None => render_question_mode(
            frame, area, thread_id, title, repo, question, options, registry,
        ),
        Some((text, cursor)) => render_free_form_mode(
            frame, area, thread_id, title, repo, question, text, cursor, registry,
        ),
    }
}

// ============================================================================
// Question Mode Rendering
// ============================================================================

/// Render the question mode with option buttons
///
/// Layout:
///   Row 0: "{title} \u{00b7} {repo}" (header line)
///   Row 1: blank
///   Row 2-4: question text (wrapped, max 3 lines)
///   Row 5: blank
///   Row 6: [option0] [option1] [option2] [Other...]
#[allow(clippy::too_many_arguments)]
fn render_question_mode(
    frame: &mut Frame,
    area: Rect,
    thread_id: &str,
    title: &str,
    repo: &str,
    question: &str,
    options: &[String],
    registry: &mut HitAreaRegistry,
) {
    let mut y = area.y;

    // Header: title \u{00b7} repo
    let header = format!("{} \u{00b7} {}", title, repo);
    let header_line = Line::styled(header, Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(header_line, Rect::new(area.x, y, area.width, 1));
    y += 2; // Skip blank line

    // Question text (wrap to 3 lines max)
    let question_lines = wrap_text(question, area.width as usize, 3);
    for line in &question_lines {
        frame.render_widget(Line::raw(line.clone()), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }
    // Ensure we advance past the question area even if fewer than 3 lines
    y = (area.y + 2 + 3).min(y.max(area.y + 2 + question_lines.len() as u16));
    y += 1; // Skip blank line

    // Option buttons
    let mut btn_x = area.x;
    let btn_y = y;

    for (i, opt) in options.iter().enumerate() {
        let btn_text = format!("[{}]", opt);
        let btn_width = btn_text.len() as u16;

        // Check if there's room for this button
        if btn_x + btn_width > area.x + area.width {
            break;
        }

        let btn_area = Rect::new(btn_x, btn_y, btn_width, 1);

        // Style: white text
        let style = Style::default().fg(Color::White);
        frame.render_widget(Span::styled(&btn_text, style), btn_area);

        registry.register(
            btn_area,
            ClickAction::SelectOption {
                thread_id: thread_id.to_string(),
                index: i,
            },
            Some(Style::default().bg(Color::DarkGray)),
        );
        btn_x += btn_width + 2; // +2 for spacing
    }

    // [Other...] button
    let other_btn = "[Other...]";
    let other_width = other_btn.len() as u16;

    // Check if there's room for the Other button
    if btn_x + other_width <= area.x + area.width {
        let other_area = Rect::new(btn_x, btn_y, other_width, 1);
        frame.render_widget(Span::raw(other_btn), other_area);
        registry.register(
            other_area,
            ClickAction::ShowFreeFormInput(thread_id.to_string()),
            Some(Style::default().bg(Color::DarkGray)),
        );
    }
}

// ============================================================================
// Free Form Mode Rendering
// ============================================================================

/// Render the free-form input mode
///
/// Layout:
///   Row 0: "{title} \u{00b7} {repo}"
///   Row 1: blank
///   Row 2: question truncated with "..."
///   Row 3: blank
///   Row 4-5: input box with borders
///   Row 6: blank
///   Row 7: [<- back]  [send]
#[allow(clippy::too_many_arguments)]
fn render_free_form_mode(
    frame: &mut Frame,
    area: Rect,
    thread_id: &str,
    title: &str,
    repo: &str,
    question: &str,
    input_text: &str,
    cursor_pos: usize,
    registry: &mut HitAreaRegistry,
) {
    let mut y = area.y;

    // Header
    let header = format!("{} \u{00b7} {}", title, repo);
    let header_line = Line::styled(header, Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(header_line, Rect::new(area.x, y, area.width, 1));
    y += 2; // Skip blank line

    // Truncated question
    let truncated_q = truncate_with_ellipsis(question, area.width.saturating_sub(4) as usize);
    frame.render_widget(
        Line::raw(truncated_q),
        Rect::new(area.x, y, area.width, 1),
    );
    y += 2; // Skip blank line

    // Input box
    let input_width = area.width.saturating_sub(4);
    if input_width > 2 {
        // Top border
        let top_border = format!(
            "\u{256d}{}\u{256e}",
            "\u{2500}".repeat(input_width as usize)
        );
        frame.render_widget(Line::raw(&top_border), Rect::new(area.x, y, area.width, 1));
        y += 1;

        // Input content with cursor
        let inner_width = input_width.saturating_sub(2) as usize;
        let (visible_text, display_cursor) = get_visible_input(input_text, cursor_pos, inner_width);

        // Build the input line with cursor
        let mut spans = Vec::new();
        spans.push(Span::raw("\u{2502} "));

        if display_cursor < visible_text.len() {
            // Cursor is in the middle of visible text
            spans.push(Span::raw(&visible_text[..display_cursor]));
            spans.push(Span::styled(
                &visible_text[display_cursor..display_cursor + 1],
                Style::default().bg(Color::White).fg(Color::Black),
            ));
            spans.push(Span::raw(&visible_text[display_cursor + 1..]));
            // Pad the rest
            let remaining = inner_width.saturating_sub(visible_text.len());
            if remaining > 0 {
                spans.push(Span::raw(" ".repeat(remaining)));
            }
        } else {
            // Cursor is at the end
            spans.push(Span::raw(&visible_text));
            spans.push(Span::styled(
                " ",
                Style::default().bg(Color::White).fg(Color::Black),
            ));
            // Pad the rest
            let remaining = inner_width.saturating_sub(visible_text.len() + 1);
            if remaining > 0 {
                spans.push(Span::raw(" ".repeat(remaining)));
            }
        }
        spans.push(Span::raw(" \u{2502}"));

        frame.render_widget(
            Line::from(spans),
            Rect::new(area.x, y, area.width, 1),
        );
        y += 1;

        // Bottom border
        let bot_border = format!(
            "\u{2570}{}\u{256f}",
            "\u{2500}".repeat(input_width as usize)
        );
        frame.render_widget(Line::raw(&bot_border), Rect::new(area.x, y, area.width, 1));
        y += 2; // Skip blank line
    } else {
        y += 4; // Skip input box area if too narrow
    }

    // Buttons
    let back_btn = "[\u{2190} back]";
    let send_btn = "[send]";

    let back_width = back_btn.chars().count() as u16;
    let send_width = send_btn.len() as u16;

    // Back button (left aligned)
    let back_area = Rect::new(area.x, y, back_width, 1);
    frame.render_widget(Span::raw(back_btn), back_area);
    registry.register(
        back_area,
        ClickAction::BackToOptions(thread_id.to_string()),
        Some(Style::default().bg(Color::DarkGray)),
    );

    // Send button (right aligned)
    let send_x = area.x + area.width - send_width;
    let send_area = Rect::new(send_x, y, send_width, 1);
    frame.render_widget(Span::raw(send_btn), send_area);
    registry.register(
        send_area,
        ClickAction::SubmitFreeForm(thread_id.to_string()),
        Some(Style::default().bg(Color::DarkGray)),
    );
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the visible portion of input text and the cursor position within it
///
/// When the input text is longer than the available width, we need to show
/// a "window" into the text centered around the cursor position.
fn get_visible_input(input: &str, cursor_pos: usize, width: usize) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let char_count = chars.len();

    if char_count <= width {
        // Everything fits
        return (input.to_string(), cursor_pos.min(char_count));
    }

    // Calculate the visible window
    // Keep cursor roughly in the middle of the visible area
    let half_width = width / 2;
    let start = if cursor_pos <= half_width {
        0
    } else if cursor_pos >= char_count.saturating_sub(half_width) {
        char_count.saturating_sub(width)
    } else {
        cursor_pos.saturating_sub(half_width)
    };

    let end = (start + width).min(char_count);
    let visible: String = chars[start..end].iter().collect();
    let display_cursor = cursor_pos.saturating_sub(start);
    let visible_len = visible.chars().count();

    (visible, display_cursor.min(visible_len))
}

/// Wrap text to fit within a given width, up to max_lines
///
/// If the text exceeds max_lines, the last line is truncated with "..."
///
/// # Arguments
/// * `text` - The text to wrap
/// * `width` - Maximum width per line
/// * `max_lines` - Maximum number of lines to return
///
/// # Returns
/// A vector of wrapped lines
pub fn wrap_text(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    if width == 0 || max_lines == 0 {
        return vec![];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        let word_len = word.chars().count();

        // If word alone is longer than width, we need to break it
        if word_len > width {
            // First, push current line if not empty
            if !current_line.is_empty() {
                if lines.len() >= max_lines - 1 {
                    current_line.push_str("...");
                    lines.push(current_line);
                    return lines;
                }
                lines.push(current_line);
                current_line = String::new();
            }

            // Break the word into chunks
            let chars: Vec<char> = word.chars().collect();
            for chunk in chars.chunks(width) {
                let chunk_str: String = chunk.iter().collect();
                if lines.len() >= max_lines - 1 {
                    let truncated = truncate_with_ellipsis(&chunk_str, width);
                    lines.push(truncated);
                    return lines;
                }
                lines.push(chunk_str);
            }
            continue;
        }

        // Check if adding this word exceeds width
        let line_len = current_line.chars().count();
        let needed = if current_line.is_empty() {
            word_len
        } else {
            line_len + 1 + word_len
        };

        if needed > width {
            // Need to start a new line
            if lines.len() >= max_lines - 1 && !current_line.is_empty() {
                current_line.push_str("...");
                lines.push(current_line);
                return lines;
            }
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
            }
        }

        if !current_line.is_empty() {
            current_line.push(' ');
        }
        current_line.push_str(word);
    }

    // Don't forget the last line
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

/// Truncate a string with ellipsis if it exceeds max_len
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_len` - Maximum length including the ellipsis
///
/// # Returns
/// The original string if it fits, or truncated with "..." if it doesn't
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        ".".repeat(max_len)
    } else {
        let chars: Vec<char> = s.chars().take(max_len - 3).collect();
        format!("{}...", chars.into_iter().collect::<String>())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------- wrap_text Tests --------------------

    #[test]
    fn test_wrap_text_short_text() {
        let result = wrap_text("Hello world", 20, 3);
        assert_eq!(result, vec!["Hello world"]);
    }

    #[test]
    fn test_wrap_text_exact_fit() {
        let result = wrap_text("Hello", 5, 3);
        assert_eq!(result, vec!["Hello"]);
    }

    #[test]
    fn test_wrap_text_wraps_at_word_boundary() {
        let result = wrap_text("Hello world", 7, 3);
        assert_eq!(result, vec!["Hello", "world"]);
    }

    #[test]
    fn test_wrap_text_multiple_lines() {
        let result = wrap_text("The quick brown fox jumps over the lazy dog", 10, 5);
        assert_eq!(
            result,
            vec!["The quick", "brown fox", "jumps over", "the lazy", "dog"]
        );
    }

    #[test]
    fn test_wrap_text_truncates_with_ellipsis() {
        // When max_lines is reached, the function adds "..." to indicate truncation
        // But only if there would be more content - wrapping "brown fox" to line 2
        // adds ellipsis because there are more words after
        let result = wrap_text("The quick brown fox jumps over the lazy dog", 10, 2);
        assert_eq!(result, vec!["The quick", "brown fox..."]);
    }

    #[test]
    fn test_wrap_text_single_line_limit() {
        let result = wrap_text("Hello world from Rust", 10, 1);
        assert_eq!(result, vec!["Hello..."]);
    }

    #[test]
    fn test_wrap_text_empty_input() {
        let result = wrap_text("", 20, 3);
        assert!(result.is_empty());
    }

    #[test]
    fn test_wrap_text_zero_width() {
        let result = wrap_text("Hello", 0, 3);
        assert!(result.is_empty());
    }

    #[test]
    fn test_wrap_text_zero_max_lines() {
        let result = wrap_text("Hello", 20, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_wrap_text_long_word() {
        let result = wrap_text("supercalifragilisticexpialidocious", 10, 5);
        // Word gets broken into chunks
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "supercalif");
        assert_eq!(result[1], "ragilistic");
        assert_eq!(result[2], "expialidoc");
        assert_eq!(result[3], "ious");
    }

    #[test]
    fn test_wrap_text_long_word_truncated() {
        // Long word gets broken into 10-char chunks
        // When we hit max_lines limit, the second chunk is returned as-is
        // since truncate_with_ellipsis is only called when the chunk itself
        // needs to be shortened (which it doesn't here - it's exactly 10 chars)
        let result = wrap_text("supercalifragilisticexpialidocious", 10, 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "supercalif");
        assert_eq!(result[1], "ragilistic"); // 10 chars, no need for ellipsis
    }

    #[test]
    fn test_wrap_text_preserves_whitespace_handling() {
        // Multiple spaces between words should be treated as single separator
        let result = wrap_text("Hello    world", 20, 3);
        assert_eq!(result, vec!["Hello world"]);
    }

    // -------------------- truncate_with_ellipsis Tests --------------------

    #[test]
    fn test_truncate_no_truncation() {
        let result = truncate_with_ellipsis("Hello", 10);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_truncate_exact_fit() {
        let result = truncate_with_ellipsis("Hello", 5);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_truncate_with_ellipsis_basic() {
        let result = truncate_with_ellipsis("Hello, World!", 8);
        assert_eq!(result, "Hello...");
    }

    #[test]
    fn test_truncate_short_max_len() {
        let result = truncate_with_ellipsis("Hello", 3);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_very_short_max_len() {
        let result = truncate_with_ellipsis("Hello", 2);
        assert_eq!(result, "..");
    }

    #[test]
    fn test_truncate_zero_max_len() {
        let result = truncate_with_ellipsis("Hello", 0);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_empty_string() {
        let result = truncate_with_ellipsis("", 10);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_unicode() {
        // "Hello \u{4e16}\u{754c}!" has 10 characters
        let result = truncate_with_ellipsis("Hello \u{4e16}\u{754c}!", 10);
        assert_eq!(result, "Hello \u{4e16}\u{754c}!");
    }

    #[test]
    fn test_truncate_unicode_truncated() {
        // "\u{65e5}\u{672c}\u{8a9e}\u{30c6}\u{30b9}\u{30c8}" has 6 characters
        let result = truncate_with_ellipsis("\u{65e5}\u{672c}\u{8a9e}\u{30c6}\u{30b9}\u{30c8}", 5);
        assert_eq!(result, "\u{65e5}\u{672c}...");
    }

    #[test]
    fn test_truncate_one_char_with_ellipsis() {
        let result = truncate_with_ellipsis("Hello", 4);
        assert_eq!(result, "H...");
    }

    // -------------------- get_visible_input Tests --------------------

    #[test]
    fn test_get_visible_input_short_text() {
        let (visible, cursor) = get_visible_input("Hello", 3, 20);
        assert_eq!(visible, "Hello");
        assert_eq!(cursor, 3);
    }

    #[test]
    fn test_get_visible_input_cursor_at_start() {
        let (visible, cursor) = get_visible_input("Hello World!", 0, 5);
        assert_eq!(visible, "Hello");
        assert_eq!(cursor, 0);
    }

    #[test]
    fn test_get_visible_input_cursor_at_end() {
        let (visible, cursor) = get_visible_input("Hello World!", 12, 5);
        assert_eq!(visible, "orld!");
        assert_eq!(cursor, 5);
    }

    #[test]
    fn test_get_visible_input_cursor_in_middle() {
        let (visible, cursor) = get_visible_input("Hello World!", 6, 5);
        // Cursor at 'W', should center around it
        assert!(visible.len() == 5);
        assert!(cursor <= 5);
    }

    #[test]
    fn test_get_visible_input_empty() {
        let (visible, cursor) = get_visible_input("", 0, 10);
        assert_eq!(visible, "");
        assert_eq!(cursor, 0);
    }
}
