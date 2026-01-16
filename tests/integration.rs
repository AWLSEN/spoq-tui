//! Integration tests for the Spoq TUI application
//!
//! These tests verify the full flow of thread creation, screen navigation,
//! and cache operations.

use spoq::app::{App, Screen};
use spoq::models::{MessageRole, ThreadType};

#[tokio::test]
async fn test_full_thread_creation_flow() {
    // 1. Create App instance
    let mut app = App::new().expect("Failed to create app");
    let initial_thread_count = app.cache.thread_count();

    // 2. Simulate typing in input_box (use insert_char)
    app.input_box.insert_char('H');
    app.input_box.insert_char('e');
    app.input_box.insert_char('l');
    app.input_box.insert_char('l');
    app.input_box.insert_char('o');
    app.input_box.insert_char(' ');
    app.input_box.insert_char('w');
    app.input_box.insert_char('o');
    app.input_box.insert_char('r');
    app.input_box.insert_char('l');
    app.input_box.insert_char('d');

    assert_eq!(app.input_box.content(), "Hello world");

    // 3. Call submit_input()
    app.submit_input(ThreadType::Conversation);

    // 4. Verify thread created in cache
    assert_eq!(
        app.cache.thread_count(),
        initial_thread_count + 1,
        "Thread count should increase by 1"
    );

    // 5. Verify screen changed to Conversation
    assert_eq!(
        app.screen,
        Screen::Conversation,
        "Screen should be Conversation after submit"
    );

    // 6. Verify active_thread_id is set
    assert!(
        app.active_thread_id.is_some(),
        "active_thread_id should be set"
    );

    // Verify the thread has the correct messages
    let thread_id = app.active_thread_id.as_ref().unwrap();
    let messages = app.cache.get_messages(thread_id).expect("Messages should exist");

    assert_eq!(messages.len(), 2, "Thread should have 2 messages (user + assistant)");
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[0].content, "Hello world");
    assert_eq!(messages[1].role, MessageRole::Assistant);
}

#[tokio::test]
async fn test_screen_navigation() {
    // 1. Start at CommandDeck
    let mut app = App::new().expect("Failed to create app");
    assert_eq!(
        app.screen,
        Screen::CommandDeck,
        "App should start at CommandDeck"
    );

    // 2. Create thread (switches to Conversation)
    app.input_box.insert_char('T');
    app.input_box.insert_char('e');
    app.input_box.insert_char('s');
    app.input_box.insert_char('t');
    app.submit_input(ThreadType::Conversation);

    assert_eq!(
        app.screen,
        Screen::Conversation,
        "Screen should be Conversation after creating thread"
    );

    // 3. Navigate back to CommandDeck
    app.navigate_to_command_deck();

    // 4. Verify screen state
    assert_eq!(
        app.screen,
        Screen::CommandDeck,
        "Screen should be CommandDeck after navigation"
    );

    // active_thread_id is cleared when navigating back to CommandDeck
    // This allows the next submit to create a new thread
    assert!(
        app.active_thread_id.is_none(),
        "active_thread_id should be cleared after navigation to CommandDeck"
    );
}

#[tokio::test]
async fn test_thread_appears_in_right_panel() {
    // 1. Create thread via submit_input
    let mut app = App::new().expect("Failed to create app");

    // Type a message
    let message = "Test message for right panel";
    for c in message.chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    // Get the thread ID that was just created
    let thread_id = app.active_thread_id.clone().expect("Thread should be created");

    // 2. Navigate to CommandDeck
    app.navigate_to_command_deck();

    // 3. Verify cache.threads() includes new thread
    let threads = app.cache.threads();
    let thread_ids: Vec<&String> = threads.iter().map(|t| &t.id).collect();

    assert!(
        thread_ids.contains(&&thread_id),
        "New thread should appear in cache.threads()"
    );

    // Verify the thread is at the front (most recent)
    assert_eq!(
        threads[0].id, thread_id,
        "New thread should be at the front of the list"
    );

    // Verify thread title matches the message
    assert_eq!(
        threads[0].title, message,
        "Thread title should match the input message"
    );
}

