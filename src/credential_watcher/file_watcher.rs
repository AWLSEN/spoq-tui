//! File-based credential change detection using the `notify` crate.
//!
//! Will be fully implemented in Phase 4.

use notify::RecommendedWatcher;
use tokio::sync::mpsc;

use crate::app::AppMessage;

/// Spawn the file watcher system.
///
/// Returns the watcher handle (MUST be kept alive - dropping it stops watching).
pub fn spawn_file_watcher(
    _message_tx: mpsc::UnboundedSender<AppMessage>,
) -> notify::Result<RecommendedWatcher> {
    // Placeholder - will be implemented in Phase 4
    todo!("Phase 4: Implement file watcher")
}
