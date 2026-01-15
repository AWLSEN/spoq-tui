//! Message rendering functions
//!
//! Implements the message area, tool events, thinking blocks, and error banners.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::markdown::render_markdown;
use crate::models::{Message, MessageRole, MessageSegment, ToolEvent, ToolEventStatus};
use crate::state::ToolDisplayStatus;

use super::helpers::{format_tool_args, get_tool_icon, inner_rect, MAX_VISIBLE_ERRORS, SPINNER_FRAMES};
use super::input::render_permission_prompt;
use super::theme::{
    COLOR_ACCENT, COLOR_ACTIVE, COLOR_DIM, COLOR_TOOL_ERROR, COLOR_TOOL_ICON,
    COLOR_TOOL_RUNNING, COLOR_TOOL_SUCCESS,
};

// ============================================================================
// Inline Error Banners
// ============================================================================

/// Render inline error banners for a thread
/// Returns the lines to be added to the messages area
pub fn render_inline_error_banners(app: &App) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    // Get errors for the active thread
    let errors = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_errors(id));

    let Some(errors) = errors else {
        return lines;
    };

    if errors.is_empty() {
        return lines;
    }

    let focused_index = app.cache.focused_error_index();
    let total_errors = errors.len();

    // Only show up to MAX_VISIBLE_ERRORS
    for (i, error) in errors.iter().take(MAX_VISIBLE_ERRORS).enumerate() {
        let is_focused = i == focused_index;
        let border_color = if is_focused { Color::Red } else { Color::DarkGray };
        let border_char_top = if is_focused { "═" } else { "─" };
        let border_char_bottom = if is_focused { "═" } else { "─" };

        // Top border with error code
        let header = format!("─[!] {} ", error.error_code);
        let remaining_width = 50_usize.saturating_sub(header.len());
        let top_border = format!(
            "┌{}{}┐",
            header,
            border_char_top.repeat(remaining_width)
        );
        lines.push(Line::from(Span::styled(
            top_border,
            Style::default().fg(border_color),
        )));

        // Error message line
        let msg_display = if error.message.len() > 46 {
            format!("{}...", &error.message[..43])
        } else {
            error.message.clone()
        };
        let msg_padding = 48_usize.saturating_sub(msg_display.len());
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(border_color)),
            Span::styled(msg_display, Style::default().fg(Color::White)),
            Span::styled(
                format!("{:>width$}│", "", width = msg_padding),
                Style::default().fg(border_color),
            ),
        ]));

        // Dismiss hint line
        let dismiss_text = "[d]ismiss";
        let dismiss_padding = 48_usize.saturating_sub(dismiss_text.len());
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(border_color)),
            Span::styled(
                format!("{:>width$}", "", width = dismiss_padding),
                Style::default().fg(border_color),
            ),
            Span::styled(
                dismiss_text,
                Style::default().fg(COLOR_DIM),
            ),
            Span::styled(" │", Style::default().fg(border_color)),
        ]));

        // Bottom border
        let bottom_border = format!(
            "└{}┘",
            border_char_bottom.repeat(48)
        );
        lines.push(Line::from(Span::styled(
            bottom_border,
            Style::default().fg(border_color),
        )));

        lines.push(Line::from(""));
    }

    // Show "+N more" if there are more errors
    if total_errors > MAX_VISIBLE_ERRORS {
        let more_count = total_errors - MAX_VISIBLE_ERRORS;
        lines.push(Line::from(vec![
            Span::styled(
                format!("  +{} more error{}", more_count, if more_count > 1 { "s" } else { "" }),
                Style::default().fg(Color::Red).add_modifier(Modifier::ITALIC),
            ),
        ]));
        lines.push(Line::from(""));
    }

    lines
}

// ============================================================================
// Thinking/Reasoning Block
// ============================================================================

