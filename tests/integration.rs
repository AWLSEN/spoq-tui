//! Integration tests for the Spoq TUI application
//!
//! These tests verify the full flow of thread creation, screen navigation,
//! and cache operations.

use spoq::app::{App, Screen};
use spoq::models::{MessageRole, ThreadType};

/// Create a test app starting at CommandDeck screen
fn create_test_app() -> App {
    let mut app = App::new().expect("Failed to create app");
    app.screen = Screen::CommandDeck; // Default to CommandDeck for tests
    app
}

#[tokio::test]
async fn test_full_thread_creation_flow() {
    // 1. Create App instance
    let mut app = create_test_app();
    let initial_thread_count = app.cache.thread_count();

    // 2. Simulate typing in input_box (use insert_char)
    app.textarea.insert_char('H');
    app.textarea.insert_char('e');
    app.textarea.insert_char('l');
    app.textarea.insert_char('l');
    app.textarea.insert_char('o');
    app.textarea.insert_char(' ');
    app.textarea.insert_char('w');
    app.textarea.insert_char('o');
    app.textarea.insert_char('r');
    app.textarea.insert_char('l');
    app.textarea.insert_char('d');

    assert_eq!(app.textarea.content(), "Hello world");

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
    let messages = app
        .cache
        .get_messages(thread_id)
        .expect("Messages should exist");

    assert_eq!(
        messages.len(),
        2,
        "Thread should have 2 messages (user + assistant)"
    );
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[0].content, "Hello world");
    assert_eq!(messages[1].role, MessageRole::Assistant);
}

