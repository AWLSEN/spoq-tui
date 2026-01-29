//! Keychain-based credential change detection via polling.
//!
//! macOS Keychain has no change notification API, so we must poll.
//! Polls every 30 seconds and compares hashes to detect changes.
//!
//! # Dependency Injection
//!
//! This module supports dependency injection for testing:
//! - Use `spawn_keychain_poller()` in production (uses RealKeychain)
//! - Use `spawn_keychain_poller_with_provider()` in tests (with MockKeychain)

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::keychain_provider::{KeychainProvider, RealKeychain};
use crate::app::AppMessage;
use crate::conductor::read_claude_keychain_credentials;

/// Polling interval for Keychain checks (30 seconds).
pub const POLL_INTERVAL_SECS: u64 = 30;

/// Compute a hash of the current Keychain credentials using direct read.
///
/// Returns None if credentials can't be read.
fn compute_keychain_hash() -> Option<u64> {
    let credentials = read_claude_keychain_credentials()?;
    let mut hasher = DefaultHasher::new();
    credentials.hash(&mut hasher);
    Some(hasher.finish())
}

/// Compute a hash using an injected provider (for testing).
///
/// Returns None if credentials can't be read.
pub fn compute_keychain_hash_with_provider(provider: &dyn KeychainProvider) -> Option<u64> {
    let credentials = provider.read_credentials()?;
    let mut hasher = DefaultHasher::new();
    credentials.hash(&mut hasher);
    Some(hasher.finish())
}

/// Spawn keychain poller with an injected provider (for testing).
///
/// This allows using MockKeychain in tests without touching real credentials.
///
/// # Arguments
///
/// * `message_tx` - Channel to send credential change notifications
/// * `provider` - Keychain provider (MockKeychain for tests, RealKeychain for production)
///
/// # Returns
///
/// A JoinHandle that can be used to abort the task on shutdown.
pub fn spawn_keychain_poller_with_provider(
    message_tx: mpsc::UnboundedSender<AppMessage>,
    provider: Arc<dyn KeychainProvider>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(
            "Keychain poller started (interval: {}s)",
            POLL_INTERVAL_SECS
        );

        // Capture initial hash (don't trigger sync on startup for existing creds)
        let mut last_hash = compute_keychain_hash_with_provider(provider.as_ref());
        tracing::debug!("Initial Keychain hash: {:?}", last_hash);

        let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));

        loop {
            interval.tick().await;

            let current_hash = compute_keychain_hash_with_provider(provider.as_ref());

            // Detect changes
            match (&last_hash, &current_hash) {
                (Some(old), Some(new)) if old != new => {
                    // Hash changed - credentials updated
                    tracing::info!(
                        "Keychain credentials changed (hash: {} -> {})",
                        old,
                        new
                    );
                    last_hash = current_hash;

                    if message_tx.send(AppMessage::CredentialKeychainChanged).is_err() {
                        tracing::debug!("Message channel closed, stopping Keychain poller");
                        break;
                    }
                }
                (None, Some(new)) => {
                    // Credentials appeared (first login)
                    tracing::info!("Keychain credentials appeared (hash: {})", new);
                    last_hash = current_hash;

                    if message_tx.send(AppMessage::CredentialKeychainChanged).is_err() {
                        break;
                    }
                }
                (Some(_), None) => {
                    // Credentials disappeared (logout)
                    tracing::warn!("Keychain credentials disappeared");
                    last_hash = None;
                    // Don't trigger sync - nothing to sync
                }
                _ => {
                    // No change
                    tracing::trace!("Keychain unchanged");
                }
            }
        }

        tracing::debug!("Keychain poller stopped");
    })
}

/// Spawn the Keychain polling task (production version).
///
/// Uses RealKeychain to read from macOS Keychain.
///
/// Returns a JoinHandle that can be used to abort the task on shutdown.
/// The task polls every 30 seconds and sends `CredentialKeychainChanged`
/// when a change is detected.
pub fn spawn_keychain_poller(message_tx: mpsc::UnboundedSender<AppMessage>) -> JoinHandle<()> {
    spawn_keychain_poller_with_provider(message_tx, Arc::new(RealKeychain))
}

/// Get the current Keychain hash without triggering any events.
///
/// Useful for initializing state.
pub fn get_current_hash() -> Option<u64> {
    compute_keychain_hash()
}

