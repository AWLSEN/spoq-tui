//! Permission prompt rendering.
//!
//! Implements the permission prompt UI including the AskUserQuestion dialog.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::state::session::{AskUserQuestionData, AskUserQuestionState, PermissionRequest};

use super::super::helpers::render_dialog_background;
use super::super::layout::LayoutContext;
use super::super::theme::COLOR_DIM;

// ============================================================================
// Permission Box Constants
// ============================================================================

/// Minimum width for the permission box (must fit keyboard options)
pub const MIN_PERMISSION_BOX_WIDTH: u16 = 30;
/// Default/maximum width for the permission box
pub const DEFAULT_PERMISSION_BOX_WIDTH: u16 = 60;
/// Default height for the permission box
pub const DEFAULT_PERMISSION_BOX_HEIGHT: u16 = 10;
/// Minimum height for a compact permission box (skips preview)
pub const MIN_PERMISSION_BOX_HEIGHT: u16 = 6;

// ============================================================================
// AskUserQuestion Parsing
// ============================================================================

/// Parse AskUserQuestion tool input into structured data.
///
/// Attempts to deserialize the tool_input JSON value into an `AskUserQuestionData`.
/// Returns `None` if the input doesn't match the expected structure.
pub fn parse_ask_user_question(tool_input: &serde_json::Value) -> Option<AskUserQuestionData> {
    serde_json::from_value(tool_input.clone()).ok()
}

// ============================================================================
// Permission Prompt Rendering
// ============================================================================

/// Render an inline permission prompt in the message flow.
///
/// Shows a Claude Code-style permission box with:
/// - Tool name and description
/// - Preview of the action (file path, command, etc.)
/// - Keyboard options: [y] Yes, [a] Always, [n] No
///
/// For AskUserQuestion tools, renders a special tabbed question UI instead.
pub fn render_permission_prompt(
    frame: &mut Frame,
    area: Rect,
    pending_permission: Option<&PermissionRequest>,
    question_state: &AskUserQuestionState,
    terminal_width: u16,
    terminal_height: u16,
) {
    if let Some(perm) = pending_permission {
        let ctx = LayoutContext::new(terminal_width, terminal_height);
        render_permission_box(frame, area, perm, question_state, &ctx);
    }
}

