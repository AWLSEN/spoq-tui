//! Message rendering functions
//!
//! Implements the message area, tool events, thinking blocks, and error banners.
//! Uses `LayoutContext` for responsive layout calculations.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::markdown::MarkdownCache;
use crate::models::{Message, MessageRole, MessageSegment, SubagentEvent, SubagentEventStatus, ToolEvent, ToolEventStatus};
use crate::state::ToolDisplayStatus;

use super::helpers::{format_tool_args, get_subagent_icon, get_tool_icon, inner_rect, MAX_VISIBLE_ERRORS, SPINNER_FRAMES};
use super::input::render_permission_prompt;
use super::layout::LayoutContext;
use super::theme::{
    COLOR_ACCENT, COLOR_DIM, COLOR_SUBAGENT_COMPLETE, COLOR_SUBAGENT_RUNNING,
    COLOR_TOOL_ERROR, COLOR_TOOL_ICON, COLOR_TOOL_RUNNING, COLOR_TOOL_SUCCESS,
};

// ============================================================================
// Inline Error Banners
// ============================================================================

/// Render inline error banners for a thread
///
/// Uses `LayoutContext` for responsive banner width that adapts to terminal size.
/// Returns the lines to be added to the messages area
pub fn render_inline_error_banners(app: &App, ctx: &LayoutContext) -> Vec<Line<'static>> {
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

    // Calculate responsive error box width based on terminal width
    // Use 80% of terminal width, clamped between 40 and 80
    let box_width = ctx.bounded_width(80, 40, 80) as usize;
    let inner_width = box_width.saturating_sub(2); // Account for border chars

    // Only show up to MAX_VISIBLE_ERRORS
    for (i, error) in errors.iter().take(MAX_VISIBLE_ERRORS).enumerate() {
        let is_focused = i == focused_index;
        let border_color = if is_focused { Color::Red } else { Color::DarkGray };
        let border_char_top = if is_focused { "═" } else { "─" };
        let border_char_bottom = if is_focused { "═" } else { "─" };

        // Top border with error code
        let header = format!("─[!] {} ", error.error_code);
        let remaining_width = inner_width.saturating_sub(header.len());
        let top_border = format!(
            "┌{}{}┐",
            header,
            border_char_top.repeat(remaining_width)
        );
        lines.push(Line::from(Span::styled(
            top_border,
            Style::default().fg(border_color),
        )));

        // Error message line - truncate based on responsive width
        let max_msg_len = inner_width.saturating_sub(4); // Account for borders and padding
        let msg_display = if error.message.len() > max_msg_len {
            super::helpers::truncate_string(&error.message, max_msg_len)
        } else {
            error.message.clone()
        };
        let msg_padding = inner_width.saturating_sub(msg_display.len() + 2); // +2 for "│ " prefix
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(border_color)),
            Span::styled(msg_display, Style::default().fg(Color::White)),
            Span::styled(
                format!("{:>width$}│", "", width = msg_padding),
                Style::default().fg(border_color),
            ),
        ]));

        // Dismiss hint line - abbreviate on narrow terminals
        let dismiss_text = if ctx.is_extra_small() {
            "[d]"
        } else {
            "[d]ismiss"
        };
        let dismiss_padding = inner_width.saturating_sub(dismiss_text.len() + 2);
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
            border_char_bottom.repeat(inner_width)
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
/// Uses `LayoutContext` for responsive border width.
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
    ctx: &LayoutContext,
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

    // Calculate responsive bottom border width (use available content width)
    let border_width = ctx.text_wrap_width(0).min(80) as usize;

    // Header line - abbreviate toggle hint on narrow terminals
    let toggle_hint = if ctx.is_extra_small() {
        " [t]"
    } else {
        "  [t] toggle"
    };

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
                toggle_hint,
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
                toggle_hint,
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
            let show_cursor = (tick_count / 5).is_multiple_of(2);
            if show_cursor {
                lines.push(Line::from(vec![
                    Span::styled(
                        "│ █",
                        Style::default().fg(Color::Magenta),
                    ),
                ]));
            }
        }

        // Bottom border - responsive width
        lines.push(Line::from(vec![
            Span::styled(
                format!("└{}", "─".repeat(border_width.saturating_sub(1))),
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
/// Uses `LayoutContext` for responsive args display truncation.
///
/// # Display format
/// - Running:  `[icon] [spinner] [tool_name]: [args_display]` (gray)
/// - Complete: `[icon] ✓ [tool_name]: [args_display] (duration)` (green)
/// - Failed:   `[icon] ✗ [tool_name]: [args_display]` (red)
pub fn render_tool_event(event: &ToolEvent, tick_count: u64, ctx: &LayoutContext) -> Line<'static> {
    // Get the appropriate icon for this tool
    let icon = get_tool_icon(&event.function_name);

    // Format the arguments display
    // Use pre-computed args_display if available, otherwise format from JSON
    let args_display = event.args_display.clone().unwrap_or_else(|| {
        format_tool_args(&event.function_name, &event.args_json)
    });

    // Calculate responsive max length for args display
    // Account for icon (2), spinner (2), tool name (~15), status (2), and padding
    let max_args_len = ctx.text_wrap_width(0).saturating_sub(25) as usize;
    let args_display = if args_display.len() > max_args_len && max_args_len > 3 {
        super::helpers::truncate_string(&args_display, max_args_len)
    } else {
        args_display
    };

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

            // Check if tool result was an error (e.g., permission denied)
            // Use dimmed style for failed/denied tools
            if event.result_is_error {
                Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        format!("{} ", icon),
                        Style::default().fg(COLOR_DIM),
                    ),
                    Span::styled(
                        "✗ ",
                        Style::default().fg(COLOR_DIM),
                    ),
                    Span::styled(
                        format!("{}: ", event.function_name),
                        Style::default().fg(COLOR_DIM),
                    ),
                    Span::styled(
                        args_display,
                        Style::default().fg(COLOR_DIM),
                    ),
                    Span::styled(
                        duration_str,
                        Style::default().fg(COLOR_DIM),
                    ),
                ])
            } else {
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
/// # Arguments
/// * `tool` - The tool event to render a preview for
/// * `max_preview_len` - Maximum length for the preview text (responsive to terminal width)
///
/// # Returns
/// - `None` if the tool has no result preview or the preview is empty
/// - `Some(Line)` with the formatted, truncated preview
pub fn render_tool_result_preview(tool: &ToolEvent, max_preview_len: usize) -> Option<Line<'static>> {
    // Return None if no result preview
    let preview = tool.result_preview.as_ref()?;

    // Return None if preview is empty
    if preview.trim().is_empty() {
        return None;
    }

    // Truncate the preview using responsive max length:
    // - Find first 2 newlines or max_preview_len chars, whichever comes first
    // - Append '...' if truncated
    let truncated = truncate_preview(preview, max_preview_len, 2);

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
// Subagent Event Rendering
// ============================================================================

/// Tree connector for subagent display
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TreeConnector {
    /// Single item (no tree structure needed)
    Single,
    /// Non-last item in a group: ├──
    Branch,
    /// Last item in a group: └──
    LastBranch,
}

impl TreeConnector {
    /// Get the string representation of the tree connector
    pub fn as_str(&self) -> &'static str {
        match self {
            TreeConnector::Single => "● ",
            TreeConnector::Branch => "├── ",
            TreeConnector::LastBranch => "└── ",
        }
    }
}

/// Render a single subagent event as a Line with optional tree connector
///
/// Uses subagent-specific icons, color-coded status indicators, and tree connectors
/// to provide rich visual feedback about subagent execution status.
///
/// Uses `LayoutContext` for responsive description and summary truncation.
///
/// # Display format
/// - Running:  `[connector] [spinner] Task(description)` (cyan)
/// - Complete: `[connector] Done (N tool uses · summary)` (green)
pub fn render_subagent_event(
    event: &SubagentEvent,
    tick_count: u64,
    connector: TreeConnector,
    ctx: &LayoutContext,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let icon = get_subagent_icon(&event.subagent_type);
    let connector_str = connector.as_str();

    // Calculate responsive truncation lengths based on terminal width
    // Account for connector (~5), spinner (2), icon (2), padding, and chrome
    let max_description_len = ctx.text_wrap_width(0).saturating_sub(20) as usize;
    let max_summary_len = if ctx.is_extra_small() {
        25
    } else if ctx.is_narrow() {
        35
    } else {
        60
    };

    match event.status {
        SubagentEventStatus::Running => {
            // Animated spinner - cycle through frames
            let frame_index = (tick_count % 10) as usize;
            let spinner = SPINNER_FRAMES[frame_index];

            // Truncate description if needed
            let description = if event.description.len() > max_description_len && max_description_len > 3 {
                super::helpers::truncate_string(&event.description, max_description_len)
            } else {
                event.description.clone()
            };

            // Main line: connector + spinner + Task(description)
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    connector_str,
                    Style::default().fg(COLOR_SUBAGENT_RUNNING),
                ),
                Span::styled(
                    format!("{} ", spinner),
                    Style::default().fg(COLOR_SUBAGENT_RUNNING),
                ),
                Span::styled(
                    format!("{} Task(", icon),
                    Style::default().fg(COLOR_SUBAGENT_RUNNING),
                ),
                Span::styled(
                    description,
                    Style::default().fg(COLOR_SUBAGENT_RUNNING),
                ),
                Span::styled(
                    ")",
                    Style::default().fg(COLOR_SUBAGENT_RUNNING),
                ),
            ]));

            // Progress line if available
            if let Some(ref progress) = event.progress_message {
                let indent = if connector == TreeConnector::LastBranch || connector == TreeConnector::Single {
                    "      " // No continuation line
                } else {
                    "  │   " // Continuation line for non-last items
                };
                // Truncate progress message for narrow terminals
                let progress_text = if progress.len() > max_description_len && max_description_len > 3 {
                    super::helpers::truncate_string(progress, max_description_len)
                } else {
                    progress.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled(indent, Style::default().fg(COLOR_SUBAGENT_RUNNING)),
                    Span::styled(
                        progress_text,
                        Style::default().fg(COLOR_DIM),
                    ),
                ]));
            }
        }
        SubagentEventStatus::Complete => {
            // Format: connector + Done (N tool uses · summary) or just (N tool uses)
            let tool_count_str = if event.tool_call_count == 1 {
                "1 tool use".to_string()
            } else {
                format!("{} tool uses", event.tool_call_count)
            };

            let display_text = if let Some(ref summary) = event.summary {
                // Truncate summary using responsive max length
                let truncated_summary = if summary.len() > max_summary_len {
                    super::helpers::truncate_string(summary, max_summary_len)
                } else {
                    summary.clone()
                };
                format!("Done ({} · {})", tool_count_str, truncated_summary)
            } else {
                format!("Done ({})", tool_count_str)
            };

            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    connector_str,
                    Style::default().fg(COLOR_SUBAGENT_COMPLETE),
                ),
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(COLOR_SUBAGENT_COMPLETE),
                ),
                Span::styled(
                    display_text,
                    Style::default().fg(COLOR_SUBAGENT_COMPLETE),
                ),
            ]));
        }
    }

    lines
}

