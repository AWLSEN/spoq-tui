//! Cursor blink state management module.
//!
//! This module provides a [`CursorBlinkState`] struct that encapsulates all
//! cursor blink logic with proper blinkwait behavior, similar to Vim's
//! blinkwait/blinkon/blinkoff settings.

/// Manages cursor blink state with blinkwait behavior.
///
/// The cursor remains visible for a "blinkwait" period after any cursor activity
/// (movement, typing, etc.), then begins blinking with configurable on/off cycles.
///
/// Default timing at 60fps:
/// - blinkwait: 31 ticks (~500ms) - cursor stays visible after activity
/// - blink_half_cycle: 16 ticks (~250ms) - duration of each visible/hidden phase
#[derive(Debug, Clone)]
pub struct CursorBlinkState {
    /// Tick count when cursor last had activity (moved, typed, etc.)
    last_activity_tick: u64,
    /// Current visibility state of the cursor
    is_visible: bool,
    /// Number of ticks to wait before starting to blink (default: 31 ticks = ~500ms at 60fps)
    blinkwait_ticks: u64,
    /// Number of ticks per blink phase (on or off) (default: 16 ticks = ~250ms at 60fps)
    blink_half_cycle_ticks: u64,
}

impl Default for CursorBlinkState {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorBlinkState {
    /// Create a new CursorBlinkState with default timing.
    ///
    /// Defaults:
    /// - blinkwait: 31 ticks (~500ms at 60fps)
    /// - blink_half_cycle: 16 ticks (~250ms at 60fps)
    pub fn new() -> Self {
        Self {
            last_activity_tick: 0,
            is_visible: true,
            blinkwait_ticks: 31,        // ~500ms at 60fps
            blink_half_cycle_ticks: 16, // ~250ms at 60fps
        }
    }

    /// Reset the blink timer due to cursor activity.
    ///
    /// Call this whenever the cursor moves, text is typed, or any other
    /// cursor-related activity occurs. This makes the cursor visible and
    /// restarts the blinkwait period.
    pub fn reset(&mut self, current_tick: u64) {
        self.last_activity_tick = current_tick;
        self.is_visible = true;
    }

    /// Update the blink state based on the current tick.
    ///
    /// Returns `true` if the visibility state changed, `false` otherwise.
    /// Use this to determine if a redraw is needed.
    ///
    /// # Algorithm
    ///
    /// 1. During blinkwait period: cursor stays visible
    /// 2. After blinkwait: cursor blinks based on blink_half_cycle_ticks
    ///    - First half of each full cycle: visible
    ///    - Second half of each full cycle: hidden
    pub fn update(&mut self, current_tick: u64) -> bool {
        let ticks_since_activity = current_tick.saturating_sub(self.last_activity_tick);

        let new_visibility = if ticks_since_activity < self.blinkwait_ticks {
            // Still in blinkwait period - cursor stays visible
            true
        } else {
            // Blinking period - calculate visibility based on blink cycle
            let ticks_into_blink = ticks_since_activity - self.blinkwait_ticks;
            let full_cycle_ticks = self.blink_half_cycle_ticks * 2;
            let position_in_cycle = ticks_into_blink % full_cycle_ticks;

            // Visible during first half of cycle, hidden during second half
            position_in_cycle < self.blink_half_cycle_ticks
        };

        let changed = self.is_visible != new_visibility;
        self.is_visible = new_visibility;
        changed
    }

    /// Get the current visibility state of the cursor.
    /// Always returns true for solid caret (no blinking).
    pub fn is_visible(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_visible_cursor() {
        let state = CursorBlinkState::new();
        assert!(state.is_visible());
    }

    #[test]
    fn test_default_timing_values() {
        let state = CursorBlinkState::new();
        assert_eq!(state.blinkwait_ticks, 31);
        assert_eq!(state.blink_half_cycle_ticks, 16);
    }

    #[test]
    fn test_reset_makes_cursor_visible() {
        let mut state = CursorBlinkState::new();
        state.is_visible = false;
        state.reset(100);
        assert!(state.is_visible());
        assert_eq!(state.last_activity_tick, 100);
    }

    #[test]
    fn test_cursor_stays_visible_during_blinkwait() {
        let mut state = CursorBlinkState::new();
        state.reset(0);

        // Should stay visible throughout blinkwait period (31 ticks)
        for tick in 0..31 {
            state.update(tick);
            assert!(
                state.is_visible(),
                "Cursor should be visible at tick {} during blinkwait",
                tick
            );
        }
    }

    #[test]
    fn test_cursor_always_visible_solid_mode() {
        // Cursor is now always visible (solid caret mode)
        let mut state = CursorBlinkState::new();
        state.reset(0);

        // Move past blinkwait period
        state.update(31);
        assert!(
            state.is_visible(),
            "Cursor should always be visible in solid mode"
        );

        // Even in what would be hidden phase, cursor stays visible
        state.update(47);
        assert!(
            state.is_visible(),
            "Cursor should always be visible in solid mode"
        );
    }

    #[test]
    fn test_cursor_stays_visible_at_all_ticks() {
        // Cursor is now always visible (solid caret mode)
        let mut state = CursorBlinkState::new();
        state.reset(0);

        // Test various tick values - cursor should always be visible
        for tick in [0, 31, 47, 63, 79, 100, 200, 500] {
            state.update(tick);
            assert!(
                state.is_visible(),
                "Cursor should be visible at tick {}",
                tick
            );
        }
    }

    #[test]
    fn test_update_returns_change_status() {
        // Internal state still tracks blink cycles, but is_visible() always returns true
        let mut state = CursorBlinkState::new();
        state.reset(0);

        // No change during blinkwait
        let changed = state.update(10);
        assert!(!changed, "Should return false when visibility unchanged");

        // Internal state changes when entering hidden phase
        let changed = state.update(47);
        assert!(changed, "Should return true when internal state changes");

        // Cursor is still visible externally
        assert!(state.is_visible(), "External visibility always true");
    }

    #[test]
    fn test_reset_always_keeps_visible() {
        // Cursor is now always visible (solid caret mode)
        let mut state = CursorBlinkState::new();
        state.reset(0);

        // Move to what would be hidden phase
        state.update(50);
        assert!(state.is_visible(), "Cursor should be visible");

        // Reset at tick 100
        state.reset(100);
        assert!(state.is_visible(), "Cursor should be visible after reset");

        // Should stay visible always
        state.update(120);
        assert!(state.is_visible(), "Cursor should be visible");

        state.update(147);
        assert!(state.is_visible(), "Cursor should always be visible");
    }

    #[test]
    fn test_default_impl() {
        let state1 = CursorBlinkState::new();
        let state2 = CursorBlinkState::default();
        assert_eq!(state1.blinkwait_ticks, state2.blinkwait_ticks);
        assert_eq!(state1.blink_half_cycle_ticks, state2.blink_half_cycle_ticks);
        assert_eq!(state1.is_visible, state2.is_visible);
    }

    #[test]
    fn test_saturating_sub_handles_overflow() {
        let mut state = CursorBlinkState::new();
        state.last_activity_tick = 100;

        // Current tick less than last_activity should not panic
        let changed = state.update(50);
        // Should be visible (as if no time has passed)
        assert!(state.is_visible());
        assert!(!changed);
    }
}
