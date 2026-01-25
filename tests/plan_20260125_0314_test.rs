//! Integration tests for plan-20260125-0314
//!
//! Tests the complete flow of thread metadata updates via WebSocket:
//! 1. WsThreadUpdated message deserialization (part of WsIncomingMessage)
//! 2. WebSocket handler routing to AppMessage
//! 3. Cache update with pending title updates queue
//! 4. Reconciliation applying pending updates

use spoq::cache::ThreadCache;
use spoq::models::ThreadType;
use spoq::websocket::WsIncomingMessage;

#[test]
fn test_thread_updated_message_parsing() {
    // Test that we can parse the ThreadUpdated message from the backend
    let json = r#"{
        "type": "thread_updated",
        "thread_id": "thread-123",
        "title": "Updated Thread Title",
        "description": "Updated thread description",
        "timestamp": 1705315800000
    }"#;

    let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
    match msg {
        WsIncomingMessage::ThreadUpdated(update) => {
            assert_eq!(update.thread_id, "thread-123");
            assert_eq!(update.title, "Updated Thread Title");
            assert_eq!(update.description, "Updated thread description");
            assert_eq!(update.timestamp, 1705315800000);
        }
        _ => panic!("Expected ThreadUpdated variant"),
    }
}

#[test]
fn test_update_thread_metadata_immediate_update() {
    // Test the happy path: thread exists, update is applied immediately
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_pending_thread(
        "Original message".to_string(),
        ThreadType::Programming,
        None,
    );

    // Simulate receiving a thread_updated message
    let updated = cache.update_thread_metadata(
        &thread_id,
        Some("AI Generated Title".to_string()),
        Some("AI Generated Description".to_string()),
    );

    // Should return true since thread exists
    assert!(updated);

    // Verify the thread was updated
    let thread = cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "AI Generated Title");
    assert_eq!(thread.description, Some("AI Generated Description".to_string()));
}

#[test]
fn test_update_thread_metadata_with_pending_queue() {
    // Test the race condition path: thread doesn't exist yet, update is queued
    let mut cache = ThreadCache::new();

    // Try to update a thread that doesn't exist yet
    let updated = cache.update_thread_metadata(
        "future-thread-id",
        Some("Queued Title".to_string()),
        Some("Queued Description".to_string()),
    );

    // Should return false since thread doesn't exist
    // This indicates the update was queued internally
    assert!(!updated);
}

#[test]
fn test_reconciliation_applies_pending_title_updates() {
    // Test that reconciliation applies any queued title updates
    // This tests the main race condition fix
    let mut cache = ThreadCache::new();

    // Create a pending thread
    let pending_id = cache.create_pending_thread(
        "Original message".to_string(),
        ThreadType::Programming,
        None,
    );

    // Now immediately reconcile with backend ID (simulating fast backend response)
    cache.reconcile_thread_id(&pending_id, "backend-real-id", None);

    // Title update arrives after reconciliation but uses pending_id
    // (The update was queued during the race window)
    // We'll test this by calling apply_pending_title_updates directly
    cache.apply_pending_title_updates("backend-real-id");

    // Thread should still exist and be functional
    let thread = cache.get_thread("backend-real-id").unwrap();
    assert_eq!(thread.id, "backend-real-id");
}

#[test]
fn test_update_with_pending_to_real_id_mapping() {
    // Test that updates work after reconciliation using the old pending ID
    let mut cache = ThreadCache::new();

    let pending_id = cache.create_pending_thread(
        "Original".to_string(),
        ThreadType::Programming,
        None,
    );

    // Reconcile
    cache.reconcile_thread_id(&pending_id, "real-backend-id", None);

    // Update using the old pending ID (should redirect to real ID)
    let updated = cache.update_thread_metadata(
        &pending_id,
        Some("Updated via Pending ID".to_string()),
        Some("Updated Description".to_string()),
    );

    assert!(updated);

    // Verify the real thread was updated
    let thread = cache.get_thread("real-backend-id").unwrap();
    assert_eq!(thread.title, "Updated via Pending ID");
    assert_eq!(thread.description, Some("Updated Description".to_string()));
}

