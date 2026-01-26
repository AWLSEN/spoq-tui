//! App-level caching for unified picker data.
//!
//! Implements a tiered caching strategy:
//! - Repos: Cached for entire session (rarely change)
//! - Threads: Cached with 5-minute TTL (change more often)
//! - Folders: Cached for entire session (local, rarely change)

use std::time::{Duration, Instant};

use crate::models::picker::PickerItem;

/// Time-to-live for thread cache (5 minutes)
const THREADS_TTL: Duration = Duration::from_secs(5 * 60);

/// Cached data with timestamp
#[derive(Debug, Clone)]
pub struct CachedData {
    /// Cached items
    pub items: Vec<PickerItem>,
    /// When the data was cached
    pub cached_at: Instant,
}

impl CachedData {
    /// Create new cached data with current timestamp
    pub fn new(items: Vec<PickerItem>) -> Self {
        Self {
            items,
            cached_at: Instant::now(),
        }
    }

    /// Check if cache is older than given duration
    pub fn is_older_than(&self, duration: Duration) -> bool {
        self.cached_at.elapsed() > duration
    }

    /// Check if cache has data
    pub fn has_data(&self) -> bool {
        !self.items.is_empty()
    }
}

/// App-level cache for picker data
#[derive(Debug, Clone, Default)]
pub struct AppCache {
    /// Cached repos (session-level, rarely change)
    pub repos: Option<CachedData>,
    /// Cached threads (5-minute TTL)
    pub threads: Option<CachedData>,
    /// Cached folders (session-level, local)
    pub folders: Option<CachedData>,
    /// Whether initial preload has been triggered
    pub preload_started: bool,
}

impl AppCache {
    /// Create new empty cache
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if repos are cached
    pub fn has_repos(&self) -> bool {
        self.repos.as_ref().map(|c| c.has_data()).unwrap_or(false)
    }

    /// Check if threads are cached and fresh (within TTL)
    pub fn has_fresh_threads(&self) -> bool {
        self.threads
            .as_ref()
            .map(|c| c.has_data() && !c.is_older_than(THREADS_TTL))
            .unwrap_or(false)
    }

    /// Check if folders are cached
    pub fn has_folders(&self) -> bool {
        self.folders.as_ref().map(|c| c.has_data()).unwrap_or(false)
    }

    /// Get cached repos
    pub fn get_repos(&self) -> Option<&Vec<PickerItem>> {
        self.repos.as_ref().map(|c| &c.items)
    }

    /// Get cached threads (only if fresh)
    pub fn get_fresh_threads(&self) -> Option<&Vec<PickerItem>> {
        self.threads
            .as_ref()
            .filter(|c| !c.is_older_than(THREADS_TTL))
            .map(|c| &c.items)
    }

    /// Get cached folders
    pub fn get_folders(&self) -> Option<&Vec<PickerItem>> {
        self.folders.as_ref().map(|c| &c.items)
    }

    /// Cache repos
    pub fn set_repos(&mut self, items: Vec<PickerItem>) {
        self.repos = Some(CachedData::new(items));
    }

    /// Cache threads
    pub fn set_threads(&mut self, items: Vec<PickerItem>) {
        self.threads = Some(CachedData::new(items));
    }

    /// Cache folders
    pub fn set_folders(&mut self, items: Vec<PickerItem>) {
        self.folders = Some(CachedData::new(items));
    }

    /// Mark that preload has been started
    pub fn mark_preload_started(&mut self) {
        self.preload_started = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_data_creation() {
        let items = vec![PickerItem::Folder {
            name: "test".to_string(),
            path: "/test".to_string(),
        }];
        let cached = CachedData::new(items.clone());

        assert!(cached.has_data());
        assert!(!cached.is_older_than(Duration::from_secs(1)));
    }

    #[test]
    fn test_app_cache_repos() {
        let mut cache = AppCache::new();
        assert!(!cache.has_repos());

        let items = vec![PickerItem::Repo {
            name: "test/repo".to_string(),
            local_path: None,
            url: "https://github.com/test/repo".to_string(),
        }];
        cache.set_repos(items);

        assert!(cache.has_repos());
        assert!(cache.get_repos().is_some());
    }

    #[test]
    fn test_app_cache_threads_ttl() {
        let mut cache = AppCache::new();
        assert!(!cache.has_fresh_threads());

        let items = vec![PickerItem::Thread {
            id: "123".to_string(),
            title: "Test".to_string(),
            working_directory: None,
        }];
        cache.set_threads(items);

        // Should be fresh immediately
        assert!(cache.has_fresh_threads());
        assert!(cache.get_fresh_threads().is_some());
    }
}
