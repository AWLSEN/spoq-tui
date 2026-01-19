//! Text wrapping utilities for message rendering
//!
//! Provides functions to wrap styled text lines while maintaining prefixes
//! and optional background colors for visual continuity.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

/// Apply background color to a single line (text content only, no padding).
/// Only applies bg to existing spans, does not pad to full width.
pub fn apply_background_to_line(line: &mut Line<'static>, bg_color: Color, _max_width: usize) {
    // Apply background to all existing spans (content only, no padding)
    for span in line.spans.iter_mut() {
        span.style = span.style.bg(bg_color);
    }
}

/// Wrap a line of styled spans to fit within max_width, prepending a prefix to each wrapped line.
///
/// This handles the case where text content is longer than the viewport width.
/// Instead of relying on ratatui's Wrap (which doesn't add prefix to continuation lines),
/// we pre-wrap the content so each visual line gets the vertical bar prefix.
///
/// # Arguments
/// * `line` - The line to wrap (may contain multiple styled spans)
/// * `prefix` - The prefix to prepend to each line (e.g., "| ")
/// * `prefix_style` - Style for the prefix
/// * `max_width` - Maximum width including prefix
/// * `bg_color` - Optional background color to apply to all lines
///
/// # Returns
/// Vec of Lines, each fitting within max_width and having the prefix
pub fn wrap_line_with_prefix(
    line: Line<'static>,
    prefix: &'static str,
    prefix_style: Style,
    max_width: usize,
    bg_color: Option<Color>,
) -> Vec<Line<'static>> {
    let prefix_width = prefix.width();
    let content_width = max_width.saturating_sub(prefix_width);

    // If content width is too small, just return the line with prefix (edge case)
    if content_width < 5 {
        let mut spans = vec![Span::styled(prefix, prefix_style)];
        spans.extend(line.spans);
        let mut result_line = Line::from(spans);
        if let Some(bg) = bg_color {
            apply_background_to_line(&mut result_line, bg, max_width);
        }
        return vec![result_line];
    }

    // Collect all text content with style information
    // We'll rebuild spans as we wrap
    let mut segments: Vec<(String, Style)> = Vec::new();
    for span in line.spans {
        if !span.content.is_empty() {
            segments.push((span.content.to_string(), span.style));
        }
    }

    // If empty, return single line with just prefix
    if segments.is_empty() {
        let mut result_line = Line::from(vec![Span::styled(prefix, prefix_style)]);
        if let Some(bg) = bg_color {
            apply_background_to_line(&mut result_line, bg, max_width);
        }
        return vec![result_line];
    }

    // Calculate total width
    let total_width: usize = segments.iter().map(|(s, _)| s.width()).sum();

    // Fast path: if it fits, no wrapping needed
    if total_width <= content_width {
        let mut spans = vec![Span::styled(prefix, prefix_style)];
        for (text, style) in segments {
            spans.push(Span::styled(text, style));
        }
        let mut result_line = Line::from(spans);
        if let Some(bg) = bg_color {
            apply_background_to_line(&mut result_line, bg, max_width);
        }
        return vec![result_line];
    }

    // Need to wrap - process character by character, tracking style
    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current_line_spans: Vec<Span<'static>> = vec![Span::styled(prefix, prefix_style)];
    let mut current_line_width: usize = 0;
    let mut current_word = String::new();
    let mut current_word_style: Option<Style> = None;

    // Helper to flush current word to current line or start new line
    let flush_word = |current_line_spans: &mut Vec<Span<'static>>,
                          current_line_width: &mut usize,
                          result: &mut Vec<Line<'static>>,
                          word: &mut String,
                          word_style: Style| {
        if word.is_empty() {
            return;
        }

        let word_width = word.width();

        // If word fits on current line, add it
        if *current_line_width + word_width <= content_width {
            current_line_spans.push(Span::styled(std::mem::take(word), word_style));
            *current_line_width += word_width;
        } else {
            // Word doesn't fit - start new line
            // First, save current line if it has content beyond prefix
            if current_line_spans.len() > 1 || *current_line_width > 0 {
                result.push(Line::from(std::mem::take(current_line_spans)));
                *current_line_spans = vec![Span::styled(prefix, prefix_style)];
                *current_line_width = 0;
            }

            // If word itself is wider than content_width, break it
            if word_width > content_width {
                let mut remaining = std::mem::take(word);
                while !remaining.is_empty() {
                    let mut chunk = String::new();
                    let mut chunk_width = 0;
                    let mut chars = remaining.chars().peekable();

                    while let Some(c) = chars.peek() {
                        let c_width = unicode_width::UnicodeWidthChar::width(*c).unwrap_or(1);
                        if chunk_width + c_width > content_width && !chunk.is_empty() {
                            break;
                        }
                        chunk.push(chars.next().unwrap());
                        chunk_width += c_width;
                    }

                    remaining = chars.collect();

                    if !remaining.is_empty() {
                        // More to come, finish this line
                        current_line_spans.push(Span::styled(chunk, word_style));
                        result.push(Line::from(std::mem::take(current_line_spans)));
                        *current_line_spans = vec![Span::styled(prefix, prefix_style)];
                        *current_line_width = 0;
                    } else {
                        // Last chunk
                        current_line_spans.push(Span::styled(chunk.clone(), word_style));
                        *current_line_width = chunk.width();
                    }
                }
            } else {
                // Word fits on new line
                current_line_spans.push(Span::styled(std::mem::take(word), word_style));
                *current_line_width = word_width;
            }
        }
    };

    for (text, style) in segments {
        for c in text.chars() {
            if c == ' ' || c == '\t' {
                // Flush current word
                if let Some(ws) = current_word_style {
                    flush_word(&mut current_line_spans, &mut current_line_width, &mut result, &mut current_word, ws);
                }
                current_word_style = None;

                // Add space if it fits
                let space_width = 1;
                if current_line_width + space_width <= content_width {
                    current_line_spans.push(Span::styled(" ", style));
                    current_line_width += space_width;
                }
                // If space doesn't fit, just skip it (line break)
            } else {
                // Accumulate character into current word
                if current_word_style.is_none() {
                    current_word_style = Some(style);
                }
                // If style changed mid-word, flush and start new word segment
                if current_word_style != Some(style) {
                    if let Some(ws) = current_word_style {
                        flush_word(&mut current_line_spans, &mut current_line_width, &mut result, &mut current_word, ws);
                    }
                    current_word_style = Some(style);
                }
                current_word.push(c);
            }
        }
    }

    // Flush any remaining word
    if let Some(ws) = current_word_style {
        flush_word(&mut current_line_spans, &mut current_line_width, &mut result, &mut current_word, ws);
    }

    // Add final line if it has content
    if current_line_spans.len() > 1 || current_line_width > 0 {
        result.push(Line::from(current_line_spans));
    } else if result.is_empty() {
        // Ensure at least one line with prefix
        result.push(Line::from(vec![Span::styled(prefix, prefix_style)]));
    }

    // Apply background to all result lines if specified
    if let Some(bg) = bg_color {
        for line in &mut result {
            apply_background_to_line(line, bg, max_width);
        }
    }

    result
}

