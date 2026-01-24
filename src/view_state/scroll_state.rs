//! Scroll state for virtualized rendering
//!
//! This module provides a view-only struct for scroll position and viewport
//! information that UI components need for rendering.

use crate::app::ScrollBoundary;

/// Scroll state for virtualized message rendering
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Unified scroll offset (0 = input visible at bottom, higher = scrolled up)
    pub unified_scroll: u16,
    /// Maximum scroll value (calculated during render)
    pub max_scroll: u16,
    /// True when user manually scrolled (disables auto-scroll)
    pub user_has_scrolled: bool,
    /// Scroll velocity for momentum scrolling
    pub scroll_velocity: f32,
    /// Precise scroll position for smooth scrolling
    pub scroll_position: f32,
    /// Scroll boundary hit state (for visual feedback)
    pub scroll_boundary_hit: Option<ScrollBoundary>,
    /// Tick counter when boundary was hit (for timing the highlight)
    pub boundary_hit_tick: u64,
    /// Line index where input section begins
    pub input_section_start: usize,
    /// Total content lines from last render
    pub total_content_lines: usize,
}

impl ScrollState {
    /// Create a new scroll state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scroll state with given values
    pub fn with_values(
        unified_scroll: u16,
        max_scroll: u16,
        user_has_scrolled: bool,
        scroll_velocity: f32,
        scroll_position: f32,
    ) -> Self {
        Self {
            unified_scroll,
            max_scroll,
            user_has_scrolled,
            scroll_velocity,
            scroll_position,
            ..Self::default()
        }
    }

    /// Check if we're at the bottom (following new content)
    pub fn is_at_bottom(&self) -> bool {
        self.unified_scroll == 0 && !self.user_has_scrolled
    }

    /// Check if we're at the top (oldest content)
    pub fn is_at_top(&self) -> bool {
        self.unified_scroll >= self.max_scroll
    }

    /// Get scroll percentage (0-100)
    pub fn scroll_percentage(&self) -> u8 {
        if self.max_scroll == 0 {
            100
        } else {
            ((self.unified_scroll as f32 / self.max_scroll as f32) * 100.0) as u8
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_state_default() {
        let state = ScrollState::default();
        assert_eq!(state.unified_scroll, 0);
        assert_eq!(state.max_scroll, 0);
        assert!(!state.user_has_scrolled);
        assert_eq!(state.scroll_velocity, 0.0);
        assert_eq!(state.scroll_position, 0.0);
    }

    #[test]
    fn test_scroll_state_with_values() {
        let state = ScrollState::with_values(50, 100, true, 0.5, 50.5);
        assert_eq!(state.unified_scroll, 50);
        assert_eq!(state.max_scroll, 100);
        assert!(state.user_has_scrolled);
        assert_eq!(state.scroll_velocity, 0.5);
        assert_eq!(state.scroll_position, 50.5);
    }

    #[test]
    fn test_is_at_bottom() {
        let state = ScrollState::default();
        assert!(state.is_at_bottom());

        let state = ScrollState::with_values(0, 100, true, 0.0, 0.0);
        assert!(!state.is_at_bottom()); // user_has_scrolled is true

        let state = ScrollState::with_values(50, 100, false, 0.0, 0.0);
        assert!(!state.is_at_bottom()); // not at scroll 0
    }

    #[test]
    fn test_is_at_top() {
        let state = ScrollState::with_values(100, 100, false, 0.0, 0.0);
        assert!(state.is_at_top());

        let state = ScrollState::with_values(50, 100, false, 0.0, 0.0);
        assert!(!state.is_at_top());

        let state = ScrollState::with_values(0, 0, false, 0.0, 0.0);
        assert!(state.is_at_top()); // 0 >= 0
    }

    #[test]
    fn test_scroll_percentage() {
        let state = ScrollState::with_values(50, 100, false, 0.0, 0.0);
        assert_eq!(state.scroll_percentage(), 50);

        let state = ScrollState::with_values(0, 100, false, 0.0, 0.0);
        assert_eq!(state.scroll_percentage(), 0);

        let state = ScrollState::with_values(100, 100, false, 0.0, 0.0);
        assert_eq!(state.scroll_percentage(), 100);

        // Edge case: max_scroll is 0
        let state = ScrollState::with_values(0, 0, false, 0.0, 0.0);
        assert_eq!(state.scroll_percentage(), 100);
    }
}
