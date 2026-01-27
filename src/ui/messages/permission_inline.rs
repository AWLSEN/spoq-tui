//! Inline permission prompt rendering for message flow.
//!
//! This module builds permission prompts as `Vec<Line>` for inline rendering
//! within the message area. Unlike the overlay approach, these lines match
//! the message styling with vertical bar prefixes.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::state::session::{AskUserQuestionData, AskUserQuestionState, PermissionRequest};
use crate::ui::input::parse_ask_user_question;
use crate::ui::layout::LayoutContext;

// ============================================================================
// Constants
// ============================================================================

/// Server timeout for permission requests (seconds)
const PERMISSION_TIMEOUT_SECS: u64 = 55;

/// Countdown threshold for urgent styling (red color)
const URGENT_THRESHOLD_SECS: u64 = 5;

/// Vertical bar prefix for message continuity
const VERTICAL_BAR: &str = "\u{2502}";

// ============================================================================
// Public API
// ============================================================================

/// Build permission prompt lines for inline rendering in the message flow.
///
/// This function generates a `Vec<Line>` that can be appended to the message
/// area, styled with vertical bar prefixes to match other message content.
///
/// # Arguments
/// * `perm` - The permission request to render
/// * `question_state` - UI state for AskUserQuestion prompts
/// * `ctx` - Layout context for responsive sizing
/// * `_tick_count` - Animation tick counter (for blinking cursors)
///
/// # Returns
/// A vector of styled lines representing the permission prompt.
pub fn build_permission_lines(
    perm: &PermissionRequest,
    question_state: &AskUserQuestionState,
    ctx: &LayoutContext,
    _tick_count: u64,
) -> Vec<Line<'static>> {
    // Calculate countdown
    let elapsed_secs = perm.received_at.elapsed().as_secs();
    let remaining_secs = PERMISSION_TIMEOUT_SECS.saturating_sub(elapsed_secs);

    // Check if this is an AskUserQuestion tool
    if perm.tool_name == "AskUserQuestion" {
        if let Some(ref tool_input) = perm.tool_input {
            if let Some(data) = parse_ask_user_question(tool_input) {
                return build_ask_user_question_lines(&data, question_state, ctx, remaining_secs);
            }
        }
    }

    // Standard permission prompt
    build_standard_permission_lines(perm, ctx, remaining_secs)
}

// ============================================================================
// Standard Permission Prompt
// ============================================================================