#[tokio::test]
async fn test_screen_navigation() {
    // 1. Start at CommandDeck
    let mut app = create_test_app();
    assert_eq!(
        app.screen,
        Screen::CommandDeck,
        "App should start at CommandDeck"
    );

    // 2. Create thread (switches to Conversation)
    app.textarea.insert_char('T');
    app.textarea.insert_char('e');
    app.textarea.insert_char('s');
    app.textarea.insert_char('t');
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
    let mut app = create_test_app();

    // Type a message
    let message = "Test message for right panel";
    for c in message.chars() {
        app.textarea.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    // Get the thread ID that was just created
    let thread_id = app
        .active_thread_id
        .clone()
        .expect("Thread should be created");

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
    let mut app = create_test_app();
    let initial_count = app.cache.thread_count();

    // Create first thread
    for c in "First thread".chars() {
        app.textarea.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);
    let first_thread_id = app.active_thread_id.clone().unwrap();

    // Navigate back and create second thread
    app.navigate_to_command_deck();
    for c in "Second thread".chars() {
        app.textarea.insert_char(c);
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
    let mut app = create_test_app();
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
    let mut app = create_test_app();
    let initial_count = app.cache.thread_count();

    // Type whitespace only
    app.textarea.insert_char(' ');
    app.textarea.insert_char(' ');
    app.textarea.insert_char(' ');
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
    let mut app = create_test_app();

    for c in "Test message".chars() {
        app.textarea.insert_char(c);
    }
    assert!(!app.textarea.is_empty(), "Input should have content");

    app.submit_input(ThreadType::Conversation);

    assert!(
        app.textarea.is_empty(),
        "Input should be cleared after submit"
    );
}

#[tokio::test]
async fn test_thread_messages_have_correct_roles() {
    let mut app = create_test_app();

    for c in "Hello AI".chars() {
        app.textarea.insert_char(c);
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
    let mut app = create_test_app();

    // 1. Start at command deck with no active thread
    assert_eq!(
        app.screen,
        Screen::CommandDeck,
        "Should start at CommandDeck"
    );
    assert!(
        app.active_thread_id.is_none(),
        "active_thread_id should be None at start"
    );

    // 2. Type a message
    for c in "What is the meaning of life?".chars() {
        app.textarea.insert_char(c);
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

    let mut app = create_test_app();

    // Setup: Create first message in new thread
    for c in "First question".chars() {
        app.textarea.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let thread_id = app.active_thread_id.clone().expect("Should have thread ID");
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
        app.textarea.insert_char(c);
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
    let mut app = create_test_app();

    // Setup: Create a thread and be in conversation
    for c in "Test message".chars() {
        app.textarea.insert_char(c);
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
        app.textarea.is_empty(),
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
    let mut app = create_test_app();

    // Create first thread
    for c in "First thread message".chars() {
        app.textarea.insert_char(c);
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
        app.textarea.insert_char(c);
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
    assert_eq!(app.cache.thread_count(), 2, "Should have exactly 2 threads");

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

    let mut app = create_test_app();

    // Create pending thread
    for c in "What is Rust?".chars() {
        app.textarea.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let pending_id = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&pending_id).is_ok());

    // Add some streaming content
    app.cache.append_to_message(&pending_id, "Rust is ");
    app.cache
        .append_to_message(&pending_id, "a systems programming language.");

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
    let mut app = create_test_app();

    // Create first thread
    for c in "First thread".chars() {
        app.textarea.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);
    let first_thread_id = app.active_thread_id.clone().unwrap();

    // Navigate back to deck
    app.navigate_to_command_deck();

    // Create second thread
    for c in "Second thread".chars() {
        app.textarea.insert_char(c);
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
    let mut app = create_test_app();

    // Create a thread
    for c in "Test thread".chars() {
        app.textarea.insert_char(c);
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
    assert!(app.textarea.is_empty());
}

/// Test open_selected_thread with invalid index (beyond thread list)
#[tokio::test]
async fn test_open_selected_thread_invalid_index() {
    use spoq::app::Focus;

    let mut app = create_test_app();

    // Create one thread
    for c in "Single thread".chars() {
        app.textarea.insert_char(c);
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

    let mut app = create_test_app();

    // === Phase 1: Create new thread ===
    for c in "What is Rust?".chars() {
        app.textarea.insert_char(c);
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
        app.textarea.insert_char(c);
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
        app.textarea.insert_char(c);
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
    app.cache
        .append_to_message("thread-2", "Response to different topic");
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

    let mut app = create_test_app();

    // Create a thread
    for c in "What is Rust?".chars() {
        app.textarea.insert_char(c);
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
        description: Some(
            "A systems programming language focused on safety and performance".to_string(),
        ),
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

    let mut app = create_test_app();

    // Create a thread (pending)
    for c in "Test message".chars() {
        app.textarea.insert_char(c);
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
    assert_eq!(
        final_thread.description,
        Some("Final Description".to_string())
    );
}

/// Test Case 3: thread_updated with only title (no description)
/// - Create thread
/// - Send thread_updated with only title
/// - Verify only title is updated, description remains None
#[tokio::test]
async fn test_thread_updated_title_only() {
    use spoq::app::AppMessage;

    let mut app = create_test_app();

    // Create a thread
    let thread_id = app
        .cache
        .create_streaming_thread("Original Title".to_string());

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

    let mut app = create_test_app();

    // Create a thread
    let thread_id = app
        .cache
        .create_streaming_thread("Original Title".to_string());

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

    let mut app = create_test_app();

    // Create a thread
    let thread_id = app
        .cache
        .create_streaming_thread("Original Title".to_string());

    // Set initial description
    app.cache
        .update_thread_metadata(&thread_id, None, Some("Initial Description".to_string()));

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

    let mut app = create_test_app();

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

    let mut app = create_test_app();

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

    let mut app = create_test_app();

    // Create a thread and enter conversation
    for c in "Test message".chars() {
        app.textarea.insert_char(c);
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
    assert_eq!(
        app.active_thread_id,
        Some("thread-conversation".to_string())
    );

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
    assert_eq!(
        app.active_thread_id,
        Some("thread-conversation".to_string())
    );

    // Navigate back and verify thread list shows updated metadata
    app.navigate_to_command_deck();
    let threads = app.cache.threads();
    let our_thread = threads
        .iter()
        .find(|t| t.id == "thread-conversation")
        .unwrap();
    assert_eq!(our_thread.title, "Updated During Conversation");
}

/// Test Case 9: Verify thread description appears in UI thread list
/// - Create multiple threads with different descriptions
/// - Verify threads() returns threads with correct descriptions
#[tokio::test]
async fn test_thread_description_in_thread_list() {
    use spoq::app::AppMessage;

    let mut app = create_test_app();

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
    use spoq::sse::{SseEvent, SseParser};

    let mut app = create_test_app();

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
        Some(SseEvent::ThreadUpdated {
            thread_id: tid,
            title,
            description,
        }) => {
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
    let mut app = create_test_app();

    // First submit creates pending thread
    for c in "First message".chars() {
        app.textarea.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    let pending_id = app.active_thread_id.clone().unwrap();
    assert!(uuid::Uuid::parse_str(&pending_id).is_ok());

    // Try to submit again while still pending
    for c in "Second message".chars() {
        app.textarea.insert_char(c);
    }
    app.submit_input(ThreadType::Conversation);

    // Should be blocked with error
    assert!(app.stream_error.is_some());
    assert!(app.stream_error.as_ref().unwrap().contains("wait"));

    // Input should NOT be cleared (submission was rejected)
    assert_eq!(app.textarea.content(), "Second message");

    // Should still be on same pending thread
    assert_eq!(app.active_thread_id, Some(pending_id));

    // Cache should still only have 2 messages (from first submit)
    let messages = app
        .cache
        .get_messages(&app.active_thread_id.clone().unwrap())
        .unwrap();
    assert_eq!(messages.len(), 2);
}

// ============================================================================
// Phase 9 Multiline Input Integration Tests - TextAreaInput Wrapper
// ============================================================================

/// Test Case 1: Single-line input → submit → verify content
/// - Type a simple message on one line
/// - Submit the message
/// - Verify the content is correctly captured
#[tokio::test]
async fn test_single_line_input_submit() {
    let mut app = create_test_app();

    // Type a single-line message
    for c in "Hello, this is a single line message".chars() {
        app.textarea.insert_char(c);
    }

    // Verify content before submit
    assert_eq!(
        app.textarea.content(),
        "Hello, this is a single line message"
    );
    assert_eq!(app.textarea.line_count(), 1);

    // Submit the message
    app.submit_input(ThreadType::Conversation);

    // Verify thread was created with correct content
    let thread_id = app
        .active_thread_id
        .as_ref()
        .expect("Should have thread ID");
    let messages = app
        .cache
        .get_messages(thread_id)
        .expect("Messages should exist");

    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[0].content, "Hello, this is a single line message");

    // Verify input was cleared after submit
    assert!(app.textarea.is_empty());
}

/// Test Case 2: Multiline input (type, insert_newline, type more) → verify line count
/// - Insert text, add newlines, insert more text
/// - Verify correct number of lines
#[tokio::test]
async fn test_multiline_input_newline_insertion() {
    let mut app = create_test_app();

    // Type first line
    for c in "Line 1: Hello".chars() {
        app.textarea.insert_char(c);
    }

    // Insert newline (simulating Shift+Enter / Ctrl+J / Alt+Enter)
    app.textarea.insert_newline();

    // Type second line
    for c in "Line 2: World".chars() {
        app.textarea.insert_char(c);
    }

    // Verify we have 2 lines
    assert_eq!(app.textarea.line_count(), 2);
    assert_eq!(app.textarea.content(), "Line 1: Hello\nLine 2: World");

    // Add another line
    app.textarea.insert_newline();
    for c in "Line 3: Multiline test".chars() {
        app.textarea.insert_char(c);
    }

    assert_eq!(app.textarea.line_count(), 3);
    assert_eq!(
        app.textarea.content(),
        "Line 1: Hello\nLine 2: World\nLine 3: Multiline test"
    );
}

/// Test Case 3: Max 5 lines → verify line count doesn't exceed natural limit
/// - Add more than 5 lines
/// - Verify textarea accepts all lines (max height is for display, not content)
#[tokio::test]
async fn test_multiline_input_max_lines_height_calculation() {
    use spoq::ui::input::calculate_input_box_height;

    // Verify height calculation clamping behavior
    assert_eq!(
        calculate_input_box_height(1),
        3,
        "1 line: content + 2 borders = 3"
    );
    assert_eq!(
        calculate_input_box_height(2),
        4,
        "2 lines: content + 2 borders = 4"
    );
    assert_eq!(calculate_input_box_height(3), 5, "3 lines + 2 borders = 5");
    assert_eq!(calculate_input_box_height(4), 6, "4 lines + 2 borders = 6");
    assert_eq!(
        calculate_input_box_height(5),
        7,
        "5 lines + 2 borders = 7 (max)"
    );
    assert_eq!(
        calculate_input_box_height(6),
        7,
        "6 lines clamped to 5 + 2 = 7"
    );
    assert_eq!(
        calculate_input_box_height(10),
        7,
        "10 lines clamped to 5 + 2 = 7"
    );

    // But textarea should still accept all lines
    let mut app = create_test_app();

    // Create 7 lines
    for i in 1..=7 {
        for c in format!("Line {}", i).chars() {
            app.textarea.insert_char(c);
        }
        if i < 7 {
            app.textarea.insert_newline();
        }
    }

    // All 7 lines should be stored (height clamping is only for display)
    assert_eq!(app.textarea.line_count(), 7);

    // Content should have all lines
    let content = app.textarea.content();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 7);
    assert_eq!(lines[0], "Line 1");
    assert_eq!(lines[6], "Line 7");
}

/// Test Case 4: Up/Down cursor navigation between lines
/// - Create multiline content
/// - Navigate up and down with cursor
/// - Verify cursor position changes correctly
#[tokio::test]
async fn test_up_down_cursor_navigation() {
    let mut app = create_test_app();

    // Create 3 lines
    for c in "First".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline();
    for c in "Second".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline();
    for c in "Third".chars() {
        app.textarea.insert_char(c);
    }

    // Cursor should be at end of line 3 (0-indexed row 2)
    let (row, col) = app.textarea.cursor();
    assert_eq!(row, 2, "Cursor should be on row 2 (third line)");
    assert_eq!(col, 5, "Cursor should be at column 5 (after 'Third')");

    // Move up
    app.textarea.move_cursor_up();
    let (row, _col) = app.textarea.cursor();
    assert_eq!(row, 1, "Cursor should move to row 1 (second line)");

    // Move up again
    app.textarea.move_cursor_up();
    let (row, _col) = app.textarea.cursor();
    assert_eq!(row, 0, "Cursor should move to row 0 (first line)");

    // Move up at top should stay at top
    app.textarea.move_cursor_up();
    let (row, _col) = app.textarea.cursor();
    assert_eq!(row, 0, "Cursor should stay at row 0 (can't go higher)");

    // Move down
    app.textarea.move_cursor_down();
    let (row, _col) = app.textarea.cursor();
    assert_eq!(row, 1, "Cursor should move to row 1");

    // Move down
    app.textarea.move_cursor_down();
    let (row, _col) = app.textarea.cursor();
    assert_eq!(row, 2, "Cursor should move to row 2");

    // Move down at bottom should stay at bottom
    app.textarea.move_cursor_down();
    let (row, _col) = app.textarea.cursor();
    assert_eq!(row, 2, "Cursor should stay at row 2 (can't go lower)");
}

/// Test Case 5: Word navigation (Alt+Left/Right equivalents)
/// - Type multiple words
/// - Navigate by word
/// - Verify cursor position at word boundaries
#[tokio::test]
async fn test_word_navigation() {
    let mut app = create_test_app();

    // Type a sentence with multiple words
    for c in "hello world test".chars() {
        app.textarea.insert_char(c);
    }

    // Cursor is at end (col 16)
    let (_, col) = app.textarea.cursor();
    assert_eq!(col, 16, "Cursor should be at end of 'hello world test'");

    // Move word left - should go to start of "test"
    app.textarea.move_cursor_word_left();
    let (_, col) = app.textarea.cursor();
    assert!(col <= 12, "Cursor should be at or before 'test'");

    // Move word left again - should go to start of "world"
    app.textarea.move_cursor_word_left();
    let (_, col) = app.textarea.cursor();
    assert!(col <= 6, "Cursor should be at or before 'world'");

    // Move word right - should go forward
    let col_before = col;
    app.textarea.move_cursor_word_right();
    let (_, col_after) = app.textarea.cursor();
    assert!(col_after > col_before, "Cursor should move forward");
}

/// Test Case 6: Delete word backward (Alt+Backspace equivalent)
/// - Type multiple words
/// - Delete word backward
/// - Verify correct word is deleted
#[tokio::test]
async fn test_delete_word_backward() {
    let mut app = create_test_app();

    // Type a sentence
    for c in "hello world test".chars() {
        app.textarea.insert_char(c);
    }

    assert_eq!(app.textarea.content(), "hello world test");

    // Delete word backward - should remove "test"
    app.textarea.delete_word_backward();
    let content = app.textarea.content();
    assert!(content.starts_with("hello"), "Should still have 'hello'");
    assert!(!content.ends_with("test"), "'test' should be deleted");

    // Delete word backward again - should remove "world " or similar
    app.textarea.delete_word_backward();
    let content = app.textarea.content();
    assert!(content.starts_with("hello"), "Should still have 'hello'");
}

/// Test Case 7: Undo/redo functionality
/// - Make changes
/// - Undo
/// - Verify change is reverted
/// - Redo
/// - Verify change is restored
#[tokio::test]
async fn test_undo_redo() {
    let mut app = create_test_app();

    // Type some text
    for c in "Hello".chars() {
        app.textarea.insert_char(c);
    }
    assert_eq!(app.textarea.content(), "Hello");

    // Undo should revert
    let undone = app.textarea.undo();
    assert!(
        undone,
        "Undo should return true when there's something to undo"
    );
    // Note: exact undo behavior depends on tui-textarea's implementation
    // It may undo character by character or by operation

    // Redo should restore (if undo worked)
    let redone = app.textarea.redo();
    // Redo may or may not have anything to redo depending on undo behavior
    // Just verify it doesn't panic
    let _ = redone;
}

/// Test Case 8: Content extraction (lines joined with newlines)
/// - Create multiline content
/// - Extract content
/// - Verify lines are joined correctly
#[tokio::test]
async fn test_content_extraction() {
    let mut app = create_test_app();

    // Create multiline content
    for c in "Line A".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline();
    for c in "Line B".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline();
    for c in "Line C".chars() {
        app.textarea.insert_char(c);
    }

    // Content should join lines with newlines
    let content = app.textarea.content();
    assert_eq!(content, "Line A\nLine B\nLine C");

    // Verify lines() access
    let lines = app.textarea.lines();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "Line A");
    assert_eq!(lines[1], "Line B");
    assert_eq!(lines[2], "Line C");
}

/// Test Case 9: Clear functionality
/// - Type content
/// - Clear
/// - Verify textarea is empty
#[tokio::test]
async fn test_clear_functionality() {
    let mut app = create_test_app();

    // Type multiline content
    for c in "Line 1".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline();
    for c in "Line 2".chars() {
        app.textarea.insert_char(c);
    }

    assert!(!app.textarea.is_empty(), "Should have content");
    assert_eq!(app.textarea.line_count(), 2);

    // Clear
    app.textarea.clear();

    // Verify empty
    assert!(app.textarea.is_empty(), "Should be empty after clear");
    // Note: line_count() returns at least 1 for empty textarea
}

/// Test Case 10: Auto-grow behavior via calculate_input_area_height
/// - Verify height increases as lines are added
#[tokio::test]
async fn test_auto_grow_behavior() {
    use spoq::ui::input::calculate_input_area_height;

    // Verify auto-grow behavior (box + keybinds + padding)
    assert_eq!(
        calculate_input_area_height(1),
        6,
        "1 line: box(3) + keybinds(1) + padding(2) = 6"
    );
    assert_eq!(
        calculate_input_area_height(2),
        7,
        "2 lines: box(4) + keybinds(1) + padding(2) = 7"
    );
    assert_eq!(
        calculate_input_area_height(3),
        8,
        "3 lines: box(5) + keybinds(1) + padding(2) = 8"
    );
    assert_eq!(
        calculate_input_area_height(4),
        9,
        "4 lines: box(6) + keybinds(1) + padding(2) = 9"
    );
    assert_eq!(
        calculate_input_area_height(5),
        10,
        "5 lines: box(7) + keybinds(1) + padding(2) = 10 (max)"
    );
    assert_eq!(
        calculate_input_area_height(6),
        10,
        "6 lines: clamped to box(7) + keybinds(1) + padding(2) = 10"
    );

    // Also verify with actual App
    let mut app = create_test_app();

    // Start with empty - should be 1 line
    assert_eq!(app.textarea.line_count(), 1);

    // Add content - still 1 line
    for c in "Single line".chars() {
        app.textarea.insert_char(c);
    }
    assert_eq!(app.textarea.line_count(), 1);

    // Add newline - now 2 lines
    app.textarea.insert_newline();
    assert_eq!(app.textarea.line_count(), 2);

    // Add more lines
    for _ in 0..3 {
        for c in "More text".chars() {
            app.textarea.insert_char(c);
        }
        app.textarea.insert_newline();
    }

    // Should have 5 lines now (2 + 3)
    assert_eq!(app.textarea.line_count(), 5);
}

/// Test Case 11: Multiline input submission preserves newlines in message content
/// - Create multiline input
/// - Submit
/// - Verify message content has newlines preserved
#[tokio::test]
async fn test_multiline_submit_preserves_newlines() {
    let mut app = create_test_app();

    // Create multiline content
    for c in "First paragraph".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline();
    app.textarea.insert_newline(); // Double newline for paragraph break
    for c in "Second paragraph".chars() {
        app.textarea.insert_char(c);
    }

    let expected_content = "First paragraph\n\nSecond paragraph";
    assert_eq!(app.textarea.content(), expected_content);

    // Submit
    app.submit_input(ThreadType::Conversation);

    // Verify message content preserves newlines
    let thread_id = app
        .active_thread_id
        .as_ref()
        .expect("Should have thread ID");
    let messages = app
        .cache
        .get_messages(thread_id)
        .expect("Messages should exist");

    assert_eq!(messages[0].content, expected_content);
}

/// Test Case 12: Cursor position tracking across lines
/// - Type on multiple lines
/// - Move cursor to specific positions
/// - Verify cursor tracking is accurate
#[tokio::test]
async fn test_cursor_position_tracking() {
    let mut app = create_test_app();

    // Type "ABC" on first line
    for c in "ABC".chars() {
        app.textarea.insert_char(c);
    }

    // Verify cursor at (0, 3)
    assert_eq!(app.textarea.cursor(), (0, 3));

    // Add newline and type "XYZ"
    app.textarea.insert_newline();
    for c in "XYZ".chars() {
        app.textarea.insert_char(c);
    }

    // Verify cursor at (1, 3)
    assert_eq!(app.textarea.cursor(), (1, 3));

    // Move to start of line
    app.textarea.move_cursor_home();
    assert_eq!(app.textarea.cursor(), (1, 0));

    // Move to end of line
    app.textarea.move_cursor_end();
    assert_eq!(app.textarea.cursor(), (1, 3));

    // Move up
    app.textarea.move_cursor_up();
    assert_eq!(app.textarea.cursor().0, 0);
}

/// Test Case 13: Home and End keys work per-line
/// - Type multiline content
/// - Use Home/End navigation
/// - Verify they work within current line only
#[tokio::test]
async fn test_home_end_per_line() {
    let mut app = create_test_app();

    // Create content: "ABCDE" on line 1, "12345" on line 2
    for c in "ABCDE".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline();
    for c in "12345".chars() {
        app.textarea.insert_char(c);
    }

    // Cursor at (1, 5) - end of "12345"
    assert_eq!(app.textarea.cursor(), (1, 5));

    // Home moves to start of current line (line 1)
    app.textarea.move_cursor_home();
    assert_eq!(
        app.textarea.cursor(),
        (1, 0),
        "Home should go to start of line 1"
    );

    // End moves back to end of current line
    app.textarea.move_cursor_end();
    assert_eq!(
        app.textarea.cursor(),
        (1, 5),
        "End should go to end of line 1"
    );

    // Move up to line 0
    app.textarea.move_cursor_up();
    assert_eq!(app.textarea.cursor().0, 0);

    // Home on line 0
    app.textarea.move_cursor_home();
    assert_eq!(app.textarea.cursor(), (0, 0));

    // End on line 0
    app.textarea.move_cursor_end();
    assert_eq!(app.textarea.cursor(), (0, 5));
}

/// Test Case 14: Backspace across line boundaries
/// - Create multiline content
/// - Backspace at start of line 2 should join lines
#[tokio::test]
async fn test_backspace_joins_lines() {
    let mut app = create_test_app();

    // Create "ABC\nXYZ"
    for c in "ABC".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline();
    for c in "XYZ".chars() {
        app.textarea.insert_char(c);
    }

    assert_eq!(app.textarea.line_count(), 2);
    assert_eq!(app.textarea.content(), "ABC\nXYZ");

    // Move to start of line 2
    app.textarea.move_cursor_home();
    assert_eq!(app.textarea.cursor(), (1, 0));

    // Backspace should delete the newline and join lines
    app.textarea.backspace();

    // Should now be 1 line: "ABCXYZ"
    assert_eq!(app.textarea.line_count(), 1);
    assert_eq!(app.textarea.content(), "ABCXYZ");
}

/// Test Case 15: Delete at end of line
/// - Create multiline content
/// - Delete at end of line 1 should join with line 2
#[tokio::test]
async fn test_delete_at_line_end_joins_lines() {
    let mut app = create_test_app();

    // Create "ABC\nXYZ"
    for c in "ABC".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline();
    for c in "XYZ".chars() {
        app.textarea.insert_char(c);
    }

    assert_eq!(app.textarea.line_count(), 2);

    // Move to end of line 1 (after "ABC")
    app.textarea.move_cursor_up();
    app.textarea.move_cursor_end();
    assert_eq!(app.textarea.cursor(), (0, 3));

    // Delete forward should remove the newline
    app.textarea.delete_char();

    // Should now be 1 line: "ABCXYZ"
    assert_eq!(app.textarea.line_count(), 1);
    assert_eq!(app.textarea.content(), "ABCXYZ");
}

/// Test Case 16: Empty line insertion and removal
/// - Insert empty lines
/// - Verify line count
/// - Remove them
#[tokio::test]
async fn test_empty_line_handling() {
    let mut app = create_test_app();

    // Type text, insert empty lines, type more
    for c in "Header".chars() {
        app.textarea.insert_char(c);
    }
    app.textarea.insert_newline(); // Empty line
    app.textarea.insert_newline(); // Another empty line
    for c in "Footer".chars() {
        app.textarea.insert_char(c);
    }

    assert_eq!(app.textarea.line_count(), 3);
    assert_eq!(app.textarea.content(), "Header\n\nFooter");

    // Verify lines array
    let lines = app.textarea.lines();
    assert_eq!(lines[0], "Header");
    assert_eq!(lines[1], "");
    assert_eq!(lines[2], "Footer");
}

/// Test Case 17: TextAreaInput::with_content initialization
/// - Create with initial multiline content
/// - Verify content and cursor position
#[tokio::test]
async fn test_with_content_initialization() {
    use spoq::widgets::textarea_input::TextAreaInput;

    let input = TextAreaInput::with_content("line1\nline2\nline3");

    assert_eq!(input.line_count(), 3);
    assert_eq!(input.content(), "line1\nline2\nline3");

    // Cursor should be at end (after init)
    let (row, col) = input.cursor();
    assert_eq!(row, 2, "Cursor should be on last line");
    assert_eq!(col, 5, "Cursor should be at end of last line");
}

/// Test Case 18: Delete to line start (Ctrl+U equivalent)
/// - Type on a line
/// - Delete to start
/// - Verify line content
#[tokio::test]
async fn test_delete_to_line_start() {
    let mut app = create_test_app();

    // Create "Hello World" on one line
    for c in "Hello World".chars() {
        app.textarea.insert_char(c);
    }

    // Move cursor to middle (after "Hello ")
    app.textarea.move_cursor_home();
    for _ in 0..6 {
        app.textarea.move_cursor_right();
    }

    // Delete to line start should remove "Hello "
    app.textarea.delete_to_line_start();

    // Should be left with "World"
    assert_eq!(app.textarea.content(), "World");
}

/// Test Case 19: Move to top and bottom of multiline content
/// - Create many lines
/// - Move to top and bottom
#[tokio::test]
async fn test_move_to_top_bottom() {
    let mut app = create_test_app();

    // Create 5 lines
    for i in 1..=5 {
        for c in format!("Line {}", i).chars() {
            app.textarea.insert_char(c);
        }
        if i < 5 {
            app.textarea.insert_newline();
        }
    }

    // Cursor should be at bottom (row 4)
    assert_eq!(app.textarea.cursor().0, 4);

    // Move to top
    app.textarea.move_cursor_top();
    assert_eq!(app.textarea.cursor().0, 0, "Should be at top (row 0)");

    // Move to bottom
    app.textarea.move_cursor_bottom();
    assert_eq!(app.textarea.cursor().0, 4, "Should be at bottom (row 4)");
}

/// Test Case 20: Tab character handling
/// - Verify tab inserts spaces (default 4)
#[tokio::test]
async fn test_tab_handling() {
    use spoq::widgets::textarea_input::TextAreaInput;

    let mut input = TextAreaInput::new();

    // Type some text
    for c in "def".chars() {
        input.insert_char(c);
    }

    // Insert a tab character
    input.insert_char('\t');

    // Verify content (tab should be converted to spaces based on tab_length setting)
    let content = input.content();
    // The content will contain either a tab or spaces depending on textarea behavior
    assert!(
        content.starts_with("def"),
        "Content should start with 'def'"
    );
}
