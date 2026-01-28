//! State tracking for credential change detection and sync coordination.

use std::time::{Duration, Instant};

/// Exponential backoff for sync failures.
///
/// Doubles the delay on each failure up to a maximum cap.
/// Resets to base delay on success.
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    /// Base delay (first retry)
    base_delay: Duration,
    /// Current delay (increases on each failure)
    current_delay: Duration,
    /// Maximum delay cap
    max_delay: Duration,
    /// Number of consecutive failures
    failure_count: u32,
    /// Time of last failure (for calculating next retry time)
    last_failure: Option<Instant>,
}

impl ExponentialBackoff {
    /// Create a new backoff with default settings.
    ///
    /// Default: 30s base, 300s (5 min) max.
    pub fn new() -> Self {
        Self {
            base_delay: Duration::from_secs(30),
            current_delay: Duration::from_secs(30),
            max_delay: Duration::from_secs(300), // 5 minutes cap
            failure_count: 0,
            last_failure: None,
        }
    }

    /// Create a new backoff with custom settings.
    pub fn with_config(base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            base_delay,
            current_delay: base_delay,
            max_delay,
            failure_count: 0,
            last_failure: None,
        }
    }

    /// Record a failure and advance the backoff.
    ///
    /// Doubles the current delay up to the maximum.
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());
        // Double the delay, up to max
        self.current_delay = (self.current_delay * 2).min(self.max_delay);
        tracing::warn!(
            "Sync failed (attempt {}), next retry in {:?}",
            self.failure_count,
            self.current_delay
        );
    }

    /// Reset backoff on success.
    ///
    /// Returns to base delay and clears failure count.
    pub fn reset(&mut self) {
        if self.failure_count > 0 {
            tracing::info!("Sync succeeded, resetting backoff");
        }
        self.failure_count = 0;
        self.current_delay = self.base_delay;
        self.last_failure = None;
    }

    /// Check if we're currently in backoff period.
    ///
    /// Returns true if the backoff period hasn't elapsed since last failure.
    pub fn is_in_backoff(&self) -> bool {
        if let Some(last_failure) = self.last_failure {
            last_failure.elapsed() < self.current_delay
        } else {
            false
        }
    }

    /// Time remaining until backoff expires.
    ///
    /// Returns None if not in backoff period.
    pub fn time_until_retry(&self) -> Option<Duration> {
        self.last_failure.and_then(|last| {
            let elapsed = last.elapsed();
            if elapsed < self.current_delay {
                Some(self.current_delay - elapsed)
            } else {
                None
            }
        })
    }

    /// Current failure count.
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Current delay duration.
    pub fn current_delay(&self) -> Duration {
        self.current_delay
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self::new()
    }
}

/// Main state for credential watching.
///
/// Tracks the state of the credential watcher system including:
/// - Last known Keychain hash (for change detection)
/// - Sync status and pending changes
/// - Backoff state for failed syncs
#[derive(Debug)]
pub struct CredentialWatchState {
    /// Hash of last seen Keychain credentials
    pub keychain_hash: Option<u64>,

    /// Timestamp of last successful sync
    pub last_sync: Option<Instant>,

    /// Backoff state for failed syncs
    pub backoff: ExponentialBackoff,

    /// Whether a sync is currently in progress
    pub sync_pending: bool,

    /// Timestamp of pending change (for debouncing)
    pub pending_change: Option<Instant>,

    /// Whether the watcher system is enabled
    pub enabled: bool,
}

impl CredentialWatchState {
    /// Create a new watcher state with defaults.
    pub fn new() -> Self {
        Self {
            keychain_hash: None,
            last_sync: None,
            backoff: ExponentialBackoff::new(),
            sync_pending: false,
            pending_change: None,
            enabled: true,
        }
    }

    /// Check if we should trigger a sync now.
    ///
    /// Returns true if:
    /// - Watching is enabled
    /// - No sync is currently pending
    /// - We're not in a backoff period
    pub fn should_sync(&self) -> bool {
        self.enabled && !self.sync_pending && !self.backoff.is_in_backoff()
    }