#[tokio::test]
async fn test_multiple_threads_ordering() {
    let mut app = App::new().expect("Failed to create app");
    let initial_count = app.cache.thread_count();

    // Create first thread
    for c in "First thread".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);
    let first_thread_id = app.active_thread_id.clone().unwrap();

    // Navigate back and create second thread
    app.navigate_to_command_deck();
    for c in "Second thread".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);
    let second_thread_id = app.active_thread_id.clone().unwrap();

    // Verify both threads exist
    assert_eq!(
        app.cache.thread_count(),
        initial_count + 2,
        "Two new threads should be created"
    );

    // Verify ordering (most recent first)
    let threads = app.cache.threads();
    assert_eq!(
        threads[0].id, second_thread_id,
        "Second thread should be first (most recent)"
    );

    // First thread should be after the second
    let first_thread_pos = threads.iter().position(|t| t.id == first_thread_id);
    let second_thread_pos = threads.iter().position(|t| t.id == second_thread_id);
    assert!(
        second_thread_pos < first_thread_pos,
        "Second thread should come before first thread in the list"
    );
}

#[tokio::test]
async fn test_empty_input_does_not_create_thread() {
    let mut app = App::new().expect("Failed to create app");
    let initial_count = app.cache.thread_count();

    // Submit with empty input
    app.submit_input(ThreadType::Conversation);

    // Verify no thread was created
    assert_eq!(
        app.cache.thread_count(),
        initial_count,
        "Empty input should not create a thread"
    );
    assert_eq!(
        app.screen,
        Screen::CommandDeck,
        "Screen should remain on CommandDeck"
    );
    assert!(
        app.active_thread_id.is_none(),
        "active_thread_id should remain None"
    );
}

#[tokio::test]
async fn test_whitespace_only_input_does_not_create_thread() {
    let mut app = App::new().expect("Failed to create app");
    let initial_count = app.cache.thread_count();

    // Type whitespace only
    app.input_box.insert_char(' ');
    app.input_box.insert_char(' ');
    app.input_box.insert_char(' ');
    app.submit_input(ThreadType::Conversation);

    // Verify no thread was created
    assert_eq!(
        app.cache.thread_count(),
        initial_count,
        "Whitespace-only input should not create a thread"
    );
}

#[tokio::test]
async fn test_input_cleared_after_submit() {
    let mut app = App::new().expect("Failed to create app");

    for c in "Test message".chars() {
        app.input_box.insert_char(c);
    }
    assert!(!app.input_box.is_empty(), "Input should have content");

    app.submit_input(ThreadType::Conversation);

    assert!(
        app.input_box.is_empty(),
        "Input should be cleared after submit"
    );
}

#[tokio::test]
async fn test_thread_messages_have_correct_roles() {
    let mut app = App::new().expect("Failed to create app");

    for c in "Hello AI".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let thread_id = app.active_thread_id.as_ref().unwrap();
    let messages = app.cache.get_messages(thread_id).unwrap();

    // First message should be User
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[0].thread_id, *thread_id);

    // Second message should be Assistant (stub response)
    assert_eq!(messages[1].role, MessageRole::Assistant);
    assert_eq!(messages[1].thread_id, *thread_id);
}

// ============================================================================
// Phase 9 Integration Tests - Thread System Architecture
// ============================================================================

/// Test Case 1: New Thread Flow
/// - Start at command deck (active_thread_id = None)
/// - Type message, press Enter
/// - Verify: Navigates to conversation (screen = Conversation)
/// - Verify: Cache creates thread with client-generated UUID
/// - Verify: active_thread_id is set to the UUID
#[tokio::test]
async fn test_new_thread_flow_complete() {
    let mut app = App::new().expect("Failed to create app");

    // 1. Start at command deck with no active thread
    assert_eq!(app.screen, Screen::CommandDeck, "Should start at CommandDeck");
    assert!(
        app.active_thread_id.is_none(),
        "active_thread_id should be None at start"
    );

    // 2. Type a message
    for c in "What is the meaning of life?".chars() {
        app.input_box.insert_char(c);
    }

    // 3. Submit (press Enter equivalent)
    app.submit_input(ThreadType::Conversation);

    // 4. Verify: Navigates to conversation
    assert_eq!(
        app.screen,
        Screen::Conversation,
        "Should navigate to Conversation screen after submit"
    );

    // 5. Verify: Cache creates thread with a valid UUID
    let thread_id = app
        .active_thread_id
        .as_ref()
        .expect("active_thread_id should be set");
    assert!(
        uuid::Uuid::parse_str(thread_id).is_ok(),
        "Thread ID should be a valid UUID, got: {}",
        thread_id
    );

    // 6. Verify thread exists in cache
    let thread = app
        .cache
        .get_thread(thread_id)
        .expect("Thread should exist in cache");
    assert_eq!(thread.id, *thread_id);

    // 7. Verify messages were created (user message + streaming assistant)
    let messages = app
        .cache
        .get_messages(thread_id)
        .expect("Messages should exist");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[0].content, "What is the meaning of life?");
    assert_eq!(messages[1].role, MessageRole::Assistant);
    assert!(messages[1].is_streaming);
}

