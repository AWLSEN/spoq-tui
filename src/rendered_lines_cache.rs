use std::collections::HashMap;
use ratatui::text::Line;

/// Maximum number of cached message renders to keep in memory
const MAX_RENDER_CACHE_SIZE: usize = 500;

/// Key for the rendered lines cache: (thread_id, message_id, render_version)
pub type RenderCacheKey = (String, i64, u64);

/// Cache for pre-rendered message lines.
/// This avoids re-rendering messages on every frame tick.
///
/// The cache tracks viewport width and automatically invalidates
/// when the terminal is resized, ensuring wrapped lines are correct.
#[derive(Debug, Default)]
pub struct RenderedLinesCache {
    cache: HashMap<RenderCacheKey, Vec<Line<'static>>>,
    access_order: Vec<RenderCacheKey>,
    /// Last viewport width used for rendering. Cache is cleared on width change.
    last_viewport_width: Option<u16>,
}

impl RenderedLinesCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if viewport width changed and invalidate cache if so.
    /// Call this at the start of each render pass.
    /// Returns true if cache was invalidated.
    #[inline]
    pub fn invalidate_if_width_changed(&mut self, viewport_width: u16) -> bool {
        match self.last_viewport_width {
            Some(last_width) if last_width == viewport_width => {
                // Width unchanged, cache is valid
                false
            }
            _ => {
                // Width changed or first render - clear cache
                if !self.cache.is_empty() {
                    self.cache.clear();
                    self.access_order.clear();
                }
                self.last_viewport_width = Some(viewport_width);
                true
            }
        }
    }

    pub fn get(&mut self, thread_id: &str, message_id: i64, render_version: u64) -> Option<&Vec<Line<'static>>> {
        let key = (thread_id.to_string(), message_id, render_version);
        if self.cache.contains_key(&key) {
            self.access_order.retain(|k| *k != key);
            self.access_order.push(key.clone());
            self.cache.get(&key)
        } else {
            None
        }
    }

    pub fn insert(&mut self, thread_id: &str, message_id: i64, render_version: u64, lines: Vec<Line<'static>>) {
        let key = (thread_id.to_string(), message_id, render_version);
        while self.cache.len() >= MAX_RENDER_CACHE_SIZE {
            if let Some(oldest_key) = self.access_order.first().cloned() {
                self.cache.remove(&oldest_key);
                self.access_order.remove(0);
            } else {
                break;
            }
        }
        // Remove old versions of the same message in the same thread
        self.cache.retain(|k, _| k.0 != thread_id || k.1 != message_id || k.2 == render_version);
        self.access_order.retain(|k| k.0 != thread_id || k.1 != message_id || k.2 == render_version);
        self.cache.insert(key.clone(), lines);
        self.access_order.push(key);
    }

    pub fn contains(&self, thread_id: &str, message_id: i64, render_version: u64) -> bool {
        self.cache.contains_key(&(thread_id.to_string(), message_id, render_version))
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Span;

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = RenderedLinesCache::new();
        let lines = vec![Line::from(vec![Span::raw("Hello")])];
        cache.insert("thread1", 1, 0, lines.clone());

        let retrieved = cache.get("thread1", 1, 0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().len(), 1);
    }

    #[test]
    fn test_cache_version_invalidation() {
        let mut cache = RenderedLinesCache::new();
        cache.insert("thread1", 1, 0, vec![Line::from("v0")]);
        cache.insert("thread1", 1, 1, vec![Line::from("v1")]);

        // Old version should be gone
        assert!(!cache.contains("thread1", 1, 0));
        // New version should be present
        assert!(cache.contains("thread1", 1, 1));
    }

    #[test]
    fn test_cache_lru_eviction() {
        let mut cache = RenderedLinesCache::new();
        // Insert MAX + 1 entries
        for i in 0..=MAX_RENDER_CACHE_SIZE {
            cache.insert("thread1", i as i64, 0, vec![Line::from("")]);
        }
        // First entry should have been evicted
        assert!(!cache.contains("thread1", 0, 0));
        // Last entry should still be present
        assert!(cache.contains("thread1", MAX_RENDER_CACHE_SIZE as i64, 0));
    }

    #[test]
    fn test_cache_different_threads_same_message_id() {
        let mut cache = RenderedLinesCache::new();
        cache.insert("thread1", 1, 0, vec![Line::from("thread1 content")]);
        cache.insert("thread2", 1, 0, vec![Line::from("thread2 content")]);

        // Both should exist independently
        assert!(cache.contains("thread1", 1, 0));
        assert!(cache.contains("thread2", 1, 0));
        assert_eq!(cache.len(), 2);

        // Contents should be different
        let t1_content = cache.get("thread1", 1, 0).unwrap();
        assert_eq!(t1_content[0].to_string(), "thread1 content");

        let t2_content = cache.get("thread2", 1, 0).unwrap();
        assert_eq!(t2_content[0].to_string(), "thread2 content");
    }

    #[test]
    fn test_cache_version_invalidation_per_thread() {
        let mut cache = RenderedLinesCache::new();
        // Insert same message_id in two threads
        cache.insert("thread1", 1, 0, vec![Line::from("t1v0")]);
        cache.insert("thread2", 1, 0, vec![Line::from("t2v0")]);

        // Update version in thread1 only
        cache.insert("thread1", 1, 1, vec![Line::from("t1v1")]);

        // thread1's old version should be gone, new version present
        assert!(!cache.contains("thread1", 1, 0));
        assert!(cache.contains("thread1", 1, 1));

        // thread2's version should be unaffected
        assert!(cache.contains("thread2", 1, 0));
    }

    #[test]
    fn test_cache_width_invalidation_same_width() {
        let mut cache = RenderedLinesCache::new();
        cache.insert("thread1", 1, 0, vec![Line::from("content")]);

        // First call sets width, doesn't invalidate (cache was empty before width set)
        let invalidated = cache.invalidate_if_width_changed(80);
        assert!(invalidated); // First time always "invalidates" (sets width)

        // Re-insert after width is set
        cache.insert("thread1", 1, 0, vec![Line::from("content")]);

        // Same width should not invalidate
        let invalidated = cache.invalidate_if_width_changed(80);
        assert!(!invalidated);
        assert!(cache.contains("thread1", 1, 0)); // Cache should still have entry
    }

    #[test]
    fn test_cache_width_invalidation_different_width() {
        let mut cache = RenderedLinesCache::new();

        // Set initial width
        cache.invalidate_if_width_changed(80);
        cache.insert("thread1", 1, 0, vec![Line::from("content")]);
        assert_eq!(cache.len(), 1);

        // Different width should invalidate
        let invalidated = cache.invalidate_if_width_changed(120);
        assert!(invalidated);
        assert_eq!(cache.len(), 0); // Cache should be cleared
        assert!(!cache.contains("thread1", 1, 0));
    }

    #[test]
    fn test_cache_width_invalidation_no_unnecessary_clear() {
        let mut cache = RenderedLinesCache::new();

        // Empty cache, set width
        let invalidated = cache.invalidate_if_width_changed(80);
        assert!(invalidated); // Returns true because width was set

        // Same width on empty cache - should not "invalidate" again
        let invalidated = cache.invalidate_if_width_changed(80);
        assert!(!invalidated);
    }
}
