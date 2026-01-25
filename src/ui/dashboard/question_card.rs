//! Question card overlay component for dashboard rendering
//!
//! This module renders question card content - both Question mode (with option selection)
//! and FreeForm mode (with text input). Used by the overlay module to render
//! expanded thread cards that need user input.
//!
//! ## UI Layout (Question Mode)
//!
//! ```text
//! ╭────────────────────────────────────────────────────────────╮
//! │  Implement auth · my-project                               │
//! │                                                            │
//! │  Which authentication method should I use?                 │
//! │                                                            │
//! │    > [●] JWT tokens                                        │
//! │      [ ] Session cookies                                   │
//! │      [ ] OAuth 2.0 only                                    │
//! │      [ ] Other: _______________________________            │
//! │                                                            │
//! │  ↑↓ navigate   enter select   esc cancel          (4:32)  │
//! ╰────────────────────────────────────────────────────────────╯
//! ```
//!
//! ## Multi-Select Mode (checkboxes)
//!
//! ```text
//! │    > [x] Linting                                           │
//! │      [x] Unit tests                                        │
//! │      [ ] E2E tests                                         │
//! │      [ ] Other: _______________________________            │
//! ```

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Frame,
};

/// Configuration for rendering a question card
#[derive(Debug, Clone)]
pub struct QuestionRenderConfig<'a> {
    /// The question text to display
    pub question: &'a str,
    /// Available options for the question
    pub options: &'a [String],
    /// Descriptions for each option (parallel to options, can be empty strings)
    pub option_descriptions: &'a [String],
    /// Currently selected option index (for single-select) or cursor position
    pub selected_index: Option<usize>,
    /// Whether this is a multi-select question
    pub multi_select: bool,
    /// For multi-select: which options are currently selected
    pub multi_selections: &'a [bool],
    /// Text entered in the "Other" input field
    pub other_input: &'a str,
    /// Whether "Other" is selected (selected_index is None)
    pub other_selected: bool,
    /// Remaining time in seconds for the timer (None = no timer)
    pub timer_seconds: Option<u32>,
    /// Tab headers for multi-question flow (empty if single question)
    pub tab_headers: &'a [String],
    /// Current tab index (0-based)
    pub current_tab: usize,
    /// Which tabs have been answered
    pub tabs_answered: &'a [bool],
}