/// Test Case 2: Continue Thread Flow
/// - In conversation (active_thread_id = Some(real_id))
/// - Type second message, press Enter
/// - Verify: Uses existing thread_id (not creating new)
/// - Verify: Messages added to same thread via add_streaming_message
#[tokio::test]
async fn test_continue_thread_flow() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Setup: Create first message in new thread
    for c in "First question".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let thread_id = app
        .active_thread_id
        .clone()
        .expect("Should have thread ID");
    assert!(uuid::Uuid::parse_str(&thread_id).is_ok());

    // Simulate backend response (echoes back same UUID)
    app.handle_message(AppMessage::ThreadCreated {
        pending_id: thread_id.clone(),
        real_id: thread_id.clone(),
        title: Some("First question".to_string()),
    });

    // Finalize first response
    app.cache.append_to_message(&thread_id, "First response");
    app.cache.finalize_message(&thread_id, 1);

    // Verify we're still in conversation with same thread ID
    assert_eq!(
        app.active_thread_id,
        Some(thread_id.clone()),
        "active_thread_id should remain the same"
    );
    assert_eq!(app.screen, Screen::Conversation);

    let initial_message_count = app.cache.get_messages(&thread_id).unwrap().len();

    // Type second message
    for c in "Follow-up question".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    // Verify: Should still be on same thread (NOT a new thread)
    assert_eq!(
        app.active_thread_id,
        Some(thread_id.clone()),
        "Should continue using same thread ID"
    );

    // Verify: Messages were added to existing thread
    let messages = app.cache.get_messages(&thread_id).unwrap();
    assert_eq!(
        messages.len(),
        initial_message_count + 2,
        "Should add 2 new messages (user + streaming assistant)"
    );

    // Verify the new user message
    let new_user_msg = &messages[messages.len() - 2];
    assert_eq!(new_user_msg.role, MessageRole::User);
    assert_eq!(new_user_msg.content, "Follow-up question");

    // Verify the new streaming assistant message
    let new_assistant_msg = &messages[messages.len() - 1];
    assert_eq!(new_assistant_msg.role, MessageRole::Assistant);
    assert!(new_assistant_msg.is_streaming);
}

/// Test Case 3: Back to Deck Flow
/// - Press Escape (or navigate_to_command_deck)
/// - Verify: Returns to command deck (screen = CommandDeck)
/// - Verify: active_thread_id is cleared (None)
#[tokio::test]
async fn test_back_to_deck_flow() {
    let mut app = App::new().expect("Failed to create app");

    // Setup: Create a thread and be in conversation
    for c in "Test message".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    assert_eq!(app.screen, Screen::Conversation);
    assert!(app.active_thread_id.is_some());

    let original_thread_id = app.active_thread_id.clone().unwrap();

    // Navigate back to command deck
    app.navigate_to_command_deck();

    // Verify: Returns to CommandDeck
    assert_eq!(
        app.screen,
        Screen::CommandDeck,
        "Screen should return to CommandDeck"
    );

    // Verify: active_thread_id is cleared
    assert!(
        app.active_thread_id.is_none(),
        "active_thread_id should be cleared after navigating to CommandDeck"
    );

    // Verify: The thread still exists in cache (not deleted)
    assert!(
        app.cache.get_thread(&original_thread_id).is_some(),
        "Thread should still exist in cache"
    );

    // Verify: Input is cleared
    assert!(
        app.input_box.is_empty(),
        "Input box should be cleared after navigation"
    );
}

/// Test Case 4: New Thread After Returning
/// - From command deck (after returning from thread)
/// - Type new message, press Enter
/// - Verify: Creates NEW thread (not continues old one)
/// - Verify: New pending ID is different from previous
#[tokio::test]
async fn test_new_thread_after_returning_to_deck() {
    let mut app = App::new().expect("Failed to create app");

    // Create first thread
    for c in "First thread message".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let first_thread_id = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&first_thread_id).is_ok());

    // Navigate back to command deck
    app.navigate_to_command_deck();

    assert_eq!(app.screen, Screen::CommandDeck);
    assert!(app.active_thread_id.is_none());

    // Create second thread from command deck
    for c in "Second thread message".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    // Verify: Creates NEW thread
    let second_thread_id = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&second_thread_id).is_ok());

    // Verify: New pending ID is DIFFERENT from previous
    assert_ne!(
        first_thread_id, second_thread_id,
        "New thread should have different ID from previous thread"
    );

    // Verify both threads exist in cache
    assert!(
        app.cache.get_thread(&first_thread_id).is_some(),
        "First thread should still exist"
    );
    assert!(
        app.cache.get_thread(&second_thread_id).is_some(),
        "Second thread should exist"
    );

    // Verify thread count
    assert_eq!(
        app.cache.thread_count(),
        2,
        "Should have exactly 2 threads"
    );

    // Verify second thread is at the front (most recent)
    let threads = app.cache.threads();
    assert_eq!(threads[0].id, second_thread_id);
    assert_eq!(threads[1].id, first_thread_id);
}

