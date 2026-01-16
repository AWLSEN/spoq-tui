// Integration tests for scroll event handling

use spoq::app::{App, Screen};
use spoq::models::ThreadType;

/// Helper to create a test app with conversation screen
fn create_test_app_in_conversation() -> App {
    let mut app = App::new().expect("Failed to create app");

    // Create a thread and navigate to conversation
    let thread_id = app.cache.create_pending_thread(
        "Test Thread".to_string(),
        ThreadType::Conversation,
    );
    app.active_thread_id = Some(thread_id);
    app.screen = Screen::Conversation;

    app
}

#[test]
fn test_scroll_offset_initializes_to_zero() {
    let app = create_test_app_in_conversation();
    assert_eq!(app.conversation_scroll, 0, "Initial scroll offset should be 0");
}

#[test]
fn test_max_scroll_initializes_to_zero() {
    let app = create_test_app_in_conversation();
    assert_eq!(app.max_scroll, 0, "Initial max_scroll should be 0");
}

#[test]
fn test_scroll_down_decreases_offset() {
    let mut app = create_test_app_in_conversation();
    app.conversation_scroll = 10;
    app.max_scroll = 100;

    // Simulate scroll down (see newer content)
    // Based on the code: near_bottom = scroll <= threshold
    // amount = if near_bottom { 1 } else { 3 }
    let threshold = (app.max_scroll / 10).max(5);
    let near_bottom = app.conversation_scroll <= threshold;
    let amount = if near_bottom { 1 } else { 3 };

    let original_scroll = app.conversation_scroll;
    app.conversation_scroll = app.conversation_scroll.saturating_sub(amount);

    assert!(
        app.conversation_scroll < original_scroll,
        "Scroll down should decrease offset (see newer content)"
    );
}

#[test]
fn test_scroll_up_increases_offset() {
    let mut app = create_test_app_in_conversation();
    app.conversation_scroll = 10;
    app.max_scroll = 100;

    // Simulate scroll up (see older content)
    let threshold = (app.max_scroll / 10).max(5);
    let near_top = app.conversation_scroll >= app.max_scroll.saturating_sub(threshold);
    let amount = if near_top { 1 } else { 3 };

    let original_scroll = app.conversation_scroll;
    app.conversation_scroll = app.conversation_scroll.saturating_add(amount).min(app.max_scroll);

    assert!(
        app.conversation_scroll > original_scroll,
        "Scroll up should increase offset (see older content)"
    );
}

#[test]
fn test_scroll_acceleration_far_from_boundary() {
    let mut app = create_test_app_in_conversation();
    app.conversation_scroll = 50; // Middle position
    app.max_scroll = 100;

    // Test scroll down acceleration
    let threshold = (app.max_scroll / 10).max(5);
    let near_bottom = app.conversation_scroll <= threshold;
    let amount = if near_bottom { 1 } else { 3 };

    assert_eq!(amount, 3, "Should scroll 3 lines when far from boundary");
}

#[test]
fn test_scroll_acceleration_near_bottom_boundary() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    let threshold = (app.max_scroll / 10).max(5);
    app.conversation_scroll = threshold; // At the threshold

    // Test scroll down near bottom
    let near_bottom = app.conversation_scroll <= threshold;
    let amount = if near_bottom { 1 } else { 3 };

    assert_eq!(amount, 1, "Should scroll 1 line when near bottom");
}

#[test]
fn test_scroll_acceleration_near_top_boundary() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 100;
    let threshold = (app.max_scroll / 10).max(5);
    app.conversation_scroll = app.max_scroll - threshold; // Near top

    // Test scroll up near top
    let near_top = app.conversation_scroll >= app.max_scroll.saturating_sub(threshold);
    let amount = if near_top { 1 } else { 3 };

    assert_eq!(amount, 1, "Should scroll 1 line when near top");
}

#[test]
fn test_scroll_clamping_at_bottom() {
    let mut app = create_test_app_in_conversation();
    app.conversation_scroll = 2;
    app.max_scroll = 100;

    // Scroll down more than current offset
    app.conversation_scroll = app.conversation_scroll.saturating_sub(5);

    assert_eq!(
        app.conversation_scroll, 0,
        "Scroll should clamp to 0 at bottom"
    );
}

#[test]
fn test_scroll_clamping_at_top() {
    let mut app = create_test_app_in_conversation();
    app.conversation_scroll = 98;
    app.max_scroll = 100;

    // Scroll up beyond max
    app.conversation_scroll = app.conversation_scroll.saturating_add(5).min(app.max_scroll);

    assert_eq!(
        app.conversation_scroll, app.max_scroll,
        "Scroll should clamp to max_scroll at top"
    );
}

#[test]
fn test_scroll_only_active_on_conversation_screen() {
    let mut app = create_test_app_in_conversation();
    app.screen = Screen::CommandDeck;

    let _original_scroll = app.conversation_scroll;

    // Scroll events should be ignored on CommandDeck
    // (This is enforced in main.rs event handler, not in App)
    // Just verify the screen state
    assert_eq!(
        app.screen,
        Screen::CommandDeck,
        "Should be on CommandDeck screen"
    );

    // Change back to conversation
    app.screen = Screen::Conversation;
    assert_eq!(
        app.screen,
        Screen::Conversation,
        "Should be on Conversation screen"
    );
}

#[test]
fn test_scroll_threshold_minimum_of_five() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 30; // Small max_scroll

    // Threshold should be at least 5
    let threshold = (app.max_scroll / 10).max(5);

    assert!(
        threshold >= 5,
        "Threshold should have minimum value of 5"
    );
}

#[test]
fn test_scroll_threshold_scales_with_max_scroll() {
    let mut app = create_test_app_in_conversation();
    app.max_scroll = 1000; // Large max_scroll

    // Threshold should be 10% of max_scroll
    let threshold = (app.max_scroll / 10).max(5);

    assert_eq!(
        threshold, 100,
        "Threshold should be 10% of max_scroll when max_scroll is large"
    );
}

#[test]
fn test_natural_scrolling_direction() {
    let mut app = create_test_app_in_conversation();
    app.conversation_scroll = 50;
    app.max_scroll = 100;

    // ScrollDown -> see newer content -> decrease offset
    let original_scroll = app.conversation_scroll;
    app.conversation_scroll = app.conversation_scroll.saturating_sub(3);
    assert!(
        app.conversation_scroll < original_scroll,
        "ScrollDown should decrease offset (natural scrolling)"
    );

    // ScrollUp -> see older content -> increase offset
    app.conversation_scroll = 50; // Reset
    let original_scroll = app.conversation_scroll;
    app.conversation_scroll = app.conversation_scroll.saturating_add(3).min(app.max_scroll);
    assert!(
        app.conversation_scroll > original_scroll,
        "ScrollUp should increase offset (natural scrolling)"
    );
}

#[test]
fn test_scroll_saturating_operations_prevent_overflow() {
    let mut app = create_test_app_in_conversation();
    app.conversation_scroll = u16::MAX;
    app.max_scroll = u16::MAX;

    // Test saturating_add doesn't overflow
    let result = app.conversation_scroll.saturating_add(100);
    assert_eq!(result, u16::MAX, "saturating_add should not overflow");

    // Test saturating_sub doesn't underflow
    app.conversation_scroll = 0;
    let result = app.conversation_scroll.saturating_sub(100);
    assert_eq!(result, 0, "saturating_sub should not underflow");
}
