//! Keychain-based credential change detection via polling.
//!
//! Will be fully implemented in Phase 5.

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::app::AppMessage;

/// Spawn the Keychain polling task.
pub fn spawn_keychain_poller(
    _message_tx: mpsc::UnboundedSender<AppMessage>,
) -> JoinHandle<()> {
    // Placeholder - will be implemented in Phase 5
    tokio::spawn(async {
        // TODO: Implement in Phase 5
    })
}

/// Get the current hash without triggering any events.
pub fn get_current_hash() -> Option<u64> {
    // Placeholder - will be implemented in Phase 5
    None
}
