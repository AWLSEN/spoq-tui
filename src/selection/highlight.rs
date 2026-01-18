//! Selection highlighting utilities
//!
//! This module provides helpers for applying selection highlighting to
//! rendered text. It handles splitting spans at selection boundaries
//! and applying the selection background style.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use super::state::SelectionRange;

/// Default selection highlight color (subtle blue background)
pub const SELECTION_BG_COLOR: Color = Color::Rgb(60, 80, 120);

/// Apply selection highlighting to a line of text.
///
/// Given a line and the selection columns for that line, returns a new Line
/// with the selected portion highlighted with a different background color.
///
/// # Arguments
/// * `line` - The line to apply highlighting to
/// * `start_col` - Starting column of selection (inclusive)
/// * `end_col` - Ending column of selection (exclusive)
/// * `highlight_style` - Style to apply to selected text
///
/// # Returns
/// A new Line with selection highlighting applied
pub fn highlight_line_selection<'a>(
    line: &Line<'a>,
    start_col: usize,
    end_col: usize,
    highlight_style: Style,
) -> Line<'static> {
    if start_col >= end_col {
        // No selection on this line
        return Line::from(
            line.spans
                .iter()
                .map(|s| Span::styled(s.content.to_string(), s.style))
                .collect::<Vec<_>>(),
        );
    }

    let mut result_spans: Vec<Span<'static>> = Vec::new();
    let mut current_col = 0;

    for span in &line.spans {
        let span_len = span.content.chars().count();
        let span_start = current_col;
        let span_end = current_col + span_len;

        // Calculate overlap with selection
        let sel_start_in_span = start_col.saturating_sub(span_start);
        let sel_end_in_span = end_col.saturating_sub(span_start).min(span_len);

        if sel_start_in_span >= span_len || sel_end_in_span == 0 {
            // No overlap - keep original span
            result_spans.push(Span::styled(span.content.to_string(), span.style));
        } else if sel_start_in_span == 0 && sel_end_in_span >= span_len {
            // Entire span is selected
            let merged_style = span.style.bg(highlight_style.bg.unwrap_or(SELECTION_BG_COLOR));
            result_spans.push(Span::styled(span.content.to_string(), merged_style));
        } else {
            // Partial selection - need to split the span
            let chars: Vec<char> = span.content.chars().collect();

            // Part before selection
            if sel_start_in_span > 0 {
                let before: String = chars[..sel_start_in_span].iter().collect();
                result_spans.push(Span::styled(before, span.style));
            }

            // Selected part
            let selected: String = chars[sel_start_in_span..sel_end_in_span].iter().collect();
            let merged_style = span.style.bg(highlight_style.bg.unwrap_or(SELECTION_BG_COLOR));
            result_spans.push(Span::styled(selected, merged_style));

            // Part after selection
            if sel_end_in_span < span_len {
                let after: String = chars[sel_end_in_span..].iter().collect();
                result_spans.push(Span::styled(after, span.style));
            }
        }

        current_col = span_end;
    }

    Line::from(result_spans)
}

/// Apply selection highlighting to multiple lines.
///
/// # Arguments
/// * `lines` - The lines to highlight
/// * `selection` - The selection range to apply
/// * `first_line_index` - The content line index of the first line in `lines`
/// * `highlight_style` - Style to apply to selected text
///
/// # Returns
/// A new vector of Lines with selection highlighting applied
pub fn highlight_lines_selection(
    lines: &[Line<'_>],
    selection: &SelectionRange,
    first_line_index: usize,
    highlight_style: Style,
) -> Vec<Line<'static>> {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let content_line = first_line_index + i;

            // Get the selected columns for this line
            if let Some((start_col, end_col)) =
                selection.columns_for_line(content_line, line.width())
            {
                highlight_line_selection(line, start_col, end_col, highlight_style)
            } else {
                // No selection on this line - clone it
                Line::from(
                    line.spans
                        .iter()
                        .map(|s| Span::styled(s.content.to_string(), s.style))
                        .collect::<Vec<_>>(),
                )
            }
        })
        .collect()
}

/// Get the default selection highlight style
pub fn default_highlight_style() -> Style {
    Style::default().bg(SELECTION_BG_COLOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_no_selection() {
        let line = Line::from("Hello World");
        let result = highlight_line_selection(&line, 5, 5, default_highlight_style());

        assert_eq!(result.spans.len(), 1);
        assert_eq!(result.spans[0].content, "Hello World");
    }

    #[test]
    fn test_highlight_full_selection() {
        let line = Line::from("Hello");
        let result = highlight_line_selection(&line, 0, 5, default_highlight_style());

        assert_eq!(result.spans.len(), 1);
        assert_eq!(result.spans[0].content, "Hello");
        assert_eq!(result.spans[0].style.bg, Some(SELECTION_BG_COLOR));
    }

    #[test]
    fn test_highlight_partial_selection() {
        let line = Line::from("Hello World");
        let result = highlight_line_selection(&line, 6, 11, default_highlight_style());

        // Should have: "Hello " (unselected), "World" (selected)
        assert_eq!(result.spans.len(), 2);
        assert_eq!(result.spans[0].content, "Hello ");
        assert_eq!(result.spans[0].style.bg, None);
        assert_eq!(result.spans[1].content, "World");
        assert_eq!(result.spans[1].style.bg, Some(SELECTION_BG_COLOR));
    }

    #[test]
    fn test_highlight_middle_selection() {
        let line = Line::from("Hello World!");
        let result = highlight_line_selection(&line, 2, 7, default_highlight_style());

        // Should have: "He" (before), "llo W" (selected), "orld!" (after)
        assert_eq!(result.spans.len(), 3);
        assert_eq!(result.spans[0].content, "He");
        assert_eq!(result.spans[1].content, "llo W");
        assert_eq!(result.spans[1].style.bg, Some(SELECTION_BG_COLOR));
        assert_eq!(result.spans[2].content, "orld!");
    }

    #[test]
    fn test_highlight_multiple_spans() {
        let line = Line::from(vec![
            Span::raw("Hello "),
            Span::styled("World", Style::default().fg(Color::Red)),
        ]);
        let result = highlight_line_selection(&line, 3, 8, default_highlight_style());

        // Selection spans across both original spans
        // "Hel" (unselected), "lo " (selected from first), "Wo" (selected from second, keeps red fg), "rld" (unselected, keeps red fg)
        assert!(result.spans.len() >= 2);

        // Verify the selected parts have the highlight background
        let selected_parts: Vec<_> = result.spans.iter()
            .filter(|s| s.style.bg == Some(SELECTION_BG_COLOR))
            .collect();
        assert!(!selected_parts.is_empty());
    }

    #[test]
    fn test_highlight_preserves_original_style() {
        let line = Line::from(vec![
            Span::styled("Hello", Style::default().fg(Color::Red)),
        ]);
        let result = highlight_line_selection(&line, 0, 5, default_highlight_style());

        assert_eq!(result.spans.len(), 1);
        assert_eq!(result.spans[0].style.fg, Some(Color::Red));
        assert_eq!(result.spans[0].style.bg, Some(SELECTION_BG_COLOR));
    }
}