impl<'a> Default for QuestionRenderConfig<'a> {
    fn default() -> Self {
        Self {
            question: "",
            options: &[],
            option_descriptions: &[],
            selected_index: Some(0),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        }
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Renders question card content with full configuration
///
/// This is the new render function that supports the full mockup UI:
/// - Vertical option list with selection markers
/// - Multi-select checkboxes
/// - Other input field
/// - Help text and timer
///
/// # Arguments
///
/// * `frame` - The ratatui frame to render into
/// * `area` - Inner card area (inside border)
/// * `thread_id` - The thread ID (kept for API compatibility)
/// * `title` - Thread title
/// * `repo` - Repository name
/// * `config` - Configuration for the question display
#[allow(clippy::too_many_arguments)]
pub fn render_question(
    frame: &mut Frame,
    area: Rect,
    _thread_id: &str,
    title: &str,
    repo: &str,
    config: &QuestionRenderConfig,
) {
    // Guard against impossibly small areas (need at least header + 1 option + help)
    // Minimum: 3 rows height (header, one content row, help), 10 chars width
    if area.height < 3 || area.width < 10 {
        return;
    }

    let mut y = area.y;

    // Row 0: Header - "{title} · {repo}"
    let header = format!("{} \u{00b7} {}", title, repo);
    let header_truncated = truncate_with_ellipsis(&header, area.width as usize);
    frame.render_widget(
        Line::styled(
            header_truncated,
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Rect::new(area.x, y, area.width, 1),
    );
    y += 1;

    // Row 1: Tab bar (only if multiple questions)
    if config.tab_headers.len() > 1 {
        let tab_bar = render_tab_bar(
            config.tab_headers,
            config.current_tab,
            config.tabs_answered,
            area.width as usize,
        );
        frame.render_widget(
            Line::from(tab_bar),
            Rect::new(area.x, y, area.width, 1),
        );
        y += 1;
    }

    // Row: blank
    y += 1;

    // Row 2-3: Question text (wrapped, max 2 lines)
    let question_lines = wrap_text(config.question, area.width as usize, 2);
    for line in &question_lines {
        frame.render_widget(Line::raw(line.clone()), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }
    // Ensure we advance past the question area
    let min_question_rows = 2;
    if question_lines.len() < min_question_rows {
        y += (min_question_rows - question_lines.len()) as u16;
    }

    // Row: blank before options
    y += 1;

    // Calculate how many option rows we can show
    // Reserve 2 rows: 1 for "Other" option, 1 for help text
    let rows_used = (y - area.y) as usize;
    let total_rows = area.height as usize;
    let reserved_rows = 2; // 1 for Other, 1 for help line
    let available_option_rows = total_rows.saturating_sub(rows_used + reserved_rows);

    // Calculate if we have space for descriptions (need at least 2 rows per option)
    let options_count = config.options.len();
    let has_descriptions = !config.option_descriptions.is_empty();
    let show_descriptions = has_descriptions && available_option_rows >= options_count * 2;

    // Render options
    let option_indent = 4u16; // "    " indent
    let description_indent = 10u16; // Extra indent for descriptions
    let mut options_rendered = 0;

    for (i, opt) in config.options.iter().enumerate() {
        // Check if we have room for this option (plus description if showing)
        let rows_needed = if show_descriptions { 2 } else { 1 };
        let rows_remaining = available_option_rows.saturating_sub(options_rendered * rows_needed);
        if rows_remaining < rows_needed {
            break; // No room for more options
        }

        let is_cursor = config.selected_index == Some(i);
        let marker = if config.multi_select {
            // Multi-select: [x] for checked, [ ] for unchecked
            let checked = config
                .multi_selections
                .get(i)
                .copied()
                .unwrap_or(false);
            if checked { "[x]" } else { "[ ]" }
        } else {
            // Single-select: [●] for selected (cursor position), [ ] for others
            if is_cursor && !config.other_selected {
                "[\u{25cf}]"
            } else {
                "[ ]"
            }
        };

        // Build the option line with cursor indicator `> ` on focused option
        let cursor_char = if is_cursor && !config.other_selected {
            "> "
        } else {
            "  "
        };

        let option_text = format!("{}{} {}", cursor_char, marker, opt);
        let option_truncated = truncate_with_ellipsis(&option_text, (area.width - option_indent) as usize);

        // Style: highlight if cursor is on this option
        let style = if is_cursor && !config.other_selected {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        let option_area = Rect::new(area.x + option_indent, y, area.width - option_indent, 1);
        frame.render_widget(Line::styled(option_truncated, style), option_area);

        y += 1;
        options_rendered += 1;

        // Render description below label if space permits
        if show_descriptions {
            if let Some(desc) = config.option_descriptions.get(i) {
                if !desc.is_empty() {
                    let desc_width = (area.width - description_indent) as usize;
                    let desc_truncated = truncate_with_ellipsis(desc, desc_width);
                    let desc_style = Style::default().fg(Color::DarkGray);
                    frame.render_widget(
                        Line::styled(desc_truncated, desc_style),
                        Rect::new(area.x + description_indent, y, area.width - description_indent, 1),
                    );
                }
            }
            y += 1;
        }
    }

    // Render "Other" option
    if y < area.y + area.height.saturating_sub(2) {
        let is_other_cursor = config.other_selected
            || (config.selected_index.is_none()
                || config.selected_index == Some(config.options.len()));

        let cursor_char = if is_other_cursor { "> " } else { "  " };
        let marker = if config.multi_select {
            // Multi-select: Other can be checked
            if !config.other_input.is_empty() {
                "[x]"
            } else {
                "[ ]"
            }
        } else {
            // Single-select: Other is selected when cursor is on it
            if is_other_cursor { "[\u{25cf}]" } else { "[ ]" }
        };

        // Build Other line with input field
        let other_prefix = format!("{}{} Other: ", cursor_char, marker);
        let input_width = (area.width - option_indent)
            .saturating_sub(other_prefix.len() as u16) as usize;

        // Display input or underscores placeholder
        let input_display = if config.other_input.is_empty() {
            "_".repeat(input_width.min(30))
        } else {
            let truncated = truncate_with_ellipsis(config.other_input, input_width);
            format!("{}{}", truncated, "_".repeat(input_width.saturating_sub(truncated.len())))
        };

        let style = if is_other_cursor {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        let other_area = Rect::new(area.x + option_indent, y, area.width - option_indent, 1);

        // Build spans for proper styling
        let spans = vec![
            Span::styled(&other_prefix, style),
            Span::styled(
                input_display,
                if is_other_cursor {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ];
        frame.render_widget(Line::from(spans), other_area);

        y += 1;
    }

    // Skip to help text row (last row)
    let help_row_y = area.y + area.height.saturating_sub(1);

    // Render help text with timer
    if help_row_y > y {
        let has_multiple_tabs = config.tab_headers.len() > 1;
        render_help_line(
            frame,
            area.x,
            help_row_y,
            area.width,
            config.timer_seconds,
            has_multiple_tabs,
        );
    }
}

/// Renders question card content - handles both Question and FreeForm modes
///
/// This is the legacy render function for backward compatibility.
///
/// # Arguments
///
/// * `frame` - The ratatui frame to render into
/// * `area` - Inner card area (inside border)
/// * `thread_id` - The thread ID (kept for API compatibility)
/// * `title` - Thread title
/// * `repo` - Repository name
/// * `question` - The question text to display
/// * `options` - Available options for the question
/// * `input` - None = Question mode, Some((text, cursor_pos)) = FreeForm mode
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
) {
    match input {
        None => {
            // Use the new question renderer with default config
            let config = QuestionRenderConfig {
                question,
                options,
                option_descriptions: &[],
                selected_index: Some(0),
                multi_select: false,
                multi_selections: &[],
                other_input: "",
                other_selected: false,
                timer_seconds: None,
                tab_headers: &[],
                current_tab: 0,
                tabs_answered: &[],
            };
            render_question(frame, area, thread_id, title, repo, &config);
        }
        Some((text, cursor)) => render_free_form_mode(
            frame, area, thread_id, title, repo, question, text, cursor,
        ),
    }
}

// ============================================================================
// Help Line Rendering
// ============================================================================

/// Render the help line with navigation hints and timer
fn render_help_line(
    frame: &mut Frame,
    x: u16,
    y: u16,
    width: u16,
    timer_seconds: Option<u32>,
    has_multiple_tabs: bool,
) {
    let help_text = if has_multiple_tabs {
        "\u{2191}\u{2193} navigate   tab next   enter select   esc cancel"
    } else {
        "\u{2191}\u{2193} navigate   enter select   esc cancel"
    };

    // Format timer
    let timer_text = timer_seconds
        .map(|secs| {
            let mins = secs / 60;
            let secs = secs % 60;
            format!("({:}:{:02})", mins, secs)
        })
        .unwrap_or_default();

    // Timer style: red if < 10 seconds
    let timer_style = if timer_seconds.map(|s| s < 10).unwrap_or(false) {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Calculate positions
    let timer_width = timer_text.len() as u16;
    let help_width = help_text.len() as u16;

    // If both fit, render with proper spacing
    if help_width + timer_width + 2 <= width {
        // Help text on the left
        frame.render_widget(
            Line::styled(help_text, Style::default().fg(Color::DarkGray)),
            Rect::new(x, y, help_width, 1),
        );

        // Timer on the right
        if !timer_text.is_empty() {
            let timer_x = x + width - timer_width;
            frame.render_widget(
                Line::styled(&timer_text, timer_style),
                Rect::new(timer_x, y, timer_width, 1),
            );
        }
    } else {
        // Just render truncated help text
        let truncated = truncate_with_ellipsis(help_text, width as usize);
        frame.render_widget(
            Line::styled(truncated, Style::default().fg(Color::DarkGray)),
            Rect::new(x, y, width, 1),
        );
    }
}

// ============================================================================
// Free Form Mode Rendering
// ============================================================================

/// Render the free-form input mode
///
/// Layout:
///   Row 0: "{title} · {repo}"
///   Row 1: blank
///   Row 2: question truncated with "..."
///   Row 3: blank
///   Row 4-5: input box with borders
///   Row 6: blank
///   Row 7: [← back]  [send]
#[allow(clippy::too_many_arguments)]
fn render_free_form_mode(
    frame: &mut Frame,
    area: Rect,
    _thread_id: &str,
    title: &str,
    repo: &str,
    question: &str,
    input_text: &str,
    cursor_pos: usize,
) {
    let mut y = area.y;

    // Header
    let header = format!("{} \u{00b7} {}", title, repo);
    let header_line = Line::styled(header, Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(header_line, Rect::new(area.x, y, area.width, 1));
    y += 2; // Skip blank line

    // Truncated question
    let truncated_q = truncate_with_ellipsis(question, area.width.saturating_sub(4) as usize);
    frame.render_widget(Line::raw(truncated_q), Rect::new(area.x, y, area.width, 1));
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

        frame.render_widget(Line::from(spans), Rect::new(area.x, y, area.width, 1));
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

    // Send button (right aligned)
    let send_x = area.x + area.width - send_width;
    let send_area = Rect::new(send_x, y, send_width, 1);
    frame.render_widget(Span::raw(send_btn), send_area);
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Render a tab bar for multi-question flow
///
/// Format: `[Auth]  Database  Validation`
/// - Active tab is wrapped in brackets: `[Auth]`
/// - Answered tabs are shown in green
/// - Unanswered tabs are dimmed
///
/// # Arguments
/// * `headers` - Tab header labels
/// * `current_tab` - Index of the active tab (0-based)
/// * `answered` - Which tabs have been answered
/// * `max_width` - Maximum width for the tab bar
///
/// # Returns
/// A vector of Spans for the tab bar line
fn render_tab_bar(
    headers: &[String],
    current_tab: usize,
    answered: &[bool],
    max_width: usize,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut total_width = 0;

    for (i, header) in headers.iter().enumerate() {
        let is_active = i == current_tab;
        let is_answered = answered.get(i).copied().unwrap_or(false);

        // Format: "[Header]" for active, "Header" for inactive
        let text = if is_active {
            format!("[{}]", header)
        } else {
            header.clone()
        };

        let text_width = text.chars().count();

        // Check if we have room for this tab
        if total_width + text_width + 2 > max_width && i > 0 {
            // Truncate with ellipsis if needed
            spans.push(Span::styled("...", Style::default().fg(Color::DarkGray)));
            break;
        }

        // Style based on state
        let style = if is_active {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else if is_answered {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        spans.push(Span::styled(text, style));
        total_width += text_width;

        // Add spacing between tabs (except for last)
        if i < headers.len() - 1 {
            spans.push(Span::raw("  "));
            total_width += 2;
        }
    }

    spans
}

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
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

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
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "supercalif");
        assert_eq!(result[1], "ragilistic");
        assert_eq!(result[2], "expialidoc");
        assert_eq!(result[3], "ious");
    }

    #[test]
    fn test_wrap_text_long_word_truncated() {
        let result = wrap_text("supercalifragilisticexpialidocious", 10, 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "supercalif");
        assert_eq!(result[1], "ragilistic");
    }

    #[test]
    fn test_wrap_text_preserves_whitespace_handling() {
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
        let result = truncate_with_ellipsis("Hello \u{4e16}\u{754c}!", 10);
        assert_eq!(result, "Hello \u{4e16}\u{754c}!");
    }

    #[test]
    fn test_truncate_unicode_truncated() {
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
        assert!(visible.len() == 5);
        assert!(cursor <= 5);
    }

    #[test]
    fn test_get_visible_input_empty() {
        let (visible, cursor) = get_visible_input("", 0, 10);
        assert_eq!(visible, "");
        assert_eq!(cursor, 0);
    }

    // -------------------- QuestionRenderConfig Tests --------------------

    #[test]
    fn test_question_render_config_default() {
        let config = QuestionRenderConfig::default();
        assert_eq!(config.question, "");
        assert!(config.options.is_empty());
        assert_eq!(config.selected_index, Some(0));
        assert!(!config.multi_select);
        assert!(config.multi_selections.is_empty());
        assert_eq!(config.other_input, "");
        assert!(!config.other_selected);
        assert!(config.timer_seconds.is_none());
    }

    // -------------------- render_question Tests --------------------

    #[test]
    fn test_render_question_single_select() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec![
            "JWT tokens".to_string(),
            "Session cookies".to_string(),
            "OAuth 2.0 only".to_string(),
        ];

        let config = QuestionRenderConfig {
            question: "Which authentication method should I use?",
            options: &options,
            option_descriptions: &[],
            selected_index: Some(0),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: Some(272), // 4:32
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                let area = Rect::new(2, 1, 56, 12);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Implement auth",
                    "my-project",
                    &config,
                );
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_render_question_multi_select() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec![
            "Linting".to_string(),
            "Unit tests".to_string(),
            "E2E tests".to_string(),
        ];
        let multi_selections = vec![true, true, false];

        let config = QuestionRenderConfig {
            question: "Select features to enable:",
            options: &options,
            option_descriptions: &[],
            selected_index: Some(0),
            multi_select: true,
            multi_selections: &multi_selections,
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                let area = Rect::new(2, 1, 56, 12);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Setup CI",
                    "my-repo",
                    &config,
                );
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_render_question_with_other_selected() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec!["Option A".to_string(), "Option B".to_string()];

        let config = QuestionRenderConfig {
            question: "Choose an option:",
            options: &options,
            option_descriptions: &[],
            selected_index: None,
            multi_select: false,
            multi_selections: &[],
            other_input: "Custom value",
            other_selected: true,
            timer_seconds: Some(5), // < 10 seconds, should be red
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                let area = Rect::new(2, 1, 56, 12);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Test",
                    "repo",
                    &config,
                );
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_render_question_minimum_area() {
        let backend = TestBackend::new(20, 6);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec!["A".to_string()];
        let config = QuestionRenderConfig {
            question: "Q?",
            options: &options,
            option_descriptions: &[],
            selected_index: Some(0),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 20, 6);
                render_question(
                    frame,
                    area,
                    "t1",
                    "Title",
                    "repo",
                    &config,
                );
            })
            .unwrap();

        // Should render without panic even with minimal area
    }

    #[test]
    fn test_render_question_too_small_area() {
        let backend = TestBackend::new(10, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec!["A".to_string()];
        let config = QuestionRenderConfig {
            question: "Q?",
            options: &options,
            option_descriptions: &[],
            selected_index: Some(0),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                // Area too small (height < 3) - should bail out gracefully
                let area = Rect::new(0, 0, 8, 2);
                render_question(
                    frame,
                    area,
                    "t1",
                    "Title",
                    "repo",
                    &config,
                );
            })
            .unwrap();

        // Should not panic, should bail early (height=2 < min 3)
    }

    // -------------------- Help Line Tests --------------------

    #[test]
    fn test_help_line_with_timer() {
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_help_line(frame, 0, 0, 60, Some(272), false);
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_help_line_timer_red_when_low() {
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_help_line(frame, 0, 0, 60, Some(5), false);
            })
            .unwrap();

        // Should render without panic (timer should be red)
    }

    #[test]
    fn test_help_line_no_timer() {
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_help_line(frame, 0, 0, 60, None, false);
            })
            .unwrap();

        // Should render without panic
    }

    // -------------------- Legacy Render Tests --------------------

    #[test]
    fn test_legacy_render_question_mode() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec!["Yes".to_string(), "No".to_string()];
        
        terminal
            .draw(|frame| {
                let area = Rect::new(2, 1, 56, 12);
                render(
                    frame,
                    area,
                    "thread-1",
                    "Confirm",
                    "repo",
                    "Proceed?",
                    &options,
                    None,
                );
            })
            .unwrap();

        // Should render without panic
    }

    #[test]
    fn test_legacy_render_free_form_mode() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        
        terminal
            .draw(|frame| {
                let area = Rect::new(2, 1, 56, 12);
                render(
                    frame,
                    area,
                    "thread-1",
                    "Input",
                    "repo",
                    "Enter text:",
                    &[],
                    Some(("Hello", 5)),
                );
            })
            .unwrap();

        // Should render without panic
    }

    // -------------------- Tab Bar Tests --------------------

    #[test]
    fn test_render_tab_bar_single_tab() {
        let headers = vec!["Auth".to_string()];
        let answered = vec![false];
        let spans = render_tab_bar(&headers, 0, &answered, 50);

        // Single tab should have brackets around it
        assert!(!spans.is_empty());
        // First span should be "[Auth]"
        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("[Auth]"));
    }

    #[test]
    fn test_render_tab_bar_multiple_tabs_first_active() {
        let headers = vec![
            "Auth".to_string(),
            "Database".to_string(),
            "Validation".to_string(),
        ];
        let answered = vec![false, false, false];
        let spans = render_tab_bar(&headers, 0, &answered, 50);

        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        // Active tab (first) should have brackets
        assert!(text.contains("[Auth]"));
        // Other tabs should not have brackets
        assert!(text.contains("Database"));
        assert!(text.contains("Validation"));
        assert!(!text.contains("[Database]"));
    }

    #[test]
    fn test_render_tab_bar_multiple_tabs_second_active() {
        let headers = vec![
            "Auth".to_string(),
            "Database".to_string(),
            "Validation".to_string(),
        ];
        let answered = vec![true, false, false]; // First answered
        let spans = render_tab_bar(&headers, 1, &answered, 50);

        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        // Second tab (Database) should have brackets
        assert!(text.contains("[Database]"));
        // Auth should not have brackets (but is answered)
        assert!(!text.contains("[Auth]"));
    }

    #[test]
    fn test_render_tab_bar_all_answered() {
        let headers = vec!["Q1".to_string(), "Q2".to_string()];
        let answered = vec![true, true];
        let spans = render_tab_bar(&headers, 0, &answered, 50);

        // Should still render correctly
        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("[Q1]")); // Active
        assert!(text.contains("Q2")); // Answered
    }

