//! Message virtualization
//!
//! Provides virtualization support for efficient rendering of long message lists,
//! only rendering messages within the visible viewport.

use crate::models::{Message, MessageRole, MessageSegment};

/// Represents the height in visual lines of a single message.
/// Used for virtualization to determine which messages are visible.
#[derive(Debug, Clone)]
pub struct MessageHeight {
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

    // Binary search: find the first message where message_end > scroll_from_top
    // partition_point finds the first element where the predicate is FALSE
    // We want: first message where (message_end <= scroll_from_top) is FALSE
    let start_index = message_heights
        .partition_point(|h| h.cumulative_offset + h.visual_lines <= scroll_from_top);

    if start_index >= message_heights.len() {
        return (message_heights.len(), message_heights.len(), 0);
    }

    let first_message_line_offset = scroll_from_top.saturating_sub(
        message_heights
            .get(start_index)
            .map(|h| h.cumulative_offset)
            .unwrap_or(0),
    );

    // Binary search: find the first message that starts after the visible range
    let visible_end = scroll_from_top + viewport_height;
    let end_index = message_heights
        .partition_point(|h| h.cumulative_offset < visible_end);

    (start_index, end_index, first_message_line_offset)
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
            MessageHeight { visual_lines: 5, cumulative_offset: 0 },
            MessageHeight { visual_lines: 20, cumulative_offset: 5 },
            MessageHeight { visual_lines: 5, cumulative_offset: 25 },
            MessageHeight { visual_lines: 10, cumulative_offset: 30 },
            MessageHeight { visual_lines: 15, cumulative_offset: 40 },
        ];

        // Viewport at offset 10 (middle of message 1) with height 20
        let (start, end, offset) = calculate_visible_range(&heights, 10, 20);
        assert_eq!(start, 1);
        assert_eq!(end, 3);
        assert_eq!(offset, 5);
    }

    #[test]
    fn test_calculate_visible_range_single_message() {
        let heights = vec![
            MessageHeight { visual_lines: 50, cumulative_offset: 0 },
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
