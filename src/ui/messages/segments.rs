//! Message segment rendering
//!
//! Renders message segments (text, tool events, subagent events) with proper
//! grouping and tree connectors.

use ratatui::{style::Style, text::Line};

use crate::markdown::MarkdownCache;
use crate::models::MessageSegment;

use super::super::layout::LayoutContext;
use super::subagent_events::render_subagent_events_block;
use super::text_wrapping::{wrap_line_with_prefix, wrap_lines_with_prefix};
use super::tool_events::render_tool_event;

/// Render message segments, grouping consecutive subagent events for proper tree connectors
///
/// This function processes segments in order, but groups consecutive SubagentEvent segments
/// to render them with proper tree connectors (branch and last-branch for parallel agents).
///
/// Uses `LayoutContext` for responsive text truncation across all segment types.
///
/// # Arguments
/// * `segments` - The message segments to render
/// * `tick_count` - Current tick for animations
/// * `label` - Label prefix (e.g., "| " for user messages)
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

    // Calculate max width for wrapping (text_wrap_width accounts for borders/margins)
    // We use indent_level=0 since the prefix handles indentation
    let max_width = ctx.text_wrap_width(0) as usize;

    while i < segments.len() {
        match &segments[i] {
            MessageSegment::Text(text) => {
                let segment_lines = markdown_cache.render(text);
                // Wrap and prepend vertical bar to ALL text lines
                // This ensures wrapped continuations also get the prefix
                lines.extend(wrap_lines_with_prefix(
                    (*segment_lines).clone(),
                    label,
                    label_style,
                    max_width,
                    None,
                ));
                if !lines.is_empty() {
                    is_first_line = false;
                }
                i += 1;
            }
            MessageSegment::ToolEvent(event) => {
                // Tool events are usually short, but wrap if needed
                let tool_line = render_tool_event(event, tick_count, ctx);
                lines.extend(wrap_line_with_prefix(
                    tool_line,
                    label,
                    label_style,
                    max_width,
                    None,
                ));
                is_first_line = false;
                i += 1;
            }
            MessageSegment::SubagentEvent(_) => {
                // Collect consecutive subagent events
                let mut subagent_events = Vec::new();
                while i < segments.len() {
                    if let MessageSegment::SubagentEvent(event) = &segments[i] {
                        subagent_events.push(event);
                        i += 1;
                    } else {
                        break;
                    }
                }

                // Render the block with tree connectors, wrap if needed
                for line in render_subagent_events_block(&subagent_events, tick_count, ctx) {
                    lines.extend(wrap_line_with_prefix(
                        line,
                        label,
                        label_style,
                        max_width,
                        None,
                    ));
                }
                is_first_line = false;
            }
        }
    }

    (lines, is_first_line)
}
