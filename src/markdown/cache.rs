//! Memoization cache for markdown rendering
//!
//! Caches parsed output keyed by a hash of the input content.
//! When the same content is requested, returns cached lines instead of re-parsing.

use ratatui::text::Line;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::markdown::{render_markdown, MARKDOWN_CACHE_MAX_ENTRIES};

/// Cached result from markdown rendering
#[derive(Clone)]
pub(crate) struct CachedLines {
    /// The rendered lines
    pub lines: Vec<Line<'static>>,
}

/// Memoization cache for markdown rendering.
///
/// Caches parsed output keyed by a hash of the input content.
/// When the same content is requested, returns cached lines instead of re-parsing.
///
/// This is critical for performance because:
/// - `render_markdown()` creates a new `pulldown_cmark::Parser` for every call
/// - It parses markdown syntax, builds style stacks, and generates spans
/// - This happens up to 60 times/second for ALL visible messages
/// - By caching, completed messages never need re-parsing
pub struct MarkdownCache {
    /// Cache entries keyed by content hash
    entries: HashMap<u64, CachedLines>,
    /// Insertion order for LRU-style eviction (oldest first)
    insertion_order: Vec<u64>,
    /// Statistics: cache hits
    hits: u64,
    /// Statistics: cache misses
    misses: u64,
}

impl Default for MarkdownCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownCache {
    /// Create a new empty markdown cache
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            insertion_order: Vec::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Compute a hash for the given content string
    fn hash_content(content: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Render markdown with caching.
    ///
    /// If the content has been rendered before, returns the cached result.
    /// Otherwise, parses the markdown, caches the result, and returns it.
    pub fn render(&mut self, content: &str) -> Vec<Line<'static>> {
        let hash = Self::hash_content(content);

        // Check cache
        if let Some(cached) = self.entries.get(&hash) {
            self.hits += 1;
            return cached.lines.clone();
        }

        // Cache miss - render and store
        self.misses += 1;
        let lines = render_markdown(content);

        // Evict oldest entries if at capacity
        while self.entries.len() >= MARKDOWN_CACHE_MAX_ENTRIES && !self.insertion_order.is_empty() {
            let oldest_hash = self.insertion_order.remove(0);
            self.entries.remove(&oldest_hash);
        }

        // Store the new entry
        self.entries.insert(
            hash,
            CachedLines {
                lines: lines.clone(),
            },
        );
        self.insertion_order.push(hash);

        lines
    }

    /// Get cache statistics (hits, misses)
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }

    /// Get the number of entries currently in the cache
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries from the cache
    pub fn clear(&mut self) {
        self.entries.clear();
        self.insertion_order.clear();
        // Don't reset stats - they're useful for debugging
    }

    /// Invalidate a specific content entry (useful when content changes)
    pub fn invalidate(&mut self, content: &str) {
        let hash = Self::hash_content(content);
        if self.entries.remove(&hash).is_some() {
            self.insertion_order.retain(|&h| h != hash);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::MARKDOWN_CACHE_MAX_ENTRIES;

    #[test]
    fn test_cache_new() {
        let cache = MarkdownCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.stats(), (0, 0));
    }

    #[test]
    fn test_cache_default() {
        let cache = MarkdownCache::default();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_hit_and_miss() {
        let mut cache = MarkdownCache::new();

        // First render - should be a miss
        let content = "Hello, **world**!";
        let lines1 = cache.render(content);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 1);
        assert_eq!(cache.len(), 1);

        // Second render of same content - should be a hit
        let lines2 = cache.render(content);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 1);
        assert_eq!(cache.len(), 1);

        // Results should be identical
        assert_eq!(lines1.len(), lines2.len());
        for (l1, l2) in lines1.iter().zip(lines2.iter()) {
            assert_eq!(l1.spans.len(), l2.spans.len());
            for (s1, s2) in l1.spans.iter().zip(l2.spans.iter()) {
                assert_eq!(s1.content, s2.content);
            }
        }
    }

    #[test]
    fn test_cache_different_content() {
        let mut cache = MarkdownCache::new();

        // Render different content
        cache.render("Content A");
        cache.render("Content B");
        cache.render("Content C");

        assert_eq!(cache.len(), 3);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 3);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = MarkdownCache::new();

        // Add some entries
        cache.render("Content 1");
        cache.render("Content 2");
        assert_eq!(cache.len(), 2);

        // Clear the cache
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        // Stats should be preserved (useful for debugging)
        let (_hits, misses) = cache.stats();
        assert_eq!(misses, 2);
    }

    #[test]
    fn test_cache_invalidate() {
        let mut cache = MarkdownCache::new();

        let content = "Some markdown content";
        cache.render(content);
        assert_eq!(cache.len(), 1);

        // Invalidate the entry
        cache.invalidate(content);
        assert!(cache.is_empty());

        // Re-render should be a miss
        cache.render(content);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 2);
    }

    #[test]
    fn test_cache_invalidate_nonexistent() {
        let mut cache = MarkdownCache::new();
        cache.render("Existing content");

        // Invalidating non-existent content should be a no-op
        cache.invalidate("Non-existent content");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = MarkdownCache::new();

        // Fill cache beyond max capacity
        for i in 0..(MARKDOWN_CACHE_MAX_ENTRIES + 50) {
            cache.render(&format!("Content {}", i));
        }

        // Cache should not exceed max entries
        assert!(cache.len() <= MARKDOWN_CACHE_MAX_ENTRIES);
    }

    #[test]
    fn test_cache_complex_markdown() {
        let mut cache = MarkdownCache::new();

        let complex_md = r#"# Heading

This is **bold** and *italic* text.

```rust
fn main() {
    println!("Hello, world!");
}
```

- List item 1
- List item 2

`inline code`
"#;

        // First render
        let lines1 = cache.render(complex_md);
        assert!(!lines1.is_empty());

        // Second render should return cached result
        let lines2 = cache.render(complex_md);
        let (hits, misses) = cache.stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 1);

        // Verify identical results
        assert_eq!(lines1.len(), lines2.len());
    }

    #[test]
    fn test_cache_hash_collision_resistant() {
        let mut cache = MarkdownCache::new();

        // Test that different but similar content produces different cache entries
        let content_a = "Hello World";
        let content_b = "Hello World!";
        let content_c = "hello world";

        cache.render(content_a);
        cache.render(content_b);
        cache.render(content_c);

        // All three should be separate cache entries
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn test_cache_empty_content() {
        let mut cache = MarkdownCache::new();

        // Empty content should still cache
        let lines = cache.render("");
        assert!(!lines.is_empty()); // render_markdown returns at least one empty line
        assert_eq!(cache.len(), 1);

        // Second render should hit cache
        cache.render("");
        let (hits, _) = cache.stats();
        assert_eq!(hits, 1);
    }

    #[test]
    fn test_cache_whitespace_sensitive() {
        let mut cache = MarkdownCache::new();

        // Whitespace differences should produce different cache entries
        cache.render("word");
        cache.render(" word");
        cache.render("word ");
        cache.render("  word  ");

        assert_eq!(cache.len(), 4);
    }
}