/// Wrap multiple lines with prefix, used for text content from markdown rendering
pub fn wrap_lines_with_prefix(
    lines: Vec<Line<'static>>,
    prefix: &'static str,
    prefix_style: Style,
    max_width: usize,
    bg_color: Option<Color>,
) -> Vec<Line<'static>> {
    let mut result = Vec::new();
    for line in lines {
        result.extend(wrap_line_with_prefix(line, prefix, prefix_style, max_width, bg_color));
    }
    result
}

/// Estimate the number of visual lines after word wrapping.
///
/// This function calculates how many visual lines a set of logical lines will
/// occupy when rendered with word wrapping enabled, given a specific viewport width.
///
/// Each logical line wraps to ceil(char_count / viewport_width) visual lines.
/// Empty lines count as 1 visual line.
///
/// # Arguments
/// * `lines` - The logical lines to estimate
/// * `viewport_width` - The width of the viewport in characters
///
/// # Returns
/// The estimated number of visual lines after wrapping
pub fn estimate_wrapped_line_count(lines: &[Line], viewport_width: usize) -> usize {
    if viewport_width == 0 {
        return lines.len();
    }

    lines.iter().map(|line| {
        let char_count: usize = line.spans.iter()
            .map(|s| s.content.chars().count())
            .sum();
        if char_count == 0 {
            1 // Empty line still takes 1 row
        } else {
            // Ceiling division: (char_count + viewport_width - 1) / viewport_width
            char_count.div_ceil(viewport_width)
        }
    }).sum()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_wrapped_line_count_empty() {
        let lines: Vec<Line> = vec![];
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 0);
    }

    #[test]
    fn test_estimate_wrapped_line_count_single_short_line() {
        let lines = vec![Line::from("Hello")];
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 1);
    }

    #[test]
    fn test_estimate_wrapped_line_count_single_empty_line() {
        let lines = vec![Line::from("")];
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 1);
    }

    #[test]
    fn test_estimate_wrapped_line_count_line_wraps_once() {
        // 100 characters in an 80-character viewport should wrap to 2 lines
        let long_text = "a".repeat(100);
        let lines = vec![Line::from(long_text)];
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 2);
    }

    #[test]
    fn test_estimate_wrapped_line_count_line_wraps_twice() {
        // 200 characters in an 80-character viewport should wrap to 3 lines
        let long_text = "a".repeat(200);
        let lines = vec![Line::from(long_text)];
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 3);
    }

    #[test]
    fn test_estimate_wrapped_line_count_exact_fit() {
        // Exactly 80 characters should fit in 1 line
        let exact_text = "a".repeat(80);
        let lines = vec![Line::from(exact_text)];
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 1);
    }

    #[test]
    fn test_estimate_wrapped_line_count_one_over() {
        // 81 characters should wrap to 2 lines
        let text = "a".repeat(81);
        let lines = vec![Line::from(text)];
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 2);
    }

    #[test]
    fn test_estimate_wrapped_line_count_multiple_lines() {
        // 3 short lines
        let lines = vec![
            Line::from("Hello"),
            Line::from("World"),
            Line::from("Test"),
        ];
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 3);
    }

    #[test]
    fn test_estimate_wrapped_line_count_mixed_lengths() {
        // Mix of short and long lines
        let lines = vec![
            Line::from("Short"),           // 1 line
            Line::from("a".repeat(100)),   // 2 lines (in 80-char viewport)
            Line::from(""),                // 1 line (empty)
            Line::from("Another short"),   // 1 line
        ];
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 5);
    }

    #[test]
    fn test_estimate_wrapped_line_count_zero_width() {
        // Zero width should return raw line count
        let lines = vec![
            Line::from("Hello"),
            Line::from("World"),
        ];
        assert_eq!(estimate_wrapped_line_count(&lines, 0), 2);
    }

    #[test]
    fn test_estimate_wrapped_line_count_with_spans() {
        // Line with multiple spans
        let lines = vec![
            Line::from(vec![
                Span::raw("Hello "),
                Span::raw("World"),
            ]),
        ];
        // "Hello World" = 11 chars, fits in 80-char line
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 1);
    }

    #[test]
    fn test_estimate_wrapped_line_count_spans_wrap() {
        // Line with multiple spans that together wrap
        let lines = vec![
            Line::from(vec![
                Span::raw("a".repeat(50)),
                Span::raw("b".repeat(50)),
            ]),
        ];
        // 100 chars should wrap to 2 lines in 80-char viewport
        assert_eq!(estimate_wrapped_line_count(&lines, 80), 2);
    }

    #[test]
    fn test_estimate_wrapped_line_count_narrow_viewport() {
        // Very narrow viewport causes more wrapping
        let lines = vec![Line::from("Hello World")]; // 11 chars
        // In 5-char viewport: ceil(11/5) = 3 lines
        assert_eq!(estimate_wrapped_line_count(&lines, 5), 3);
    }

    #[test]
    fn test_wrap_line_short_content_no_wrap() {
        let line = Line::from("Hello world");
        let result = wrap_line_with_prefix(line, "| ", Style::default(), 80, None);
        assert_eq!(result.len(), 1);
        // First span should be the prefix
        assert_eq!(result[0].spans[0].content.as_ref(), "| ");
    }

    #[test]
    fn test_wrap_line_empty_content() {
        let line = Line::from("");
        let result = wrap_line_with_prefix(line, "| ", Style::default(), 80, None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spans[0].content.as_ref(), "| ");
    }

    #[test]
    fn test_wrap_line_long_content_wraps() {
        // Create a line that's 100 chars, should wrap in 50-char viewport
        let long_text = "word ".repeat(20); // 100 chars
        let line = Line::from(long_text);
        let result = wrap_line_with_prefix(line, "| ", Style::default(), 50, None);
        // Should produce multiple lines
        assert!(result.len() > 1, "Expected multiple lines, got {}", result.len());
        // Each line should have the prefix
        for (i, l) in result.iter().enumerate() {
            assert_eq!(l.spans[0].content.as_ref(), "| ", "Line {} missing prefix", i);
        }
    }

    #[test]
    fn test_wrap_line_preserves_style() {
        let line = Line::from(vec![
            Span::styled("hello ", Style::default().fg(Color::Red)),
            Span::styled("world", Style::default().fg(Color::Blue)),
        ]);
        let result = wrap_line_with_prefix(line, "| ", Style::default(), 80, None);
        assert_eq!(result.len(), 1);
        // Should have prefix + two styled spans
        assert!(result[0].spans.len() >= 2); // prefix + content
    }

    #[test]
    fn test_wrap_lines_multiple() {
        let lines = vec![
            Line::from("Short line"),
            Line::from("Another short line"),
        ];
        let result = wrap_lines_with_prefix(lines, "| ", Style::default(), 80, None);
        assert_eq!(result.len(), 2);
        // Both should have prefix
        for l in &result {
            assert_eq!(l.spans[0].content.as_ref(), "| ");
        }
    }

    #[test]
    fn test_apply_background_to_line() {
        let mut line = Line::from(vec![
            Span::raw("| "),
            Span::raw("Hello"),
        ]);
        apply_background_to_line(&mut line, Color::Rgb(35, 40, 48), 20);

        // All spans should have background
        for span in &line.spans {
            assert!(span.style.bg.is_some());
        }
        // Should NOT have padding - background only applies to text content
        assert_eq!(line.spans.len(), 2);
    }

    #[test]
    fn test_wrap_with_background_applies_to_all_lines() {
        let line = Line::from(vec![Span::raw("Test content")]);
        let result = wrap_line_with_prefix(
            line,
            "| ",
            Style::default(),
            30,
            Some(Color::Rgb(35, 40, 48)),
        );

        // All spans in all lines should have background
        for line in &result {
            for span in &line.spans {
                assert!(span.style.bg.is_some(), "Span '{}' missing background", span.content);
            }
        }
    }

    #[test]
    fn test_wrap_without_background() {
        let line = Line::from(vec![Span::raw("Test")]);
        let result = wrap_line_with_prefix(line, "| ", Style::default(), 30, None);

        // No spans should have background
        for line in &result {
            for span in &line.spans {
                assert!(span.style.bg.is_none());
            }
        }
    }
}
