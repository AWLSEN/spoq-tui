//! Integration tests for the Spoq TUI application
//!
//! These tests verify the full flow of thread creation, screen navigation,
//! and cache operations.

use spoq::app::{App, Screen};
use spoq::models::MessageRole;

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
    app.submit_input();

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
    app.submit_input();

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

    // active_thread_id should still be set (we didn't clear it)
    assert!(
        app.active_thread_id.is_some(),
        "active_thread_id should persist after navigation"
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
    app.submit_input();

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
    app.submit_input();
    let first_thread_id = app.active_thread_id.clone().unwrap();

    // Navigate back and create second thread
    app.navigate_to_command_deck();
    for c in "Second thread".chars() {
        app.input_box.insert_char(c);
    }
    app.submit_input();
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
    app.submit_input();

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
    app.submit_input();

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

    app.submit_input();

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
    app.submit_input();

    let thread_id = app.active_thread_id.as_ref().unwrap();
    let messages = app.cache.get_messages(thread_id).unwrap();

    // First message should be User
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[0].thread_id, *thread_id);

    // Second message should be Assistant (stub response)
    assert_eq!(messages[1].role, MessageRole::Assistant);
    assert_eq!(messages[1].thread_id, *thread_id);
}
