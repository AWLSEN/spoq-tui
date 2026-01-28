//! Debouncing logic for credential change events.
//!
//! When editors save files, they often trigger multiple write events
//! in rapid succession. The debouncer coalesces these into a single
//! sync operation.
//!
//! # How it works
//!
//! 1. First change event starts a 500ms timer
//! 2. Subsequent changes within the window are ignored (timer already running)
//! 3. When timer fires, trigger sync if changes are still pending
//! 4. Reset state after sync starts

use std::time::{Duration, Instant};

/// Debounce window - changes within this window are coalesced
pub const DEBOUNCE_MS: u64 = 500;

/// Manages debouncing of credential change events.
///
/// Coalesces rapid file changes (like editor save events) into a single
/// sync operation by waiting for a quiet period before triggering.
#[derive(Debug)]
pub struct Debouncer {
    /// Timestamp of first change in current burst
    pending_since: Option<Instant>,
    /// Whether a debounce timer is active
    timer_active: bool,
}

impl Debouncer {
    /// Create a new debouncer with no pending changes.
    pub fn new() -> Self {
        Self {
            pending_since: None,
            timer_active: false,
        }
    }

    /// Record a change event.
    ///
    /// Returns true if we should start a debounce timer (first change in burst).
    /// Returns false if a timer is already running (subsequent change in burst).
    pub fn on_change(&mut self) -> bool {
        let now = Instant::now();

        if self.pending_since.is_none() {
            // First change in a potential burst
            self.pending_since = Some(now);
            self.timer_active = true;
            tracing::debug!(
                "Debouncer: first change, starting {}ms timer",
                DEBOUNCE_MS
            );
            true // Start timer
        } else {
            // Already have a pending change, timer should already be running
            tracing::trace!("Debouncer: subsequent change, timer already running");
            false
        }
    }

    /// Called when debounce timer fires.
    ///
    /// Returns true if we should trigger a sync (there are pending changes).
    /// Returns false if no pending changes (shouldn't happen in normal flow).
    pub fn on_timer_fire(&mut self) -> bool {
        self.timer_active = false;

        if self.pending_since.is_some() {
            tracing::debug!("Debouncer: timer fired, triggering sync");
            self.pending_since = None;
            true // Sync now
        } else {
            tracing::trace!("Debouncer: timer fired but no pending changes");
            false // No pending changes
        }
    }

    /// Check if we have a pending change.
    pub fn has_pending(&self) -> bool {
        self.pending_since.is_some()
    }

    /// Check if a timer is currently active.
    pub fn is_timer_active(&self) -> bool {
        self.timer_active
    }

    /// Reset state (e.g., on sync start or error).
    pub fn reset(&mut self) {
        self.pending_since = None;
        self.timer_active = false;
        tracing::trace!("Debouncer: state reset");
    }

    /// Get the time since the first pending change.
    ///
    /// Returns None if no change is pending.
    pub fn time_since_first_change(&self) -> Option<Duration> {
        self.pending_since.map(|t| t.elapsed())
    }
}

impl Default for Debouncer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debouncer_new() {
        let debouncer = Debouncer::new();
        assert!(!debouncer.has_pending());
        assert!(!debouncer.is_timer_active());
    }

    #[test]
    fn test_first_change_starts_timer() {
        let mut debouncer = Debouncer::new();
        assert!(debouncer.on_change()); // Should start timer
        assert!(debouncer.has_pending());
        assert!(debouncer.is_timer_active());
    }

    #[test]
    fn test_subsequent_changes_no_timer() {
        let mut debouncer = Debouncer::new();
        assert!(debouncer.on_change()); // First - start timer
        assert!(!debouncer.on_change()); // Second - no new timer
        assert!(!debouncer.on_change()); // Third - no new timer

        // Should still have pending and timer active
        assert!(debouncer.has_pending());
        assert!(debouncer.is_timer_active());
    }

    #[test]
    fn test_timer_fire_triggers_sync() {
        let mut debouncer = Debouncer::new();
        debouncer.on_change();
        assert!(debouncer.on_timer_fire()); // Should sync

        // Should be reset after timer fire
        assert!(!debouncer.has_pending());
        assert!(!debouncer.is_timer_active());
    }

    #[test]
    fn test_timer_fire_without_pending_no_sync() {
        let mut debouncer = Debouncer::new();
        assert!(!debouncer.on_timer_fire()); // No pending, no sync
    }

    #[test]
    fn test_reset() {
        let mut debouncer = Debouncer::new();
        debouncer.on_change();
        assert!(debouncer.has_pending());
        assert!(debouncer.is_timer_active());

        debouncer.reset();
        assert!(!debouncer.has_pending());
        assert!(!debouncer.is_timer_active());
    }

    #[test]
    fn test_time_since_first_change() {
        let mut debouncer = Debouncer::new();

        // No pending change
        assert!(debouncer.time_since_first_change().is_none());

        // After change
        debouncer.on_change();
        let duration = debouncer.time_since_first_change();
        assert!(duration.is_some());
        assert!(duration.unwrap() < Duration::from_millis(100)); // Should be very recent
    }

    #[test]
    fn test_change_after_timer_fire_starts_new_cycle() {
        let mut debouncer = Debouncer::new();

        // First cycle
        assert!(debouncer.on_change());
        assert!(debouncer.on_timer_fire());

        // New cycle should start fresh
        assert!(debouncer.on_change()); // Should return true again
        assert!(debouncer.has_pending());
    }

    #[test]
    fn test_debounce_constant() {
        assert_eq!(DEBOUNCE_MS, 500);
    }
}
