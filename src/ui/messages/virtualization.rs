//! Message virtualization
//!
//! Provides virtualization support for efficient rendering of long message lists,
//! only rendering messages within the visible viewport.

use crate::models::{Message, MessageRole, MessageSegment};

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
/// # Arguments
/// * `message_heights` - Pre-computed heights for each message
/// * `scroll_from_top` - The scroll offset from the top (in visual lines)
/// * `viewport_height` - The height of the viewport in visual lines
///
/// # Returns
/// (start_index, end_index, first_message_line_offset) tuple defining which messages to render
pub fn calculate_visible_range(
    message_heights: &[MessageHeight],
    scroll_from_top: usize,
    viewport_height: usize,
) -> (usize, usize, usize) {
    if message_heights.is_empty() || viewport_height == 0 {
        return (0, 0, 0);
    }

    let total_message_lines = message_heights
        .last()
        .map(|h| h.cumulative_offset + h.visual_lines)
        .unwrap_or(0);
    if scroll_from_top >= total_message_lines {
        return (message_heights.len(), message_heights.len(), 0);
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

    if start_index >= message_heights.len() {
        return (message_heights.len(), message_heights.len(), 0);
    }

    let first_message_line_offset = scroll_from_top.saturating_sub(
        message_heights
            .get(start_index)
            .map(|h| h.cumulative_offset)
            .unwrap_or(0),
    );

    // Find the first message that starts after the visible range
    let visible_end = scroll_from_top + viewport_height;
    let mut end_index = message_heights.len();
    for (i, height) in message_heights.iter().enumerate() {
        if height.cumulative_offset >= visible_end {
            end_index = i;
            break;
        }
    }

    (start_index, end_index, first_message_line_offset)
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
#[allow(dead_code)]
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

/// Fast height estimation for virtualization - takes only &Message, no mutable App access.
///
/// This enables reference-based iteration over messages without cloning the entire Vec.
/// The estimates are approximate but sufficient for virtualization to determine
/// which messages are in the visible viewport.
pub fn estimate_message_height_fast(message: &Message, viewport_width: usize) -> usize {
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
pub fn estimate_message_height(
    thread_id: &str,
    message: &Message,
    app: &mut crate::app::App,
    viewport_width: usize,
    ctx: &super::super::layout::LayoutContext,
) -> usize {
    use super::text_wrapping::estimate_wrapped_line_count;
    use super::render_single_message;

    // For completed messages, try cache first
    if !message.is_streaming {
        if let Some(cached_lines) = app.rendered_lines_cache.get(thread_id, message.id, message.render_version)
        {
            return estimate_wrapped_line_count(cached_lines, viewport_width);
        }
    }

    let estimated_lines = estimate_message_height_fast(message, viewport_width);

    // For more accurate estimates on completed messages, render and cache
    if !message.is_streaming && estimated_lines > 10 {
        let rendered = render_single_message(thread_id, message, app, ctx);
        return estimate_wrapped_line_count(&rendered, viewport_width);
    }

    estimated_lines
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_visible_range_empty() {
        let heights: Vec<MessageHeight> = vec![];
        let (start, end, offset) = calculate_visible_range(&heights, 0, 20);
        assert_eq!(start, 0);
        assert_eq!(end, 0);
        assert_eq!(offset, 0);
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

        let (start, end, offset) = calculate_visible_range(&heights, 0, 100);
        assert_eq!(start, 0);
        assert_eq!(end, 5);
        assert_eq!(offset, 0);
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

        let (start, end, offset) = calculate_visible_range(&heights, 0, 25);
        assert_eq!(start, 0);
        assert_eq!(end, 3);
        assert_eq!(offset, 0);
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

        let (start, end, offset) = calculate_visible_range(&heights, 100, 30);
        assert_eq!(start, 10);
        assert_eq!(end, 13);
        assert_eq!(offset, 0);
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

        let (start, end, offset) = calculate_visible_range(&heights, 75, 25);
        assert_eq!(start, 7);
        assert_eq!(end, 10);
        assert_eq!(offset, 5);
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
        let (start, end, offset) = calculate_visible_range(&heights, 10, 20);
        assert_eq!(start, 1);
        assert_eq!(end, 3);
        assert_eq!(offset, 5);
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
    fn test_calculate_visible_range_single_message() {
        let heights = vec![
            MessageHeight { message_index: 0, visual_lines: 50, cumulative_offset: 0 },
        ];

        let (start, end, offset) = calculate_visible_range(&heights, 0, 30);
        assert_eq!(start, 0);
        assert_eq!(end, 1);
        assert_eq!(offset, 0);
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
            let (start, end, _offset) = calculate_visible_range(&heights, scroll, 50);

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
