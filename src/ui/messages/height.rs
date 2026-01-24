//! Message height calculation
//!
//! This module provides utilities for calculating message heights for
//! virtualization. Height calculations are performed in the prepare phase
//! rather than during rendering.

use crate::models::{Message, MessageSegment};

/// Estimate the height of a message in visual lines.
///
/// This is a fast estimation that doesn't require mutable access to caches.
/// The estimate is approximate but sufficient for virtualization to determine
/// which messages are in the visible viewport.
///
/// # Arguments
/// * `message` - The message to estimate height for
/// * `viewport_width` - The current viewport width
///
/// # Returns
/// The estimated number of visual lines the message will occupy
pub fn estimate_height(message: &Message, viewport_width: usize) -> usize {
    // Delegate to the virtualization module's implementation
    super::virtualization::estimate_message_height_fast(message, viewport_width)
}

/// Estimate the height of message content text.
///
/// # Arguments
/// * `text` - The text content to measure
/// * `viewport_width` - The available width for wrapping
///
/// # Returns
/// The estimated number of lines
pub fn estimate_text_height(text: &str, viewport_width: usize) -> usize {
    if text.is_empty() {
        return 0;
    }

    // Account for the vertical bar prefix
    let effective_width = viewport_width.saturating_sub(2);
    if effective_width == 0 {
        return text.lines().count();
    }

    // Estimate wrapped lines for each paragraph
    text.lines()
        .map(|line| {
            if line.is_empty() {
                1
            } else {
                // Simple estimation: divide by effective width, round up
                line.len().div_ceil(effective_width)
            }
        })
        .sum()
}

/// Estimate the height contribution of tool events in a message.
///
/// Each tool event typically takes 1-2 lines depending on the display.
pub fn estimate_tool_events_height(segments: &[MessageSegment]) -> usize {
    segments
        .iter()
        .filter_map(|seg| {
            if let MessageSegment::ToolEvent(_event) = seg {
                // Each tool event is approximately 1 line
                Some(1)
            } else {
                None
            }
        })
        .sum()
}

/// Estimate the height contribution of subagent events in a message.
///
/// Subagent events are typically 1-2 lines each.
pub fn estimate_subagent_events_height(segments: &[MessageSegment]) -> usize {
    segments
        .iter()
        .filter_map(|seg| {
            if let MessageSegment::SubagentEvent(_event) = seg {
                // Each subagent event is approximately 1 line
                Some(1)
            } else {
                None
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_text_height_empty() {
        assert_eq!(estimate_text_height("", 80), 0);
    }

    #[test]
    fn test_estimate_text_height_single_line() {
        assert_eq!(estimate_text_height("Hello world", 80), 1);
    }

    #[test]
    fn test_estimate_text_height_multiple_lines() {
        let text = "Line 1\nLine 2\nLine 3";
        assert_eq!(estimate_text_height(text, 80), 3);
    }

    #[test]
    fn test_estimate_text_height_wrapping() {
        // Create a long line that should wrap
        let text = "a".repeat(100);
        // With effective width of 78 (80 - 2 for prefix), should wrap to 2 lines
        assert_eq!(estimate_text_height(&text, 80), 2);
    }

    #[test]
    fn test_estimate_tool_events_height_empty() {
        let segments: Vec<MessageSegment> = vec![];
        assert_eq!(estimate_tool_events_height(&segments), 0);
    }

    #[test]
    fn test_estimate_subagent_events_height_empty() {
        let segments: Vec<MessageSegment> = vec![];
        assert_eq!(estimate_subagent_events_height(&segments), 0);
    }
}