/// Test Case 5: Thread Reconciliation
/// - Create pending thread
/// - Send ThreadCreated message with real_id
/// - Verify: Pending ID is replaced with real ID in cache
/// - Verify: active_thread_id is updated if it was the pending ID
/// - Verify: Messages accessible by new real ID
#[tokio::test]
async fn test_thread_reconciliation_complete() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Create pending thread
    for c in "What is Rust?".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let pending_id = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&pending_id).is_ok());

    // Add some streaming content
    app.cache.append_to_message(&pending_id, "Rust is ");
    app.cache.append_to_message(&pending_id, "a systems programming language.");

    // Verify pending thread exists with content
    let messages_before = app.cache.get_messages(&pending_id).unwrap();
    assert_eq!(messages_before.len(), 2);
    assert_eq!(
        messages_before[1].partial_content,
        "Rust is a systems programming language."
    );

    // Send ThreadCreated message (simulating backend response)
    let real_id = "backend-thread-abc123".to_string();
    app.handle_message(AppMessage::ThreadCreated {
        pending_id: pending_id.clone(),
        real_id: real_id.clone(),
        title: Some("Rust Programming".to_string()),
    });

    // Verify: Pending ID is replaced in cache
    assert!(
        app.cache.get_thread(&pending_id).is_none(),
        "Pending thread should no longer exist"
    );
    assert!(
        app.cache.get_thread(&real_id).is_some(),
        "Real thread should exist"
    );

    // Verify: active_thread_id is updated
    assert_eq!(
        app.active_thread_id,
        Some(real_id.clone()),
        "active_thread_id should be updated to real ID"
    );

    // Verify: Messages accessible by new real ID
    let messages_after = app.cache.get_messages(&real_id).unwrap();
    assert_eq!(messages_after.len(), 2);

    // Verify: Message thread_ids are updated
    for msg in messages_after {
        assert_eq!(
            msg.thread_id, real_id,
            "Message thread_id should be updated to real ID"
        );
    }

    // Verify: Content preserved
    let assistant_msg = &app.cache.get_messages(&real_id).unwrap()[1];
    assert_eq!(
        assistant_msg.partial_content,
        "Rust is a systems programming language."
    );

    // Verify: Title updated
    let thread = app.cache.get_thread(&real_id).unwrap();
    assert_eq!(thread.title, "Rust Programming");

    // Verify: Can finalize the message using new ID
    app.cache.finalize_message(&real_id, 42);
    let finalized_msg = &app.cache.get_messages(&real_id).unwrap()[1];
    assert!(!finalized_msg.is_streaming);
    assert_eq!(finalized_msg.id, 42);
    assert_eq!(
        finalized_msg.content,
        "Rust is a systems programming language."
    );
}

/// Test Case 6: Open Thread Flow (Phase 4B)
/// - Create thread, navigate back to deck
/// - Call open_selected_thread()
/// - Verify: active_thread_id set to selected thread
/// - Verify: screen = Conversation
#[tokio::test]
async fn test_open_thread_flow() {
    let mut app = App::new().expect("Failed to create app");

    // Create first thread
    for c in "First thread".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);
    let first_thread_id = app.active_thread_id.clone().unwrap();

    // Navigate back to deck
    app.navigate_to_command_deck();

    // Create second thread
    for c in "Second thread".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);
    let second_thread_id = app.active_thread_id.clone().unwrap();

    // Navigate back to deck
    app.navigate_to_command_deck();

    assert_eq!(app.screen, Screen::CommandDeck);
    assert!(app.active_thread_id.is_none());

    // Verify thread order: second thread is at index 0 (most recent)
    let threads = app.cache.threads();
    assert_eq!(threads[0].id, second_thread_id);
    assert_eq!(threads[1].id, first_thread_id);

    // Select and open the second thread (at index 0)
    app.threads_index = 0;
    app.open_selected_thread();

    // Verify: active_thread_id set to selected thread
    assert_eq!(
        app.active_thread_id,
        Some(second_thread_id.clone()),
        "active_thread_id should be set to selected thread"
    );

    // Verify: screen = Conversation
    assert_eq!(
        app.screen,
        Screen::Conversation,
        "Screen should be Conversation"
    );

    // Navigate back and select first thread (at index 1)
    app.navigate_to_command_deck();
    app.threads_index = 1;
    app.open_selected_thread();

    // Verify: opens the first thread
    assert_eq!(
        app.active_thread_id,
        Some(first_thread_id.clone()),
        "Should open first thread when index=1"
    );
    assert_eq!(app.screen, Screen::Conversation);
}

