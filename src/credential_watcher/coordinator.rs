//! Coordinates credential change detection and sync triggering.
//!
//! This is the main orchestration layer that ties together:
//! - File watcher events
//! - Keychain poller events
//! - Debouncing
//! - Backoff management
//! - Sync triggering

use std::time::Duration;

use super::debouncer::{Debouncer, DEBOUNCE_MS};
use super::state::CredentialWatchState;
use crate::app::AppMessage;
use tokio::sync::mpsc;

/// Spawn a debounce timer that fires after DEBOUNCE_MS.
///
/// When the timer fires, sends `CredentialDebounceExpired` to the message channel.
pub fn spawn_debounce_timer(message_tx: mpsc::UnboundedSender<AppMessage>) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;
        let _ = message_tx.send(AppMessage::CredentialDebounceExpired);
    });
}

/// Handles a credential change event (from file watcher or Keychain poller).
///
/// Applies debouncing to coalesce rapid changes. Only triggers sync
/// if not already pending and not in backoff period.
pub fn handle_credential_change(
    state: &mut CredentialWatchState,
    debouncer: &mut Debouncer,
    message_tx: &mpsc::UnboundedSender<AppMessage>,
    source: &str,
) {
    tracing::info!("Credential change detected from: {}", source);

    // Check if we should even try to sync
    if !state.enabled {
        tracing::debug!("Credential watching disabled, ignoring change");
        return;
    }

    if state.sync_pending {
        tracing::debug!("Sync already pending, ignoring change");
        return;
    }

    if state.backoff.is_in_backoff() {
        if let Some(remaining) = state.backoff.time_until_retry() {
            tracing::debug!("In backoff period, {:?} until next retry", remaining);
        }
        return;
    }

    // Debounce the change
    if debouncer.on_change() {
        tracing::debug!("Starting debounce timer ({}ms)", DEBOUNCE_MS);
        spawn_debounce_timer(message_tx.clone());
    }
}

/// Handles debounce timer expiration.
///
/// If there are pending changes and sync is allowed, triggers the sync.
pub fn handle_debounce_expired(
    state: &mut CredentialWatchState,
    debouncer: &mut Debouncer,
    message_tx: &mpsc::UnboundedSender<AppMessage>,
) {
    if !debouncer.on_timer_fire() {
        tracing::debug!("Debounce timer fired but no pending changes");
        return;
    }

    if !state.should_sync() {
        tracing::debug!("Debounce expired but sync not allowed");
        debouncer.reset();
        return;
    }

    tracing::info!("Debounce expired, triggering auto-sync");
    state.sync_started();
    debouncer.reset();

    // Trigger sync using existing infrastructure
    let _ = message_tx.send(AppMessage::TriggerSync);
}

/// Handles sync completion (success).
///
/// Resets backoff state and updates last sync timestamp.
pub fn handle_sync_complete(state: &mut CredentialWatchState) {
    state.sync_succeeded();
    tracing::info!("Auto-sync completed successfully");
}

/// Handles sync failure.
///
/// Records failure in backoff state for exponential retry delays.
pub fn handle_sync_failed(state: &mut CredentialWatchState, error: &str) {
    state.sync_failed();
    tracing::warn!("Auto-sync failed: {}", error);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_credential_change_disabled() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut state = CredentialWatchState::new();
        let mut debouncer = Debouncer::new();

        state.disable();
        handle_credential_change(&mut state, &mut debouncer, &tx, "test");

        // Debouncer should not have been triggered
        assert!(!debouncer.has_pending());
    }

    #[test]
    fn test_handle_credential_change_sync_pending() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut state = CredentialWatchState::new();
        let mut debouncer = Debouncer::new();

        state.sync_started();
        handle_credential_change(&mut state, &mut debouncer, &tx, "test");

        // Debouncer should not have been triggered
        assert!(!debouncer.has_pending());
    }

    #[tokio::test]
    async fn test_handle_credential_change_starts_debounce() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut state = CredentialWatchState::new();
        let mut debouncer = Debouncer::new();

        handle_credential_change(&mut state, &mut debouncer, &tx, "test");

        // Debouncer should be active
        assert!(debouncer.has_pending());
        assert!(debouncer.is_timer_active());
    }

    #[tokio::test]
    async fn test_handle_debounce_expired_triggers_sync() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = CredentialWatchState::new();
        let mut debouncer = Debouncer::new();

        // Simulate a change followed by debounce expiration
        debouncer.on_change();
        handle_debounce_expired(&mut state, &mut debouncer, &tx);

        // Should have sent TriggerSync
        let msg = rx.recv().await;
        assert!(matches!(msg, Some(AppMessage::TriggerSync)));

        // State should indicate sync started
        assert!(state.sync_pending);
    }

    #[tokio::test]
    async fn test_handle_debounce_expired_no_pending() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = CredentialWatchState::new();
        let mut debouncer = Debouncer::new();

        // No change, just fire timer
        handle_debounce_expired(&mut state, &mut debouncer, &tx);

        // Should not have sent anything
        let result = rx.try_recv();
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_sync_complete_resets_backoff() {
        let mut state = CredentialWatchState::new();

        // Simulate some failures
        state.sync_started();
        state.sync_failed();
        state.sync_started();
        state.sync_failed();
        assert_eq!(state.backoff.failure_count(), 2);

        // Now succeed
        state.sync_started();
        handle_sync_complete(&mut state);

        assert!(!state.sync_pending);
        assert_eq!(state.backoff.failure_count(), 0);
    }

    #[test]
    fn test_handle_sync_failed_increments_backoff() {
        let mut state = CredentialWatchState::new();

        state.sync_started();
        handle_sync_failed(&mut state, "test error");

        assert!(!state.sync_pending);
        assert_eq!(state.backoff.failure_count(), 1);
    }
}