/// Render a collapsible thinking block for assistant messages.
///
/// Collapsed: ▸ Thinking... (847 tokens)
/// Expanded:
/// ▾ Thinking
/// │ Let me analyze this step by step...
/// │ First, I need to understand the structure.
/// └──────────────────────────────────────────
pub fn render_thinking_block(
    message: &Message,
    tick_count: u64,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    // Only render for assistant messages with reasoning content
    if message.role != MessageRole::Assistant {
        return lines;
    }

    if message.reasoning_content.is_empty() {
        return lines;
    }

    let token_count = message.reasoning_token_count();
    let collapsed = message.reasoning_collapsed;

    // Determine the arrow and style based on collapsed state
    let (arrow, header_color) = if collapsed {
        ("▸", Color::Magenta)
    } else {
        ("▾", Color::Magenta)
    };

    // Header line
    if collapsed {
        // Collapsed: ▸ Thinking... (847 tokens)
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", arrow),
                Style::default().fg(header_color),
            ),
            Span::styled(
                "Thinking...",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC),
            ),
            Span::styled(
                format!(" ({} tokens)", token_count),
                Style::default().fg(COLOR_DIM),
            ),
            Span::styled(
                "  [t] toggle",
                Style::default().fg(COLOR_DIM),
            ),
        ]));
    } else {
        // Expanded header: ▾ Thinking
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", arrow),
                Style::default().fg(header_color),
            ),
            Span::styled(
                "Thinking",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({} tokens)", token_count),
                Style::default().fg(COLOR_DIM),
            ),
            Span::styled(
                "  [t] toggle",
                Style::default().fg(COLOR_DIM),
            ),
        ]));

        // Render the reasoning content with box-drawing border
        let content = &message.reasoning_content;
        for line in content.lines() {
            lines.push(Line::from(vec![
                Span::styled(
                    "│ ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        // If streaming, add a blinking cursor at the end
        if message.is_streaming {
            let show_cursor = (tick_count / 5) % 2 == 0;
            if show_cursor {
                lines.push(Line::from(vec![
                    Span::styled(
                        "│ █",
                        Style::default().fg(Color::Magenta),
                    ),
                ]));
            }
        }

        // Bottom border
        lines.push(Line::from(vec![
            Span::styled(
                "└──────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    lines.push(Line::from("")); // Add spacing after thinking block

    lines
}

// ============================================================================
// Tool Event Rendering
// ============================================================================

/// Render a single tool event as a Line
///
/// Uses tool-specific icons, color-coded status indicators, and formatted arguments
/// to provide rich visual feedback about tool execution status.
///
/// # Display format
/// - Running:  `[icon] [spinner] [tool_name]: [args_display]` (gray)
/// - Complete: `[icon] ✓ [tool_name]: [args_display] (duration)` (green)
/// - Failed:   `[icon] ✗ [tool_name]: [args_display]` (red)
pub fn render_tool_event(event: &ToolEvent, tick_count: u64) -> Line<'static> {
    // Get the appropriate icon for this tool
    let icon = get_tool_icon(&event.function_name);

    // Format the arguments display
    // Use pre-computed args_display if available, otherwise format from JSON
    let args_display = event.args_display.clone().unwrap_or_else(|| {
        format_tool_args(&event.function_name, &event.args_json)
    });

    match event.status {
        ToolEventStatus::Running => {
            // Animated spinner - cycle through frames ~100ms per frame (assuming 10 ticks/sec)
            let frame_index = (tick_count % 10) as usize;
            let spinner = SPINNER_FRAMES[frame_index];
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(COLOR_TOOL_ICON),
                ),
                Span::styled(
                    format!("{} ", spinner),
                    Style::default().fg(COLOR_TOOL_RUNNING),
                ),
                Span::styled(
                    format!("{}: ", event.function_name),
                    Style::default().fg(COLOR_TOOL_RUNNING),
                ),
                Span::styled(
                    args_display,
                    Style::default().fg(COLOR_TOOL_RUNNING),
                ),
            ])
        }
        ToolEventStatus::Complete => {
            let duration_str = event.duration_secs
                .map(|d| format!(" ({:.1}s)", d))
                .unwrap_or_default();
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(COLOR_TOOL_ICON),
                ),
                Span::styled(
                    "✓ ",
                    Style::default().fg(COLOR_TOOL_SUCCESS),
                ),
                Span::styled(
                    format!("{}: ", event.function_name),
                    Style::default().fg(COLOR_TOOL_SUCCESS),
                ),
                Span::styled(
                    args_display,
                    Style::default().fg(COLOR_TOOL_SUCCESS),
                ),
                Span::styled(
                    duration_str,
                    Style::default().fg(COLOR_TOOL_RUNNING),
                ),
            ])
        }
        ToolEventStatus::Failed => {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(COLOR_TOOL_ICON),
                ),
                Span::styled(
                    "✗ ",
                    Style::default().fg(COLOR_TOOL_ERROR),
                ),
                Span::styled(
                    format!("{}: ", event.function_name),
                    Style::default().fg(COLOR_TOOL_ERROR),
                ),
                Span::styled(
                    args_display,
                    Style::default().fg(COLOR_TOOL_ERROR),
                ),
            ])
        }
    }
}

