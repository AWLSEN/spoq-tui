//! Message rendering functions
//!
//! Implements the message area, tool events, thinking blocks, and error banners.
//! Uses `LayoutContext` for responsive layout calculations.

mod errors;
mod segments;
mod subagent_events;
mod text_wrapping;
mod thinking;
mod tool_events;
mod virtualization;

// Re-export public APIs at crate::ui::messages::*
// Note: Some exports are only used in tests
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
use virtualization::{
    calculate_skip_lines, calculate_visible_range, estimate_message_height_fast, MessageHeight,
    VIRTUALIZATION_BUFFER,
};

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
use super::input::render_permission_prompt;
use super::layout::LayoutContext;
use super::theme::{COLOR_ACCENT, COLOR_DIM, COLOR_HUMAN_BG};

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
        // Display streaming content with blinking cursor
        // Blink cursor every ~500ms (assuming 10 ticks/sec, toggle every 5 ticks)
        let show_cursor = (app.tick_count / 5).is_multiple_of(2);
        let cursor_span = Span::styled(
            if show_cursor { "\u{2588}" } else { " " },
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

            // Wrap and prepend vertical bar to ALL lines, append cursor to last line
            if content_lines.is_empty() {
                // No content yet, just show vertical bar with cursor
                let mut empty_line = Line::from(vec![
                    Span::styled(label, label_style),
                    cursor_span,
                ]);
                if message.role == MessageRole::User {
                    apply_background_to_line(&mut empty_line, COLOR_HUMAN_BG, max_width);
                }
                lines.push(empty_line);
            } else {
                // Wrap lines with prefix, then append cursor to last line
                let bg = if message.role == MessageRole::User { Some(COLOR_HUMAN_BG) } else { None };
                let mut wrapped_lines = wrap_lines_with_prefix(content_lines, label, label_style, max_width, bg);
                if let Some(last_line) = wrapped_lines.last_mut() {
                    last_line.spans.push(cursor_span);
                }
                lines.extend(wrapped_lines);
            }
        }
    } else {
        // Display completed message - try cache first
        if let Some(cached_lines) = app.rendered_lines_cache.get(thread_id, message.id, message.render_version) {
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
            let content_lines = app.markdown_cache.render(&message.content);

            if content_lines.is_empty() {
                // Empty content, just show vertical bar
                let mut empty_line = Line::from(vec![Span::styled(label, label_style)]);
                if message.role == MessageRole::User {
                    apply_background_to_line(&mut empty_line, COLOR_HUMAN_BG, max_width);
                }
                message_lines.push(empty_line);
            } else {
                // Wrap and prepend vertical bar to ALL lines
                let bg = if message.role == MessageRole::User { Some(COLOR_HUMAN_BG) } else { None };
                message_lines.extend(wrap_lines_with_prefix(content_lines, label, label_style, max_width, bg));
            }
        }

        // Cache and add to output
        app.rendered_lines_cache.insert(thread_id, message.id, message.render_version, message_lines.clone());
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
    // Reset link visibility flag at the start of each render pass
    app.has_visible_links = false;

    let inner = inner_rect(area, 1);
    let viewport_height = inner.height as usize;
    let viewport_width = inner.width as usize;

    // Invalidate rendered lines cache if viewport width changed (terminal resize)
    // This ensures wrapped lines are re-rendered with correct width
    app.rendered_lines_cache.invalidate_if_width_changed(inner.width);

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
            "\u{2550}".repeat(divider_width),
            Style::default().fg(Color::Red),
        )]));
    }

    let header_visual_lines = estimate_wrapped_line_count(&header_lines, viewport_width);

    // Phase 1: Calculate heights using FAST estimation with reference-based iteration
    // This avoids cloning the entire message Vec on every 16ms frame
    let current_thread_id = app.active_thread_id.clone();
    let (message_heights, total_visual_lines, message_count) = {
        let cached_messages = current_thread_id
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

                // Calculate heights using cached values when possible
                let mut heights: Vec<MessageHeight> = Vec::with_capacity(messages.len());
                let mut cumulative_offset = header_visual_lines;

                for (i, message) in messages.iter().enumerate() {
                    // Check cache first using (thread_id, message_id) key
                    let cache_key = (thread_id.clone(), message.id);
                    let cached = app.cached_message_heights.get(&cache_key);
                    let visual_lines = if let Some((version, height)) = cached {
                        if *version == message.render_version {
                            *height  // Cache hit
                        } else {
                            // Stale cache, recalculate
                            let height = estimate_message_height_fast(message, viewport_width);
                            app.cached_message_heights.insert(cache_key, (message.render_version, height));
                            height
                        }
                    } else {
                        // Cache miss, calculate and store
                        let height = estimate_message_height_fast(message, viewport_width);
                        app.cached_message_heights.insert(cache_key, (message.render_version, height));
                        height
                    };

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
            Span::styled("Waiting for your message...", Style::default().fg(COLOR_DIM)),
        ]));
        lines.push(Line::from(""));

        // === UNIFIED SCROLL: Record where input section starts ===
        app.input_section_start = lines.len();

        // === UNIFIED SCROLL: Append input section ===
        let input_lines = super::input::build_input_section(app, inner.width);
        lines.extend(input_lines);

        // === UNIFIED SCROLL: Record total for scroll calculations ===
        app.total_content_lines = lines.len();

        // === UNIFIED SCROLL: Calculate scroll ===
        let total_visual = lines.len() as u16;
        let unified_scroll_from_top = if total_visual <= inner.height {
            0 // Content fits, no scroll
        } else if !app.user_has_scrolled {
            total_visual.saturating_sub(inner.height)
        } else {
            let unified_max_scroll = total_visual.saturating_sub(inner.height);
            unified_max_scroll.saturating_sub(app.unified_scroll)
        };

        app.max_scroll = total_visual.saturating_sub(inner.height);

        let messages_widget = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((unified_scroll_from_top, 0));
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

    // Clamp user's scroll to valid range (unified_scroll is the source of truth)
    let clamped_scroll = app.unified_scroll.min(max_scroll);

    // Convert from "scroll from bottom" to ratatui's "scroll from top"
    // If user_scroll=0, show bottom -> actual_scroll = max_scroll
    // If user_scroll=max, show top -> actual_scroll = 0
    let scroll_from_top = (max_scroll.saturating_sub(clamped_scroll)) as usize;

    crate::app::log_thread_update(&format!(
        "RENDER: total_visual_lines={}, max_scroll={}, unified_scroll={}, scroll_from_top={}",
        total_visual_lines,
        max_scroll,
        app.unified_scroll,
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
    // Extract thread_id for passing to render_single_message
    let thread_id = app.active_thread_id.clone().unwrap_or_default();
    for message in visible_messages.iter() {
        let message_lines = render_single_message(&thread_id, message, app, ctx);
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

    // === UNIFIED SCROLL: Record where input section starts ===
    app.input_section_start = lines.len();

    // === UNIFIED SCROLL: Append input section ===
    let input_lines = super::input::build_input_section(app, inner.width);
    lines.extend(input_lines);

    // === UNIFIED SCROLL: Record total for scroll calculations ===
    app.total_content_lines = lines.len();

    // === UNIFIED SCROLL: Calculate scroll to show input by default ===
    let total_visual = lines.len() as u16;
    let unified_scroll_from_top = if total_visual <= inner.height {
        0 // Content fits, no scroll
    } else if !app.user_has_scrolled {
        // Auto-scroll: show input at bottom
        total_visual.saturating_sub(inner.height)
    } else {
        // User scrolled: convert unified_scroll (from-bottom) to from-top
        let unified_max_scroll = total_visual.saturating_sub(inner.height);
        unified_max_scroll.saturating_sub(app.unified_scroll)
    };

    // Update max_scroll for event handlers
    app.max_scroll = total_visual.saturating_sub(inner.height);

    let messages_widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((unified_scroll_from_top, 0));
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
