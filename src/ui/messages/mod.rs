//! Message rendering functions
//!
//! Implements the message area, tool events, thinking blocks, and error banners.
//! Uses `LayoutContext` for responsive layout calculations.

mod errors;
pub mod height;
mod permission_inline;
mod plan_events;
mod segments;
mod subagent_events;
mod text_wrapping;
mod thinking;
mod tool_events;
pub mod virtualization;

// Re-export public APIs at crate::ui::messages::*
// Note: Some exports are only used in tests
pub use permission_inline::build_permission_lines;
#[allow(unused_imports)]
pub use subagent_events::{render_subagent_event, render_subagent_events_block, TreeConnector};
pub use text_wrapping::estimate_wrapped_line_count;
#[allow(unused_imports)]
pub use tool_events::{render_tool_event, truncate_preview};

// Used by this module's main functions
use errors::render_inline_error_banners;
use segments::render_message_segments;
use text_wrapping::{apply_background_to_line, wrap_lines_with_prefix};
use thinking::render_thinking_block;
use virtualization::{estimate_message_height_fast, MessageHeight};

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::models::{Message, MessageRole};

use super::helpers::inner_rect;
use super::layout::LayoutContext;
use super::theme::{COLOR_DIM, COLOR_HUMAN_BG};

/// Check if the input section should be shown in conversation view.
///
/// Returns false if the active thread has a pending permission,
/// since users must respond to the permission before sending more input.
fn should_show_input_section(app: &App) -> bool {
    app.active_thread_id
        .as_ref()
        .and_then(|tid| app.dashboard.get_pending_permission(tid))
        .is_none()
}