/// Get the current hash using an injected provider (for testing).
pub fn get_current_hash_with_provider(provider: &dyn KeychainProvider) -> Option<u64> {
    compute_keychain_hash_with_provider(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credential_watcher::keychain_provider::MockKeychain;

    #[test]
    fn test_hash_is_deterministic() {
        // Same input should produce same hash
        let hash1 = {
            let mut h = DefaultHasher::new();
            "test_credentials".hash(&mut h);
            h.finish()
        };
        let hash2 = {
            let mut h = DefaultHasher::new();
            "test_credentials".hash(&mut h);
            h.finish()
        };
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_input_different_hash() {
        let hash1 = {
            let mut h = DefaultHasher::new();
            "credentials_v1".hash(&mut h);
            h.finish()
        };
        let hash2 = {
            let mut h = DefaultHasher::new();
            "credentials_v2".hash(&mut h);
            h.finish()
        };
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_poll_interval_is_30_seconds() {
        assert_eq!(POLL_INTERVAL_SECS, 30);
    }

    // =========================================================================
    // MockKeychain-based tests
    // =========================================================================

    #[test]
    fn test_compute_hash_with_mock_provider() {
        let mock = MockKeychain::with_credentials("test_token");
        let hash = compute_keychain_hash_with_provider(&mock);
        assert!(hash.is_some());
    }

    #[test]
    fn test_compute_hash_with_empty_mock() {
        let mock = MockKeychain::new();
        let hash = compute_keychain_hash_with_provider(&mock);
        assert!(hash.is_none());
    }

    #[test]
    fn test_hash_changes_when_credentials_change() {
        let mock = MockKeychain::with_credentials("token_v1");

        let hash1 = compute_keychain_hash_with_provider(&mock);
        assert!(hash1.is_some());

        mock.set_credentials(Some("token_v2".to_string()));

        let hash2 = compute_keychain_hash_with_provider(&mock);
        assert!(hash2.is_some());
        assert_ne!(hash1, hash2, "Hash should change when credentials change");
    }

    #[test]
    fn test_hash_same_when_credentials_unchanged() {
        let mock = MockKeychain::with_credentials("same_token");

        let hash1 = compute_keychain_hash_with_provider(&mock);
        let hash2 = compute_keychain_hash_with_provider(&mock);

        assert_eq!(hash1, hash2, "Hash should be same when credentials unchanged");
    }

    #[test]
    fn test_credentials_appear() {
        let mock = MockKeychain::new();

        let hash_before = compute_keychain_hash_with_provider(&mock);
        assert!(hash_before.is_none(), "Should be None when no credentials");

        mock.set_credentials(Some("new_token".to_string()));

        let hash_after = compute_keychain_hash_with_provider(&mock);
        assert!(hash_after.is_some(), "Should be Some after credentials appear");
    }

    #[test]
    fn test_credentials_disappear() {
        let mock = MockKeychain::with_credentials("token");

        let hash_before = compute_keychain_hash_with_provider(&mock);
        assert!(hash_before.is_some());

        mock.set_credentials(None);

        let hash_after = compute_keychain_hash_with_provider(&mock);
        assert!(hash_after.is_none(), "Should be None after credentials disappear");
    }

    #[test]
    fn test_get_current_hash_with_provider() {
        let mock = MockKeychain::with_credentials("test");
        let hash = get_current_hash_with_provider(&mock);
        assert!(hash.is_some());
    }

    #[tokio::test]
    async fn test_change_detection_sends_message() {
        let mock = Arc::new(MockKeychain::with_credentials("token_v1"));
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Capture initial hash BEFORE spawning task
        let initial_hash = compute_keychain_hash_with_provider(mock.as_ref());

        // Change credentials BEFORE starting "poll"
        mock.set_credentials(Some("token_v2".to_string()));

        // Simulate poll cycle that detects the change
        let provider = mock.clone();
        let handle = tokio::spawn(async move {
            let current_hash = compute_keychain_hash_with_provider(provider.as_ref());

            if initial_hash != current_hash {
                let _ = tx.send(AppMessage::CredentialKeychainChanged);
            }
        });

        handle.await.unwrap();

        // Should have received change message
        let msg = rx.try_recv();
        assert!(msg.is_ok(), "Should receive CredentialKeychainChanged message");
    }

    #[tokio::test]
    async fn test_no_change_no_message() {
        let mock = Arc::new(MockKeychain::with_credentials("token_v1"));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let provider = mock.clone();
        let handle = tokio::spawn(async move {
            let last_hash = compute_keychain_hash_with_provider(provider.as_ref());
            tokio::time::sleep(Duration::from_millis(10)).await;
            let current_hash = compute_keychain_hash_with_provider(provider.as_ref());

            if last_hash != current_hash {
                let _ = tx.send(AppMessage::CredentialKeychainChanged);
            }
        });

        // DON'T change credentials
        handle.await.unwrap();

        // Should NOT receive message
        assert!(rx.try_recv().is_err(), "Should NOT receive message when unchanged");
    }
}
