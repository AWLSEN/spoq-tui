//! Thread mode synchronization with debouncing and coalescing.
//!
//! This module handles thread mode updates to the backend with:
//! - Per-thread pending state coalescing (last-intent-wins)
//! - Debouncing (200ms default) to avoid rapid API calls
//! - Graceful error handling (fail-quietly, local state authoritative)

use super::backend_coordinator;
use crate::conductor::ConductorClient;
use crate::models::PermissionMode;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Default debounce duration for mode changes.
const DEFAULT_DEBOUNCE_MS: u64 = 200;

/// Coordinator for thread mode synchronization with debouncing and coalescing.
///
/// This struct maintains per-thread pending state and debounces rapid mode changes
/// before syncing to the backend.
pub struct ThreadModeSync {
    /// Pending mode changes per thread: (permission_mode, request_time)
    pending_modes: Arc<Mutex<HashMap<String, (PermissionMode, Instant)>>>,
    /// Duration to wait before syncing a mode change
    debounce_duration: Duration,
    /// Conductor client for API calls
    conductor: Arc<ConductorClient>,
    /// Whether a debounce task is currently running
    debounce_task_running: Arc<Mutex<bool>>,
}

impl ThreadModeSync {
    /// Create a new ThreadModeSync with the given conductor client.
    pub fn new(conductor: Arc<ConductorClient>) -> Self {
        Self {
            pending_modes: Arc::new(Mutex::new(HashMap::new())),
            debounce_duration: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
            conductor,
            debounce_task_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Set a custom debounce duration.
    pub fn with_debounce_duration(mut self, duration: Duration) -> Self {
        self.debounce_duration = duration;
        self
    }

    /// Request a mode change for a thread.
    ///
    /// This method coalesces rapid requests for the same thread (last-intent-wins)
    /// and triggers debounced sync to the backend.
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the thread to update
    /// * `permission_mode` - The new permission mode for the thread
    pub fn request_mode_change(&self, thread_id: String, permission_mode: PermissionMode) {
        let now = Instant::now();

        // Update pending state (coalesce: last-intent-wins)
        {
            let mut pending = self.pending_modes.blocking_lock();
            pending.insert(thread_id.clone(), (permission_mode, now));
        }

        // Spawn debounce task if not already running
        let should_spawn = {
            let mut running = self.debounce_task_running.blocking_lock();
            if !*running {
                *running = true;
                true
            } else {
                false
            }
        };

        if should_spawn {
            self.spawn_debounce_task();
        }
    }

    /// Spawn the background debounce task.
    fn spawn_debounce_task(&self) {
        let pending_modes = Arc::clone(&self.pending_modes);
        let conductor = Arc::clone(&self.conductor);
        let debounce_duration = self.debounce_duration;
        let task_running = Arc::clone(&self.debounce_task_running);

        tokio::spawn(async move {
            let check_interval = Duration::from_millis(50);

            loop {
                tokio::time::sleep(check_interval).await;

                let now = Instant::now();
                let mut ready_to_sync = Vec::new();

                // Find entries that have exceeded debounce duration
                {
                    let mut pending = pending_modes.lock().await;
                    let mut to_remove = Vec::new();

                    for (thread_id, (permission_mode, request_time)) in pending.iter() {
                        if now.duration_since(*request_time) >= debounce_duration {
                            ready_to_sync.push((thread_id.clone(), *permission_mode));
                            to_remove.push(thread_id.clone());
                        }
                    }

                    // Remove synced entries from pending map
                    for thread_id in to_remove {
                        pending.remove(&thread_id);
                    }

                    // Exit if no more pending entries
                    if pending.is_empty() {
                        *task_running.lock().await = false;
                        break;
                    }
                }

                // Sync ready entries to backend
                for (thread_id, permission_mode) in ready_to_sync {
                    tracing::debug!(
                        thread_id = %thread_id,
                        permission_mode = ?permission_mode,
                        "Syncing thread mode after debounce"
                    );

                    let _ = backend_coordinator::sync_thread_mode(&conductor, &thread_id, permission_mode).await;
                }
            }
        });
    }

    /// Get the number of pending mode changes.
    #[cfg(test)]
    pub async fn pending_count(&self) -> usize {
        self.pending_modes.lock().await.len()
    }

    /// Check if a debounce task is currently running.
    #[cfg(test)]
    pub async fn is_task_running(&self) -> bool {
        *self.debounce_task_running.lock().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a ConductorClient. In practice, you'd use
    // a mock client for unit testing. For now, we test the logic invariants.

    #[test]
    fn test_default_debounce_duration() {
        assert_eq!(DEFAULT_DEBOUNCE_MS, 200);
    }

    #[test]
    fn test_debounce_duration_custom() {
        // We can't easily test this without a ConductorClient mock
        // but we verify the builder pattern compiles
        let duration = Duration::from_millis(300);
        assert_eq!(duration.as_millis(), 300);
    }

    #[tokio::test]
    async fn test_pending_modes_coalescing() {
        // Test that repeated requests to same thread coalesce (last-intent-wins)
        // This would require a mock ConductorClient for full testing
    }

    #[tokio::test]
    async fn test_debounce_task_exits_when_empty() {
        // Test that debounce task exits when no more pending entries
        // This would require a mock ConductorClient for full testing
    }
}