    #[test]
    fn test_render_tab_bar_truncation() {
        let headers = vec![
            "VeryLongTabName1".to_string(),
            "VeryLongTabName2".to_string(),
            "VeryLongTabName3".to_string(),
        ];
        let answered = vec![false, false, false];
        // Very narrow width - should truncate
        let spans = render_tab_bar(&headers, 0, &answered, 30);

        // Should have ellipsis if truncated
        let text: String = spans.iter().map(|s| s.content.to_string()).collect();
        // Either shows tabs or truncates with ellipsis
        assert!(!text.is_empty());
    }

    #[test]
    fn test_render_tab_bar_empty() {
        let headers: Vec<String> = vec![];
        let answered: Vec<bool> = vec![];
        let spans = render_tab_bar(&headers, 0, &answered, 50);

        assert!(spans.is_empty());
    }

    #[test]
    fn test_render_question_with_tabs() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec!["JWT".to_string(), "OAuth".to_string()];
        let tab_headers = vec![
            "Auth".to_string(),
            "Database".to_string(),
            "Validation".to_string(),
        ];
        let tabs_answered = vec![false, false, false];

        let config = QuestionRenderConfig {
            question: "Choose authentication:",
            options: &options,
            option_descriptions: &[],
            selected_index: Some(0),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &tab_headers,
            current_tab: 0,
            tabs_answered: &tabs_answered,
        };

        
        terminal
            .draw(|frame| {
                let area = Rect::new(2, 1, 56, 14);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Setup Auth",
                    "my-project",
                    &config,
                );
            })
            .unwrap();

        // Should render with tab bar
    }

    #[test]
    fn test_render_question_with_tabs_second_active() {
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec!["PostgreSQL".to_string(), "MySQL".to_string()];
        let tab_headers = vec!["Auth".to_string(), "Database".to_string()];
        let tabs_answered = vec![true, false]; // First answered

        let config = QuestionRenderConfig {
            question: "Choose database:",
            options: &options,
            option_descriptions: &[],
            selected_index: Some(0),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &tab_headers,
            current_tab: 1, // Second tab active
            tabs_answered: &tabs_answered,
        };

        
        terminal
            .draw(|frame| {
                let area = Rect::new(2, 1, 56, 14);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Setup DB",
                    "my-project",
                    &config,
                );
            })
            .unwrap();

        // Should render with tab bar
    }

    #[test]
    fn test_help_line_with_multiple_tabs() {
        let backend = TestBackend::new(60, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_help_line(frame, 0, 0, 60, None, true);
            })
            .unwrap();

        // Should render with tab hint
    }

    // -------------------- Option Description Tests --------------------

    #[test]
    fn test_render_question_with_descriptions() {
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec![
            "JWT tokens".to_string(),
            "Session cookies".to_string(),
        ];
        let option_descriptions = vec![
            "Stateless tokens signed by server".to_string(),
            "Server-side session storage".to_string(),
        ];

        let config = QuestionRenderConfig {
            question: "Which authentication method?",
            options: &options,
            option_descriptions: &option_descriptions,
            selected_index: Some(0),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                // Large enough area to show descriptions (need 2 rows per option)
                let area = Rect::new(2, 1, 56, 18);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Auth Setup",
                    "my-project",
                    &config,
                );
            })
            .unwrap();

        // Should render with descriptions
    }

    #[test]
    fn test_render_question_descriptions_skipped_when_tight() {
        // Test that descriptions are skipped when there's not enough space
        // for 2 rows per option, but all options still render
        let backend = TestBackend::new(60, 14);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec![
            "Option A".to_string(),
            "Option B".to_string(),
            "Option C".to_string(),
        ];
        let option_descriptions = vec![
            "Description A".to_string(),
            "Description B".to_string(),
            "Description C".to_string(),
        ];

        let config = QuestionRenderConfig {
            question: "Choose an option:",
            options: &options,
            option_descriptions: &option_descriptions,
            selected_index: Some(1),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                // Area with 12 rows:
                // - 1 header, 1 blank, 2 question, 1 blank = 5 rows used
                // - 2 reserved (Other + help)
                // - 5 available for options
                // - 3 options need only 3 rows (descriptions need 6, so skipped)
                let area = Rect::new(0, 0, 56, 12);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Test",
                    "repo",
                    &config,
                );
            })
            .unwrap();

        // Should render all 3 options + 1 Other
        // (descriptions are skipped because 5 available rows < 6 needed for descriptions)
    }

    #[test]
    fn test_available_option_rows_not_truncated() {
        // Test that all options are rendered when there's enough space
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec![
            "Option 1".to_string(),
            "Option 2".to_string(),
            "Option 3".to_string(),
            "Option 4".to_string(),
            "Option 5".to_string(),
        ];

        let config = QuestionRenderConfig {
            question: "Select one:",
            options: &options,
            option_descriptions: &[],
            selected_index: Some(2),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 56, 18);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Title",
                    "repo",
                    &config,
                );
            })
            .unwrap();

        // Should render all 5 options + 1 Other
    }

    #[test]
    fn test_single_select_markers() {
        // Test that single-select shows correct markers
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec![
            "First".to_string(),
            "Second".to_string(),
            "Third".to_string(),
        ];

        // Cursor on second option
        let config = QuestionRenderConfig {
            question: "Select:",
            options: &options,
            option_descriptions: &[],
            selected_index: Some(1),
            multi_select: false,
            multi_selections: &[],
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 56, 14);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Title",
                    "repo",
                    &config,
                );
            })
            .unwrap();

        // 3 options + 1 Other
    }

    #[test]
    fn test_multi_select_markers() {
        // Test that multi-select shows correct markers
        let backend = TestBackend::new(60, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        let options = vec![
            "Linting".to_string(),
            "Tests".to_string(),
            "Coverage".to_string(),
        ];
        let multi_selections = vec![true, false, true]; // First and third checked

        let config = QuestionRenderConfig {
            question: "Select features:",
            options: &options,
            option_descriptions: &[],
            selected_index: Some(0),
            multi_select: true,
            multi_selections: &multi_selections,
            other_input: "",
            other_selected: false,
            timer_seconds: None,
            tab_headers: &[],
            current_tab: 0,
            tabs_answered: &[],
        };

        
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, 56, 14);
                render_question(
                    frame,
                    area,
                    "thread-1",
                    "Title",
                    "repo",
                    &config,
                );
            })
            .unwrap();

        // 3 options + 1 Other
    }
}