/// Test open_thread method directly
#[tokio::test]
async fn test_open_thread_direct() {
    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    for c in "Test thread".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);
    let thread_id = app.active_thread_id.clone().unwrap();

    // Navigate away
    app.navigate_to_command_deck();
    assert!(app.active_thread_id.is_none());

    // Open thread directly by ID
    app.open_thread(thread_id.clone());

    // Verify state
    assert_eq!(app.active_thread_id, Some(thread_id));
    assert_eq!(app.screen, Screen::Conversation);
    assert!(app.input_box.is_empty());
}

/// Test open_selected_thread with invalid index (beyond thread list)
#[tokio::test]
async fn test_open_selected_thread_invalid_index() {
    use spoq::app::Focus;

    let mut app = App::new().expect("Failed to create app");

    // Create one thread
    for c in "Single thread".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    // Navigate back
    app.navigate_to_command_deck();

    // Set index beyond thread list (e.g., "New Thread" button position)
    app.threads_index = 5;
    app.open_selected_thread();

    // Should not navigate to conversation, just focus input
    assert_eq!(
        app.screen,
        Screen::CommandDeck,
        "Should stay on CommandDeck when index is invalid"
    );
    assert!(
        app.active_thread_id.is_none(),
        "active_thread_id should remain None"
    );
    assert_eq!(app.focus, Focus::Input, "Focus should move to Input");
}

/// Test complete end-to-end workflow: create -> continue -> return -> new -> open
#[tokio::test]
async fn test_complete_end_to_end_workflow() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // === Phase 1: Create new thread ===
    for c in "What is Rust?".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let pending_id1 = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&pending_id1).is_ok());
    assert_eq!(app.screen, Screen::Conversation);

    // Simulate backend response
    app.handle_message(AppMessage::ThreadCreated {
        pending_id: pending_id1.clone(),
        real_id: "thread-1".to_string(),
        title: Some("Rust Intro".to_string()),
    });
    app.cache.append_to_message("thread-1", "Rust is awesome!");
    app.cache.finalize_message("thread-1", 1);

    assert_eq!(app.active_thread_id, Some("thread-1".to_string()));

    // === Phase 2: Continue thread ===
    for c in "Tell me more".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    assert_eq!(
        app.active_thread_id,
        Some("thread-1".to_string()),
        "Should still be on same thread"
    );
    assert_eq!(
        app.cache.get_messages("thread-1").unwrap().len(),
        4,
        "Should have 4 messages now"
    );

    // Simulate second response
    app.cache.append_to_message("thread-1", "More Rust info!");
    app.cache.finalize_message("thread-1", 3);

    // === Phase 3: Return to deck ===
    app.navigate_to_command_deck();

    assert_eq!(app.screen, Screen::CommandDeck);
    assert!(app.active_thread_id.is_none());

    // === Phase 4: Create second thread ===
    for c in "Different topic".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let pending_id2 = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&pending_id2).is_ok());
    assert_ne!(pending_id2, "thread-1");

    // Simulate backend response for second thread
    app.handle_message(AppMessage::ThreadCreated {
        pending_id: pending_id2.clone(),
        real_id: "thread-2".to_string(),
        title: Some("Different Topic".to_string()),
    });
    app.cache.append_to_message("thread-2", "Response to different topic");
    app.cache.finalize_message("thread-2", 4);

    assert_eq!(app.active_thread_id, Some("thread-2".to_string()));

    // === Phase 5: Return to deck and open first thread ===
    app.navigate_to_command_deck();

    // Thread 2 should be at index 0 (most recent), Thread 1 at index 1
    let threads = app.cache.threads();
    assert_eq!(threads[0].id, "thread-2");
    assert_eq!(threads[1].id, "thread-1");

    // Open the first thread (index 1)
    app.threads_index = 1;
    app.open_selected_thread();

    assert_eq!(app.active_thread_id, Some("thread-1".to_string()));
    assert_eq!(app.screen, Screen::Conversation);

    // Verify first thread still has all its messages
    let messages = app.cache.get_messages("thread-1").unwrap();
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].content, "What is Rust?");
    assert_eq!(messages[1].content, "Rust is awesome!");
    assert_eq!(messages[2].content, "Tell me more");
    assert_eq!(messages[3].content, "More Rust info!");
}

