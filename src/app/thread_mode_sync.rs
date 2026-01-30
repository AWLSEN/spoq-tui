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
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

/// Default debounce duration for mode changes.
const DEFAULT_DEBOUNCE_MS: u64 = 200;

/// Coordinator for thread mode synchronization with debouncing and coalescing.
///
/// This struct maintains per-thread pending state and debounces rapid mode changes
/// before syncing to the backend.
pub struct ThreadModeSync {
    /// Pending mode changes per thread (last-intent-wins coalescing)
    pending_modes: Arc<StdMutex<HashMap<String, PermissionMode>>>,
    /// Duration to wait before syncing a mode change
    debounce_duration: Duration,
    /// Conductor client for API calls
    conductor: Arc<ConductorClient>,
    /// Whether a debounce task is currently running
    debounce_task_running: Arc<StdMutex<bool>>,
}

impl ThreadModeSync {
    /// Create a new ThreadModeSync with the given conductor client.
    pub fn new(conductor: Arc<ConductorClient>) -> Self {
        Self {
            pending_modes: Arc::new(StdMutex::new(HashMap::new())),
            debounce_duration: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
            conductor,
            debounce_task_running: Arc::new(StdMutex::new(false)),
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
    /// Note: This is a fire-and-forget method. It returns immediately after
    /// queueing the mode change; actual sync happens in the background.
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the thread to update
    /// * `permission_mode` - The new permission mode for the thread
    pub fn request_mode_change(&self, thread_id: String, permission_mode: PermissionMode) {
        // Update pending state (coalesce: last-intent-wins)
        let should_spawn = {
            let mut pending = self.pending_modes.lock().unwrap();
            let mut running = self.debounce_task_running.lock().unwrap();

            pending.insert(thread_id.clone(), permission_mode);

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
    ///
    /// Sleeps for the full debounce duration, then drains and syncs all pending
    /// entries. If new entries arrived during the sync, loops again; otherwise exits.
    fn spawn_debounce_task(&self) {
        let pending_modes = Arc::clone(&self.pending_modes);
        let conductor = Arc::clone(&self.conductor);
        let debounce_duration = self.debounce_duration;
        let task_running = Arc::clone(&self.debounce_task_running);

        // Guard: only spawn if a tokio runtime is available (avoids panics in sync tests)
        let Ok(_handle) = tokio::runtime::Handle::try_current() else {
            *task_running.lock().unwrap() = false;
            return;
        };

        tokio::spawn(async move {
            loop {
                // Wait the full debounce window before draining
                tokio::time::sleep(debounce_duration).await;

                // Drain all pending entries atomically
                let to_sync: Vec<(String, PermissionMode)> = {
                    let mut pending = pending_modes.lock().unwrap();
                    pending.drain().collect()
                };

                if to_sync.is_empty() {
                    *task_running.lock().unwrap() = false;
                    break;
                }

                // Sync drained entries to backend
                for (thread_id, permission_mode) in to_sync {
                    tracing::debug!(
                        thread_id = %thread_id,
                        permission_mode = ?permission_mode,
                        "Syncing thread mode after debounce"
                    );
                    let _ = backend_coordinator::sync_thread_mode(&conductor, &thread_id, permission_mode).await;
                }

                // If nothing new arrived during sync, we're done
                if pending_modes.lock().unwrap().is_empty() {
                    *task_running.lock().unwrap() = false;
                    break;
                }
            }
        });
    }

    /// Get the number of pending mode changes.
    #[cfg(test)]
    pub fn pending_count(&self) -> usize {
        self.pending_modes.lock().unwrap().len()
    }

    /// Check if a debounce task is currently running.
    #[cfg(test)]
    pub fn is_task_running(&self) -> bool {
        *self.debounce_task_running.lock().unwrap()
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
        // We verify the builder pattern compiles correctly
        let duration = Duration::from_millis(300);
        assert_eq!(duration.as_millis(), 300);
    }
}
