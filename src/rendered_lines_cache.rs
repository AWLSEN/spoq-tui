use std::collections::HashMap;
use ratatui::text::Line;

/// Maximum number of cached message renders to keep in memory
const MAX_RENDER_CACHE_SIZE: usize = 500;

/// Key for the rendered lines cache: (message_id, render_version)
pub type RenderCacheKey = (i64, u64);

/// Cache for pre-rendered message lines.
/// This avoids re-rendering messages on every frame tick.
#[derive(Debug, Default)]
pub struct RenderedLinesCache {
    cache: HashMap<RenderCacheKey, Vec<Line<'static>>>,
    access_order: Vec<RenderCacheKey>,
}

impl RenderedLinesCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&mut self, message_id: i64, render_version: u64) -> Option<&Vec<Line<'static>>> {
        let key = (message_id, render_version);
        if self.cache.contains_key(&key) {
            self.access_order.retain(|k| *k != key);
            self.access_order.push(key);
            self.cache.get(&key)
        } else {
            None
        }
    }

    pub fn insert(&mut self, message_id: i64, render_version: u64, lines: Vec<Line<'static>>) {
        let key = (message_id, render_version);
        while self.cache.len() >= MAX_RENDER_CACHE_SIZE {
            if let Some(oldest_key) = self.access_order.first().copied() {
                self.cache.remove(&oldest_key);
                self.access_order.remove(0);
            } else {
                break;
            }
        }
        self.cache.retain(|k, _| k.0 != message_id || k.1 == render_version);
        self.access_order.retain(|k| k.0 != message_id || k.1 == render_version);
        self.cache.insert(key, lines);
        self.access_order.push(key);
    }

    pub fn contains(&self, message_id: i64, render_version: u64) -> bool {
        self.cache.contains_key(&(message_id, render_version))
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
        cache.insert(1, 0, lines.clone());

        let retrieved = cache.get(1, 0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().len(), 1);
    }

    #[test]
    fn test_cache_version_invalidation() {
        let mut cache = RenderedLinesCache::new();
        cache.insert(1, 0, vec![Line::from("v0")]);
        cache.insert(1, 1, vec![Line::from("v1")]);

        // Old version should be gone
        assert!(!cache.contains(1, 0));
        // New version should be present
        assert!(cache.contains(1, 1));
    }

    #[test]
    fn test_cache_lru_eviction() {
        let mut cache = RenderedLinesCache::new();
        // Insert MAX + 1 entries
        for i in 0..=MAX_RENDER_CACHE_SIZE {
            cache.insert(i as i64, 0, vec![Line::from("")]);
        }
        // First entry should have been evicted
        assert!(!cache.contains(0, 0));
        // Last entry should still be present
        assert!(cache.contains(MAX_RENDER_CACHE_SIZE as i64, 0));
    }
}
