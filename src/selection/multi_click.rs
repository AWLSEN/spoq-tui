//! Multi-click detection for text selection
//!
//! This module provides click pattern detection to distinguish between:
//! - Single-click: Set cursor position (character selection mode)
//! - Double-click: Select word (word selection mode)
//! - Triple-click: Select line (line selection mode)
//!
//! # Usage
//!
//! ```ignore
//! use spoq::selection::multi_click::ClickDetector;
//! use spoq::selection::ScreenPosition;
//!
//! let mut detector = ClickDetector::new();
//!
//! // On mouse down
//! let mode = detector.register_click(ScreenPosition::new(10, 5));
//! // mode will be Character, Word, or Line based on click pattern
//! ```

use std::time::Instant;

use super::position::ScreenPosition;
use super::state::SelectionMode;

/// Time threshold for multi-click detection (300ms is standard)
const MULTI_CLICK_THRESHOLD_MS: u128 = 300;

/// Maximum distance (in cells) for clicks to be considered part of a multi-click
const MAX_CLICK_DISTANCE: u16 = 3;

/// Tracks click patterns to detect single, double, and triple clicks
///
/// The detector maintains state about the last click position and timestamp
/// to determine if subsequent clicks should be counted as part of a
/// multi-click sequence.
///
/// # Click Counting Rules
///
/// 1. A click counts as part of a multi-click if:
///    - It occurs within 300ms of the previous click
///    - It is within 3 cells of the previous click position
///
/// 2. Click count cycles: 1 -> 2 -> 3 -> 1 (resets after triple-click)
///
/// 3. If conditions aren't met, the count resets to 1 (single-click)
#[derive(Debug, Clone)]
pub struct ClickDetector {
    /// Position of the last click
    last_click_position: Option<ScreenPosition>,
    /// Timestamp of the last click
    last_click_time: Option<Instant>,
    /// Current click count in the multi-click sequence (1, 2, or 3)
    click_count: u8,
}

impl Default for ClickDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ClickDetector {
    /// Create a new click detector with no previous click history
    pub fn new() -> Self {
        Self {
            last_click_position: None,
            last_click_time: None,
            click_count: 0,
        }
    }

    /// Register a click and return the appropriate selection mode
    ///
    /// This method should be called on every mouse down event. It will:
    /// 1. Check if this click is part of a multi-click sequence
    /// 2. Update internal state
    /// 3. Return the selection mode based on the click count
    ///
    /// # Arguments
    /// * `position` - The screen position where the click occurred
    ///
    /// # Returns
    /// - `SelectionMode::Character` for single-click
    /// - `SelectionMode::Word` for double-click
    /// - `SelectionMode::Line` for triple-click
    pub fn register_click(&mut self, position: ScreenPosition) -> SelectionMode {
        let now = Instant::now();

        // Check if this click qualifies as a multi-click
        let is_multi_click = self.is_multi_click(position, now);

        if is_multi_click {
            // Increment click count, cycling 1 -> 2 -> 3 -> 1
            self.click_count = match self.click_count {
                1 => 2,
                2 => 3,
                _ => 1, // Reset after triple-click or initial state
            };
        } else {
            // Not a multi-click, reset to single-click
            self.click_count = 1;
        }

        // Update last click state
        self.last_click_position = Some(position);
        self.last_click_time = Some(now);

        // Return selection mode based on click count
        self.get_selection_mode()
    }

    /// Get the current click count (1, 2, or 3)
    pub fn click_count(&self) -> u8 {
        self.click_count
    }

    /// Get the selection mode for the current click count
    pub fn get_selection_mode(&self) -> SelectionMode {
        match self.click_count {
            2 => SelectionMode::Word,
            3 => SelectionMode::Line,
            _ => SelectionMode::Character,
        }
    }

    /// Reset the click detector to its initial state
    pub fn reset(&mut self) {
        self.last_click_position = None;
        self.last_click_time = None;
        self.click_count = 0;
    }

    /// Check if a click qualifies as part of a multi-click sequence
    fn is_multi_click(&self, position: ScreenPosition, now: Instant) -> bool {
        match (self.last_click_position, self.last_click_time) {
            (Some(last_pos), Some(last_time)) => {
                // Check time threshold
                let elapsed = now.duration_since(last_time).as_millis();
                if elapsed > MULTI_CLICK_THRESHOLD_MS {
                    return false;
                }

                // Check position proximity
                self.is_position_close(position, last_pos)
            }
            _ => false, // No previous click to compare against
        }
    }

