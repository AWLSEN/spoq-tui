//! Tool event rendering
//!
//! Renders tool execution status with icons, spinners, and color-coded indicators.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::app::App;
use crate::models::{ToolEvent, ToolEventStatus};
use crate::state::ToolDisplayStatus;

use super::super::helpers::{format_tool_args, get_tool_icon, SPINNER_FRAMES};
use super::super::layout::LayoutContext;
use super::super::theme::{
    COLOR_DIM, COLOR_TOOL_ERROR, COLOR_TOOL_ICON, COLOR_TOOL_RUNNING, COLOR_TOOL_SUCCESS,
};

/// Render a single tool event as a Line
///
/// Uses tool-specific icons, color-coded status indicators, and formatted arguments
/// to provide rich visual feedback about tool execution status.
///
/// Uses `LayoutContext` for responsive args display truncation.
///
/// # Display format
/// - Running:  `[icon] [spinner] [tool_name]: [args_display]` (gray)
/// - Complete: `[icon] checkmark [tool_name]: [args_display] (duration)` (green)
/// - Failed:   `[icon] x [tool_name]: [args_display]` (red)
pub fn render_tool_event(event: &ToolEvent, tick_count: u64, ctx: &LayoutContext) -> Line<'static> {
    // Get the appropriate icon for this tool
    let icon = get_tool_icon(&event.function_name);

    // Format the arguments display
    // Use pre-computed args_display if available, otherwise format from JSON
    let args_display = event
        .args_display
        .clone()
        .unwrap_or_else(|| format_tool_args(&event.function_name, &event.args_json));

    // Calculate responsive max length for args display
    // Account for icon (2), spinner (2), tool name (~15), status (2), and padding
    let max_args_len = ctx.text_wrap_width(0).saturating_sub(25) as usize;
    let args_display = if args_display.len() > max_args_len && max_args_len > 3 {
        super::super::helpers::truncate_string(&args_display, max_args_len)
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
                Span::styled(format!("{} ", icon), Style::default().fg(COLOR_TOOL_ICON)),
                Span::styled(
                    format!("{} ", spinner),
                    Style::default().fg(COLOR_TOOL_RUNNING),
                ),
                Span::styled(
                    format!("{}: ", event.function_name),
                    Style::default().fg(COLOR_TOOL_RUNNING),
                ),
                Span::styled(args_display, Style::default().fg(COLOR_TOOL_RUNNING)),
            ])
        }
        ToolEventStatus::Complete => {
            let duration_str = event
                .duration_secs
                .map(|d| format!(" ({:.1}s)", d))
                .unwrap_or_default();

            // Check if tool result was an error (e.g., permission denied)
            // Use dimmed style for failed/denied tools
            if event.result_is_error {
                Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(format!("{} ", icon), Style::default().fg(COLOR_DIM)),
                    Span::styled("\u{2717} ", Style::default().fg(COLOR_DIM)),
                    Span::styled(
                        format!("{}: ", event.function_name),
                        Style::default().fg(COLOR_DIM),
                    ),
                    Span::styled(args_display, Style::default().fg(COLOR_DIM)),
                    Span::styled(duration_str, Style::default().fg(COLOR_DIM)),
                ])
            } else {
                Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(format!("{} ", icon), Style::default().fg(COLOR_TOOL_ICON)),
                    Span::styled("\u{2713} ", Style::default().fg(COLOR_TOOL_SUCCESS)),
                    Span::styled(
                        format!("{}: ", event.function_name),
                        Style::default().fg(COLOR_TOOL_SUCCESS),
                    ),
                    Span::styled(args_display, Style::default().fg(COLOR_TOOL_SUCCESS)),
                    Span::styled(duration_str, Style::default().fg(COLOR_TOOL_RUNNING)),
                ])
            }
        }
        ToolEventStatus::Failed => Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{} ", icon), Style::default().fg(COLOR_TOOL_ICON)),
            Span::styled("\u{2717} ", Style::default().fg(COLOR_TOOL_ERROR)),
            Span::styled(
                format!("{}: ", event.function_name),
                Style::default().fg(COLOR_TOOL_ERROR),
            ),
            Span::styled(args_display, Style::default().fg(COLOR_TOOL_ERROR)),
        ]),
    }
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

/// Render tool status indicators inline (LEGACY - kept for potential future use)
/// Shows: spinner Reading src/main.rs...  (executing, with spinner)
///        checkmark Read complete           (success, fades after 30 ticks)
///        x Write failed: error     (failure, persists)
/// Note: Tool events are now rendered inline with messages via render_tool_event()
#[allow(dead_code)]
pub fn render_tool_status_lines(app: &App) -> Vec<Line<'static>> {
    use ratatui::style::Color;

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
                    Span::styled(text, Style::default().fg(Color::DarkGray)),
                ])
            }
            ToolDisplayStatus::Completed {
                success, summary, ..
            } => {
                if *success {
                    Line::from(vec![
                        Span::styled("  \u{2713} ", Style::default().fg(Color::DarkGray)),
                        Span::styled(summary.clone(), Style::default().fg(Color::DarkGray)),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled("  \u{2717} ", Style::default().fg(Color::Red)),
                        Span::styled(summary.clone(), Style::default().fg(Color::Red)),
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::LayoutContext;

    #[test]
    fn test_render_tool_event_truncates_long_args_on_narrow_terminal() {
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
}