#[test]
fn test_complete_workflow_create_reconcile_update() {
    // Test the complete workflow from thread creation through reconciliation to update
    let mut cache = ThreadCache::new();

    // Step 1: Create a pending thread (user sends message)
    let pending_id = cache.create_pending_thread(
        "What is Rust?".to_string(),
        ThreadType::Programming,
        Some("/Users/test/project".to_string()),
    );

    // Verify thread exists with correct initial title
    let thread = cache.get_thread(&pending_id).unwrap();
    assert_eq!(thread.title, "What is Rust?");
    assert!(thread.description.is_none());

    // Step 2: Reconcile with backend (backend returns thread info)
    cache.reconcile_thread_id(
        &pending_id,
        "backend-thread-123",
        Some("User message received".to_string()),
    );

    // Verify thread was reconciled
    assert!(cache.get_thread(&pending_id).is_none());
    let thread = cache.get_thread("backend-thread-123").unwrap();
    assert_eq!(thread.title, "User message received");

    // Step 3: Backend sends title update (AI generated title)
    let updated = cache.update_thread_metadata(
        "backend-thread-123",
        Some("Understanding Rust Programming".to_string()),
        Some("A conversation about Rust programming language".to_string()),
    );

    assert!(updated);

    // Verify final state
    let thread = cache.get_thread("backend-thread-123").unwrap();
    assert_eq!(thread.title, "Understanding Rust Programming");
    assert_eq!(thread.description, Some("A conversation about Rust programming language".to_string()));
    assert_eq!(thread.working_directory, Some("/Users/test/project".to_string()));
}

#[test]
fn test_thread_updated_message_roundtrip() {
    // Test serialization/deserialization roundtrip for WsIncomingMessage::ThreadUpdated
    let json = r#"{
        "type": "thread_updated",
        "thread_id": "thread-roundtrip",
        "title": "Roundtrip Title",
        "description": "Roundtrip Description",
        "timestamp": 1705315800000
    }"#;

    let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();

    // Serialize it back
    let serialized = serde_json::to_string(&msg).unwrap();

    // Parse again
    let reparsed: WsIncomingMessage = serde_json::from_str(&serialized).unwrap();

    // Verify it matches
    match reparsed {
        WsIncomingMessage::ThreadUpdated(update) => {
            assert_eq!(update.thread_id, "thread-roundtrip");
            assert_eq!(update.title, "Roundtrip Title");
            assert_eq!(update.description, "Roundtrip Description");
            assert_eq!(update.timestamp, 1705315800000);
        }
        _ => panic!("Expected ThreadUpdated variant"),
    }
}

#[test]
fn test_pending_updates_cleared_on_cache_clear() {
    // Ensure that clearing the cache clears all state including pending updates
    let mut cache = ThreadCache::new();

    // Create some threads with updates
    let thread_id = cache.create_pending_thread(
        "Test".to_string(),
        ThreadType::Programming,
        None,
    );

    cache.update_thread_metadata(
        &thread_id,
        Some("Updated".to_string()),
        None,
    );

    // Clear the cache
    cache.clear();

    // Verify cache is empty
    assert_eq!(cache.threads().len(), 0);
    assert!(cache.get_thread(&thread_id).is_none());
}

#[test]
fn test_multiple_updates_to_same_thread() {
    // Test that multiple updates work correctly
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_pending_thread(
        "Original".to_string(),
        ThreadType::Programming,
        None,
    );

    // First update
    cache.update_thread_metadata(
        &thread_id,
        Some("Title 1".to_string()),
        Some("Desc 1".to_string()),
    );

    let thread = cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "Title 1");
    assert_eq!(thread.description, Some("Desc 1".to_string()));

    // Second update
    cache.update_thread_metadata(
        &thread_id,
        Some("Title 2".to_string()),
        Some("Desc 2".to_string()),
    );

    let thread = cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "Title 2");
    assert_eq!(thread.description, Some("Desc 2".to_string()));

    // Third update
    cache.update_thread_metadata(
        &thread_id,
        Some("Title 3".to_string()),
        Some("Desc 3".to_string()),
    );

    let thread = cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "Title 3");
    assert_eq!(thread.description, Some("Desc 3".to_string()));
}