/// Render a block of consecutive subagent events with proper tree connectors
///
/// When multiple subagent events are adjacent (indicating parallel execution),
/// this function renders them with Claude Code CLI-style tree connectors:
/// - Single subagent: ● Task(description)
/// - Multiple parallel: ├── for non-last, └── for last
///
/// Uses `LayoutContext` for responsive text truncation.
pub fn render_subagent_events_block(
    events: &[&SubagentEvent],
    tick_count: u64,
    ctx: &LayoutContext,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if events.is_empty() {
        return lines;
    }

    let count = events.len();

    for (i, event) in events.iter().enumerate() {
        let connector = if count == 1 {
            TreeConnector::Single
        } else if i == count - 1 {
            TreeConnector::LastBranch
        } else {
            TreeConnector::Branch
        };

        lines.extend(render_subagent_event(event, tick_count, connector, ctx));
    }

    lines
}

/// Render message segments, grouping consecutive subagent events for proper tree connectors
///
/// This function processes segments in order, but groups consecutive SubagentEvent segments
/// to render them with proper tree connectors (├── └── for parallel agents).
///
/// Uses `LayoutContext` for responsive text truncation across all segment types.
///
/// # Arguments
/// * `segments` - The message segments to render
/// * `tick_count` - Current tick for animations
/// * `label` - Label prefix (e.g., "│ " for user messages)
/// * `label_style` - Style for the label
/// * `ctx` - Layout context for responsive sizing
/// * `markdown_cache` - Cache for markdown rendering
pub fn render_message_segments(
    segments: &[MessageSegment],
    tick_count: u64,
    label: &'static str,
    label_style: Style,
    ctx: &LayoutContext,
    markdown_cache: &mut MarkdownCache,
) -> (Vec<Line<'static>>, bool) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut is_first_line = true;
    let mut i = 0;

    // Helper to prepend vertical bar to a line
    let prepend_bar = |line: Line<'static>| -> Line<'static> {
        let mut spans = vec![Span::styled(label, label_style)];
        spans.extend(line.spans);
        Line::from(spans)
    };

    while i < segments.len() {
        match &segments[i] {
            MessageSegment::Text(text) => {
                let segment_lines = markdown_cache.render(text);
                // Prepend vertical bar to ALL text lines
                for line in segment_lines {
                    lines.push(prepend_bar(line));
                }
                if !lines.is_empty() {
                    is_first_line = false;
                }
                i += 1;
            }
            MessageSegment::ToolEvent(event) => {
                // Prepend vertical bar to tool event line (with responsive args truncation)
                let tool_line = render_tool_event(event, tick_count, ctx);
                lines.push(prepend_bar(tool_line));
                is_first_line = false;

                // Add result preview if available (only for completed tools)
                // Use responsive max preview length from context
                if event.duration_secs.is_some() {
                    let max_preview_len = ctx.max_preview_length();
                    if let Some(preview_line) = render_tool_result_preview(event, max_preview_len) {
                        lines.push(prepend_bar(preview_line));
                    }
                }
                i += 1;
            }
            MessageSegment::SubagentEvent(_) => {
                // Collect consecutive subagent events
                let mut subagent_events: Vec<&SubagentEvent> = Vec::new();
                while i < segments.len() {
                    if let MessageSegment::SubagentEvent(event) = &segments[i] {
                        subagent_events.push(event);
                        i += 1;
                    } else {
                        break;
                    }
                }

                // Render the block with tree connectors (with responsive truncation), prepend bar to each line
                for line in render_subagent_events_block(&subagent_events, tick_count, ctx) {
                    lines.push(prepend_bar(line));
                }
                is_first_line = false;
            }
        }
    }

    (lines, is_first_line)
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
// Wrapped Line Estimation
// ============================================================================

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
// Message Virtualization
// ============================================================================

/// Number of messages to render as buffer above and below the visible viewport.
/// This provides smooth scrolling when messages come into view.
const VIRTUALIZATION_BUFFER: usize = 5;

/// Represents the height in visual lines of a single message.
/// Used for virtualization to determine which messages are visible.
#[derive(Debug, Clone)]
pub struct MessageHeight {
    /// Index of the message in the messages array
    #[allow(dead_code)]
    pub message_index: usize,
    /// Number of visual lines this message occupies (after wrapping)
    pub visual_lines: usize,
    /// Cumulative visual line offset from the start of all messages
    pub cumulative_offset: usize,
}

/// Calculate the visible range of message indices based on scroll position.
///
/// Returns (start_index, end_index) where:
/// - start_index is the first message to render (inclusive)
/// - end_index is the last message to render (exclusive)
///
/// The range includes a buffer of messages above and below the viewport
/// for smooth scrolling.
///
/// # Arguments
/// * `message_heights` - Pre-computed heights for each message
/// * `scroll_from_top` - The scroll offset from the top (in visual lines)
/// * `viewport_height` - The height of the viewport in visual lines
///
/// # Returns
/// (start_index, end_index) tuple defining which messages to render
pub fn calculate_visible_range(
    message_heights: &[MessageHeight],
    scroll_from_top: usize,
    viewport_height: usize,
) -> (usize, usize) {
    if message_heights.is_empty() {
        return (0, 0);
    }

    // Find the first message that starts within or after the visible range
    let mut start_index = 0;
    for (i, height) in message_heights.iter().enumerate() {
        let message_end = height.cumulative_offset + height.visual_lines;
        if message_end > scroll_from_top {
            start_index = i;
            break;
        }
        start_index = i + 1;
    }

    // Apply buffer to start (go back N messages)
    start_index = start_index.saturating_sub(VIRTUALIZATION_BUFFER);

    // Find the first message that starts after the visible range
    let visible_end = scroll_from_top + viewport_height;
    let mut end_index = message_heights.len();
    for (i, height) in message_heights.iter().enumerate() {
        if height.cumulative_offset >= visible_end {
            end_index = i;
            break;
        }
    }

    // Apply buffer to end (render N more messages)
    end_index = (end_index + VIRTUALIZATION_BUFFER).min(message_heights.len());

    (start_index, end_index)
}

/// Calculate the number of visual lines to skip when rendering virtualized messages.
///
/// When we skip messages at the beginning, we need to tell ratatui how many
/// visual lines those skipped messages would have occupied, so the scroll
/// position remains correct.
///
/// # Arguments
/// * `message_heights` - Pre-computed heights for each message
/// * `start_index` - The first message index we're rendering
///
/// # Returns
/// The number of visual lines occupied by messages before start_index
pub fn calculate_skip_lines(message_heights: &[MessageHeight], start_index: usize) -> usize {
    if start_index == 0 || message_heights.is_empty() {
        return 0;
    }

    // The cumulative offset of the first rendered message is exactly
    // how many lines we've skipped
    message_heights
        .get(start_index)
        .map(|h| h.cumulative_offset)
        .unwrap_or(0)
}

// ============================================================================
// Messages Area
// ============================================================================

/// Render a single message and return its lines.
///
/// This is a helper function used by the virtualized message renderer.
/// It handles both streaming and completed messages, using the cache
/// for completed messages when available.
fn render_single_message(
    message: &Message,
    app: &mut App,
    ctx: &LayoutContext,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Add blank line gap between messages (no divider line)
    lines.push(Line::from(""));

    // Render thinking/reasoning block for assistant messages (before content)
    if message.role == MessageRole::Assistant {
        lines.extend(render_thinking_block(message, app.tick_count, ctx));
    }

    // Use vertical bar prefix for user messages only, empty for assistant messages
    let (label, label_style) = if message.role == MessageRole::User {
        ("│ ", Style::default().fg(COLOR_DIM))
    } else {
        ("", Style::default().fg(COLOR_DIM))
    };

    // Handle streaming vs completed messages
    if message.is_streaming {
        // Display streaming content with blinking cursor
        // Blink cursor every ~500ms (assuming 10 ticks/sec, toggle every 5 ticks)
        let show_cursor = (app.tick_count / 5).is_multiple_of(2);
        let cursor_span = Span::styled(
            if show_cursor { "█" } else { " " },
            Style::default().fg(COLOR_ACCENT),
        );

        // For assistant messages with segments, render segments in order (interleaved)
        // This shows text, tool events, and subagent events in the order they occurred
        if message.role == MessageRole::Assistant && !message.segments.is_empty() {
            let (segment_lines, is_first_line) = render_message_segments(
                &message.segments,
                app.tick_count,
                label,
                label_style,
                ctx,
                &mut app.markdown_cache,
            );
            lines.extend(segment_lines);

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

            let content_lines = app.markdown_cache.render(&message.partial_content);

            // Prepend vertical bar to ALL lines, append cursor to last line
            if content_lines.is_empty() {
                // No content yet, just show vertical bar with cursor
                lines.push(Line::from(vec![
                    Span::styled(label, label_style),
                    cursor_span,
                ]));
            } else {
                // Prepend vertical bar to all lines
                let line_count = content_lines.len();
                for (idx, line) in content_lines.into_iter().enumerate() {
                    let mut spans = vec![Span::styled(label, label_style)];
                    spans.extend(line.spans);
                    // Append cursor to last line
                    if idx == line_count - 1 {
                        spans.push(cursor_span.clone());
                    }
                    lines.push(Line::from(spans));
                }
            }
        }
    } else {
        // Display completed message - try cache first
        if let Some(cached_lines) = app.rendered_lines_cache.get(message.id, message.render_version) {
            // Use iter().cloned() to avoid cloning the entire Vec; we only clone each Line as needed
            lines.extend(cached_lines.iter().cloned());
            lines.push(Line::from(""));
            return lines;
        }

        // Not cached - render and cache
        let mut message_lines: Vec<Line<'static>> = Vec::new();

        // For assistant messages with segments, render segments in order
        if message.role == MessageRole::Assistant && !message.segments.is_empty() {
            let (segment_lines, is_first_line) = render_message_segments(
                &message.segments,
                app.tick_count,
                label,
                label_style,
                ctx,
                &mut app.markdown_cache,
            );
            message_lines.extend(segment_lines);

            // If we never added any content, show just the label
            if is_first_line {
                message_lines.push(Line::from(vec![Span::styled(label, label_style)]));
            }
        } else {
            // Fall back to content field for non-assistant messages or empty segments
            let content_lines = app.markdown_cache.render(&message.content);

            if content_lines.is_empty() {
                // Empty content, just show vertical bar
                message_lines.push(Line::from(vec![Span::styled(label, label_style)]));
            } else {
                // Prepend vertical bar to ALL lines
                for line in content_lines {
                    let mut spans = vec![Span::styled(label, label_style)];
                    spans.extend(line.spans);
                    message_lines.push(Line::from(spans));
                }
            }
        }

        // Cache and add to output
        app.rendered_lines_cache.insert(message.id, message.render_version, message_lines.clone());
        lines.extend(message_lines);
    }

    lines.push(Line::from(""));
    lines
}

/// Fast height estimation for virtualization - takes only &Message, no mutable App access.
///
/// This enables reference-based iteration over messages without cloning the entire Vec.
/// The estimates are approximate but sufficient for virtualization to determine
/// which messages are in the visible viewport.
fn estimate_message_height_fast(message: &Message, viewport_width: usize) -> usize {
    // Base lines: 1 blank at start + 1 blank at end = 2
    let mut estimated_lines = 2;

    // Add thinking block lines if applicable
    if message.role == MessageRole::Assistant && !message.reasoning_content.is_empty() {
        if message.reasoning_collapsed {
            estimated_lines += 2;
        } else {
            let reasoning_lines = message.reasoning_content.lines().count();
            estimated_lines += 1 + reasoning_lines + 1 + 1;
        }
    }

    // Estimate content lines based on character count
    let content = if message.is_streaming {
        &message.partial_content
    } else {
        &message.content
    };

    let char_count = content.chars().count();
    let logical_lines = if char_count == 0 { 1 } else { (char_count / 60).max(1) };
    let wrap_factor = if viewport_width > 0 { 60_usize.div_ceil(viewport_width) } else { 1 };
    estimated_lines += logical_lines * wrap_factor;

    // Add lines for tool events in segments
    if message.role == MessageRole::Assistant {
        let tool_count = message
            .segments
            .iter()
            .filter(|s| matches!(s, MessageSegment::ToolEvent(_) | MessageSegment::SubagentEvent(_)))
            .count();
        estimated_lines += tool_count * 2;
    }

    estimated_lines
}

/// Estimate the visual line count for a single message without full rendering.
///
/// This is used for virtualization to calculate which messages are visible
/// without actually rendering all message content. For cached messages,
/// this can use the cached line count. For non-cached messages, it provides
/// an estimate based on content length and viewport width.
#[allow(dead_code)]
fn estimate_message_height(
    message: &Message,
    app: &mut App,
    viewport_width: usize,
    ctx: &LayoutContext,
) -> usize {
    // For completed messages, try cache first
    if !message.is_streaming {
        if let Some(cached_lines) = app.rendered_lines_cache.get(message.id, message.render_version)
        {
            return estimate_wrapped_line_count(cached_lines, viewport_width);
        }
    }

    let estimated_lines = estimate_message_height_fast(message, viewport_width);

    // For more accurate estimates on completed messages, render and cache
    if !message.is_streaming && estimated_lines > 10 {
        let rendered = render_single_message(message, app, ctx);
        return estimate_wrapped_line_count(&rendered, viewport_width);
    }

    estimated_lines
}

/// Render the messages area with user messages and AI responses
///
/// Uses `LayoutContext` for responsive layout:
/// - Tool event args are truncated based on available width
/// - Tool result previews are truncated based on available width
/// - Subagent descriptions and summaries adapt to terminal size
/// - Message wrapping uses actual viewport width
/// - Error banners adapt to terminal size
///
/// Implements message virtualization to only render messages within the
/// visible viewport plus a small buffer, significantly improving performance
/// for long conversation threads.
pub fn render_messages_area(frame: &mut Frame, area: Rect, app: &mut App, ctx: &LayoutContext) {
    let inner = inner_rect(area, 1);
    let viewport_height = inner.height as usize;
    let viewport_width = inner.width as usize;

    // Collect header lines (error banners, stream errors)
    let mut header_lines: Vec<Line> = Vec::new();

    // Show inline error banners for the thread
    header_lines.extend(render_inline_error_banners(app, ctx));

    // Show stream error banner if there's a stream error (legacy, for non-thread errors)
    if let Some(error) = &app.stream_error {
        // Truncate error message based on available width
        let max_error_len = ctx.max_preview_length();
        let display_error = if error.len() > max_error_len {
            super::helpers::truncate_string(error, max_error_len)
        } else {
            error.clone()
        };
        header_lines.push(Line::from(vec![
            Span::styled(
                "  ⚠ ERROR: ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                display_error,
                Style::default().fg(Color::Red),
            ),
        ]));
        // Responsive divider width
        let divider_width = ctx.text_wrap_width(0).min(80) as usize;
        header_lines.push(Line::from(vec![Span::styled(
            "═".repeat(divider_width),
            Style::default().fg(Color::Red),
        )]));
    }

    let header_visual_lines = estimate_wrapped_line_count(&header_lines, viewport_width);

    // Phase 1: Calculate heights using FAST estimation with reference-based iteration
    // This avoids cloning the entire message Vec on every 16ms frame
    let (message_heights, total_visual_lines, message_count) = {
        let cached_messages = app
            .active_thread_id
            .as_ref()
            .and_then(|id| {
                crate::app::log_thread_update(&format!(
                    "RENDER: Looking for messages for thread_id: {}",
                    id
                ));
                let msgs = app.cache.get_messages(id);
                crate::app::log_thread_update(&format!(
                    "RENDER: Found {} messages",
                    msgs.map(|m| m.len()).unwrap_or(0)
                ));
                msgs
            });

        match cached_messages {
            None => (Vec::new(), header_visual_lines, 0usize),
            Some(messages) if messages.is_empty() => (Vec::new(), header_visual_lines, 0usize),
            Some(messages) => {
                // Log first message to debug
                if let Some(first_msg) = messages.first() {
                    crate::app::log_thread_update(&format!(
                        "RENDER: First message role={:?}, content_len={}, is_streaming={}, segments_len={}, content_preview={:?}",
                        first_msg.role,
                        first_msg.content.len(),
                        first_msg.is_streaming,
                        first_msg.segments.len(),
                        first_msg.content.chars().take(50).collect::<String>()
                    ));
                }

                // Calculate heights using fast estimation (no mutable App access needed)
                let mut heights: Vec<MessageHeight> = Vec::with_capacity(messages.len());
                let mut cumulative_offset = header_visual_lines;

                for (i, message) in messages.iter().enumerate() {
                    let visual_lines = estimate_message_height_fast(message, viewport_width);
                    heights.push(MessageHeight {
                        message_index: i,
                        visual_lines,
                        cumulative_offset,
                    });
                    cumulative_offset += visual_lines;
                }

                let count = messages.len();
                (heights, cumulative_offset, count)
            }
        }
    };
    // Borrow of app.cache is now dropped

    if message_count == 0 {
        // No messages yet - show placeholder with vertical bar
        let mut lines = header_lines;
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(COLOR_DIM)),
            Span::styled("Waiting for your message...", Style::default().fg(COLOR_DIM)),
        ]));
        lines.push(Line::from(""));

        let total_lines = estimate_wrapped_line_count(&lines, viewport_width);
        let max_scroll = total_lines.saturating_sub(viewport_height) as u16;
        app.max_scroll = max_scroll;

        let messages_widget = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((0, 0));
        frame.render_widget(messages_widget, inner);

        // Render inline permission prompt if pending
        if app.session_state.has_pending_permission() {
            render_permission_prompt(frame, inner, app);
        }
        return;
    }

    // Calculate max scroll (how far up we can scroll from bottom)
    // scroll=0 means showing the bottom (latest content)
    // scroll=max means showing the top (oldest content)
    let max_scroll = total_visual_lines.saturating_sub(viewport_height) as u16;
    app.max_scroll = max_scroll;

    // Clamp user's scroll to valid range
    let clamped_scroll = app.conversation_scroll.min(max_scroll);

    // Convert from "scroll from bottom" to ratatui's "scroll from top"
    // If user_scroll=0, show bottom → actual_scroll = max_scroll
    // If user_scroll=max, show top → actual_scroll = 0
    let scroll_from_top = (max_scroll.saturating_sub(clamped_scroll)) as usize;

    crate::app::log_thread_update(&format!(
        "RENDER: total_visual_lines={}, max_scroll={}, user_scroll={}, scroll_from_top={}",
        total_visual_lines,
        max_scroll,
        app.conversation_scroll,
        scroll_from_top
    ));

    // Phase 2: Calculate visible range with buffer
    let (start_index, end_index) = calculate_visible_range(
        &message_heights,
        scroll_from_top.saturating_sub(header_visual_lines),
        viewport_height,
    );

    crate::app::log_thread_update(&format!(
        "RENDER: Virtualization - rendering messages {}..{} of {} (buffer={})",
        start_index,
        end_index,
        message_count,
        VIRTUALIZATION_BUFFER
    ));

    // Phase 3: Render only visible messages
    // Clone ONLY the visible range instead of all messages - major optimization
    let visible_messages: Vec<Message> = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_messages(id))
        .map(|msgs| {
            msgs.iter()
                .skip(start_index)
                .take(end_index - start_index)
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    let mut lines: Vec<Line> = header_lines;

    // Add placeholder lines for messages before the visible range
    let skip_lines = calculate_skip_lines(&message_heights, start_index);
    for _ in 0..skip_lines {
        lines.push(Line::from(""));
    }

    // Render visible messages (using the cloned subset)
    for message in visible_messages.iter() {
        let message_lines = render_single_message(message, app, ctx);
        lines.extend(message_lines);
    }

    // Add placeholder lines for messages after the visible range
    let rendered_end_offset = if end_index > 0 && end_index <= message_heights.len() {
        let last_rendered = &message_heights[end_index - 1];
        last_rendered.cumulative_offset + last_rendered.visual_lines
    } else if end_index == 0 {
        header_visual_lines
    } else {
        total_visual_lines
    };
    let trailing_lines = total_visual_lines.saturating_sub(rendered_end_offset);
    for _ in 0..trailing_lines {
        lines.push(Line::from(""));
    }

    // Log what's actually in the first few lines
    for (i, line) in lines.iter().take(5).enumerate() {
        let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        crate::app::log_thread_update(&format!(
            "RENDER: Line {}: {:?}",
            i,
            content.chars().take(80).collect::<String>()
        ));
    }

    crate::app::log_thread_update(&format!(
        "RENDER: Generated {} lines for {} visible messages, viewport={}x{}",
        lines.len(),
        end_index - start_index,
        viewport_width,
        viewport_height
    ));

    let messages_widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_from_top as u16, 0));
    frame.render_widget(messages_widget, inner);

    // Render inline permission prompt if pending (overlays on top of messages)
    if app.session_state.has_pending_permission() {
        render_permission_prompt(frame, inner, app);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::LayoutContext;

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

    // ========================================================================
    // Responsive Layout Tests
    // ========================================================================

    #[test]
    fn test_layout_context_max_preview_length_extra_small() {
        let ctx = LayoutContext::new(50, 24);
        assert!(ctx.is_extra_small());
        assert_eq!(ctx.max_preview_length(), 40);
    }

    #[test]
    fn test_layout_context_max_preview_length_small() {
        let ctx = LayoutContext::new(70, 24);
        assert!(ctx.is_narrow());
        assert!(!ctx.is_extra_small());
        assert_eq!(ctx.max_preview_length(), 60);
    }

    #[test]
    fn test_layout_context_max_preview_length_medium() {
        let ctx = LayoutContext::new(100, 24);
        assert!(!ctx.is_narrow());
        assert_eq!(ctx.max_preview_length(), 100);
    }

    #[test]
    fn test_layout_context_max_preview_length_large() {
        let ctx = LayoutContext::new(160, 24);
        assert_eq!(ctx.max_preview_length(), 150);
    }

    #[test]
    fn test_layout_context_text_wrap_width() {
        let ctx = LayoutContext::new(100, 40);
        // 100 - 4 (borders/padding) = 96
        assert_eq!(ctx.text_wrap_width(0), 96);
        // 100 - 4 - 4 (2 indent levels) = 92
        assert_eq!(ctx.text_wrap_width(2), 92);
    }

    #[test]
    fn test_layout_context_input_area_height_compact() {
        // Compact terminal (short or narrow)
        let ctx = LayoutContext::new(60, 40); // Narrow
        assert!(ctx.is_compact());
        assert_eq!(ctx.input_area_height(), 4);
    }

    #[test]
    fn test_layout_context_input_area_height_normal() {
        let ctx = LayoutContext::new(120, 40);
        assert!(!ctx.is_compact());
        assert_eq!(ctx.input_area_height(), 6);
    }

    #[test]
    fn test_layout_context_bounded_width() {
        let ctx = LayoutContext::new(100, 24);
        // 80% of 100 = 80, clamped between 40 and 80
        assert_eq!(ctx.bounded_width(80, 40, 80), 80);
    }

    #[test]
    fn test_layout_context_bounded_width_small_terminal() {
        let ctx = LayoutContext::new(50, 24);
        // 80% of 50 = 40, clamped between 40 and 80
        assert_eq!(ctx.bounded_width(80, 40, 80), 40);
    }

    // ========================================================================
    // Responsive Tool Event Tests
    // ========================================================================

    #[test]
    fn test_render_tool_event_truncates_long_args_on_narrow_terminal() {
        use crate::models::ToolEvent;

        let mut tool = ToolEvent::new("tool_123".to_string(), "Read".to_string());
        // Set a very long args display
        let long_path = "/very/long/path/that/should/be/truncated/when/terminal/is/narrow/file.rs";
        tool.args_display = Some(format!("Reading {}", long_path));

        // On a narrow terminal (60 cols), args should be truncated
        let narrow_ctx = LayoutContext::new(60, 24);
        let line = render_tool_event(&tool, 0, &narrow_ctx);
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain truncation ellipsis
        assert!(line_text.contains("...") || line_text.len() < 100);
    }

    #[test]
    fn test_render_tool_event_shows_full_args_on_wide_terminal() {
        use crate::models::ToolEvent;

        let mut tool = ToolEvent::new("tool_123".to_string(), "Read".to_string());
        // Set a moderate args display
        tool.args_display = Some("Reading /path/to/file.rs".to_string());

        // On a wide terminal, args should not be truncated
        let wide_ctx = LayoutContext::new(160, 40);
        let line = render_tool_event(&tool, 0, &wide_ctx);
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain full path
        assert!(line_text.contains("Reading /path/to/file.rs"));
        // Should NOT be truncated
        assert!(!line_text.contains("...") || line_text.contains("Reading /path/to/file.rs"));
    }

    // ========================================================================
    // Responsive Subagent Event Tests
    // ========================================================================

    #[test]
    fn test_render_subagent_event_truncates_summary_on_narrow_terminal() {
        use crate::models::SubagentEvent;

        let mut event = SubagentEvent::new(
            "task-narrow".to_string(),
            "Test task".to_string(),
            "Explore".to_string(),
        );
        event.tool_call_count = 3;
        // Set a long summary
        let long_summary = "This is a very long summary that should be truncated on narrow terminals";
        event.complete(Some(long_summary.to_string()));

        // On narrow terminal (60 cols), summary should be truncated more aggressively
        let narrow_ctx = LayoutContext::new(60, 24);
        let lines = render_subagent_event(&event, 0, TreeConnector::Single, &narrow_ctx);
        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();

        // Should be truncated (narrow uses max_summary_len of 35)
        assert!(line_text.contains("..."));
        // Should NOT contain the end of the summary
        assert!(!line_text.contains("narrow terminals"));
    }

    #[test]
    fn test_render_subagent_event_shows_more_summary_on_wide_terminal() {
        use crate::models::SubagentEvent;

        let mut event = SubagentEvent::new(
            "task-wide".to_string(),
            "Test task".to_string(),
            "Explore".to_string(),
        );
        event.tool_call_count = 3;
        // Set a moderate summary (within 60 char limit for wide terminals)
        let summary = "Found relevant configuration files";
        event.complete(Some(summary.to_string()));

        // On wide terminal (120 cols), full summary should be shown
        let wide_ctx = LayoutContext::new(120, 40);
        let lines = render_subagent_event(&event, 0, TreeConnector::Single, &wide_ctx);
        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();

        // Should contain full summary
        assert!(line_text.contains("Found relevant configuration files"));
    }

    #[test]
    fn test_render_subagent_event_truncates_description_on_narrow_terminal() {
        use crate::models::SubagentEvent;

        // Create event with a very long description
        let long_desc = "Exploring the entire codebase to find all relevant configuration files and settings";
        let event = SubagentEvent::new(
            "task-desc".to_string(),
            long_desc.to_string(),
            "Explore".to_string(),
        );

        // On extra-small terminal (50 cols), description should be truncated
        let xs_ctx = LayoutContext::new(50, 24);
        let lines = render_subagent_event(&event, 0, TreeConnector::Single, &xs_ctx);
        let line_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();

        // Should be truncated (visible in the ellipsis)
        assert!(line_text.contains("..."));
        // Full description should NOT be present
        assert!(!line_text.contains("configuration files and settings"));
    }

    #[test]
    fn test_render_inline_error_banners_responsive_width() {
        // Test with different contexts - error banners use bounded_width(80, 40, 80)
        let narrow_ctx = LayoutContext::new(50, 24);
        let wide_ctx = LayoutContext::new(160, 40);

        // Error banner width calculation: bounded_width(80, 40, 80)
        // For 50 cols: 80% = 40, clamped to 40-80 = 40
        assert_eq!(narrow_ctx.bounded_width(80, 40, 80), 40);
        // For 160 cols: 80% = 128, clamped to 40-80 = 80
        assert_eq!(wide_ctx.bounded_width(80, 40, 80), 80);
    }

    #[test]
    fn test_render_thinking_block_responsive_border_width() {
        // Test that thinking block border width adapts to terminal size
        let narrow_ctx = LayoutContext::new(60, 24);
        let wide_ctx = LayoutContext::new(120, 40);

        // Thinking block uses text_wrap_width(0).min(80)
        // For 60 cols: 60 - 4 = 56, min(56, 80) = 56
        assert_eq!(narrow_ctx.text_wrap_width(0).min(80), 56);
        // For 120 cols: 120 - 4 = 116, min(116, 80) = 80
        assert_eq!(wide_ctx.text_wrap_width(0).min(80), 80);
    }

    // ========================================================================
    // Message Virtualization Tests
    // ========================================================================

    #[test]
    fn test_calculate_visible_range_empty() {
        let heights: Vec<MessageHeight> = vec![];
        let (start, end) = calculate_visible_range(&heights, 0, 20);
        assert_eq!(start, 0);
        assert_eq!(end, 0);
    }

    #[test]
    fn test_calculate_visible_range_all_visible() {
        // 5 messages, each 10 lines tall, viewport is 100 lines
        // All messages should be visible
        let heights: Vec<MessageHeight> = (0..5)
            .map(|i| MessageHeight {
                message_index: i,
                visual_lines: 10,
                cumulative_offset: i * 10,
            })
            .collect();

        let (start, end) = calculate_visible_range(&heights, 0, 100);
        assert_eq!(start, 0);
        assert_eq!(end, 5);
    }

    #[test]
    fn test_calculate_visible_range_first_few_visible() {
        // 10 messages, each 10 lines tall, viewport is 25 lines
        // At scroll=0, should see messages 0-2 + buffer
        let heights: Vec<MessageHeight> = (0..10)
            .map(|i| MessageHeight {
                message_index: i,
                visual_lines: 10,
                cumulative_offset: i * 10,
            })
            .collect();

        let (start, end) = calculate_visible_range(&heights, 0, 25);
        assert_eq!(start, 0); // Start at 0 (can't go negative with buffer)
        // Messages 0, 1, 2 are visible (lines 0-29 cover messages up to index 2)
        // Plus VIRTUALIZATION_BUFFER (5) = min(2 + 5, 10) = 7
        assert!(end >= 3 && end <= 10);
    }

    #[test]
    fn test_calculate_visible_range_middle_section() {
        // 20 messages, each 10 lines tall, viewport is 30 lines
        // Scrolled to middle (offset 100 = message 10)
        let heights: Vec<MessageHeight> = (0..20)
            .map(|i| MessageHeight {
                message_index: i,
                visual_lines: 10,
                cumulative_offset: i * 10,
            })
            .collect();

        let (start, end) = calculate_visible_range(&heights, 100, 30);
        // Message 10 starts at offset 100
        // With buffer of 5, start should be around 10 - 5 = 5
        assert!(start <= 10);
        assert!(start >= 5);
        // Messages visible: 10, 11, 12 (lines 100-129)
        // Plus buffer: end should be around 13 + 5 = 18
        assert!(end >= 13);
        assert!(end <= 20);
    }

    #[test]
    fn test_calculate_visible_range_end_section() {
        // 10 messages, each 10 lines tall, viewport is 25 lines
        // Scrolled to end (offset 75 = showing last ~25 lines)
        let heights: Vec<MessageHeight> = (0..10)
            .map(|i| MessageHeight {
                message_index: i,
                visual_lines: 10,
                cumulative_offset: i * 10,
            })
            .collect();

        let (start, end) = calculate_visible_range(&heights, 75, 25);
        // End should be 10 (all messages)
        assert_eq!(end, 10);
        // Start should be around message 7 - buffer = max(7 - 5, 0) = 2
        assert!(start >= 0 && start <= 7);
    }

    #[test]
    fn test_calculate_visible_range_variable_heights() {
        // 5 messages with different heights: 5, 20, 5, 10, 15 = 55 total
        let heights = vec![
            MessageHeight { message_index: 0, visual_lines: 5, cumulative_offset: 0 },
            MessageHeight { message_index: 1, visual_lines: 20, cumulative_offset: 5 },
            MessageHeight { message_index: 2, visual_lines: 5, cumulative_offset: 25 },
            MessageHeight { message_index: 3, visual_lines: 10, cumulative_offset: 30 },
            MessageHeight { message_index: 4, visual_lines: 15, cumulative_offset: 40 },
        ];

        // Viewport at offset 10 (middle of message 1) with height 20
        let (start, end) = calculate_visible_range(&heights, 10, 20);
        // Message 1 (offset 5-24) is partially visible
        // Message 2 (offset 25-29) is fully visible
        // With buffer, should include earlier and later messages
        assert_eq!(start, 0); // Buffer goes back 5, but we only have 5 messages
        assert!(end >= 2); // At least message 2 should be included
    }

    #[test]
    fn test_calculate_skip_lines_start() {
        let heights = vec![
            MessageHeight { message_index: 0, visual_lines: 10, cumulative_offset: 0 },
            MessageHeight { message_index: 1, visual_lines: 15, cumulative_offset: 10 },
            MessageHeight { message_index: 2, visual_lines: 20, cumulative_offset: 25 },
        ];

        // Skip 0 messages = 0 lines
        assert_eq!(calculate_skip_lines(&heights, 0), 0);

        // Skip 1 message = cumulative_offset of message 1 = 10 lines
        assert_eq!(calculate_skip_lines(&heights, 1), 10);

        // Skip 2 messages = cumulative_offset of message 2 = 25 lines
        assert_eq!(calculate_skip_lines(&heights, 2), 25);
    }

    #[test]
    fn test_calculate_skip_lines_empty() {
        let heights: Vec<MessageHeight> = vec![];
        assert_eq!(calculate_skip_lines(&heights, 0), 0);
        assert_eq!(calculate_skip_lines(&heights, 5), 0);
    }

    #[test]
    fn test_virtualization_buffer_constant() {
        // Ensure the buffer constant is reasonable
        assert_eq!(VIRTUALIZATION_BUFFER, 5);
    }

    #[test]
    fn test_calculate_visible_range_with_large_buffer() {
        // Small message list where buffer exceeds message count
        // 3 messages, each 10 lines, viewport 20, buffer is 5
        let heights: Vec<MessageHeight> = (0..3)
            .map(|i| MessageHeight {
                message_index: i,
                visual_lines: 10,
                cumulative_offset: i * 10,
            })
            .collect();

        let (start, end) = calculate_visible_range(&heights, 0, 20);
        // Buffer would want to go back 5, but only 3 messages
        assert_eq!(start, 0);
        // Buffer would want to extend to 7, but only 3 messages
        assert_eq!(end, 3);
    }

    #[test]
    fn test_calculate_visible_range_single_message() {
        let heights = vec![
            MessageHeight { message_index: 0, visual_lines: 50, cumulative_offset: 0 },
        ];

        let (start, end) = calculate_visible_range(&heights, 0, 30);
        assert_eq!(start, 0);
        assert_eq!(end, 1);
    }

    #[test]
    fn test_visible_range_consistency() {
        // Verify that for any scroll position, the visible range includes
        // the messages that would be visible
        let heights: Vec<MessageHeight> = (0..50)
            .map(|i| MessageHeight {
                message_index: i,
                visual_lines: 10,
                cumulative_offset: i * 10,
            })
            .collect();

        // Test various scroll positions
        for scroll in [0, 50, 100, 200, 400] {
            let (start, end) = calculate_visible_range(&heights, scroll, 50);

            // Verify range is valid
            assert!(start <= end, "start ({}) should be <= end ({})", start, end);
            assert!(end <= heights.len(), "end ({}) should be <= heights.len() ({})", end, heights.len());

            // Verify visible messages are included
            for (i, h) in heights.iter().enumerate() {
                let msg_start = h.cumulative_offset;
                let msg_end = h.cumulative_offset + h.visual_lines;
                let visible_start = scroll;
                let visible_end = scroll + 50;

                // If message overlaps with visible area, it should be in range
                if msg_end > visible_start && msg_start < visible_end {
                    assert!(i >= start && i < end,
                        "Message {} (lines {}-{}) overlaps viewport {}-{} but not in range {}-{}",
                        i, msg_start, msg_end, visible_start, visible_end, start, end);
                }
            }
        }
    }
}
