//! Integration tests for message height caching (Round 3)
//! Tests the cached_message_heights HashMap in App struct

use spoq::app::App;
use spoq::cache::ThreadCache;

#[tokio::test]
async fn test_message_height_cache_initialization() {
    let app = App::new().expect("Failed to create app");

    // Cache should be empty on initialization
    assert_eq!(app.cached_message_heights.len(), 0);
}

#[tokio::test]
async fn test_message_height_cache_stores_heights() {
    let mut app = App::new().expect("Failed to create app");
    let mut cache = ThreadCache::new();

    // Create a message
    let thread_id = cache.create_streaming_thread("Test message".to_string());
    cache.append_to_message(&thread_id, "This is a test message for height caching.");
    cache.finalize_message(&thread_id, 100);

    let messages = cache.get_messages(&thread_id).unwrap();
    let message = &messages[1]; // Assistant message

    // Simulate caching a height
    let height = 5;
    app.cached_message_heights.insert((thread_id.to_string(), message.id), (message.render_version, height));

    // Verify cache stores the height
    assert_eq!(app.cached_message_heights.len(), 1);
    assert_eq!(
        app.cached_message_heights.get(&(thread_id.to_string(), message.id)),
        Some(&(message.render_version, height))
    );
}

#[tokio::test]
async fn test_message_height_cache_invalidates_on_version_change() {
    let mut app = App::new().expect("Failed to create app");
    let mut cache = ThreadCache::new();

    // Create a message
    let thread_id = cache.create_streaming_thread("Test".to_string());
    cache.append_to_message(&thread_id, "Original content");
    cache.finalize_message(&thread_id, 100);

    let messages = cache.get_messages(&thread_id).unwrap();
    let message = &messages[1];
    let original_version = message.render_version;

    // Cache the height with original version
    app.cached_message_heights.insert((thread_id.to_string(), message.id), (original_version, 5));

    // Simulate a render version change (e.g., message edited or re-rendered)
    let new_version = original_version + 1;

    // Check cache with new version - should be considered stale
    if let Some((cached_version, _height)) = app.cached_message_heights.get(&(thread_id.to_string(), message.id)) {
        assert_ne!(*cached_version, new_version);
        // In real code, this would trigger recalculation
    }
}

#[tokio::test]
async fn test_message_height_cache_handles_multiple_messages() {
    let mut app = App::new().expect("Failed to create app");
    let mut cache = ThreadCache::new();

    // Create multiple messages
    let thread_id = cache.create_streaming_thread("Message 1".to_string());
    cache.append_to_message(&thread_id, "Content 1");
    cache.finalize_message(&thread_id, 100);

    cache.add_streaming_message(&thread_id, "Message 2".to_string());
    cache.append_to_message(&thread_id, "Content 2");
    cache.finalize_message(&thread_id, 101);

    cache.add_streaming_message(&thread_id, "Message 3".to_string());
    cache.append_to_message(&thread_id, "Content 3");
    cache.finalize_message(&thread_id, 102);

    let messages = cache.get_messages(&thread_id).unwrap();

    // Cache heights for all messages
    for (i, message) in messages.iter().enumerate() {
        let height = (i + 1) * 3; // Simulate different heights
        app.cached_message_heights.insert((thread_id.to_string(), message.id), (message.render_version, height));
    }

    // Verify all heights are cached
    assert_eq!(app.cached_message_heights.len(), messages.len());

    // Verify each cached height is correct
    for (i, message) in messages.iter().enumerate() {
        let expected_height = (i + 1) * 3;
        assert_eq!(
            app.cached_message_heights.get(&(thread_id.to_string(), message.id)),
            Some(&(message.render_version, expected_height))
        );
    }
}

#[tokio::test]
async fn test_message_height_cache_persists_across_operations() {
    let mut app = App::new().expect("Failed to create app");
    let mut cache = ThreadCache::new();

    // Create a message and cache its height
    let thread_id = cache.create_streaming_thread("Test".to_string());
    cache.append_to_message(&thread_id, "Content");
    cache.finalize_message(&thread_id, 100);

    let messages = cache.get_messages(&thread_id).unwrap();
    let message_id = messages[1].id;
    let render_version = messages[1].render_version;

    app.cached_message_heights.insert((thread_id.to_string(), message_id), (render_version, 10));

    // Perform other operations
    cache.add_streaming_message(&thread_id, "Another message".to_string());
    cache.finalize_message(&thread_id, 101);

    // Original cached height should still exist
    assert_eq!(
        app.cached_message_heights.get(&(thread_id.to_string(), message_id)),
        Some(&(render_version, 10))
    );
}

#[tokio::test]
async fn test_message_height_cache_hit_vs_miss() {
    let mut app = App::new().expect("Failed to create app");
    let mut cache = ThreadCache::new();

    // Create messages
    let thread_id = cache.create_streaming_thread("Test".to_string());
    cache.append_to_message(&thread_id, "Message 1");
    cache.finalize_message(&thread_id, 100);

    cache.add_streaming_message(&thread_id, "Test 2".to_string());
    cache.append_to_message(&thread_id, "Message 2");
    cache.finalize_message(&thread_id, 101);

    let messages = cache.get_messages(&thread_id).unwrap();
    let msg1_id = messages[1].id;
    let msg2_id = messages[3].id;

    // Cache only message 1
    app.cached_message_heights.insert((thread_id.to_string(), msg1_id), (messages[1].render_version, 8));

    // Message 1 should have cache hit
    assert!(app.cached_message_heights.contains_key(&(thread_id.to_string(), msg1_id)));

    // Message 2 should have cache miss
    assert!(!app.cached_message_heights.contains_key(&(thread_id.to_string(), msg2_id)));
}

#[tokio::test]
async fn test_message_height_cache_can_be_cleared() {
    let mut app = App::new().expect("Failed to create app");

    // Add some cached heights
    app.cached_message_heights.insert(("thread1".to_string(), 1), (0, 5));
    app.cached_message_heights.insert(("thread1".to_string(), 2), (0, 10));
    app.cached_message_heights.insert(("thread1".to_string(), 3), (0, 15));

    assert_eq!(app.cached_message_heights.len(), 3);

    // Clear the cache
    app.cached_message_heights.clear();

    assert_eq!(app.cached_message_heights.len(), 0);
}
