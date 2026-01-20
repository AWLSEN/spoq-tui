//! Integration tests for message height caching
//! Tests the CachedHeights struct and height_cache in App struct

use spoq::app::{App, CachedHeights};
use spoq::cache::ThreadCache;
use std::sync::Arc;

#[tokio::test]
async fn test_height_cache_initialization() {
    let app = App::new().expect("Failed to create app");

    // Cache should be None on initialization
    assert!(app.height_cache.is_none());
}

#[tokio::test]
async fn test_cached_heights_stores_heights() {
    let thread_id = Arc::new("test-thread".to_string());
    let viewport_width = 80;
    let mut cache = CachedHeights::new(Arc::clone(&thread_id), viewport_width);

    // Append some heights
    cache.append(1, 0, 5);
    cache.append(2, 0, 10);
    cache.append(3, 0, 8);

    // Verify heights are stored
    assert_eq!(cache.heights.len(), 3);
    assert_eq!(cache.heights[0].visual_lines, 5);
    assert_eq!(cache.heights[1].visual_lines, 10);
    assert_eq!(cache.heights[2].visual_lines, 8);

    // Verify cumulative offsets
    assert_eq!(cache.heights[0].cumulative_offset, 0);
    assert_eq!(cache.heights[1].cumulative_offset, 5);
    assert_eq!(cache.heights[2].cumulative_offset, 15);

    // Verify total lines
    assert_eq!(cache.total_lines, 23);
}

#[tokio::test]
async fn test_cached_heights_is_valid_for() {
    let thread_id = Arc::new("test-thread".to_string());
    let viewport_width = 80;
    let cache = CachedHeights::new(Arc::clone(&thread_id), viewport_width);

    // Same thread and width should be valid
    assert!(cache.is_valid_for("test-thread", 80));

    // Different thread should be invalid
    assert!(!cache.is_valid_for("other-thread", 80));

    // Different width should be invalid
    assert!(!cache.is_valid_for("test-thread", 100));
}

#[tokio::test]
async fn test_cached_heights_truncate() {
    let thread_id = Arc::new("test-thread".to_string());
    let mut cache = CachedHeights::new(thread_id, 80);

    // Add heights
    cache.append(1, 0, 5);
    cache.append(2, 0, 10);
    cache.append(3, 0, 8);
    cache.append(4, 0, 12);

    assert_eq!(cache.heights.len(), 4);
    assert_eq!(cache.total_lines, 35);

    // Truncate to 2 messages
    cache.truncate(2);

    assert_eq!(cache.heights.len(), 2);
    assert_eq!(cache.total_lines, 15);
}

#[tokio::test]
async fn test_cached_heights_recalculate_offsets() {
    let thread_id = Arc::new("test-thread".to_string());
    let mut cache = CachedHeights::new(thread_id, 80);

    // Add heights
    cache.append(1, 0, 5);
    cache.append(2, 0, 10);
    cache.append(3, 0, 8);

    // Manually modify height (simulating stale cache)
    cache.heights[0].visual_lines = 20;

    // Recalculate from index 0
    cache.recalculate_offsets_from(0);

    // Verify offsets
    assert_eq!(cache.heights[0].cumulative_offset, 0);
    assert_eq!(cache.heights[1].cumulative_offset, 20);
    assert_eq!(cache.heights[2].cumulative_offset, 30);
    assert_eq!(cache.total_lines, 38);
}

#[tokio::test]
async fn test_height_cache_integration_with_app() {
    let mut app = App::new().expect("Failed to create app");
    let mut thread_cache = ThreadCache::new();

    // Create a message
    let thread_id = thread_cache.create_streaming_thread("Test message".to_string());
    thread_cache.append_to_message(&thread_id, "This is a test message for height caching.");
    thread_cache.finalize_message(&thread_id, 100);

    let messages = thread_cache.get_messages(&thread_id).unwrap();
    assert!(!messages.is_empty());

    // Create height cache
    let thread_id_arc = Arc::new(thread_id.clone());
    let mut cache = CachedHeights::new(thread_id_arc, 80);

    // Add heights for messages
    for message in messages.iter() {
        cache.append(message.id, message.render_version, 5);
    }

    // Store in app
    app.height_cache = Some(cache);

    // Verify cache is stored
    assert!(app.height_cache.is_some());
    let stored_cache = app.height_cache.as_ref().unwrap();
    assert!(stored_cache.is_valid_for(&thread_id, 80));
    assert_eq!(stored_cache.heights.len(), messages.len());
}

#[tokio::test]
async fn test_height_cache_invalidation_on_width_change() {
    let mut app = App::new().expect("Failed to create app");

    // Create cache with initial width
    let thread_id = Arc::new("test-thread".to_string());
    let mut cache = CachedHeights::new(thread_id, 80);
    cache.append(1, 0, 5);
    app.height_cache = Some(cache);

    // Verify cache exists
    assert!(app
        .height_cache
        .as_ref()
        .unwrap()
        .is_valid_for("test-thread", 80));

    // Width change should invalidate
    assert!(!app
        .height_cache
        .as_ref()
        .unwrap()
        .is_valid_for("test-thread", 100));
}
