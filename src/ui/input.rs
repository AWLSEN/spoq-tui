//! Input area rendering
//!
//! Implements the input box, keybind hints, and permission prompt UI.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Screen};
use crate::state::session::{AskUserQuestionData, AskUserQuestionState, PermissionRequest};
use crate::widgets::input_box::InputBoxWidget;

use super::theme::{COLOR_ACCENT, COLOR_BORDER, COLOR_DIM, COLOR_HEADER};

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
// Input Area
// ============================================================================

pub fn render_input_area(frame: &mut Frame, area: Rect, app: &App) {
    // Input is always "focused" since we removed panel focus cycling
    let input_focused = true;
    let border_color = if input_focused { COLOR_HEADER } else { COLOR_BORDER };

    let input_outer = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(input_outer, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Input box (needs 5 for border + multi-line content)
            Constraint::Length(1), // Keybinds
        ])
        .split(inner);

    // Render the InputBox widget with blinking cursor (never streaming on CommandDeck)
    let input_widget = InputBoxWidget::new(&app.input_box, "", input_focused)
        .with_tick(app.tick_count);
    frame.render_widget(input_widget, input_chunks[0]);

    // Build contextual keybind hints
    let keybinds = build_contextual_keybinds(app);

    let keybinds_widget = Paragraph::new(keybinds);
    frame.render_widget(keybinds_widget, input_chunks[1]);
}

/// Render the input area for conversation screen
pub fn render_conversation_input(frame: &mut Frame, area: Rect, app: &App) {
    // Input is always "focused" since we removed panel focus cycling
    let input_focused = true;
    let is_streaming = app.is_streaming();
    let border_color = if input_focused { COLOR_HEADER } else { COLOR_BORDER };

    let input_outer = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(input_outer, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Input box
            Constraint::Length(1), // Keybinds
        ])
        .split(inner);

    // Render the InputBox widget with appropriate border style and blinking cursor
    let input_widget = if is_streaming {
        InputBoxWidget::dashed(&app.input_box, "", input_focused)
            .with_tick(app.tick_count)
    } else {
        InputBoxWidget::new(&app.input_box, "", input_focused)
            .with_tick(app.tick_count)
    };
    frame.render_widget(input_widget, input_chunks[0]);

    // Build contextual keybind hints
    let keybinds = build_contextual_keybinds(app);

    let keybinds_widget = Paragraph::new(keybinds);
    frame.render_widget(keybinds_widget, input_chunks[1]);
}