    /// Check if two positions are close enough for multi-click
    fn is_position_close(&self, pos1: ScreenPosition, pos2: ScreenPosition) -> bool {
        let dx = pos1.x.abs_diff(pos2.x);
        let dy = pos1.y.abs_diff(pos2.y);

        dx <= MAX_CLICK_DISTANCE && dy <= MAX_CLICK_DISTANCE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    // ============= ClickDetector Basic Tests =============

    #[test]
    fn test_click_detector_new() {
        let detector = ClickDetector::new();
        assert_eq!(detector.click_count(), 0);
        assert!(detector.last_click_position.is_none());
        assert!(detector.last_click_time.is_none());
    }

    #[test]
    fn test_click_detector_default() {
        let detector = ClickDetector::default();
        assert_eq!(detector.click_count(), 0);
    }

    #[test]
    fn test_single_click() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(10, 5);

        let mode = detector.register_click(pos);

        assert_eq!(mode, SelectionMode::Character);
        assert_eq!(detector.click_count(), 1);
    }

    #[test]
    fn test_double_click_same_position() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(10, 5);

        // First click
        detector.register_click(pos);
        // Second click immediately at same position
        let mode = detector.register_click(pos);

        assert_eq!(mode, SelectionMode::Word);
        assert_eq!(detector.click_count(), 2);
    }

    #[test]
    fn test_triple_click_same_position() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(10, 5);

        // Three clicks in quick succession
        detector.register_click(pos);
        detector.register_click(pos);
        let mode = detector.register_click(pos);

        assert_eq!(mode, SelectionMode::Line);
        assert_eq!(detector.click_count(), 3);
    }

    #[test]
    fn test_quadruple_click_resets_to_single() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(10, 5);

        // Four clicks should cycle back to single
        detector.register_click(pos);
        detector.register_click(pos);
        detector.register_click(pos);
        let mode = detector.register_click(pos);

        assert_eq!(mode, SelectionMode::Character);
        assert_eq!(detector.click_count(), 1);
    }

    // ============= Position Proximity Tests =============

    #[test]
    fn test_double_click_nearby_position() {
        let mut detector = ClickDetector::new();
        let pos1 = ScreenPosition::new(10, 5);
        let pos2 = ScreenPosition::new(12, 6); // Within 3 cells

        detector.register_click(pos1);
        let mode = detector.register_click(pos2);

        assert_eq!(mode, SelectionMode::Word);
        assert_eq!(detector.click_count(), 2);
    }

    #[test]
    fn test_double_click_at_max_distance() {
        let mut detector = ClickDetector::new();
        let pos1 = ScreenPosition::new(10, 5);
        let pos2 = ScreenPosition::new(13, 8); // Exactly 3 cells in both x and y

        detector.register_click(pos1);
        let mode = detector.register_click(pos2);

        assert_eq!(mode, SelectionMode::Word);
        assert_eq!(detector.click_count(), 2);
    }

    #[test]
    fn test_click_too_far_resets() {
        let mut detector = ClickDetector::new();
        let pos1 = ScreenPosition::new(10, 5);
        let pos2 = ScreenPosition::new(20, 5); // 10 cells away, too far

        detector.register_click(pos1);
        let mode = detector.register_click(pos2);

        assert_eq!(mode, SelectionMode::Character);
        assert_eq!(detector.click_count(), 1);
    }

    #[test]
    fn test_click_far_in_y_resets() {
        let mut detector = ClickDetector::new();
        let pos1 = ScreenPosition::new(10, 5);
        let pos2 = ScreenPosition::new(10, 15); // Same x, but y is 10 cells away

        detector.register_click(pos1);
        let mode = detector.register_click(pos2);

        assert_eq!(mode, SelectionMode::Character);
        assert_eq!(detector.click_count(), 1);
    }

    // ============= Timeout Tests =============

    #[test]
    fn test_click_timeout_resets() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(10, 5);

        // First click
        detector.register_click(pos);

        // Wait longer than the threshold (300ms + buffer)
        thread::sleep(Duration::from_millis(350));

        // Second click after timeout should be a new single click
        let mode = detector.register_click(pos);

        assert_eq!(mode, SelectionMode::Character);
        assert_eq!(detector.click_count(), 1);
    }

    #[test]
    fn test_click_within_threshold() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(10, 5);

        // First click
        detector.register_click(pos);

        // Wait less than the threshold
        thread::sleep(Duration::from_millis(100));

        // Second click within time should be double-click
        let mode = detector.register_click(pos);

        assert_eq!(mode, SelectionMode::Word);
        assert_eq!(detector.click_count(), 2);
    }

    // ============= Reset Tests =============

    #[test]
    fn test_reset() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(10, 5);

        // Build up some state
        detector.register_click(pos);
        detector.register_click(pos);
        assert_eq!(detector.click_count(), 2);

        // Reset
        detector.reset();

        assert_eq!(detector.click_count(), 0);
        assert!(detector.last_click_position.is_none());
        assert!(detector.last_click_time.is_none());
    }

    #[test]
    fn test_click_after_reset() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(10, 5);

        detector.register_click(pos);
        detector.register_click(pos);
        detector.reset();

        // Click after reset should be single-click
        let mode = detector.register_click(pos);

        assert_eq!(mode, SelectionMode::Character);
        assert_eq!(detector.click_count(), 1);
    }

    // ============= Selection Mode Tests =============

    #[test]
    fn test_get_selection_mode() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(10, 5);

        // Initial state (0 clicks)
        assert_eq!(detector.get_selection_mode(), SelectionMode::Character);

        // After single click
        detector.register_click(pos);
        assert_eq!(detector.get_selection_mode(), SelectionMode::Character);

        // After double click
        detector.register_click(pos);
        assert_eq!(detector.get_selection_mode(), SelectionMode::Word);

        // After triple click
        detector.register_click(pos);
        assert_eq!(detector.get_selection_mode(), SelectionMode::Line);
    }

    // ============= Edge Cases =============

    #[test]
    fn test_click_at_origin() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(0, 0);

        let mode = detector.register_click(pos);
        assert_eq!(mode, SelectionMode::Character);

        let mode = detector.register_click(pos);
        assert_eq!(mode, SelectionMode::Word);
    }

    #[test]
    fn test_click_at_max_coordinates() {
        let mut detector = ClickDetector::new();
        let pos = ScreenPosition::new(u16::MAX, u16::MAX);

        let mode = detector.register_click(pos);
        assert_eq!(mode, SelectionMode::Character);

        let mode = detector.register_click(pos);
        assert_eq!(mode, SelectionMode::Word);
    }

    #[test]
    fn test_alternating_positions() {
        let mut detector = ClickDetector::new();
        let pos1 = ScreenPosition::new(10, 5);
        let pos2 = ScreenPosition::new(50, 25);

        // Click at position 1
        detector.register_click(pos1);
        // Click at position 2 (far away) - should reset
        let mode = detector.register_click(pos2);
        assert_eq!(mode, SelectionMode::Character);

        // Click at position 2 again - should be double click
        let mode = detector.register_click(pos2);
        assert_eq!(mode, SelectionMode::Word);
    }

    // ============= Internal Helper Tests =============

    #[test]
    fn test_is_position_close() {
        let detector = ClickDetector::new();

        // Same position
        assert!(detector.is_position_close(
            ScreenPosition::new(10, 10),
            ScreenPosition::new(10, 10)
        ));

        // Within threshold
        assert!(detector.is_position_close(
            ScreenPosition::new(10, 10),
            ScreenPosition::new(12, 12)
        ));

        // Exactly at threshold
        assert!(detector.is_position_close(
            ScreenPosition::new(10, 10),
            ScreenPosition::new(13, 13)
        ));

        // Beyond threshold
        assert!(!detector.is_position_close(
            ScreenPosition::new(10, 10),
            ScreenPosition::new(14, 14)
        ));

        // X beyond, Y within
        assert!(!detector.is_position_close(
            ScreenPosition::new(10, 10),
            ScreenPosition::new(20, 11)
        ));

        // X within, Y beyond
        assert!(!detector.is_position_close(
            ScreenPosition::new(10, 10),
            ScreenPosition::new(11, 20)
        ));
    }
}