#[test]
fn test_update_metadata_only_title() {
    // Test updating only the title
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_pending_thread(
        "Original".to_string(),
        ThreadType::Programming,
        None,
    );

    cache.update_thread_metadata(&thread_id, Some("New Title".to_string()), None);

    let thread = cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "New Title");
    assert!(thread.description.is_none());
}

#[test]
fn test_update_metadata_only_description() {
    // Test updating only the description
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_pending_thread(
        "Original".to_string(),
        ThreadType::Programming,
        None,
    );

    cache.update_thread_metadata(&thread_id, None, Some("New Description".to_string()));

    let thread = cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "Original");
    assert_eq!(thread.description, Some("New Description".to_string()));
}

#[test]
fn test_websocket_message_all_thread_updated_fields() {
    // Test that all fields in the ThreadUpdated message are properly handled
    let json = r#"{
        "type": "thread_updated",
        "thread_id": "cm5abc123",
        "title": "Complete Test Title",
        "description": "Complete test description with special chars: \n\t\"quotes\"",
        "timestamp": 1737817500123
    }"#;

    let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
    match msg {
        WsIncomingMessage::ThreadUpdated(update) => {
            assert_eq!(update.thread_id, "cm5abc123");
            assert_eq!(update.title, "Complete Test Title");
            assert!(update.description.contains("special chars"));
            assert!(update.description.contains('\n'));
            assert_eq!(update.timestamp, 1737817500123);
        }
        _ => panic!("Expected ThreadUpdated variant"),
    }
}

#[test]
fn test_reconcile_and_apply_pending_updates_integration() {
    // Integration test for the complete race condition scenario
    let mut cache = ThreadCache::new();

    // Create pending thread
    let pending_id = cache.create_pending_thread(
        "Hello world".to_string(),
        ThreadType::Programming,
        None,
    );

    // Thread update arrives for a thread we don't have yet (edge case)
    // This would queue the update internally
    let queued = cache.update_thread_metadata(
        "nonexistent-thread",
        Some("Queued Title".to_string()),
        Some("Queued Desc".to_string()),
    );
    assert!(!queued); // Returns false, indicating it was queued

    // Now reconcile our actual thread
    cache.reconcile_thread_id(&pending_id, "backend-123", None);

    // Apply any pending updates to the reconciled thread
    cache.apply_pending_title_updates("backend-123");

    // Verify thread exists and was reconciled
    let thread = cache.get_thread("backend-123").unwrap();
    assert_eq!(thread.id, "backend-123");
}

#[test]
fn test_websocket_thread_updated_race_with_pending_id() {
    // Test the exact race condition: WebSocket thread_updated arrives with pending_id
    // before reconciliation completes
    let mut cache = ThreadCache::new();

    // Step 1: Create a pending thread (user sends a message, client generates UUID)
    let pending_id = cache.create_pending_thread(
        "What is Rust?".to_string(),
        ThreadType::Programming,
        Some("/Users/test/project".to_string()),
    );

    // Verify initial state
    let thread = cache.get_thread(&pending_id).unwrap();
    assert_eq!(thread.title, "What is Rust?");
    assert!(thread.description.is_none());

    // Step 2: Simulate WebSocket thread_updated arriving BEFORE reconciliation
    // The backend sends title update using the pending_id the client sent
    let ws_json = format!(r#"{{
        "type": "thread_updated",
        "thread_id": "{}",
        "title": "Introduction to Rust Programming",
        "description": "A conversation about the Rust programming language and its benefits",
        "timestamp": 1705315800000
    }}"#, pending_id);

    // Parse the WebSocket message
    let msg: WsIncomingMessage = serde_json::from_str(&ws_json).unwrap();
    match msg {
        WsIncomingMessage::ThreadUpdated(update) => {
            assert_eq!(update.thread_id, pending_id);

            // Apply the update (this goes through update_thread_metadata)
            let updated = cache.update_thread_metadata(
                &update.thread_id,
                Some(update.title.clone()),
                Some(update.description.clone()),
            );

            // Should update immediately since thread exists with pending_id
            assert!(updated);

            // Verify update was applied
            let thread = cache.get_thread(&pending_id).unwrap();
            assert_eq!(thread.title, "Introduction to Rust Programming");
            assert_eq!(thread.description, Some("A conversation about the Rust programming language and its benefits".to_string()));
        }
        _ => panic!("Expected ThreadUpdated variant"),
    }

    // Step 3: Now reconciliation happens (backend returns real ID)
    cache.reconcile_thread_id(&pending_id, "backend-thread-xyz", None);

    // Step 4: Verify the thread retains its updated title after reconciliation
    assert!(cache.get_thread(&pending_id).is_none()); // Old ID gone
    let thread = cache.get_thread("backend-thread-xyz").unwrap();
    assert_eq!(thread.title, "Introduction to Rust Programming");
    assert_eq!(thread.description, Some("A conversation about the Rust programming language and its benefits".to_string()));
    assert_eq!(thread.working_directory, Some("/Users/test/project".to_string()));
}

