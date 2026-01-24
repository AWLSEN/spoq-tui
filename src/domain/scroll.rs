//! Scroll state management.
//!
//! This module provides [`ScrollState`], a domain object that encapsulates
//! all scroll-related state including position, velocity, and boundary tracking.

use crate::app::ScrollBoundary;

/// Scroll state encapsulating position, velocity, and boundary tracking.
///
/// This domain object manages all scroll-related concerns:
/// - Current scroll position (line-based and fractional)
/// - Scroll velocity for momentum scrolling
/// - User scroll tracking (for auto-scroll disable)
/// - Boundary hit detection for visual feedback
/// - Content dimensions for clamping
pub struct ScrollState {
    /// Maximum scroll value (calculated during render, used for clamping)
    pub max_scroll: u16,
    /// Unified scroll offset (0 = input visible at bottom, higher = scrolled up)
    pub unified_scroll: u16,
    /// True when user manually scrolled (disables auto-scroll)
    pub user_has_scrolled: bool,
    /// Scroll velocity for momentum scrolling (lines per tick, positive = up/older)
    pub scroll_velocity: f32,
    /// Precise scroll position for smooth scrolling (fractional lines)
    pub scroll_position: f32,
    /// Line index where input section begins (for scroll calculations)
    pub input_section_start: usize,
    /// Total content lines from last render
    pub total_content_lines: usize,
    /// Scroll boundary hit state (for visual feedback)
    pub scroll_boundary_hit: Option<ScrollBoundary>,
    /// Tick counter when boundary was hit (for timing the highlight)
    pub boundary_hit_tick: u64,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    /// Create a new ScrollState with default values.
    pub fn new() -> Self {
        Self {
            max_scroll: 0,
            unified_scroll: 0,
            user_has_scrolled: false,
            scroll_velocity: 0.0,
            scroll_position: 0.0,
            input_section_start: 0,
            total_content_lines: 0,
            scroll_boundary_hit: None,
            boundary_hit_tick: 0,
        }
    }

    /// Get the current scroll offset.
    pub fn get_offset(&self) -> u16 {
        self.unified_scroll
    }

    /// Set the scroll offset, clamping to valid range.
    pub fn set_offset(&mut self, offset: u16) {
        self.unified_scroll = offset.min(self.max_scroll);
        self.scroll_position = self.unified_scroll as f32;
    }

    /// Scroll up by the specified number of lines.
    ///
    /// Returns true if the scroll position changed.
    pub fn scroll_up(&mut self, lines: u16) -> bool {
        let old_scroll = self.unified_scroll;
        self.unified_scroll = self.unified_scroll.saturating_add(lines).min(self.max_scroll);
        self.scroll_position = self.unified_scroll as f32;
        self.user_has_scrolled = true;

        // Check if we hit the top boundary
        if self.unified_scroll == self.max_scroll && old_scroll != self.max_scroll {
            self.scroll_boundary_hit = Some(ScrollBoundary::Top);
        }

        old_scroll != self.unified_scroll
    }

    /// Scroll down by the specified number of lines.
    ///
    /// Returns true if the scroll position changed.
    pub fn scroll_down(&mut self, lines: u16) -> bool {
        let old_scroll = self.unified_scroll;
        self.unified_scroll = self.unified_scroll.saturating_sub(lines);
        self.scroll_position = self.unified_scroll as f32;
        self.user_has_scrolled = true;

        // Check if we hit the bottom boundary
        if self.unified_scroll == 0 && old_scroll != 0 {
            self.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
        }

        old_scroll != self.unified_scroll
    }

    /// Scroll to the top (oldest content).
    pub fn scroll_to_top(&mut self) {
        if self.unified_scroll < self.max_scroll {
            self.unified_scroll = self.max_scroll;
            self.scroll_position = self.unified_scroll as f32;
            self.user_has_scrolled = true;
        }
    }

    /// Scroll to the bottom (newest content, input visible).
    pub fn scroll_to_bottom(&mut self) {
        self.unified_scroll = 0;
        self.scroll_position = 0.0;
        self.user_has_scrolled = false; // Re-enable auto-scroll
    }

    /// Check if scrolled to the bottom (auto-scroll position).
    pub fn is_at_bottom(&self) -> bool {
        self.unified_scroll == 0
    }

    /// Check if scrolled to the top (oldest content).
    pub fn is_at_top(&self) -> bool {
        self.unified_scroll >= self.max_scroll
    }

    /// Check if user has manually scrolled.
    pub fn has_user_scrolled(&self) -> bool {
        self.user_has_scrolled
    }

    /// Reset user scroll flag (e.g., when new message arrives).
    pub fn reset_user_scroll(&mut self) {
        self.user_has_scrolled = false;
    }