// ============================================================================
// Phase 7 Integration Tests - Thread Metadata Updates (thread_updated flow)
// ============================================================================

/// Test Case 1: SSE thread_updated event → cache updated → UI renders
/// - Create thread with original title
/// - Send ThreadMetadataUpdated message with new title and description
/// - Verify cache is updated correctly
/// - Verify thread appears in UI with updated metadata
#[tokio::test]
async fn test_thread_updated_full_flow() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    for c in "What is Rust?".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let pending_id = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&pending_id).is_ok());

    // Reconcile to real ID
    app.handle_message(AppMessage::ThreadCreated {
        pending_id: pending_id.clone(),
        real_id: "thread-123".to_string(),
        title: Some("What is Rust?".to_string()),
    });

    // Verify initial state
    let thread = app.cache.get_thread("thread-123").unwrap();
    assert_eq!(thread.title, "What is Rust?");
    assert_eq!(thread.description, None);

    // Simulate thread_updated SSE event
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: "thread-123".to_string(),
        title: Some("Rust Programming Language".to_string()),
        description: Some("A systems programming language focused on safety and performance".to_string()),
    });

    // Verify cache was updated
    let updated_thread = app.cache.get_thread("thread-123").unwrap();
    assert_eq!(updated_thread.title, "Rust Programming Language");
    assert_eq!(
        updated_thread.description,
        Some("A systems programming language focused on safety and performance".to_string())
    );

    // Navigate back to command deck to see the thread in the list
    app.navigate_to_command_deck();

    // Verify thread appears in threads list with updated metadata
    let threads = app.cache.threads();
    let our_thread = threads.iter().find(|t| t.id == "thread-123").unwrap();
    assert_eq!(our_thread.title, "Rust Programming Language");
    assert_eq!(
        our_thread.description,
        Some("A systems programming language focused on safety and performance".to_string())
    );
}

/// Test Case 2: thread_updated with pending thread ID
/// - Create pending thread (before reconciliation)
/// - Send thread_updated with pending ID
/// - Verify cache resolves pending ID and updates correctly
#[tokio::test]
async fn test_thread_updated_with_pending_id() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Create a thread (pending)
    for c in "Test message".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let pending_id = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&pending_id).is_ok());

    // Verify thread exists with pending ID
    assert!(app.cache.get_thread(&pending_id).is_some());

    // Send thread_updated with pending ID (before reconciliation)
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: pending_id.clone(),
        title: Some("Updated Title via Pending ID".to_string()),
        description: Some("Description set while pending".to_string()),
    });

    // Verify update was applied to pending thread
    let pending_thread = app.cache.get_thread(&pending_id).unwrap();
    assert_eq!(pending_thread.title, "Updated Title via Pending ID");
    assert_eq!(
        pending_thread.description,
        Some("Description set while pending".to_string())
    );

    // Now reconcile to real ID
    app.handle_message(AppMessage::ThreadCreated {
        pending_id: pending_id.clone(),
        real_id: "real-thread-456".to_string(),
        title: Some("Updated Title via Pending ID".to_string()), // Backend sends current title
    });

    // Verify real thread exists with the metadata that was set on pending
    let real_thread = app.cache.get_thread("real-thread-456").unwrap();
    assert_eq!(real_thread.title, "Updated Title via Pending ID");
    assert_eq!(
        real_thread.description,
        Some("Description set while pending".to_string())
    );

    // Verify pending thread no longer exists
    assert!(app.cache.get_thread(&pending_id).is_none());

    // Send another update with the real ID
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: "real-thread-456".to_string(),
        title: Some("Final Title".to_string()),
        description: Some("Final Description".to_string()),
    });

    // Verify final update was applied
    let final_thread = app.cache.get_thread("real-thread-456").unwrap();
    assert_eq!(final_thread.title, "Final Title");
    assert_eq!(final_thread.description, Some("Final Description".to_string()));
}

