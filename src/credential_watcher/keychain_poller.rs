//! Keychain-based credential change detection via polling.
//!
//! macOS Keychain has no change notification API, so we must poll.
//! Polls every 30 seconds and compares hashes to detect changes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::app::AppMessage;
use crate::conductor::read_claude_keychain_credentials;

/// Polling interval for Keychain checks (30 seconds).
const POLL_INTERVAL_SECS: u64 = 30;

/// Compute a hash of the current Keychain credentials.
///
/// Returns None if credentials can't be read.
fn compute_keychain_hash() -> Option<u64> {
    let credentials = read_claude_keychain_credentials()?;
    let mut hasher = DefaultHasher::new();
    credentials.hash(&mut hasher);
    Some(hasher.finish())
}

/// Spawn the Keychain polling task.
///
/// Returns a JoinHandle that can be used to abort the task on shutdown.
/// The task polls every 30 seconds and sends `CredentialKeychainChanged`
/// when a change is detected.
pub fn spawn_keychain_poller(message_tx: mpsc::UnboundedSender<AppMessage>) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(
            "Keychain poller started (interval: {}s)",
            POLL_INTERVAL_SECS
        );

        // Capture initial hash (don't trigger sync on startup for existing creds)
        let mut last_hash = compute_keychain_hash();
        tracing::debug!("Initial Keychain hash: {:?}", last_hash);

        let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));

        loop {
            interval.tick().await;

            let current_hash = compute_keychain_hash();

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

/// Get the current Keychain hash without triggering any events.
///
/// Useful for initializing state.
pub fn get_current_hash() -> Option<u64> {
    compute_keychain_hash()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