/// Render a result preview line for a tool event
///
/// Returns an indented, dim-colored line showing a truncated preview of the tool result.
/// Success results are shown in dim gray, error results in red.
///
/// # Returns
/// - `None` if the tool has no result preview or the preview is empty
/// - `Some(Line)` with the formatted, truncated preview
pub fn render_tool_result_preview(tool: &ToolEvent) -> Option<Line<'static>> {
    // Return None if no result preview
    let preview = tool.result_preview.as_ref()?;

    // Return None if preview is empty
    if preview.trim().is_empty() {
        return None;
    }

    // Truncate the preview:
    // - Find first 2 newlines or ~150 chars, whichever comes first
    // - Append '...' if truncated
    let truncated = truncate_preview(preview, 150, 2);

    // Choose color based on error state
    let color = if tool.result_is_error {
        COLOR_TOOL_ERROR
    } else {
        Color::Rgb(100, 100, 100) // dim gray for success
    };

    Some(Line::from(vec![
        Span::styled("    ", Style::default()), // 4 spaces indentation
        Span::styled(truncated, Style::default().fg(color)),
    ]))
}

/// Truncate a preview string to fit display constraints
///
/// Limits output to `max_chars` characters or `max_lines` newlines, whichever is reached first.
/// Replaces newlines with spaces for single-line display and appends "..." if truncated.
pub fn truncate_preview(text: &str, max_chars: usize, max_lines: usize) -> String {
    let mut result = String::new();
    let mut char_count = 0;
    let mut line_count = 0;
    let mut truncated = false;

    for ch in text.chars() {
        if ch == '\n' {
            line_count += 1;
            if line_count >= max_lines {
                truncated = true;
                break;
            }
            // Replace newline with space for single-line display
            result.push(' ');
            char_count += 1;
        } else {
            result.push(ch);
            char_count += 1;
        }

        if char_count >= max_chars {
            truncated = true;
            break;
        }
    }

    // Check if there's more content after where we stopped
    if !truncated && char_count < text.chars().count() {
        truncated = true;
    }

    if truncated {
        // Trim trailing whitespace before adding ellipsis
        let trimmed = result.trim_end();
        format!("{}...", trimmed)
    } else {
        result.trim_end().to_string()
    }
}

// ============================================================================
// Legacy Tool Status Functions (kept for potential future use)
// ============================================================================