/// Test Case 3: thread_updated with only title (no description)
/// - Create thread
/// - Send thread_updated with only title
/// - Verify only title is updated, description remains None
#[tokio::test]
async fn test_thread_updated_title_only() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    let thread_id = app.cache.create_streaming_thread("Original Title".to_string());

    // Send thread_updated with only title
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: thread_id.clone(),
        title: Some("New Title Only".to_string()),
        description: None,
    });

    // Verify title updated, description still None
    let thread = app.cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "New Title Only");
    assert_eq!(thread.description, None);
}

/// Test Case 4: thread_updated with only description (no title)
/// - Create thread
/// - Send thread_updated with only description
/// - Verify only description is updated, title unchanged
#[tokio::test]
async fn test_thread_updated_description_only() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    let thread_id = app.cache.create_streaming_thread("Original Title".to_string());

    // Send thread_updated with only description
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: thread_id.clone(),
        title: None,
        description: Some("New Description Only".to_string()),
    });

    // Verify description updated, title unchanged
    let thread = app.cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "Original Title");
    assert_eq!(thread.description, Some("New Description Only".to_string()));
}

/// Test Case 5: thread_updated with empty strings
/// - Create thread with description
/// - Send thread_updated with empty strings
/// - Verify empty strings are applied (not treated as None)
#[tokio::test]
async fn test_thread_updated_empty_strings() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    let thread_id = app.cache.create_streaming_thread("Original Title".to_string());

    // Set initial description
    app.cache.update_thread_metadata(
        &thread_id,
        None,
        Some("Initial Description".to_string()),
    );

    // Send thread_updated with empty strings
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: thread_id.clone(),
        title: Some("".to_string()),
        description: Some("".to_string()),
    });

    // Verify empty strings are applied
    let thread = app.cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "");
    assert_eq!(thread.description, Some("".to_string()));
}

/// Test Case 6: multiple thread_updated events in sequence
/// - Create thread
/// - Send multiple thread_updated messages
/// - Verify each update is applied correctly
#[tokio::test]
async fn test_thread_updated_multiple_updates() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    let thread_id = app.cache.create_streaming_thread("Version 1".to_string());

    // First update: add description
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: thread_id.clone(),
        title: None,
        description: Some("Description v1".to_string()),
    });

    let thread = app.cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "Version 1");
    assert_eq!(thread.description, Some("Description v1".to_string()));

    // Second update: update title
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: thread_id.clone(),
        title: Some("Version 2".to_string()),
        description: None,
    });

    let thread = app.cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "Version 2");
    assert_eq!(thread.description, Some("Description v1".to_string())); // Unchanged

    // Third update: update both
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: thread_id.clone(),
        title: Some("Version 3".to_string()),
        description: Some("Description v3".to_string()),
    });

    let thread = app.cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "Version 3");
    assert_eq!(thread.description, Some("Description v3".to_string()));
}

/// Test Case 7: thread_updated for non-existent thread
/// - Send thread_updated for thread that doesn't exist
/// - Verify app doesn't panic and handles gracefully
#[tokio::test]
async fn test_thread_updated_nonexistent_thread() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Send thread_updated for non-existent thread
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: "nonexistent-thread-999".to_string(),
        title: Some("Title".to_string()),
        description: Some("Description".to_string()),
    });

    // Should not panic, just do nothing
    assert!(app.cache.get_thread("nonexistent-thread-999").is_none());
}

/// Test Case 8: thread_updated during active conversation
/// - Create thread and be in conversation view
/// - Send thread_updated while viewing the thread
/// - Verify metadata is updated without disrupting the conversation
#[tokio::test]
async fn test_thread_updated_during_active_conversation() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Create a thread and enter conversation
    for c in "Test message".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let pending_id = app.active_thread_id.clone().unwrap();

    // Reconcile to real ID
    app.handle_message(AppMessage::ThreadCreated {
        pending_id: pending_id.clone(),
        real_id: "thread-conversation".to_string(),
        title: Some("Test message".to_string()),
    });

    // Verify we're in conversation view
    assert_eq!(app.screen, Screen::Conversation);
    assert_eq!(app.active_thread_id, Some("thread-conversation".to_string()));

    // Send thread_updated while in conversation
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: "thread-conversation".to_string(),
        title: Some("Updated During Conversation".to_string()),
        description: Some("Metadata changed while viewing".to_string()),
    });

    // Verify metadata was updated
    let thread = app.cache.get_thread("thread-conversation").unwrap();
    assert_eq!(thread.title, "Updated During Conversation");
    assert_eq!(
        thread.description,
        Some("Metadata changed while viewing".to_string())
    );

    // Verify we're still in conversation view (not disrupted)
    assert_eq!(app.screen, Screen::Conversation);
    assert_eq!(app.active_thread_id, Some("thread-conversation".to_string()));

    // Navigate back and verify thread list shows updated metadata
    app.navigate_to_command_deck();
    let threads = app.cache.threads();
    let our_thread = threads.iter().find(|t| t.id == "thread-conversation").unwrap();
    assert_eq!(our_thread.title, "Updated During Conversation");
}

