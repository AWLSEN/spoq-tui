//! Subagent event rendering
//!
//! Renders subagent execution status with tree connectors, icons, and progress indicators.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::app::App;
use crate::models::{SubagentEvent, SubagentEventStatus};
use crate::state::SubagentDisplayStatus;

use super::super::helpers::{get_subagent_icon, truncate_string, SPINNER_FRAMES};
use super::super::layout::LayoutContext;
use super::super::theme::{COLOR_DIM, COLOR_SUBAGENT_COMPLETE, COLOR_SUBAGENT_RUNNING};

/// Tree connector for subagent display
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TreeConnector {
    /// Single item (no tree structure needed)
    Single,
    /// Non-last item in a group: branch connector
    Branch,
    /// Last item in a group: last branch connector
    LastBranch,
}

impl TreeConnector {
    /// Get the string representation of the tree connector
    pub fn as_str(&self) -> &'static str {
        match self {
            TreeConnector::Single => "\u{25CF} ",
            TreeConnector::Branch => "\u{251C}\u{2500}\u{2500} ",
            TreeConnector::LastBranch => "\u{2514}\u{2500}\u{2500} ",
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
/// - Complete: `[connector] Done (N tool uses - summary)` (green)
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
                truncate_string(&event.description, max_description_len)
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
                    "  \u{2502}   " // Continuation line for non-last items
                };
                // Truncate progress message for narrow terminals
                let progress_text = if progress.len() > max_description_len && max_description_len > 3 {
                    truncate_string(progress, max_description_len)
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
            // Format: connector + Done (N tool uses - summary) or just (N tool uses)
            let tool_count_str = if event.tool_call_count == 1 {
                "1 tool use".to_string()
            } else {
                format!("{} tool uses", event.tool_call_count)
            };

            let display_text = if let Some(ref summary) = event.summary {
                // Truncate summary using responsive max length
                let truncated_summary = if summary.len() > max_summary_len {
                    truncate_string(summary, max_summary_len)
                } else {
                    summary.clone()
                };
                format!("Done ({} \u{00B7} {})", tool_count_str, truncated_summary)
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
/// - Single subagent: bullet Task(description)
/// - Multiple parallel: branch for non-last, last-branch for last
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

/// Render subagent status with spinner and progress (LEGACY - kept for potential future use)
/// UI design:
/// ```text
/// top-left-corner spinner Exploring codebase structure
/// vertical-bar   Found 5 relevant files...
/// bottom-left-corner checkmark Complete (8 tool calls)
/// ```
/// Note: Subagent status may be integrated inline in future iterations
#[allow(dead_code)]
pub fn render_subagent_status_lines(app: &App) -> Vec<Line<'static>> {
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
                        format!("\u{250C} {} ", spinner),
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
                    ("\u{2514} \u{2713} ", Color::DarkGray)
                } else {
                    ("\u{2514} \u{2717} ", Color::Red)
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
                        "\u{2502}   ",
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::LayoutContext;

    #[test]
    fn test_render_subagent_event_truncates_summary_on_narrow_terminal() {
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
}