    /// Mark sync as started.
    ///
    /// Sets sync_pending to true and clears pending_change.
    pub fn sync_started(&mut self) {
        self.sync_pending = true;
        self.pending_change = None;
    }

    /// Mark sync as completed successfully.
    ///
    /// Clears sync_pending, updates last_sync, and resets backoff.
    pub fn sync_succeeded(&mut self) {
        self.sync_pending = false;
        self.last_sync = Some(Instant::now());
        self.backoff.reset();
    }

    /// Mark sync as failed.
    ///
    /// Clears sync_pending and records failure in backoff.
    pub fn sync_failed(&mut self) {
        self.sync_pending = false;
        self.backoff.record_failure();
    }

    /// Disable the watcher system.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Enable the watcher system.
    pub fn enable(&mut self) {
        self.enabled = true;
    }
}

impl Default for CredentialWatchState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_exponential_backoff_new() {
        let backoff = ExponentialBackoff::new();
        assert_eq!(backoff.failure_count(), 0);
        assert_eq!(backoff.current_delay(), Duration::from_secs(30));
        assert!(!backoff.is_in_backoff());
    }

    #[test]
    fn test_exponential_backoff_doubles() {
        let mut backoff = ExponentialBackoff::with_config(
            Duration::from_millis(10),
            Duration::from_millis(100),
        );

        backoff.record_failure();
        assert_eq!(backoff.failure_count(), 1);
        assert_eq!(backoff.current_delay(), Duration::from_millis(20));

        backoff.record_failure();
        assert_eq!(backoff.failure_count(), 2);
        assert_eq!(backoff.current_delay(), Duration::from_millis(40));

        backoff.record_failure();
        assert_eq!(backoff.failure_count(), 3);
        assert_eq!(backoff.current_delay(), Duration::from_millis(80));

        // Should cap at max
        backoff.record_failure();
        assert_eq!(backoff.failure_count(), 4);
        assert_eq!(backoff.current_delay(), Duration::from_millis(100)); // Capped
    }

    #[test]
    fn test_exponential_backoff_reset() {
        let mut backoff = ExponentialBackoff::with_config(
            Duration::from_millis(10),
            Duration::from_millis(100),
        );

        backoff.record_failure();
        backoff.record_failure();
        assert_eq!(backoff.failure_count(), 2);

        backoff.reset();
        assert_eq!(backoff.failure_count(), 0);
        assert_eq!(backoff.current_delay(), Duration::from_millis(10));
        assert!(!backoff.is_in_backoff());
    }

    #[test]
    fn test_exponential_backoff_is_in_backoff() {
        let mut backoff = ExponentialBackoff::with_config(
            Duration::from_millis(20),
            Duration::from_millis(100),
        );

        assert!(!backoff.is_in_backoff());

        backoff.record_failure();
        assert!(backoff.is_in_backoff());

        // Wait for backoff to expire (with margin for CI slowness)
        sleep(Duration::from_millis(50));
        assert!(!backoff.is_in_backoff());
    }

    #[test]
    fn test_credential_watch_state_should_sync() {
        let mut state = CredentialWatchState::new();
        assert!(state.should_sync());

        // Disable
        state.disable();
        assert!(!state.should_sync());

        // Re-enable
        state.enable();
        assert!(state.should_sync());

        // Start sync
        state.sync_started();
        assert!(!state.should_sync()); // sync_pending is true
    }

    #[test]
    fn test_credential_watch_state_sync_lifecycle() {
        let mut state = CredentialWatchState::new();

        // Start sync
        state.sync_started();
        assert!(state.sync_pending);
        assert!(state.pending_change.is_none());

        // Sync succeeds
        state.sync_succeeded();
        assert!(!state.sync_pending);
        assert!(state.last_sync.is_some());
        assert_eq!(state.backoff.failure_count(), 0);

        // Start another sync
        state.sync_started();

        // Sync fails
        state.sync_failed();
        assert!(!state.sync_pending);
        assert_eq!(state.backoff.failure_count(), 1);
    }
}