/// Build contextual keybind hints based on application state
pub fn build_contextual_keybinds(app: &App) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];

    // Check for visible elements that need special keybinds
    let has_error = app.stream_error.is_some();

    // Always show basic navigation
    if app.screen == Screen::Conversation {
        if app.is_active_thread_programming() {
            // Programming thread: show mode cycling hint
            spans.push(Span::styled("[Shift+Tab]", Style::default().fg(COLOR_ACCENT)));
            spans.push(Span::raw(" cycle mode │ "));
        }

        if has_error {
            // Error visible: show dismiss hint
            spans.push(Span::styled("d", Style::default().fg(COLOR_ACCENT)));
            spans.push(Span::raw(": dismiss error │ "));
        }

        spans.push(Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" send │ "));

        spans.push(Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" back"));
    } else {
        // CommandDeck screen
        spans.push(Span::styled("[Tab Tab]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" switch thread │ "));

        spans.push(Span::styled("[Enter]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" send │ "));

        spans.push(Span::styled("[Esc]", Style::default().fg(COLOR_ACCENT)));
        spans.push(Span::raw(" back"));
    }

    Line::from(spans)
}

// ============================================================================
// Inline Permission Prompt
// ============================================================================

/// Render an inline permission prompt in the message flow.
///
/// Shows a Claude Code-style permission box with:
/// - Tool name and description
/// - Preview of the action (file path, command, etc.)
/// - Keyboard options: [y] Yes, [a] Always, [n] No
///
/// For AskUserQuestion tools, renders a special tabbed question UI instead.
pub fn render_permission_prompt(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(ref perm) = app.session_state.pending_permission {
        render_permission_box(frame, area, perm, &app.question_state);
    }
}

/// Render the permission request box.
///
/// For AskUserQuestion tools, delegates to `render_ask_user_question_box`.
/// For other tools, renders the standard permission prompt.
pub fn render_permission_box(
    frame: &mut Frame,
    area: Rect,
    perm: &PermissionRequest,
    question_state: &AskUserQuestionState,
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

    // Standard permission box for other tools
    // Calculate box dimensions - center in the given area
    // Need at least 12 lines: border(2) + tool(1) + empty(1) + preview(5) + empty(1) + options(1) + buffer(1)
    let box_width = 60u16.min(area.width.saturating_sub(4));
    let box_height = 14u16.min(area.height.saturating_sub(2));

    // Center the box
    let x = area.x + (area.width.saturating_sub(box_width)) / 2;
    let y = area.y + (area.height.saturating_sub(box_height)) / 2;

    let box_area = Rect {
        x,
        y,
        width: box_width,
        height: box_height,
    };

    // Create the permission box with border
    let block = Block::default()
        .title(Span::styled(
            " Permission Required ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));

    // Render the block first
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

    // Tool name line
    lines.push(Line::from(vec![
        Span::styled(
            format!("{}: ", perm.tool_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&perm.description, Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from("")); // Empty line

    // Preview box - show context or tool_input
    let preview_content = get_permission_preview(perm);
    if !preview_content.is_empty() {
        // Preview border top
        lines.push(Line::from(vec![
            Span::styled(
                format!("┌{}┐", "─".repeat((inner.width as usize).saturating_sub(2))),
                Style::default().fg(COLOR_DIM),
            ),
        ]));

        // Preview content (truncated if needed)
        let max_preview_width = (inner.width as usize).saturating_sub(4);
        for line in preview_content.lines().take(3) {
            let truncated = if line.len() > max_preview_width {
                format!("{}...", &line[..max_preview_width.saturating_sub(3)])
            } else {
                line.to_string()
            };
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(COLOR_DIM)),
                Span::styled(truncated, Style::default().fg(Color::Gray)),
                Span::raw(" "),
            ]));
        }

        // Preview border bottom
        lines.push(Line::from(vec![
            Span::styled(
                format!("└{}┘", "─".repeat((inner.width as usize).saturating_sub(2))),
                Style::default().fg(COLOR_DIM),
            ),
        ]));
    }

    lines.push(Line::from("")); // Empty line

    // Calculate timeout countdown (server times out at 55s)
    let elapsed_secs = perm.received_at.elapsed().as_secs();
    let remaining_secs = 55u64.saturating_sub(elapsed_secs);

    // Style countdown: yellow when <15s, red when <5s
    let countdown_color = if remaining_secs <= 5 {
        Color::Red
    } else if remaining_secs <= 15 {
        Color::Yellow
    } else {
        Color::Gray
    };

    let countdown_text = if remaining_secs == 0 {
        " (expired)".to_string()
    } else {
        format!(" ({}s)", remaining_secs)
    };

    // Keyboard options with countdown
    lines.push(Line::from(vec![
        Span::styled("[y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" Yes  "),
        Span::styled("[a]", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        Span::raw(" Always  "),
        Span::styled("[n]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" No"),
        Span::styled(countdown_text, Style::default().fg(countdown_color)),
    ]));

    let content = Paragraph::new(lines);
    frame.render_widget(content, inner);
}

/// Extract preview content from a PermissionRequest.
pub fn get_permission_preview(perm: &crate::state::session::PermissionRequest) -> String {
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
                return super::helpers::truncate_string(content, 100);
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
    help_spans.push(Span::styled(countdown_text, Style::default().fg(countdown_color)));

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
}