/// Render the permission request box.
///
/// For AskUserQuestion tools, delegates to `render_ask_user_question_box`.
/// For other tools, renders the standard responsive permission prompt.
pub fn render_permission_box(
    frame: &mut Frame,
    area: Rect,
    perm: &PermissionRequest,
    question_state: &AskUserQuestionState,
    ctx: &LayoutContext,
) {
    // Check if this is an AskUserQuestion tool - render special UI
    if perm.tool_name == "AskUserQuestion" {
        if let Some(ref tool_input) = perm.tool_input {
            if let Some(data) = parse_ask_user_question(tool_input) {
                render_ask_user_question_box(frame, area, perm, &data, question_state);
                return;
            }
        }
    }

    // Standard responsive permission box for other tools
    // Calculate responsive box dimensions
    let available_width = area.width.saturating_sub(4);
    let box_width = if ctx.is_extra_small() {
        // Extra small: use minimum width or available space
        MIN_PERMISSION_BOX_WIDTH.min(available_width)
    } else if ctx.is_narrow() {
        // Narrow: scale down from default
        ctx.bounded_width(70, MIN_PERMISSION_BOX_WIDTH, DEFAULT_PERMISSION_BOX_WIDTH)
            .min(available_width)
    } else {
        // Normal: use default width
        DEFAULT_PERMISSION_BOX_WIDTH.min(available_width)
    };

    // Calculate responsive height
    let available_height = area.height.saturating_sub(2);
    let show_preview = !ctx.is_short() && available_height >= DEFAULT_PERMISSION_BOX_HEIGHT;
    let box_height = if show_preview {
        DEFAULT_PERMISSION_BOX_HEIGHT.min(available_height)
    } else {
        MIN_PERMISSION_BOX_HEIGHT.min(available_height)
    };

    // Center the box
    let x = area.x + (area.width.saturating_sub(box_width)) / 2;
    let y = area.y + (area.height.saturating_sub(box_height)) / 2;

    let box_area = Rect {
        x,
        y,
        width: box_width,
        height: box_height,
    };

    // Choose title based on available width
    let title = if ctx.is_narrow() {
        " Permission "
    } else {
        " Permission Required "
    };

    // Render solid background
    render_dialog_background(frame, box_area);

    // Create the permission box with border
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_DIM));

    // Render the block
    frame.render_widget(block, box_area);

    // Inner area for content
    let inner = Rect {
        x: box_area.x + 2,
        y: box_area.y + 1,
        width: box_area.width.saturating_sub(4),
        height: box_area.height.saturating_sub(2),
    };

    // Build content lines
    let mut lines: Vec<Line> = Vec::new();

    // Tool name line - truncate description on narrow terminals
    let max_desc_width = (inner.width as usize).saturating_sub(perm.tool_name.len() + 2);
    let description = if perm.description.len() > max_desc_width && max_desc_width > 3 {
        format!(
            "{}...",
            &perm.description[..max_desc_width.saturating_sub(3)]
        )
    } else {
        perm.description.clone()
    };

    lines.push(Line::from(vec![
        Span::styled(
            format!("{}: ", perm.tool_name),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(description, Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from("")); // Empty line

    // Preview box - only show if we have space
    if show_preview {
        let preview_content = get_permission_preview(perm);
        if !preview_content.is_empty() {
            // Preview border top
            lines.push(Line::from(vec![Span::styled(
                format!("┌{}┐", "─".repeat((inner.width as usize).saturating_sub(2))),
                Style::default().fg(COLOR_DIM),
            )]));

            // Preview content (truncated if needed)
            let max_preview_width = (inner.width as usize).saturating_sub(4);
            let max_preview_lines = if ctx.is_short() { 1 } else { 3 };
            for line in preview_content.lines().take(max_preview_lines) {
                let truncated = if line.len() > max_preview_width && max_preview_width > 3 {
                    format!("{}...", &line[..max_preview_width.saturating_sub(3)])
                } else {
                    line.to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled("| ", Style::default().fg(COLOR_DIM)),
                    Span::styled(truncated, Style::default().fg(COLOR_DIM)),
                    Span::raw(" "),
                ]));
            }

            // Preview border bottom
            lines.push(Line::from(vec![Span::styled(
                format!("└{}┘", "─".repeat((inner.width as usize).saturating_sub(2))),
                Style::default().fg(COLOR_DIM),
            )]));
        }

        lines.push(Line::from("")); // Empty line
    }

    lines.push(Line::from("")); // Empty line

    // Calculate timeout countdown (server times out at 55s)
    let elapsed_secs = perm.received_at.elapsed().as_secs();
    let remaining_secs = 55u64.saturating_sub(elapsed_secs);

    // Style countdown: white when urgent (<5s), dim otherwise
    let countdown_color = if remaining_secs <= 5 {
        Color::White
    } else {
        COLOR_DIM
    };

    let countdown_text = if remaining_secs == 0 {
        " (expired)".to_string()
    } else {
        format!(" ({}s)", remaining_secs)
    };

    // Keyboard options - compact on narrow terminals, with countdown
    if ctx.is_extra_small() {
        // Extra compact: [y]/[a]/[n] (countdown)
        lines.push(Line::from(vec![
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "[n]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(countdown_text, Style::default().fg(countdown_color)),
        ]));
    } else if ctx.is_narrow() {
        // Compact: [y] Y  [a] A  [n] N (countdown)
        lines.push(Line::from(vec![
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Y "),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" A "),
            Span::styled(
                "[n]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" N"),
            Span::styled(countdown_text, Style::default().fg(countdown_color)),
        ]));
    } else {
        // Full: [y] Yes  [a] Always  [n] No (countdown)
        lines.push(Line::from(vec![
            Span::styled(
                "[y]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Yes  "),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Always  "),
            Span::styled(
                "[n]",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" No"),
            Span::styled(countdown_text, Style::default().fg(countdown_color)),
        ]));
    }

    let content = Paragraph::new(lines);
    frame.render_widget(content, inner);
}

/// Extract preview content from a PermissionRequest.
pub fn get_permission_preview(perm: &PermissionRequest) -> String {
    // First try context (human-readable description)
    if let Some(ref ctx) = perm.context {
        return ctx.clone();
    }

    // Fall back to tool_input if available
    if let Some(ref input) = perm.tool_input {
        // Try to extract common fields
        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
            return path.to_string();
        }
        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            return cmd.to_string();
        }
        if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
            // Truncate long content (respecting UTF-8 boundaries)
            if content.len() > 100 {
                return super::super::helpers::truncate_string(content, 100);
            }
            return content.to_string();
        }
        // Fallback: pretty print JSON
        if let Ok(pretty) = serde_json::to_string_pretty(input) {
            return pretty;
        }
    }

    String::new()
}

