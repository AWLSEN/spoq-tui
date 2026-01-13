// Integration tests for ThreadCache streaming operations
// These tests complement the unit tests in src/cache.rs
// by testing complex streaming workflows and edge cases

use spoq::cache::ThreadCache;
use spoq::models::MessageRole;

#[test]
fn test_complete_streaming_workflow_with_multiple_tokens() {
    let mut cache = ThreadCache::new();

    // Create a streaming thread
    let thread_id = cache.create_streaming_thread("What is Rust?".to_string());

    // Verify thread exists with user message and streaming assistant message
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[1].role, MessageRole::Assistant);
    assert_eq!(messages[1].is_streaming, true);
    assert_eq!(messages[1].partial_content, "");

    // Append multiple tokens to simulate real streaming
    cache.append_to_message(&thread_id, "Rust ");
    cache.append_to_message(&thread_id, "is ");
    cache.append_to_message(&thread_id, "a ");
    cache.append_to_message(&thread_id, "systems ");
    cache.append_to_message(&thread_id, "programming ");
    cache.append_to_message(&thread_id, "language.");

    // Verify streaming content accumulates
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages[1].partial_content, "Rust is a systems programming language.");
    assert_eq!(messages[1].content, ""); // Final content still empty during streaming

    // Finalize the message
    cache.finalize_message(&thread_id, 42);

    // Verify message is finalized
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages[1].id, 42);
    assert_eq!(messages[1].content, "Rust is a systems programming language.");
    assert_eq!(messages[1].is_streaming, false);
    assert_eq!(messages[1].partial_content, "");
}

#[test]
fn test_multiple_streaming_threads_simultaneously() {
    let mut cache = ThreadCache::new();

    // Create three streaming threads
    let thread1 = cache.create_streaming_thread("Question 1".to_string());
    let thread2 = cache.create_streaming_thread("Question 2".to_string());
    let thread3 = cache.create_streaming_thread("Question 3".to_string());

    // Append tokens to each thread in interleaved fashion
    cache.append_to_message(&thread1, "Answer ");
    cache.append_to_message(&thread2, "Response ");
    cache.append_to_message(&thread1, "1");
    cache.append_to_message(&thread3, "Reply ");
    cache.append_to_message(&thread2, "2");
    cache.append_to_message(&thread3, "3");

    // Verify each thread has correct streaming content
    let msgs1 = cache.get_messages(&thread1).unwrap();
    assert_eq!(msgs1[1].partial_content, "Answer 1");

    let msgs2 = cache.get_messages(&thread2).unwrap();
    assert_eq!(msgs2[1].partial_content, "Response 2");

    let msgs3 = cache.get_messages(&thread3).unwrap();
    assert_eq!(msgs3[1].partial_content, "Reply 3");

    // Finalize threads
    cache.finalize_message(&thread1, 101);
    cache.finalize_message(&thread2, 102);
    cache.finalize_message(&thread3, 103);

    // Verify all threads are finalized correctly
    assert_eq!(cache.get_messages(&thread1).unwrap()[1].content, "Answer 1");
    assert_eq!(cache.get_messages(&thread2).unwrap()[1].content, "Response 2");
    assert_eq!(cache.get_messages(&thread3).unwrap()[1].content, "Reply 3");
}

#[test]
fn test_thread_ordering_with_streaming_operations() {
    let mut cache = ThreadCache::new();

    // Create threads in sequence
    let thread1 = cache.create_streaming_thread("First".to_string());
    let thread2 = cache.create_streaming_thread("Second".to_string());
    let thread3 = cache.create_streaming_thread("Third".to_string());

    // Verify order (most recent first)
    let threads = cache.threads();
    assert_eq!(threads.len(), 3);
    assert_eq!(threads[0].id, thread3);
    assert_eq!(threads[1].id, thread2);
    assert_eq!(threads[2].id, thread1);

    // Add content and finalize
    cache.append_to_message(&thread1, "Content 1");
    cache.finalize_message(&thread1, 201);

    // Order should remain the same after operations
    let threads_after = cache.threads();
    assert_eq!(threads_after.len(), 3);
    assert_eq!(threads_after[0].id, thread3);
    assert_eq!(threads_after[1].id, thread2);
    assert_eq!(threads_after[2].id, thread1);
}

#[test]
fn test_append_to_nonexistent_thread_does_not_panic() {
    let mut cache = ThreadCache::new();

    // Appending to non-existent thread should not panic
    cache.append_to_message("fake-thread-id", "This should be ignored");

    // Cache should still be empty
    assert_eq!(cache.threads().len(), 0);
}

#[test]
fn test_finalize_without_streaming_message_does_not_panic() {
    let mut cache = ThreadCache::new();

    // Create a thread
    let thread_id = cache.create_streaming_thread("Test".to_string());

    // Finalize it
    cache.finalize_message(&thread_id, 301);

    // Try to finalize again (no streaming message exists anymore)
    cache.finalize_message(&thread_id, 302);

    // Verify thread still has correct messages
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].id, 301); // Should still have first finalized ID
}

#[test]
fn test_empty_token_append() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test".to_string());

    // Append empty string
    cache.append_to_message(&thread_id, "");

    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages[1].partial_content, "");

    // Append some content
    cache.append_to_message(&thread_id, "Hello");

    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages[1].partial_content, "Hello");
}

#[test]
fn test_cache_clear_removes_streaming_threads() {
    let mut cache = ThreadCache::new();

    // Create streaming threads
    let _thread1 = cache.create_streaming_thread("Test 1".to_string());
    let _thread2 = cache.create_streaming_thread("Test 2".to_string());

    // Verify threads exist
    assert_eq!(cache.threads().len(), 2);

    // Clear cache
    cache.clear();

    // Verify cache is empty
    assert_eq!(cache.threads().len(), 0);
}

#[test]
fn test_get_messages_mut_allows_modification() {
    let mut cache = ThreadCache::new();
    let thread_id = cache.create_streaming_thread("Test".to_string());

    // Get mutable reference and modify
    if let Some(messages) = cache.get_messages_mut(&thread_id) {
        messages[0].content = "Modified content".to_string();
    }

    // Verify modification persisted
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages[0].content, "Modified content");
}