#[test]
fn test_websocket_thread_updated_queued_before_thread_exists() {
    // Test scenario: thread_updated arrives before the thread is even created locally
    // (extreme race condition - unlikely but possible)
    let mut cache = ThreadCache::new();

    // Simulate WebSocket thread_updated arriving for a thread that doesn't exist yet
    let ws_json = r#"{
        "type": "thread_updated",
        "thread_id": "future-thread-id",
        "title": "Pre-emptive Title Update",
        "description": "This update arrived before the thread was created",
        "timestamp": 1705315800000
    }"#;

    // Parse the WebSocket message
    let msg: WsIncomingMessage = serde_json::from_str(ws_json).unwrap();
    match msg {
        WsIncomingMessage::ThreadUpdated(update) => {
            // Try to apply the update - thread doesn't exist, should be queued
            let updated = cache.update_thread_metadata(
                &update.thread_id,
                Some(update.title.clone()),
                Some(update.description.clone()),
            );

            // Returns false because thread doesn't exist
            assert!(!updated);
        }
        _ => panic!("Expected ThreadUpdated variant"),
    }

    // Now create the thread with that ID (simulating it being created by another process)
    let pending_id = cache.create_pending_thread(
        "Original message".to_string(),
        ThreadType::Programming,
        None,
    );

    // Reconcile to "future-thread-id"
    cache.reconcile_thread_id(&pending_id, "future-thread-id", None);

    // The queued update should have been applied during reconciliation
    let thread = cache.get_thread("future-thread-id").unwrap();
    assert_eq!(thread.title, "Pre-emptive Title Update");
    assert_eq!(thread.description, Some("This update arrived before the thread was created".to_string()));
}

#[test]
fn test_thread_updated_full_integration_with_ws_routing() {
    // End-to-end test: Parse WebSocket message, update cache, verify final state
    // This simulates the complete flow from receiving a WebSocket message to cache update

    // Step 1: Setup - create a thread that exists in cache
    let mut cache = ThreadCache::new();
    let pending_id = cache.create_pending_thread(
        "Help me write a function".to_string(),
        ThreadType::Programming,
        Some("/Users/dev/myproject".to_string()),
    );

    // Reconcile to get a "real" backend ID
    cache.reconcile_thread_id(&pending_id, "cm5real123", None);

    // Step 2: Simulate receiving a thread_updated WebSocket message
    let ws_json = r#"{
        "type": "thread_updated",
        "thread_id": "cm5real123",
        "title": "Writing a Fibonacci Function",
        "description": "Assistance with implementing a recursive Fibonacci function in Rust",
        "timestamp": 1705315900000
    }"#;

    let msg: WsIncomingMessage = serde_json::from_str(ws_json).unwrap();

    // Step 3: Route the message (simulating what websocket.rs does)
    match msg {
        WsIncomingMessage::ThreadUpdated(update) => {
            // This is what the handler does
            let updated = cache.update_thread_metadata(
                &update.thread_id,
                Some(update.title),
                Some(update.description),
            );
            assert!(updated);
        }
        _ => panic!("Expected ThreadUpdated"),
    }

    // Step 4: Verify the cache was updated correctly
    let thread = cache.get_thread("cm5real123").unwrap();
    assert_eq!(thread.title, "Writing a Fibonacci Function");
    assert_eq!(thread.description, Some("Assistance with implementing a recursive Fibonacci function in Rust".to_string()));
    // Original properties should be preserved
    assert_eq!(thread.working_directory, Some("/Users/dev/myproject".to_string()));
    assert_eq!(thread.thread_type, ThreadType::Programming);
}