/// Build lines for a standard (non-AskUserQuestion) permission prompt.
fn build_standard_permission_lines(
    perm: &PermissionRequest,
    ctx: &LayoutContext,
    remaining_secs: u64,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let bar_style = Style::default().fg(Color::DarkGray);
    let bar = Span::styled(format!("{} ", VERTICAL_BAR), bar_style);

    // Empty line for spacing
    lines.push(Line::from(vec![bar.clone()]));

    // Tool name with bullet
    lines.push(Line::from(vec![
        bar.clone(),
        Span::styled(" ", Style::default()),
        Span::styled(
            "\u{25CF} ", // bullet point
            Style::default().fg(Color::White),
        ),
        Span::styled(
            perm.tool_name.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Description or preview
    let preview = get_preview_text(perm, ctx);
    if !preview.is_empty() {
        lines.push(Line::from(vec![
            bar.clone(),
            Span::styled("   ", Style::default()), // indent
            Span::styled(preview, Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Empty line
    lines.push(Line::from(vec![bar.clone()]));

    // Key options line with countdown
    let (key_spans, countdown_span) = build_key_options_and_countdown(ctx, remaining_secs);
    let mut option_line_spans = vec![bar.clone(), Span::styled("   ", Style::default())];
    option_line_spans.extend(key_spans);

    // Add padding and countdown
    let current_len: usize = option_line_spans.iter().map(|s| s.content.len()).sum();
    let countdown_len = countdown_span.content.len();
    let available_width = ctx.text_wrap_width(0) as usize;
    let padding = available_width.saturating_sub(current_len + countdown_len);
    option_line_spans.push(Span::raw(" ".repeat(padding)));
    option_line_spans.push(countdown_span);

    lines.push(Line::from(option_line_spans));

    // Trailing empty line
    lines.push(Line::from(vec![bar]));

    lines
}

/// Get preview text from a permission request.
fn get_preview_text(perm: &PermissionRequest, ctx: &LayoutContext) -> String {
    let max_len = ctx.max_preview_length();

    // Try context first
    if let Some(ref context) = perm.context {
        return truncate_string(context, max_len);
    }

    // Try extracting from tool_input
    if let Some(ref input) = perm.tool_input {
        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
            return truncate_string(path, max_len);
        }
        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            return truncate_string(cmd, max_len);
        }
        if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
            return truncate_string(content, max_len.min(100));
        }
    }

    String::new()
}

/// Truncate a string to max length with ellipsis.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

/// Build key option spans and countdown span.
fn build_key_options_and_countdown(
    ctx: &LayoutContext,
    remaining_secs: u64,
) -> (Vec<Span<'static>>, Span<'static>) {
    let key_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let key_spans = if ctx.is_extra_small() {
        // Extra small: [y]/[a]/[n]
        vec![
            Span::styled("[y]", key_style),
            Span::raw("/"),
            Span::styled("[a]", key_style),
            Span::raw("/"),
            Span::styled("[n]", key_style),
        ]
    } else if ctx.is_narrow() {
        // Narrow: [y] Y  [a] A  [n] N
        vec![
            Span::styled("[y]", key_style),
            Span::raw(" Y  "),
            Span::styled("[a]", key_style),
            Span::raw(" A  "),
            Span::styled("[n]", key_style),
            Span::raw(" N"),
        ]
    } else {
        // Normal: [y] Yes    [a] Always    [n] No
        vec![
            Span::styled("[y]", key_style),
            Span::raw(" Yes    "),
            Span::styled("[a]", key_style),
            Span::raw(" Always    "),
            Span::styled("[n]", key_style),
            Span::raw(" No"),
        ]
    };

    let countdown_span = build_countdown_span(remaining_secs);

    (key_spans, countdown_span)
}

/// Build countdown span with appropriate styling.
fn build_countdown_span(remaining_secs: u64) -> Span<'static> {
    let countdown_color = if remaining_secs <= URGENT_THRESHOLD_SECS {
        Color::Red
    } else {
        Color::DarkGray
    };

    let countdown_text = if remaining_secs == 0 {
        "(expired)".to_string()
    } else {
        format!("({}s)", remaining_secs)
    };

    Span::styled(countdown_text, Style::default().fg(countdown_color))
}

// ============================================================================
// AskUserQuestion Prompt
// ============================================================================

/// Build lines for an AskUserQuestion permission prompt.
fn build_ask_user_question_lines(
    data: &AskUserQuestionData,
    state: &AskUserQuestionState,
    ctx: &LayoutContext,
    remaining_secs: u64,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let bar_style = Style::default().fg(Color::DarkGray);
    let bar = Span::styled(format!("{} ", VERTICAL_BAR), bar_style);

    if data.questions.is_empty() {
        return lines;
    }

    let current_question = &data.questions[state.tab_index.min(data.questions.len() - 1)];

    // Empty line for spacing
    lines.push(Line::from(vec![bar.clone()]));

    // Tab bar (only if multiple questions)
    if data.questions.len() > 1 {
        let mut tab_spans = vec![bar.clone(), Span::styled(" ", Style::default())];
        for (i, q) in data.questions.iter().enumerate() {
            if i > 0 {
                tab_spans.push(Span::raw("  "));
            }
            if i == state.tab_index {
                // Active tab: white
                tab_spans.push(Span::styled(
                    format!("[{}]", q.header),
                    Style::default().fg(Color::White),
                ));
            } else {
                // Inactive tab: dark gray
                tab_spans.push(Span::styled(
                    q.header.clone(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
        lines.push(Line::from(tab_spans));
        lines.push(Line::from(vec![bar.clone()])); // Empty line after tabs
    }

    // Question text
    lines.push(Line::from(vec![
        bar.clone(),
        Span::styled(" ", Style::default()),
        Span::styled(
            current_question.question.clone(),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![bar.clone()])); // Empty line after question

    // Options
    let current_selection = state.current_selection();

    for (i, opt) in current_question.options.iter().enumerate() {
        let is_selected = current_selection == Some(i);

        if current_question.multi_select {
            // Multi-select mode with checkboxes
            let checkbox = if state.is_multi_selected(i) {
                "[\u{00D7}]" // [x]
            } else {
                "[ ]"
            };
            let marker = if is_selected { "\u{203A} " } else { "  " }; // >

            let option_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            lines.push(Line::from(vec![
                bar.clone(),
                Span::styled("   ", Style::default()),
                Span::styled(marker, option_style),
                Span::styled(format!("{} ", checkbox), option_style),
                Span::styled(opt.label.clone(), option_style),
            ]));
        } else {
            // Single-select mode with arrow marker
            let marker = if is_selected { "\u{203A} " } else { "  " }; // >

            let marker_style = Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD);
            let label_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            lines.push(Line::from(vec![
                bar.clone(),
                Span::styled("   ", Style::default()),
                Span::styled(marker, marker_style),
                Span::styled(opt.label.clone(), label_style),
            ]));
        }

        // Option description (indented)
        if !opt.description.is_empty() {
            lines.push(Line::from(vec![
                bar.clone(),
                Span::styled("       ", Style::default()), // indent
                Span::styled(
                    opt.description.clone(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        lines.push(Line::from(vec![bar.clone()])); // Spacing between options
    }

    // "Other..." option
    let is_other_selected = current_selection.is_none();
    let other_marker = if is_other_selected { "\u{203A} " } else { "  " };
    let other_style = if is_other_selected {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    lines.push(Line::from(vec![
        bar.clone(),
        Span::styled("   ", Style::default()),
        Span::styled(
            other_marker,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Other...", other_style),
    ]));

    // "Other" text input (if active)
    if state.other_active && is_other_selected {
        let other_text = state.current_other_text();
        let input_width = ctx.text_wrap_width(0).saturating_sub(12) as usize;
        let display_text: String = if other_text.len() > input_width {
            other_text[other_text.len() - input_width..].to_string()
        } else {
            other_text.to_string()
        };

        // Horizontal rule
        let rule = "\u{2500}".repeat(input_width.min(40));
        lines.push(Line::from(vec![
            bar.clone(),
            Span::styled("     ", Style::default()),
            Span::styled(rule.clone(), Style::default().fg(Color::DarkGray)),
        ]));

        // Input text with cursor
        lines.push(Line::from(vec![
            bar.clone(),
            Span::styled("     ", Style::default()),
            Span::styled(display_text, Style::default().fg(Color::White)),
            Span::styled("\u{2588}", Style::default().fg(Color::White)), // cursor block
        ]));

        // Horizontal rule
        lines.push(Line::from(vec![
            bar.clone(),
            Span::styled("     ", Style::default()),
            Span::styled(rule, Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::from(vec![bar.clone()])); // Empty line before help

    // Help line with countdown
    let help_spans = build_ask_user_help_spans(
        state,
        current_question.multi_select,
        data.questions.len() > 1,
    );
    let countdown_span = build_countdown_span(remaining_secs);

    let mut help_line_spans = vec![bar.clone(), Span::styled("   ", Style::default())];
    help_line_spans.extend(help_spans.clone());

    // Calculate padding for right-aligned countdown
    let help_len: usize = help_spans.iter().map(|s| s.content.len()).sum();
    let countdown_len = countdown_span.content.len();
    let available_width = ctx.text_wrap_width(0) as usize;
    let padding = available_width.saturating_sub(help_len + countdown_len + 6); // 6 for bar + indent
    help_line_spans.push(Span::raw(" ".repeat(padding)));
    help_line_spans.push(countdown_span);

    lines.push(Line::from(help_line_spans));

    // Trailing empty line
    lines.push(Line::from(vec![bar]));

    lines
}

/// Build help text spans for AskUserQuestion prompt.
fn build_ask_user_help_spans(
    state: &AskUserQuestionState,
    multi_select: bool,
    has_multiple_questions: bool,
) -> Vec<Span<'static>> {
    let help_style = Style::default().fg(Color::DarkGray);
    let mut spans = Vec::new();

    if state.other_active {
        // "Other" text input mode
        spans.push(Span::styled("esc", help_style));
        spans.push(Span::styled(" cancel    ", help_style));
        spans.push(Span::styled("enter", help_style));
        spans.push(Span::styled(" submit", help_style));
    } else {
        // Tab switching (only if multiple questions)
        if has_multiple_questions {
            spans.push(Span::styled("tab", help_style));
            spans.push(Span::styled(" switch    ", help_style));
        }

        if multi_select {
            spans.push(Span::styled("space", help_style));
            spans.push(Span::styled(" toggle    ", help_style));
        }

        spans.push(Span::styled("\u{2191}\u{2193}", help_style)); // up/down arrows
        spans.push(Span::styled(" navigate    ", help_style));
        spans.push(Span::styled("enter", help_style));
        spans.push(Span::styled(
            if multi_select { " submit" } else { " confirm" },
            help_style,
        ));
    }

    spans
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn make_permission(tool_name: &str, description: &str) -> PermissionRequest {
        PermissionRequest {
            permission_id: "test-perm-001".to_string(),
            thread_id: Some("test-thread".to_string()),
            tool_name: tool_name.to_string(),
            description: description.to_string(),
            context: None,
            tool_input: None,
            received_at: Instant::now(),
        }
    }

    #[test]
    fn test_build_standard_permission_lines() {
        let perm = make_permission("Bash", "Execute command");
        let state = AskUserQuestionState::default();
        let ctx = LayoutContext::new(100, 40);

        let lines = build_permission_lines(&perm, &state, &ctx, 0);

        // Should have vertical bars
        assert!(!lines.is_empty());
        for line in &lines {
            assert!(!line.spans.is_empty());
            assert!(line.spans[0].content.contains(VERTICAL_BAR));
        }
    }

    #[test]
    fn test_build_permission_lines_includes_tool_name() {
        let perm = make_permission("Write", "Write to file");
        let state = AskUserQuestionState::default();
        let ctx = LayoutContext::new(100, 40);

        let lines = build_permission_lines(&perm, &state, &ctx, 0);

        // Should include tool name somewhere
        let all_text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(all_text.contains("Write"));
    }

    #[test]
    fn test_countdown_normal() {
        let span = build_countdown_span(45);
        assert_eq!(span.content.as_ref(), "(45s)");
        assert_eq!(span.style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn test_countdown_urgent() {
        let span = build_countdown_span(5);
        assert_eq!(span.content.as_ref(), "(5s)");
        assert_eq!(span.style.fg, Some(Color::Red));
    }

    #[test]
    fn test_countdown_expired() {
        let span = build_countdown_span(0);
        assert_eq!(span.content.as_ref(), "(expired)");
        assert_eq!(span.style.fg, Some(Color::Red));
    }

    #[test]
    fn test_key_options_normal() {
        let ctx = LayoutContext::new(120, 40);
        let (spans, _) = build_key_options_and_countdown(&ctx, 50);

        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("Yes"));
        assert!(text.contains("Always"));
        assert!(text.contains("No"));
    }

    #[test]
    fn test_key_options_narrow() {
        let ctx = LayoutContext::new(70, 24);
        let (spans, _) = build_key_options_and_countdown(&ctx, 50);

        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        // Narrow uses abbreviated labels
        assert!(text.contains("[y]"));
        assert!(text.contains("[a]"));
        assert!(text.contains("[n]"));
        assert!(text.contains(" Y "));
        assert!(text.contains(" A "));
        assert!(text.contains(" N"));
    }

    #[test]
    fn test_key_options_extra_small() {
        let ctx = LayoutContext::new(50, 24);
        let (spans, _) = build_key_options_and_countdown(&ctx, 50);

        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        // Extra small uses slashes
        assert!(text.contains("[y]/[a]/[n]"));
    }

    #[test]
    fn test_truncate_string_short() {
        assert_eq!(truncate_string("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_string_exact() {
        assert_eq!(truncate_string("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_string_long() {
        assert_eq!(truncate_string("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_string_very_short_max() {
        assert_eq!(truncate_string("hello", 3), "hel");
    }

    #[test]
    fn test_build_ask_user_help_spans_other_active() {
        let mut state = AskUserQuestionState::new(1, &[2]);
        state.other_active = true;

        let spans = build_ask_user_help_spans(&state, false, false);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(text.contains("esc"));
        assert!(text.contains("cancel"));
        assert!(text.contains("enter"));
        assert!(text.contains("submit"));
    }

    #[test]
    fn test_build_ask_user_help_spans_multi_select() {
        let state = AskUserQuestionState::new(1, &[2]);
        let spans = build_ask_user_help_spans(&state, true, false);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(text.contains("space"));
        assert!(text.contains("toggle"));
        assert!(text.contains("submit")); // multi-select uses submit
    }

    #[test]
    fn test_build_ask_user_help_spans_single_select() {
        let state = AskUserQuestionState::new(1, &[2]);
        let spans = build_ask_user_help_spans(&state, false, false);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(!text.contains("space")); // no toggle for single-select
        assert!(text.contains("confirm")); // single-select uses confirm
    }

    #[test]
    fn test_build_ask_user_help_spans_multiple_questions() {
        let state = AskUserQuestionState::new(2, &[2, 2]);
        let spans = build_ask_user_help_spans(&state, false, true);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(text.contains("tab"));
        assert!(text.contains("switch"));
    }
}