/// Render a single message and return its lines.
///
/// This is a helper function used by the virtualized message renderer.
/// It handles both streaming and completed messages, using the cache
/// for completed messages when available.
pub fn render_single_message(
    thread_id: &str,
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

    // Use vertical bar prefix for all messages (user and assistant)
    let (label, label_style) = ("\u{2502} ", Style::default().fg(COLOR_DIM));

    // Calculate max width for wrapping
    let max_width = ctx.text_wrap_width(0) as usize;

    // Handle streaming vs completed messages
    if message.is_streaming {
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

            // If we never added any content, show empty label line
            if is_first_line {
                lines.push(Line::from(vec![Span::styled(label, label_style)]));
            }
        } else {
            // Fall back to partial_content for backward compatibility
            // (non-assistant messages or when segments is empty)

            let content_lines = (*app.markdown_cache.render(&message.partial_content)).clone();

            // Wrap and prepend vertical bar to ALL lines
            if content_lines.is_empty() {
                // No content yet, just show vertical bar
                let mut empty_line = Line::from(vec![Span::styled(label, label_style)]);
                if message.role == MessageRole::User {
                    apply_background_to_line(&mut empty_line, COLOR_HUMAN_BG, max_width);
                }
                lines.push(empty_line);
            } else {
                let bg = if message.role == MessageRole::User {
                    Some(COLOR_HUMAN_BG)
                } else {
                    None
                };
                let wrapped_lines =
                    wrap_lines_with_prefix(content_lines, label, label_style, max_width, bg);
                lines.extend(wrapped_lines);
            }
        }
    } else {
        // Display completed message - try cache first
        if let Some(cached_lines) =
            app.rendered_lines_cache
                .get(thread_id, message.id, message.render_version)
        {
            // Use iter().cloned() to avoid cloning the entire Vec; we only clone each Line as needed
            lines.extend(cached_lines.iter().cloned());
            // Add trailing line with vertical bar for visual continuity
            let mut trailing_line = Line::from(vec![Span::styled(label, label_style)]);
            if message.role == MessageRole::User {
                apply_background_to_line(&mut trailing_line, COLOR_HUMAN_BG, max_width);
            }
            lines.push(trailing_line);
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
            let content_lines_arc = app.markdown_cache.render(&message.content);
            let content_lines = (*content_lines_arc).clone();

            if content_lines.is_empty() {
                // Empty content, just show vertical bar
                let mut empty_line = Line::from(vec![Span::styled(label, label_style)]);
                if message.role == MessageRole::User {
                    apply_background_to_line(&mut empty_line, COLOR_HUMAN_BG, max_width);
                }
                message_lines.push(empty_line);
            } else {
                // Wrap and prepend vertical bar to ALL lines
                let bg = if message.role == MessageRole::User {
                    Some(COLOR_HUMAN_BG)
                } else {
                    None
                };
                message_lines.extend(wrap_lines_with_prefix(
                    content_lines,
                    label,
                    label_style,
                    max_width,
                    bg,
                ));
            }
        }

        // Cache and add to output
        app.rendered_lines_cache.insert(
            thread_id,
            message.id,
            message.render_version,
            message_lines.clone(),
        );
        lines.extend(message_lines);
    }

    // Add trailing line with vertical bar for visual continuity
    let mut trailing_line = Line::from(vec![Span::styled(label, label_style)]);
    if message.role == MessageRole::User {
        apply_background_to_line(&mut trailing_line, COLOR_HUMAN_BG, max_width);
    }
    lines.push(trailing_line);
    lines
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
    // Note: has_visible_links is reset in prepare_render()
    // Note: rendered_lines_cache invalidation happens in prepare_render()

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
                "  \u{26A0} ERROR: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(display_error, Style::default().fg(Color::Red)),
        ]));
        // Responsive divider width
        let divider_width = ctx.text_wrap_width(0).min(80) as usize;
        header_lines.push(Line::from(vec![Span::styled(
            "\u{2550}".repeat(divider_width),
            Style::default().fg(Color::Red),
        )]));
    }

    let header_visual_lines = estimate_wrapped_line_count(&header_lines, viewport_width);

    // Phase 1: Get heights from pre-computed cache (prepared in prepare_render)
    // The height cache is updated in prepare_render(), we just read from it here
    let current_thread_id = app.active_thread_id.clone();
    let (_message_heights, _total_visual_lines, message_count) = {
        let cached_messages = current_thread_id.as_ref().and_then(|id| {
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

        match (&current_thread_id, cached_messages) {
            (_, None) => (Vec::new(), header_visual_lines, 0usize),
            (_, Some(messages)) if messages.is_empty() => (Vec::new(), header_visual_lines, 0usize),
            (Some(thread_id), Some(messages)) => {
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

                // Use pre-computed heights from the cache (prepared in prepare_render)
                let cache_valid = app
                    .height_cache
                    .as_ref()
                    .map(|c| c.is_valid_for(thread_id, viewport_width))
                    .unwrap_or(false);

                if cache_valid {
                    let cache = app.height_cache.as_ref().unwrap();

                    // Convert to MessageHeight for the virtualization API
                    let heights: Vec<MessageHeight> = cache
                        .heights
                        .iter()
                        .map(|h| MessageHeight {
                            visual_lines: h.visual_lines,
                            cumulative_offset: h.cumulative_offset,
                        })
                        .collect();

                    let count = messages.len();
                    let total = header_visual_lines + cache.total_lines;
                    (heights, total, count)
                } else {
                    // Fallback: build heights inline if cache wasn't prepared
                    // This shouldn't happen normally since prepare_render handles it
                    let heights: Vec<MessageHeight> = messages
                        .iter()
                        .scan(0usize, |offset, msg| {
                            let height = estimate_message_height_fast(msg, viewport_width);
                            let result = MessageHeight {
                                visual_lines: height,
                                cumulative_offset: *offset,
                            };
                            *offset += height;
                            Some(result)
                        })
                        .collect();

                    let total_lines: usize = heights.iter().map(|h| h.visual_lines).sum();
                    let count = messages.len();
                    let total = header_visual_lines + total_lines;
                    (heights, total, count)
                }
            }
            (None, Some(_)) => (Vec::new(), header_visual_lines, 0usize),
        }
    };
    // Borrow of app.cache is now dropped

    if message_count == 0 {
        // No messages yet - show placeholder with vertical bar
        let mut lines = header_lines;
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("\u{2502} ", Style::default().fg(COLOR_DIM)),
            Span::styled(
                "Waiting for your message...",
                Style::default().fg(COLOR_DIM),
            ),
        ]));
        lines.push(Line::from(""));

        // Add permission lines if pending for this thread
        if let Some(thread_id) = current_thread_id.as_ref() {
            if let Some(perm) = app.dashboard.get_pending_permission(thread_id) {
                let perm_lines =
                    build_permission_lines(perm, &app.question_state, ctx, app.tick_count);
                lines.extend(perm_lines);
            }

            // Add plan mode indicators (planning spinner or plan approval UI)
            if let Some(plan_request) = app.dashboard.get_plan_request(thread_id) {
                lines.extend(plan_events::render_plan_approval(
                    &plan_request.summary,
                    ctx,
                    &mut app.markdown_cache,
                ));
            } else if app.dashboard.is_thread_planning(thread_id)
                && app.dashboard.get_pending_permission(thread_id).is_none()
            {
                lines.extend(plan_events::render_planning_indicator(app.tick_count));
            }
        }

        // === UNIFIED SCROLL: Append input section (if no pending permission) ===
        if should_show_input_section(app) {
            app.input_section_start = lines.len();
            let input_lines = super::input::build_input_section(app, inner.width);
            lines.extend(input_lines);
        }

        // === UNIFIED SCROLL: Record total for scroll calculations ===
        app.total_content_lines = lines.len();

        // === SIMPLE SCROLL ===
        let total_lines = lines.len();
        let max_possible_scroll = total_lines.saturating_sub(inner.height as usize);
        let scroll_from_top = max_possible_scroll.saturating_sub(app.unified_scroll as usize);
        app.max_scroll = max_possible_scroll as u16;

        let messages_widget = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll_from_top as u16, 0));
        frame.render_widget(messages_widget, inner);
        return;
    }

    let input_lines = super::input::build_input_section(app, inner.width);

    // SIMPLE: Render ALL messages (no virtualization)
    // This ensures smooth 1-line-at-a-time scrolling
    let all_messages: Vec<Message> = app
        .active_thread_id
        .as_ref()
        .and_then(|id| app.cache.get_messages(id))
        .map(|msgs| msgs.to_vec())
        .unwrap_or_default();

    let mut lines: Vec<Line> = header_lines;

    // Render ALL messages
    let thread_id = app.active_thread_id.clone().unwrap_or_default();
    for message in all_messages.iter() {
        let message_lines = render_single_message(&thread_id, message, app, ctx);
        lines.extend(message_lines);
    }

    // Detect if any visible lines contain hyperlinks (OSC 8 escape sequences)
    // OSC 8 format starts with: \x1b]8;;
    for line in &lines {
        for span in &line.spans {
            if span.content.contains("\x1b]8;;") {
                app.has_visible_links = true;
                break;
            }
        }
        if app.has_visible_links {
            break;
        }
    }

    // Add permission lines if pending for this thread
    if let Some(perm) = app.dashboard.get_pending_permission(&thread_id) {
        let perm_lines = build_permission_lines(perm, &app.question_state, ctx, app.tick_count);
        lines.extend(perm_lines);
    }

    // Add plan mode indicators (planning spinner or plan approval UI)
    // Priority: plan approval > planning indicator (only if no permission pending)
    if let Some(plan_request) = app.dashboard.get_plan_request(&thread_id) {
        // Plan approval is pending - show plan summary with approve/reject options
        lines.extend(plan_events::render_plan_approval(
            &plan_request.summary,
            ctx,
            &mut app.markdown_cache,
        ));
    } else if app.dashboard.is_thread_planning(&thread_id)
        && app.dashboard.get_pending_permission(&thread_id).is_none()
    {
        // Thread is actively planning and no permission prompt pending
        lines.extend(plan_events::render_planning_indicator(app.tick_count));
    }

    // Record where input section starts
    app.input_section_start = lines.len();

    // Append input section
    lines.extend(input_lines);

    // SIMPLE SCROLL CALCULATION:
    // total_lines = everything we rendered
    // max_scroll = how far we can scroll (total - viewport)
    // scroll_from_top = max_scroll - unified_scroll (converts from "from bottom" to "from top")
    let total_lines = lines.len();
    app.total_content_lines = total_lines;

    let max_scroll = total_lines.saturating_sub(viewport_height);
    app.max_scroll = max_scroll as u16;

    let scroll_from_top = max_scroll.saturating_sub(app.unified_scroll as usize);

    let messages_widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_from_top as u16, 0));
    frame.render_widget(messages_widget, inner);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::ui::LayoutContext;

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
}