// ============================================================================
// AskUserQuestion UI Renderer
// ============================================================================

/// Render the AskUserQuestion prompt as a tabbed question UI.
///
/// Displays:
/// - Tab bar for multiple questions (only if > 1 question)
/// - Current question text
/// - Options with selection highlight
/// - "Other..." option at bottom
/// - Keybind help and countdown timer
fn render_ask_user_question_box(
    frame: &mut Frame,
    area: Rect,
    perm: &PermissionRequest,
    data: &AskUserQuestionData,
    state: &AskUserQuestionState,
) {
    if data.questions.is_empty() {
        return;
    }

    // Calculate box dimensions - larger than standard permission box for questions
    let box_width = 65u16.min(area.width.saturating_sub(4));
    // Height: border(2) + tabs?(1) + question(2) + options(varies) + help(1) + padding
    let num_options = data
        .questions
        .get(state.tab_index)
        .map(|q| q.options.len())
        .unwrap_or(0);
    // Each option takes 2-3 lines (label + description)
    let options_height = ((num_options + 1) * 3) as u16; // +1 for "Other"
    let base_height = 8u16; // borders + question + help + padding
    let box_height = (base_height + options_height).min(area.height.saturating_sub(2));

    // Center the box
    let x = area.x + (area.width.saturating_sub(box_width)) / 2;
    let y = area.y + (area.height.saturating_sub(box_height)) / 2;

    let box_area = Rect {
        x,
        y,
        width: box_width,
        height: box_height,
    };

    // Render solid background
    render_dialog_background(frame, box_area);

    // Create the question box with border
    let block = Block::default()
        .title(Span::styled(
            " Question ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_DIM));

    frame.render_widget(block, box_area);

    // Inner area for content
    let inner = Rect {
        x: box_area.x + 2,
        y: box_area.y + 1,
        width: box_area.width.saturating_sub(4),
        height: box_area.height.saturating_sub(2),
    };

    let mut lines: Vec<Line> = Vec::new();
    let current_question = &data.questions[state.tab_index.min(data.questions.len() - 1)];

    // Tab bar (only for multiple questions)
    if data.questions.len() > 1 {
        let mut tab_spans: Vec<Span> = Vec::new();
        for (i, q) in data.questions.iter().enumerate() {
            if i > 0 {
                tab_spans.push(Span::raw("  "));
            }
            if i == state.tab_index {
                // Active tab: white with brackets
                tab_spans.push(Span::styled(
                    format!("[{}]", q.header),
                    Style::default().fg(Color::White),
                ));
            } else {
                // Inactive tab: dim gray
                tab_spans.push(Span::styled(
                    q.header.clone(),
                    Style::default().fg(COLOR_DIM),
                ));
            }
        }
        lines.push(Line::from(tab_spans));
        lines.push(Line::from("")); // Empty line after tabs
    }

    // Question text
    lines.push(Line::from(Span::styled(
        current_question.question.clone(),
        Style::default().fg(Color::White),
    )));
    lines.push(Line::from("")); // Empty line after question

    // Options
    let current_selection = state.current_selection();

    for (i, opt) in current_question.options.iter().enumerate() {
        let is_selected = current_selection == Some(i);

        if current_question.multi_select {
            // Multi-select mode: show checkboxes
            let checkbox = if state.is_multi_selected(i) {
                "[×]"
            } else {
                "[ ]"
            };
            let marker = if is_selected { "› " } else { "  " };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{}{} ", marker, checkbox),
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(COLOR_DIM)
                    },
                ),
                Span::styled(
                    opt.label.clone(),
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(COLOR_DIM)
                    },
                ),
            ]));
        } else {
            // Single-select mode: show arrow marker
            let marker = if is_selected { "› " } else { "  " };

            lines.push(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    opt.label.clone(),
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(COLOR_DIM)
                    },
                ),
            ]));
        }

        // Description (indented, dim)
        if !opt.description.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("    {}", opt.description),
                Style::default().fg(COLOR_DIM),
            )));
        }
        lines.push(Line::from("")); // Spacing between options
    }

    // "Other..." option
    let is_other_selected = current_selection.is_none();
    let other_marker = if is_other_selected { "› " } else { "  " };
    lines.push(Line::from(vec![
        Span::styled(
            other_marker,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Other...",
            if is_other_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_DIM)
            },
        ),
    ]));

    // "Other" text input (if active)
    if state.other_active && is_other_selected {
        let other_text = state.current_other_text();
        let input_width = inner.width.saturating_sub(8) as usize;
        let display_text = if other_text.len() > input_width {
            &other_text[other_text.len() - input_width..]
        } else {
            other_text
        };

        // Input box top border
        lines.push(Line::from(Span::styled(
            format!("    ┌{}┐", "─".repeat(input_width + 2)),
            Style::default().fg(COLOR_DIM),
        )));
        // Input content with cursor
        lines.push(Line::from(vec![
            Span::styled("    │ ", Style::default().fg(COLOR_DIM)),
            Span::styled(display_text, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::White)),
            Span::raw(" ".repeat(input_width.saturating_sub(display_text.len()))),
            Span::styled("│", Style::default().fg(COLOR_DIM)),
        ]));
        // Input box bottom border
        lines.push(Line::from(Span::styled(
            format!("    └{}┘", "─".repeat(input_width + 2)),
            Style::default().fg(COLOR_DIM),
        )));
    }

    lines.push(Line::from("")); // Empty line before help

    // Calculate timeout countdown
    let elapsed_secs = perm.received_at.elapsed().as_secs();
    let remaining_secs = 55u64.saturating_sub(elapsed_secs);

    let countdown_color = if remaining_secs <= 5 {
        Color::Red
    } else {
        COLOR_DIM
    };

    let countdown_text = if remaining_secs == 0 {
        "(expired)".to_string()
    } else {
        format!("({}s)", remaining_secs)
    };

    // Help line - varies based on mode
    let mut help_spans: Vec<Span> = Vec::new();

    if state.other_active {
        // "Other" text input mode
        help_spans.push(Span::styled("esc", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled(" cancel  ", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled("enter", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled(" submit", Style::default().fg(COLOR_DIM)));
    } else {
        // Tab switching (only if multiple questions)
        if data.questions.len() > 1 {
            help_spans.push(Span::styled("tab", Style::default().fg(COLOR_DIM)));
            help_spans.push(Span::styled(" switch  ", Style::default().fg(COLOR_DIM)));
        }

        if current_question.multi_select {
            help_spans.push(Span::styled("space", Style::default().fg(COLOR_DIM)));
            help_spans.push(Span::styled(" toggle  ", Style::default().fg(COLOR_DIM)));
        }

        help_spans.push(Span::styled("↑↓", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled(" navigate  ", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled("enter", Style::default().fg(COLOR_DIM)));
        help_spans.push(Span::styled(
            if current_question.multi_select {
                " submit"
            } else {
                " confirm"
            },
            Style::default().fg(COLOR_DIM),
        ));
    }

    // Add countdown at the end with spacing
    let help_text_len: usize = help_spans.iter().map(|s| s.content.len()).sum();
    let countdown_len = countdown_text.len();
    let padding_needed = (inner.width as usize)
        .saturating_sub(help_text_len)
        .saturating_sub(countdown_len);

    help_spans.push(Span::raw(" ".repeat(padding_needed)));
    help_spans.push(Span::styled(
        countdown_text,
        Style::default().fg(countdown_color),
    ));

    lines.push(Line::from(help_spans));

    let content = Paragraph::new(lines);
    frame.render_widget(content, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_ask_user_question_valid() {
        let input = json!({
            "questions": [
                {
                    "question": "Which library should we use?",
                    "header": "Auth method",
                    "options": [
                        {"label": "Option A", "description": "Description of A"},
                        {"label": "Option B", "description": "Description of B"}
                    ],
                    "multiSelect": false
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());

        let data = result.unwrap();
        assert_eq!(data.questions.len(), 1);
        assert_eq!(data.questions[0].question, "Which library should we use?");
        assert_eq!(data.questions[0].header, "Auth method");
        assert_eq!(data.questions[0].options.len(), 2);
        assert!(!data.questions[0].multi_select);
    }

    #[test]
    fn test_parse_ask_user_question_multi_select() {
        let input = json!({
            "questions": [
                {
                    "question": "Select features",
                    "header": "Features",
                    "options": [
                        {"label": "A", "description": "Feature A"},
                        {"label": "B", "description": "Feature B"}
                    ],
                    "multiSelect": true
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        assert!(result.unwrap().questions[0].multi_select);
    }

    #[test]
    fn test_parse_ask_user_question_missing_multi_select_defaults() {
        let input = json!({
            "questions": [
                {
                    "question": "Test?",
                    "header": "Test",
                    "options": []
                }
            ]
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        assert!(!result.unwrap().questions[0].multi_select);
    }

    #[test]
    fn test_parse_ask_user_question_with_answers() {
        let input = json!({
            "questions": [
                {
                    "question": "Test?",
                    "header": "Test",
                    "options": [{"label": "A", "description": "a"}],
                    "multiSelect": false
                }
            ],
            "answers": {"q1": "answer1"}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.answers.get("q1"), Some(&"answer1".to_string()));
    }

    #[test]
    fn test_parse_ask_user_question_multiple_questions() {
        let input = json!({
            "questions": [
                {
                    "question": "First?",
                    "header": "Q1",
                    "options": [{"label": "A", "description": "a"}],
                    "multiSelect": false
                },
                {
                    "question": "Second?",
                    "header": "Q2",
                    "options": [{"label": "B", "description": "b"}],
                    "multiSelect": true
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.questions.len(), 2);
        assert_eq!(data.questions[0].header, "Q1");
        assert_eq!(data.questions[1].header, "Q2");
    }

    #[test]
    fn test_parse_ask_user_question_invalid_missing_questions() {
        let input = json!({
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_wrong_type() {
        let input = json!({
            "questions": "not an array",
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_empty_object() {
        let input = json!({});

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_null() {
        let input = json!(null);

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_invalid_missing_required_question_fields() {
        let input = json!({
            "questions": [
                {
                    "header": "Test"
                    // missing "question" and "options"
                }
            ],
            "answers": {}
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ask_user_question_completely_unrelated_json() {
        let input = json!({
            "command": "npm install",
            "file_path": "/some/path"
        });

        let result = parse_ask_user_question(&input);
        assert!(result.is_none());
    }

    // ========================================================================
    // Permission Box Responsive Tests
    // ========================================================================

    #[test]
    fn test_permission_box_width_normal() {
        let ctx = LayoutContext::new(120, 40);
        let area = Rect::new(0, 0, 120, 40);

        // For normal width, box should be 60 chars (DEFAULT_PERMISSION_BOX_WIDTH)
        let available = area.width.saturating_sub(4);
        let expected_width = DEFAULT_PERMISSION_BOX_WIDTH.min(available);

        assert_eq!(expected_width, 60);
        // Verify ctx is not narrow or extra small
        assert!(!ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_width_narrow() {
        let ctx = LayoutContext::new(70, 24);

        // Narrow terminals should scale down
        let scaled = ctx.bounded_width(70, MIN_PERMISSION_BOX_WIDTH, DEFAULT_PERMISSION_BOX_WIDTH);

        // 70% of 70 = 49, clamped between 30 and 60
        assert!(scaled >= MIN_PERMISSION_BOX_WIDTH);
        assert!(scaled <= DEFAULT_PERMISSION_BOX_WIDTH);
    }

    #[test]
    fn test_permission_box_width_extra_small() {
        let ctx = LayoutContext::new(50, 24);

        // Extra small should use minimum width
        let available = 50u16.saturating_sub(4);
        let expected_width = MIN_PERMISSION_BOX_WIDTH.min(available);

        assert_eq!(expected_width, MIN_PERMISSION_BOX_WIDTH);
        assert!(ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_title_changes_on_narrow() {
        // On normal width, title is " Permission Required "
        // On narrow width, title is " Permission "
        let normal_ctx = LayoutContext::new(120, 40);
        let narrow_ctx = LayoutContext::new(70, 24);

        assert!(!normal_ctx.is_narrow());
        assert!(narrow_ctx.is_narrow());
    }

    #[test]
    fn test_permission_box_preview_hidden_on_short() {
        let ctx = LayoutContext::new(80, 20);

        // SM_HEIGHT is 24, so height < 24 means is_short() returns true
        assert!(ctx.is_short()); // 20 < 24

        // Preview should be hidden on short terminals
        let show_preview = !ctx.is_short();
        assert!(!show_preview, "Preview should be hidden on short terminals");
    }

    #[test]
    fn test_permission_box_keyboard_options_normal() {
        // Normal: [y] Yes  [a] Always  [n] No
        let ctx = LayoutContext::new(120, 40);
        assert!(!ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_keyboard_options_narrow() {
        // Narrow: [y] Y  [a] A  [n] N
        let ctx = LayoutContext::new(70, 24);
        assert!(ctx.is_narrow());
        assert!(!ctx.is_extra_small());
    }

    #[test]
    fn test_permission_box_keyboard_options_extra_small() {
        // Extra small: [y]/[a]/[n]
        let ctx = LayoutContext::new(50, 24);
        assert!(ctx.is_extra_small());
    }
}