    /// Update scroll limits based on content size.
    pub fn update_limits(&mut self, max_scroll: u16, total_lines: usize, input_start: usize) {
        self.max_scroll = max_scroll;
        self.total_content_lines = total_lines;
        self.input_section_start = input_start;

        // Clamp current scroll to new limits
        if self.unified_scroll > max_scroll {
            self.unified_scroll = max_scroll;
            self.scroll_position = max_scroll as f32;
        }
    }

    // Momentum scrolling methods

    /// Apply velocity for momentum scrolling.
    ///
    /// Returns true if the scroll position changed.
    pub fn apply_velocity(&mut self) -> bool {
        if self.scroll_velocity.abs() < 0.1 {
            self.scroll_velocity = 0.0;
            return false;
        }

        let old_pos = self.scroll_position;
        self.scroll_position += self.scroll_velocity;
        self.scroll_position = self.scroll_position.clamp(0.0, self.max_scroll as f32);
        self.unified_scroll = self.scroll_position.round() as u16;

        // Apply friction
        self.scroll_velocity *= 0.92;

        // Check boundaries
        if self.scroll_position <= 0.0 {
            self.scroll_velocity = 0.0;
            self.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
        } else if self.scroll_position >= self.max_scroll as f32 {
            self.scroll_velocity = 0.0;
            self.scroll_boundary_hit = Some(ScrollBoundary::Top);
        }

        (self.scroll_position - old_pos).abs() > 0.01
    }

    /// Add velocity for momentum scrolling.
    pub fn add_velocity(&mut self, velocity: f32) {
        self.scroll_velocity += velocity;
        // Cap velocity
        self.scroll_velocity = self.scroll_velocity.clamp(-50.0, 50.0);
    }

    /// Stop momentum scrolling.
    pub fn stop_momentum(&mut self) {
        self.scroll_velocity = 0.0;
    }

    /// Check if momentum scrolling is active.
    pub fn has_momentum(&self) -> bool {
        self.scroll_velocity.abs() > 0.1
    }

    // Boundary hit tracking

    /// Record a boundary hit at the given tick.
    pub fn record_boundary_hit(&mut self, boundary: ScrollBoundary, tick: u64) {
        self.scroll_boundary_hit = Some(boundary);
        self.boundary_hit_tick = tick;
    }

    /// Clear the boundary hit state if enough ticks have passed.
    pub fn maybe_clear_boundary_hit(&mut self, current_tick: u64, duration_ticks: u64) {
        if self.scroll_boundary_hit.is_some()
            && current_tick >= self.boundary_hit_tick + duration_ticks
        {
            self.scroll_boundary_hit = None;
        }
    }

    /// Get the current boundary hit state.
    pub fn get_boundary_hit(&self) -> Option<ScrollBoundary> {
        self.scroll_boundary_hit
    }

    /// Clear the boundary hit state immediately.
    pub fn clear_boundary_hit(&mut self) {
        self.scroll_boundary_hit = None;
    }

    /// Reset all scroll state.
    pub fn reset(&mut self) {
        self.max_scroll = 0;
        self.unified_scroll = 0;
        self.user_has_scrolled = false;
        self.scroll_velocity = 0.0;
        self.scroll_position = 0.0;
        self.input_section_start = 0;
        self.total_content_lines = 0;
        self.scroll_boundary_hit = None;
        self.boundary_hit_tick = 0;
    }
}

