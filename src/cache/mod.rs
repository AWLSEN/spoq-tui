//! Thread and message cache module
//!
//! Provides local caching for threads and messages with eviction and reconciliation support.

mod error;
mod message;
mod reconciliation;
mod thread;
mod tools;

use std::collections::HashMap;
use std::time::Instant;

use crate::models::{ErrorInfo, Message, Thread};

/// Eviction timeout in seconds (30 minutes)
const EVICTION_TIMEOUT_SECS: u64 = 30 * 60;

/// Local cache for threads and messages
/// Will fetch from backend in future phases
#[derive(Debug, Default)]
pub struct ThreadCache {
    /// Cached threads indexed by thread ID
    pub(crate) threads: HashMap<String, Thread>,
    /// Cached messages indexed by thread ID
    pub(crate) messages: HashMap<String, Vec<Message>>,
    /// Order of thread IDs (most recent first)
    pub(crate) thread_order: Vec<String>,
    /// Mapping from pending IDs to real IDs for redirecting tokens
    /// When a thread is reconciled, we keep track so streaming tokens using
    /// the old pending ID can be redirected to the correct thread.
    pub(crate) pending_to_real: HashMap<String, String>,
    /// Pending title updates for threads that haven't been reconciled yet.
    /// Maps thread_id → (title, description). These are applied after reconciliation.
    pub(crate) pending_title_updates: HashMap<String, (String, Option<String>)>,
    /// Inline errors per thread (displayed as banners)
    pub(crate) errors: HashMap<String, Vec<ErrorInfo>>,
    /// Index of currently focused error (for dismiss with 'd' key)
    pub(crate) focused_error_index: usize,
    /// Last accessed time for each thread (for LRU eviction)
    pub(crate) last_accessed: HashMap<String, Instant>,
}

impl ThreadCache {
    /// Create a new empty ThreadCache
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a ThreadCache populated with stub data for development
    pub fn with_stub_data() -> Self {
        let mut cache = Self::new();
        cache.populate_stub_data();
        cache
    }

    /// Clear all cached data
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.threads.clear();
        self.messages.clear();
        self.thread_order.clear();
        self.pending_title_updates.clear();
        self.errors.clear();
        self.focused_error_index = 0;
        self.last_accessed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cache_is_empty() {
        let cache = ThreadCache::new();
        assert_eq!(cache.thread_count(), 0);
        assert!(cache.threads().is_empty());
    }

    #[test]
    fn test_with_stub_data_has_threads() {
        let cache = ThreadCache::with_stub_data();
        assert_eq!(cache.thread_count(), 3);
        assert_eq!(cache.threads().len(), 3);
    }

    #[test]
    fn test_stub_data_thread_order() {
        let cache = ThreadCache::with_stub_data();
        let threads = cache.threads();

        // Most recent thread should be first
        assert_eq!(threads[0].id, "thread-001");
        assert_eq!(threads[1].id, "thread-002");
        assert_eq!(threads[2].id, "thread-003");
    }

    #[test]
    fn test_clear() {
        let mut cache = ThreadCache::with_stub_data();
        assert!(cache.thread_count() > 0);

        cache.clear();

        assert_eq!(cache.thread_count(), 0);
        assert!(cache.threads().is_empty());
        assert!(cache.get_messages("thread-001").is_none());
    }
}
