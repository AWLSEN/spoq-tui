//! Streaming state for message rendering
//!
//! This module provides a view-only struct for streaming status
//! that UI components need for rendering.

/// Streaming state for message rendering
#[derive(Debug, Clone, Default)]
pub struct StreamingState {
    /// Whether the assistant is currently streaming a response
    pub is_streaming: bool,
    /// Whether the assistant is currently thinking
    pub is_thinking: bool,
    /// Current stream error (if any)
    pub stream_error: Option<String>,
    /// Tick count for cursor animation
    pub tick_count: u64,
}

impl StreamingState {
    /// Create a new streaming state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a streaming state with given values
    pub fn with_values(
        is_streaming: bool,
        is_thinking: bool,
        stream_error: Option<String>,
        tick_count: u64,
    ) -> Self {
        Self {
            is_streaming,
            is_thinking,
            stream_error,
            tick_count,
        }
    }

    /// Check if there's any active streaming or thinking
    pub fn is_active(&self) -> bool {
        self.is_streaming || self.is_thinking
    }

    /// Check if there's an error state
    pub fn has_error(&self) -> bool {
        self.stream_error.is_some()
    }

    /// Get cursor visibility based on tick count (blinks every ~500ms at 10 ticks/sec)
    pub fn show_cursor(&self) -> bool {
        (self.tick_count / 5) % 2 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_state_default() {
        let state = StreamingState::default();
        assert!(!state.is_streaming);
        assert!(!state.is_thinking);
        assert!(state.stream_error.is_none());
        assert_eq!(state.tick_count, 0);
    }

    #[test]
    fn test_streaming_state_with_values() {
        let state = StreamingState::with_values(true, false, None, 42);
        assert!(state.is_streaming);
        assert!(!state.is_thinking);
        assert!(state.stream_error.is_none());
        assert_eq!(state.tick_count, 42);
    }

    #[test]
    fn test_is_active() {
        assert!(!StreamingState::default().is_active());

        let state = StreamingState::with_values(true, false, None, 0);
        assert!(state.is_active());

        let state = StreamingState::with_values(false, true, None, 0);
        assert!(state.is_active());

        let state = StreamingState::with_values(true, true, None, 0);
        assert!(state.is_active());
    }

    #[test]
    fn test_has_error() {
        assert!(!StreamingState::default().has_error());

        let state = StreamingState::with_values(false, false, Some("Error".to_string()), 0);
        assert!(state.has_error());
    }

    #[test]
    fn test_show_cursor_blinks() {
        // At tick 0, show cursor
        let state = StreamingState::with_values(true, false, None, 0);
        assert!(state.show_cursor());

        // At tick 5, hide cursor
        let state = StreamingState::with_values(true, false, None, 5);
        assert!(!state.show_cursor());

        // At tick 10, show cursor
        let state = StreamingState::with_values(true, false, None, 10);
        assert!(state.show_cursor());

        // At tick 15, hide cursor
        let state = StreamingState::with_values(true, false, None, 15);
        assert!(!state.show_cursor());
    }
}