impl std::fmt::Debug for ScrollState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollState")
            .field("unified_scroll", &self.unified_scroll)
            .field("max_scroll", &self.max_scroll)
            .field("user_has_scrolled", &self.user_has_scrolled)
            .field("scroll_velocity", &self.scroll_velocity)
            .field("scroll_boundary_hit", &self.scroll_boundary_hit)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_state_new() {
        let state = ScrollState::new();
        assert_eq!(state.unified_scroll, 0);
        assert_eq!(state.max_scroll, 0);
        assert!(!state.user_has_scrolled);
        assert_eq!(state.scroll_velocity, 0.0);
        assert!(state.scroll_boundary_hit.is_none());
    }

    #[test]
    fn test_scroll_state_default() {
        let state = ScrollState::default();
        assert_eq!(state.unified_scroll, 0);
        assert!(state.is_at_bottom());
    }

    #[test]
    fn test_scroll_up() {
        let mut state = ScrollState::new();
        state.max_scroll = 100;

        let changed = state.scroll_up(10);
        assert!(changed);
        assert_eq!(state.unified_scroll, 10);
        assert!(state.user_has_scrolled);
    }

    #[test]
    fn test_scroll_up_clamped() {
        let mut state = ScrollState::new();
        state.max_scroll = 50;

        state.scroll_up(100);
        assert_eq!(state.unified_scroll, 50);
        assert!(state.is_at_top());
        assert!(matches!(state.scroll_boundary_hit, Some(ScrollBoundary::Top)));
    }

    #[test]
    fn test_scroll_down() {
        let mut state = ScrollState::new();
        state.max_scroll = 100;
        state.unified_scroll = 50;
        state.scroll_position = 50.0;

        let changed = state.scroll_down(10);
        assert!(changed);
        assert_eq!(state.unified_scroll, 40);
        assert!(state.user_has_scrolled);
    }

    #[test]
    fn test_scroll_down_clamped() {
        let mut state = ScrollState::new();
        state.max_scroll = 100;
        state.unified_scroll = 10;
        state.scroll_position = 10.0;

        state.scroll_down(50);
        assert_eq!(state.unified_scroll, 0);
        assert!(state.is_at_bottom());
        assert!(matches!(state.scroll_boundary_hit, Some(ScrollBoundary::Bottom)));
    }

    #[test]
    fn test_scroll_to_top() {
        let mut state = ScrollState::new();
        state.max_scroll = 100;

        state.scroll_to_top();
        assert_eq!(state.unified_scroll, 100);
        assert!(state.is_at_top());
        assert!(state.user_has_scrolled);
    }

    #[test]
    fn test_scroll_to_bottom() {
        let mut state = ScrollState::new();
        state.max_scroll = 100;
        state.unified_scroll = 50;
        state.user_has_scrolled = true;

        state.scroll_to_bottom();
        assert_eq!(state.unified_scroll, 0);
        assert!(state.is_at_bottom());
        assert!(!state.user_has_scrolled); // Should reset
    }

    #[test]
    fn test_set_offset() {
        let mut state = ScrollState::new();
        state.max_scroll = 50;

        state.set_offset(30);
        assert_eq!(state.unified_scroll, 30);

        state.set_offset(100); // Should be clamped
        assert_eq!(state.unified_scroll, 50);
    }

    #[test]
    fn test_update_limits() {
        let mut state = ScrollState::new();
        state.unified_scroll = 100;

        state.update_limits(50, 200, 150);

        assert_eq!(state.max_scroll, 50);
        assert_eq!(state.total_content_lines, 200);
        assert_eq!(state.input_section_start, 150);
        assert_eq!(state.unified_scroll, 50); // Clamped
    }

    #[test]
    fn test_momentum_scrolling() {
        let mut state = ScrollState::new();
        state.max_scroll = 100;
        state.scroll_position = 50.0;
        state.unified_scroll = 50;

        state.add_velocity(5.0);
        assert!(state.has_momentum());

        let changed = state.apply_velocity();
        assert!(changed);
        assert!(state.scroll_position > 50.0);

        // Keep applying until momentum dies
        for _ in 0..100 {
            state.apply_velocity();
        }
        assert!(!state.has_momentum());
    }

    #[test]
    fn test_velocity_capped() {
        let mut state = ScrollState::new();
        state.max_scroll = 1000;

        state.add_velocity(100.0);
        assert_eq!(state.scroll_velocity, 50.0); // Capped

        state.add_velocity(-200.0);
        assert_eq!(state.scroll_velocity, -50.0); // Capped
    }

    #[test]
    fn test_stop_momentum() {
        let mut state = ScrollState::new();
        state.scroll_velocity = 10.0;

        state.stop_momentum();
        assert_eq!(state.scroll_velocity, 0.0);
        assert!(!state.has_momentum());
    }

    #[test]
    fn test_boundary_hit_tracking() {
        let mut state = ScrollState::new();
        assert!(state.get_boundary_hit().is_none());

        state.record_boundary_hit(ScrollBoundary::Top, 100);
        assert!(matches!(state.get_boundary_hit(), Some(ScrollBoundary::Top)));
        assert_eq!(state.boundary_hit_tick, 100);

        // Should not clear before duration
        state.maybe_clear_boundary_hit(105, 10);
        assert!(state.get_boundary_hit().is_some());

        // Should clear after duration
        state.maybe_clear_boundary_hit(115, 10);
        assert!(state.get_boundary_hit().is_none());
    }

    #[test]
    fn test_clear_boundary_hit() {
        let mut state = ScrollState::new();
        state.scroll_boundary_hit = Some(ScrollBoundary::Bottom);

        state.clear_boundary_hit();
        assert!(state.scroll_boundary_hit.is_none());
    }

    #[test]
    fn test_reset() {
        let mut state = ScrollState::new();
        state.max_scroll = 100;
        state.unified_scroll = 50;
        state.user_has_scrolled = true;
        state.scroll_velocity = 5.0;
        state.scroll_boundary_hit = Some(ScrollBoundary::Top);

        state.reset();

        assert_eq!(state.max_scroll, 0);
        assert_eq!(state.unified_scroll, 0);
        assert!(!state.user_has_scrolled);
        assert_eq!(state.scroll_velocity, 0.0);
        assert!(state.scroll_boundary_hit.is_none());
    }

    #[test]
    fn test_debug_impl() {
        let state = ScrollState::new();
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("ScrollState"));
        assert!(debug_str.contains("unified_scroll"));
    }
}