/// Test Case 9: Verify thread description appears in UI thread list
/// - Create multiple threads with different descriptions
/// - Verify threads() returns threads with correct descriptions
#[tokio::test]
async fn test_thread_description_in_thread_list() {
    use spoq::app::AppMessage;

    let mut app = App::new().expect("Failed to create app");

    // Create thread 1 with description
    let thread1_id = app.cache.create_streaming_thread("Thread 1".to_string());
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: thread1_id.clone(),
        title: None,
        description: Some("Description for thread 1".to_string()),
    });

    // Create thread 2 without description
    let thread2_id = app.cache.create_streaming_thread("Thread 2".to_string());

    // Create thread 3 with description
    let thread3_id = app.cache.create_streaming_thread("Thread 3".to_string());
    app.handle_message(AppMessage::ThreadMetadataUpdated {
        thread_id: thread3_id.clone(),
        title: None,
        description: Some("Description for thread 3".to_string()),
    });

    // Get threads list
    let threads = app.cache.threads();

    // Find our threads (order is newest first, so thread3, thread2, thread1)
    let t1 = threads.iter().find(|t| t.id == thread1_id).unwrap();
    let t2 = threads.iter().find(|t| t.id == thread2_id).unwrap();
    let t3 = threads.iter().find(|t| t.id == thread3_id).unwrap();

    // Verify descriptions
    assert_eq!(t1.description, Some("Description for thread 1".to_string()));
    assert_eq!(t2.description, None);
    assert_eq!(t3.description, Some("Description for thread 3".to_string()));
}

/// Test Case 10: End-to-end SSE parsing → event conversion → cache update
/// - Parse thread_updated SSE event
/// - Convert to ThreadUpdatedEvent
/// - Send ThreadMetadataUpdated message
/// - Verify full flow works
#[tokio::test]
async fn test_thread_updated_sse_to_cache_integration() {
    use spoq::app::AppMessage;
    use spoq::sse::{SseParser, SseEvent};

    let mut app = App::new().expect("Failed to create app");

    // Create a thread
    let thread_id = app.cache.create_streaming_thread("Original".to_string());

    // Simulate SSE stream parsing
    let mut parser = SseParser::new();

    parser.feed_line("event: thread_updated").unwrap();
    parser.feed_line(&format!(
        r#"data: {{"thread_id": "{}", "title": "Updated via SSE", "description": "SSE Description"}}"#,
        thread_id
    )).unwrap();

    let event = parser.feed_line("").unwrap();

    // Verify SSE was parsed correctly
    match event {
        Some(SseEvent::ThreadUpdated { thread_id: tid, title, description }) => {
            assert_eq!(tid, thread_id);
            assert_eq!(title, Some("Updated via SSE".to_string()));
            assert_eq!(description, Some("SSE Description".to_string()));

            // Simulate conductor converting SSE to AppMessage
            app.handle_message(AppMessage::ThreadMetadataUpdated {
                thread_id: tid,
                title,
                description,
            });
        }
        _ => panic!("Expected ThreadUpdated event"),
    }

    // Verify cache was updated
    let thread = app.cache.get_thread(&thread_id).unwrap();
    assert_eq!(thread.title, "Updated via SSE");
    assert_eq!(thread.description, Some("SSE Description".to_string()));
}

/// Test that rapid submission on pending thread is blocked
#[tokio::test]
async fn test_rapid_submit_blocked_on_pending_thread() {
    let mut app = App::new().expect("Failed to create app");

    // First submit creates pending thread
    for c in "First message".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let pending_id = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&pending_id).is_ok());

    // Try to submit again while still pending
    for c in "Second message".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    // Should be blocked with error
    assert!(app.stream_error.is_some());
    assert!(app.stream_error.as_ref().unwrap().contains("wait"));

    // Input should NOT be cleared (submission was rejected)
    assert_eq!(app.input_box.content(), "Second message");

    // Should still be on same pending thread
    assert_eq!(app.active_thread_id, Some(pending_id));

    // Cache should still only have 2 messages (from first submit)
    let messages = app
        .cache
        .get_messages(&app.active_thread_id.clone().unwrap())
        .unwrap();
    assert_eq!(messages.len(), 2);
}