/// Render tool status indicators inline (LEGACY - kept for potential future use)
/// Shows: ◐ Reading src/main.rs...  (executing, with spinner)
///        ✓ Read complete           (success, fades after 30 ticks)
///        ✗ Write failed: error     (failure, persists)
/// Note: Tool events are now rendered inline with messages via render_tool_event()
#[allow(dead_code)]
pub fn render_tool_status_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    // Get tools that should be rendered at current tick
    let tools = app.tool_tracker.tools_to_render(app.tick_count);

    if tools.is_empty() {
        return lines;
    }

    for (_tool_id, state) in tools {
        let Some(ref display_status) = state.display_status else {
            continue;
        };

        let line = match display_status {
            ToolDisplayStatus::Started { .. } | ToolDisplayStatus::Executing { .. } => {
                // Animate spinner based on tick count
                let spinner_idx = (app.tick_count % 10) as usize;
                let spinner = SPINNER_FRAMES[spinner_idx];
                let text = display_status.display_text();

                Line::from(vec![
                    Span::styled(
                        format!("  {} ", spinner),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        text,
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            }
            ToolDisplayStatus::Completed { success, summary, .. } => {
                if *success {
                    Line::from(vec![
                        Span::styled(
                            "  ✓ ",
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            summary.clone(),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(
                            "  ✗ ",
                            Style::default().fg(Color::Red),
                        ),
                        Span::styled(
                            summary.clone(),
                            Style::default().fg(Color::Red),
                        ),
                    ])
                }
            }
        };

        lines.push(line);
    }

    if !lines.is_empty() {
        lines.push(Line::from("")); // Add spacing after tool status
    }

    lines
}

/// Render subagent status with spinner and progress (LEGACY - kept for potential future use)
/// UI design:
/// ```text
/// ┌ ◐ Exploring codebase structure
/// │   Found 5 relevant files...
/// └ ✓ Complete (8 tool calls)
/// ```
/// Note: Subagent status may be integrated inline in future iterations
#[allow(dead_code)]
pub fn render_subagent_status_lines(app: &App) -> Vec<Line<'static>> {
    use crate::state::SubagentDisplayStatus;

    let mut lines: Vec<Line> = Vec::new();

    // Get subagents that should be rendered at current tick
    let subagents = app.subagent_tracker.subagents_to_render(app.tick_count);

    if subagents.is_empty() {
        return lines;
    }

    for (_subagent_id, state) in subagents {
        // Render main line with appropriate prefix and spinner/checkmark
        let main_line = match &state.display_status {
            SubagentDisplayStatus::Started { description, .. } |
            SubagentDisplayStatus::Progress { description, .. } => {
                // Animate spinner based on tick count
                let spinner_idx = (app.tick_count % 10) as usize;
                let spinner = SPINNER_FRAMES[spinner_idx];

                Line::from(vec![
                    Span::styled(
                        format!("┌ {} ", spinner),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        description.clone(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            }
            SubagentDisplayStatus::Completed { success, summary, .. } => {
                let (prefix, color) = if *success {
                    ("└ ✓ ", Color::DarkGray)
                } else {
                    ("└ ✗ ", Color::Red)
                };

                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(color)),
                    Span::styled(summary.clone(), Style::default().fg(color)),
                ])
            }
        };
        lines.push(main_line);

        // Render progress line if we have a progress message (only for in-progress subagents)
        if let SubagentDisplayStatus::Progress { progress_message, .. } = &state.display_status {
            if !progress_message.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled(
                        "│   ",
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        progress_message.clone(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    }

    if !lines.is_empty() {
        lines.push(Line::from("")); // Add spacing after subagent status
    }

    lines
}

// ============================================================================
// Messages Area
// ============================================================================

/// Render the messages area with user messages and AI responses
pub fn render_messages_area(frame: &mut Frame, area: Rect, app: &App) {
    let inner = inner_rect(area, 1);
    let mut lines: Vec<Line> = Vec::new();

    // Show inline error banners for the thread
    lines.extend(render_inline_error_banners(app));

    // Note: Tool status is now rendered inline with messages via render_tool_event()
    // The legacy render_tool_status_lines and render_subagent_status_lines functions are kept
    // for potential future use but removed from the main render flow.

    // Show stream error banner if there's a stream error (legacy, for non-thread errors)
    if let Some(error) = &app.stream_error {
        lines.push(Line::from(vec![
            Span::styled(
                "  ⚠ ERROR: ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                error.as_str(),
                Style::default().fg(Color::Red),
            ),
        ]));
        lines.push(Line::from(vec![Span::styled(
            "═══════════════════════════════════════════════",
            Style::default().fg(Color::Red),
        )]));
    }

    // Get messages from cache if we have an active thread
    let cached_messages = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_messages(id));

    if let Some(messages) = cached_messages {
        for message in messages {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "───────────────────────────────────────────────",
                Style::default().fg(COLOR_DIM),
            )]));

            // Render thinking/reasoning block for assistant messages (before content)
            if message.role == MessageRole::Assistant {
                lines.extend(render_thinking_block(message, app.tick_count));
            }

            let (label, label_style) = match message.role {
                MessageRole::User => (
                    "You: ",
                    Style::default()
                        .fg(COLOR_ACTIVE)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::Assistant => (
                    "AI: ",
                    Style::default()
                        .fg(COLOR_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::System => (
                    "System: ",
                    Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
                ),
            };

            // Handle streaming vs completed messages
            if message.is_streaming {
                // Display streaming content with blinking cursor
                // Blink cursor every ~500ms (assuming 10 ticks/sec, toggle every 5 ticks)
                let show_cursor = (app.tick_count / 5) % 2 == 0;
                let cursor_span = Span::styled(
                    if show_cursor { "█" } else { " " },
                    Style::default().fg(COLOR_ACCENT),
                );

                // For assistant messages with segments, render segments in order (interleaved)
                // This shows text and tool events in the order they occurred
                if message.role == MessageRole::Assistant && !message.segments.is_empty() {
                    let mut is_first_line = true;

                    for segment in &message.segments {
                        match segment {
                            MessageSegment::Text(text) => {
                                let mut segment_lines = render_markdown(text);
                                if is_first_line && !segment_lines.is_empty() {
                                    // Prepend label to first line of first text segment
                                    let first_line = segment_lines.remove(0);
                                    let mut first_spans = vec![Span::styled(label, label_style)];
                                    first_spans.extend(first_line.spans);
                                    lines.push(Line::from(first_spans));
                                    is_first_line = false;
                                }
                                lines.extend(segment_lines);
                            }
                            MessageSegment::ToolEvent(event) => {
                                if is_first_line {
                                    // No text before first tool event, show label first
                                    lines.push(Line::from(vec![Span::styled(label, label_style)]));
                                    is_first_line = false;
                                }
                                lines.push(render_tool_event(event, app.tick_count));

                                // Add result preview if available (only for completed tools)
                                if event.duration_secs.is_some() {
                                    if let Some(preview_line) = render_tool_result_preview(event) {
                                        lines.push(preview_line);
                                    }
                                }
                            }
                        }
                    }

                    // If we never added any content, show label with cursor
                    if is_first_line {
                        lines.push(Line::from(vec![
                            Span::styled(label, label_style),
                            cursor_span,
                        ]));
                    } else {
                        // Append cursor to last line
                        if let Some(last_pushed) = lines.last_mut() {
                            last_pushed.spans.push(cursor_span);
                        }
                    }
                } else {
                    // Fall back to partial_content for backward compatibility
                    // (non-assistant messages or when segments is empty)
                    let mut content_lines = render_markdown(&message.partial_content);

                    // Add label to first line, append cursor to last line
                    if content_lines.is_empty() {
                        // No content yet, just show label with cursor
                        lines.push(Line::from(vec![
                            Span::styled(label, label_style),
                            cursor_span,
                        ]));
                    } else {
                        // Prepend label to first line
                        let first_line = content_lines.remove(0);
                        let mut first_spans = vec![Span::styled(label, label_style)];
                        first_spans.extend(first_line.spans);
                        lines.push(Line::from(first_spans));

                        // Add middle lines as-is
                        for line in content_lines.drain(..content_lines.len().saturating_sub(1)) {
                            lines.push(line);
                        }

                        // Append cursor to last line (if there are remaining lines)
                        if let Some(last_line) = content_lines.pop() {
                            let mut last_spans = last_line.spans;
                            last_spans.push(cursor_span);
                            lines.push(Line::from(last_spans));
                        } else {
                            // Only had one line, cursor was not added yet
                            // The first line is already pushed, so add cursor separately
                            // Actually, we need to modify the last pushed line
                            if let Some(last_pushed) = lines.last_mut() {
                                last_pushed.spans.push(cursor_span);
                            }
                        }
                    }
                }
            } else {
                // Display completed message with interleaved text and tool events
                // For assistant messages with segments, render segments in order
                if message.role == MessageRole::Assistant && !message.segments.is_empty() {
                    let mut is_first_line = true;

                    for segment in &message.segments {
                        match segment {
                            MessageSegment::Text(text) => {
                                let mut segment_lines = render_markdown(text);
                                if is_first_line && !segment_lines.is_empty() {
                                    // Prepend label to first line of first text segment
                                    let first_line = segment_lines.remove(0);
                                    let mut first_spans = vec![Span::styled(label, label_style)];
                                    first_spans.extend(first_line.spans);
                                    lines.push(Line::from(first_spans));
                                    is_first_line = false;
                                }
                                lines.extend(segment_lines);
                            }
                            MessageSegment::ToolEvent(event) => {
                                if is_first_line {
                                    // No text before first tool event, show label first
                                    lines.push(Line::from(vec![Span::styled(label, label_style)]));
                                    is_first_line = false;
                                }
                                lines.push(render_tool_event(event, app.tick_count));

                                // Add result preview if available (only for completed tools)
                                if event.duration_secs.is_some() {
                                    if let Some(preview_line) = render_tool_result_preview(event) {
                                        lines.push(preview_line);
                                    }
                                }
                            }
                        }
                    }

                    // If we never added any content, show just the label
                    if is_first_line {
                        lines.push(Line::from(vec![Span::styled(label, label_style)]));
                    }
                } else {
                    // Fall back to content field for non-assistant messages or empty segments
                    let content_lines = render_markdown(&message.content);

                    if content_lines.is_empty() {
                        // Empty content, just show label
                        lines.push(Line::from(vec![Span::styled(label, label_style)]));
                    } else {
                        // Prepend label to first line
                        let mut iter = content_lines.into_iter();
                        if let Some(first_line) = iter.next() {
                            let mut first_spans = vec![Span::styled(label, label_style)];
                            first_spans.extend(first_line.spans);
                            lines.push(Line::from(first_spans));
                        }

                        // Add remaining lines as-is
                        for line in iter {
                            lines.push(line);
                        }
                    }
                }
            }

            lines.push(Line::from(""));
        }
    } else {
        // No messages yet - show placeholder
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "───────────────────────────────────────────────",
            Style::default().fg(COLOR_DIM),
        )]));
        lines.push(Line::from(vec![
            Span::styled(
                "AI: ",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Waiting for your message...", Style::default().fg(COLOR_DIM)),
        ]));
        lines.push(Line::from(""));
    }

    // Calculate content height for scroll bounds
    // With word wrap enabled, we need to estimate wrapped line count
    let viewport_height = inner.height as usize;
    let viewport_width = inner.width as usize;

    // Estimate total lines after wrapping
    let mut total_lines: usize = 0;
    for line in &lines {
        let line_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        if line_width == 0 {
            total_lines += 1; // Empty line
        } else {
            // Estimate wrapped lines (ceil division)
            total_lines += (line_width + viewport_width - 1) / viewport_width.max(1);
        }
    }

    // Calculate max scroll (how far up we can scroll from bottom)
    // scroll=0 means showing the bottom (latest content)
    // scroll=max means showing the top (oldest content)
    let max_scroll = total_lines.saturating_sub(viewport_height) as u16;

    // Clamp user's scroll to valid range
    let clamped_scroll = app.conversation_scroll.min(max_scroll);

    // Convert from "scroll from bottom" to ratatui's "scroll from top"
    // If user_scroll=0, show bottom → actual_scroll = max_scroll
    // If user_scroll=max, show top → actual_scroll = 0
    let actual_scroll = max_scroll.saturating_sub(clamped_scroll);

    let messages_widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((actual_scroll, 0));
    frame.render_widget(messages_widget, inner);

    // Render inline permission prompt if pending (overlays on top of messages)
    if app.session_state.has_pending_permission() {
        render_permission_prompt(frame, inner, app);
    }
}
