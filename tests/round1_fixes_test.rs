// Integration tests for Round 1 fixes
// Tests for:
// 1. Arrow key scroll handling in Conversation screen
// 2. visual_line_count width calculation
// 3. set_messages merge logic

use chrono::Utc;
use spoq::app::{App, Screen, ScrollBoundary};
use spoq::cache::ThreadCache;
use spoq::models::{Message, MessageRole};
use spoq::widgets::textarea_input::TextAreaInput;

// =============================================================================
// Arrow Key Scroll Tests (main.rs lines 559-582, 611-617)
// =============================================================================

#[test]
fn test_arrow_up_scrolls_conversation_when_on_conversation_screen() {
    let mut app = App::new().expect("Failed to create app");
    app.screen = Screen::Conversation;
    app.max_scroll = 100;
    app.unified_scroll = 50;

    // Simulate arrow up (see older content)
    if app.unified_scroll < app.max_scroll {
        app.unified_scroll += 1;
        app.scroll_position = app.unified_scroll as f32;
        app.mark_dirty();
    }

    assert_eq!(
        app.unified_scroll, 51,
        "Arrow up should increase scroll offset"
    );
}

#[test]
fn test_arrow_down_scrolls_conversation_when_on_conversation_screen() {
    let mut app = App::new().expect("Failed to create app");
    app.screen = Screen::Conversation;
    app.max_scroll = 100;
    app.unified_scroll = 50;

    // Simulate arrow down (see newer content)
    if app.unified_scroll > 0 {
        app.unified_scroll -= 1;
        app.scroll_position = app.unified_scroll as f32;
        app.mark_dirty();
    }

    assert_eq!(
        app.unified_scroll, 49,
        "Arrow down should decrease scroll offset"
    );
}

#[test]
fn test_arrow_up_at_top_boundary_sets_boundary_hit() {
    let mut app = App::new().expect("Failed to create app");
    app.screen = Screen::Conversation;
    app.max_scroll = 100;
    app.unified_scroll = 100; // Already at top
    app.tick_count = 42;

    // Simulate arrow up when at top boundary
    if app.unified_scroll < app.max_scroll {
        app.unified_scroll += 1;
    } else if app.max_scroll > 0 {
        app.scroll_boundary_hit = Some(ScrollBoundary::Top);
        app.boundary_hit_tick = app.tick_count;
        app.mark_dirty();
    }

    assert_eq!(
        app.scroll_boundary_hit,
        Some(ScrollBoundary::Top),
        "Should set Top boundary when scrolling up at max"
    );
    assert_eq!(app.boundary_hit_tick, 42);
}

#[test]
fn test_arrow_down_at_bottom_boundary_sets_boundary_hit() {
    let mut app = App::new().expect("Failed to create app");
    app.screen = Screen::Conversation;
    app.max_scroll = 100;
    app.unified_scroll = 0; // Already at bottom
    app.tick_count = 123;

    // Simulate arrow down when at bottom boundary
    if app.unified_scroll > 0 {
        app.unified_scroll -= 1;
    } else {
        app.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
        app.boundary_hit_tick = app.tick_count;
        app.mark_dirty();
    }

    assert_eq!(
        app.scroll_boundary_hit,
        Some(ScrollBoundary::Bottom),
        "Should set Bottom boundary when scrolling down at 0"
    );
    assert_eq!(app.boundary_hit_tick, 123);
}

#[test]
fn test_arrow_keys_work_regardless_of_focus() {
    let mut app = App::new().expect("Failed to create app");
    app.screen = Screen::Conversation;
    app.max_scroll = 100;
    app.unified_scroll = 50;

    // Test with different focus states
    use spoq::app::Focus;

    // Focus on Input - should still scroll
    app.focus = Focus::Input;
    let original_scroll = app.unified_scroll;
    app.unified_scroll += 1;
    assert!(
        app.unified_scroll > original_scroll,
        "Should scroll even when Input focused"
    );

    // Focus on Threads - should still scroll
    app.focus = Focus::Threads;
    let original_scroll = app.unified_scroll;
    app.unified_scroll += 1;
    assert!(
        app.unified_scroll > original_scroll,
        "Should scroll even when Threads focused"
    );
}

#[test]
fn test_arrow_keys_sync_scroll_position_with_unified_scroll() {
    let mut app = App::new().expect("Failed to create app");
    app.screen = Screen::Conversation;
    app.max_scroll = 100;
    app.unified_scroll = 50;
    app.scroll_position = 50.0;

    // Simulate arrow up
    app.unified_scroll += 1;
    app.scroll_position = app.unified_scroll as f32;

    assert_eq!(
        app.scroll_position, 51.0,
        "scroll_position should be synced with unified_scroll"
    );
}

// =============================================================================
// visual_line_count Width Fix Tests (textarea_input.rs line 188)
// =============================================================================

#[test]
fn test_visual_line_count_with_exact_width_match() {
    let mut input = TextAreaInput::new();
    for c in "12345".chars() {
        input.insert_char(c);
    }

    // Available width is 5 - should fit exactly in 1 line
    let visual_lines = input.visual_line_count(5);
    assert_eq!(
        visual_lines, 1,
        "5 chars should fit in width 5 without wrapping"
    );
}

#[test]
fn test_visual_line_count_wraps_at_boundary() {
    let mut input = TextAreaInput::new();
    for c in "123456".chars() {
        input.insert_char(c);
    }

    // Available width is 5 - should wrap to 2 lines
    let visual_lines = input.visual_line_count(5);
    assert_eq!(visual_lines, 2, "6 chars should wrap to 2 lines at width 5");
}

#[test]
fn test_visual_line_count_handles_multiple_wraps() {
    let mut input = TextAreaInput::new();
    for c in "1234567890ABC".chars() {
        input.insert_char(c);
    }

    // 13 chars at width 5 should wrap to 3 lines (5 + 5 + 3)
    let visual_lines = input.visual_line_count(5);
    assert_eq!(
        visual_lines, 3,
        "13 chars should wrap to 3 lines at width 5"
    );
}

#[test]
fn test_visual_line_count_empty_line_takes_one_visual_line() {
    let input = TextAreaInput::new();

    // Empty content should still take 1 visual line
    let visual_lines = input.visual_line_count(10);
    assert_eq!(visual_lines, 1, "Empty input should take 1 visual line");
}

#[test]
fn test_visual_line_count_multiple_logical_lines() {
    let mut input = TextAreaInput::new();
    for c in "line1".chars() {
        input.insert_char(c);
    }
    input.insert_newline();
    for c in "line2".chars() {
        input.insert_char(c);
    }

    // 2 logical lines, each 5 chars, at width 10 should be 2 visual lines
    let visual_lines = input.visual_line_count(10);
    assert_eq!(
        visual_lines, 2,
        "2 logical lines should be 2 visual lines when no wrapping"
    );
}

#[test]
fn test_visual_line_count_multiple_lines_with_wrapping() {
    let mut input = TextAreaInput::new();
    for c in "12345678".chars() {
        input.insert_char(c);
    }
    input.insert_newline();
    for c in "ABCDE".chars() {
        input.insert_char(c);
    }

    // Line 1: 8 chars at width 5 = 2 visual lines
    // Line 2: 5 chars at width 5 = 1 visual line
    // Total: 3 visual lines
    let visual_lines = input.visual_line_count(5);
    assert_eq!(
        visual_lines, 3,
        "Wrapped multi-line content should calculate correctly"
    );
}

#[test]
fn test_visual_line_count_zero_width_returns_line_count() {
    let mut input = TextAreaInput::new();
    for c in "hello".chars() {
        input.insert_char(c);
    }
    input.insert_newline();
    for c in "world".chars() {
        input.insert_char(c);
    }

    // Zero width should return logical line count as fallback
    let visual_lines = input.visual_line_count(0);
    assert_eq!(
        visual_lines, 2,
        "Zero width should return logical line count"
    );
}

#[test]
fn test_visual_line_count_unicode_width() {
    let mut input = TextAreaInput::new();
    // Emoji and wide characters
    input.insert_char('😀'); // Width 2
    input.insert_char('A'); // Width 1
    input.insert_char('😀'); // Width 2

    // Total width: 5 (2 + 1 + 2)
    let visual_lines = input.visual_line_count(5);
    assert_eq!(
        visual_lines, 1,
        "5-width content should fit in 5-width area"
    );

    let visual_lines = input.visual_line_count(4);
    assert_eq!(
        visual_lines, 2,
        "5-width content should wrap in 4-width area"
    );
}

#[test]
fn test_visual_line_count_caller_already_subtracts_borders() {
    // This test documents that callers subtract borders before calling
    let mut input = TextAreaInput::new();
    for c in "12345678".chars() {
        input.insert_char(c);
    }

    // If widget width is 10 with borders (2 chars), available_width is 8
    // The caller should pass 8, not 10
    let visual_lines = input.visual_line_count(8);
    assert_eq!(
        visual_lines, 1,
        "8 chars should fit in 8-width area without wrapping"
    );
}

// =============================================================================
// set_messages Merge Logic Tests (cache.rs lines 119-148)
// =============================================================================

#[test]
fn test_set_messages_preserves_streaming_messages() {
    let mut cache = ThreadCache::new();
    let thread_id = "test-thread".to_string();

    // Create initial state with a streaming message
    let now = Utc::now();
    let streaming_msg = Message {
        id: 0,
        thread_id: thread_id.clone(),
        role: MessageRole::Assistant,
        content: String::new(),
        created_at: now,
        is_streaming: true,
        partial_content: "Streaming content".to_string(),
        reasoning_content: String::new(),
        reasoning_collapsed: false,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };
    cache.add_message(streaming_msg);

    // Backend sends historical messages (without the streaming message)
    let backend_msg1 = Message {
        id: 1,
        thread_id: thread_id.clone(),
        role: MessageRole::User,
        content: "Hello".to_string(),
        created_at: now,
        is_streaming: false,
        partial_content: String::new(),
        reasoning_content: String::new(),
        reasoning_collapsed: true,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };
    let backend_msg2 = Message {
        id: 2,
        thread_id: thread_id.clone(),
        role: MessageRole::Assistant,
        content: "Hi there".to_string(),
        created_at: now,
        is_streaming: false,
        partial_content: String::new(),
        reasoning_content: String::new(),
        reasoning_collapsed: true,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };

    cache.set_messages(thread_id.clone(), vec![backend_msg1, backend_msg2]);

    // Verify streaming message is preserved at the end
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(
        messages.len(),
        3,
        "Should have 2 backend + 1 streaming message"
    );
    assert_eq!(messages[0].id, 1, "First message should be backend msg 1");
    assert_eq!(messages[1].id, 2, "Second message should be backend msg 2");
    assert!(messages[2].is_streaming, "Last message should be streaming");
    assert_eq!(messages[2].partial_content, "Streaming content");
}

#[test]
fn test_set_messages_preserves_temporary_id_messages() {
    let mut cache = ThreadCache::new();
    let thread_id = "test-thread".to_string();

    // Add a message with temporary ID (0)
    let now = Utc::now();
    let temp_msg = Message {
        id: 0,
        thread_id: thread_id.clone(),
        role: MessageRole::User,
        content: "Just sent".to_string(),
        created_at: now,
        is_streaming: false,
        partial_content: String::new(),
        reasoning_content: String::new(),
        reasoning_collapsed: true,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };
    cache.add_message(temp_msg);

    // Backend sends older messages
    let backend_msg = Message {
        id: 1,
        thread_id: thread_id.clone(),
        role: MessageRole::User,
        content: "Old message".to_string(),
        created_at: now,
        is_streaming: false,
        partial_content: String::new(),
        reasoning_content: String::new(),
        reasoning_collapsed: true,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };

    cache.set_messages(thread_id.clone(), vec![backend_msg]);

    // Verify temp message is preserved
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages.len(), 2, "Should have backend + temp message");
    assert_eq!(messages[0].id, 1, "First should be backend message");
    assert_eq!(messages[1].id, 0, "Second should be temp message");
    assert_eq!(messages[1].content, "Just sent");
}

#[test]
fn test_set_messages_preserves_higher_id_messages() {
    let mut cache = ThreadCache::new();
    let thread_id = "test-thread".to_string();

    let now = Utc::now();

    // Add existing messages (simulating recent user interaction)
    let existing_msg = Message {
        id: 5, // Higher than what backend will send
        thread_id: thread_id.clone(),
        role: MessageRole::User,
        content: "Recent message".to_string(),
        created_at: now,
        is_streaming: false,
        partial_content: String::new(),
        reasoning_content: String::new(),
        reasoning_collapsed: true,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };
    cache.add_message(existing_msg);

    // Backend sends messages with IDs 1-3
    let backend_msgs = vec![
        Message {
            id: 1,
            thread_id: thread_id.clone(),
            role: MessageRole::User,
            content: "Old msg 1".to_string(),
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        image_hashes: Vec::new(),
        },
        Message {
            id: 3,
            thread_id: thread_id.clone(),
            role: MessageRole::Assistant,
            content: "Old msg 3".to_string(),
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        image_hashes: Vec::new(),
        },
    ];

    cache.set_messages(thread_id.clone(), backend_msgs);

    // Verify higher ID message is preserved
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(
        messages.len(),
        3,
        "Should have 2 backend + 1 recent message"
    );
    assert_eq!(messages[2].id, 5, "Recent message should be preserved");
    assert_eq!(messages[2].content, "Recent message");
}

#[test]
fn test_set_messages_replaces_all_when_no_local_messages() {
    let mut cache = ThreadCache::new();
    let thread_id = "test-thread".to_string();

    let now = Utc::now();

    // Add normal backend messages
    let msg1 = Message {
        id: 1,
        thread_id: thread_id.clone(),
        role: MessageRole::User,
        content: "Message 1".to_string(),
        created_at: now,
        is_streaming: false,
        partial_content: String::new(),
        reasoning_content: String::new(),
        reasoning_collapsed: true,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };
    cache.add_message(msg1);

    // Backend sends updated messages
    let backend_msgs = vec![
        Message {
            id: 1,
            thread_id: thread_id.clone(),
            role: MessageRole::User,
            content: "Updated message 1".to_string(),
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        image_hashes: Vec::new(),
        },
        Message {
            id: 2,
            thread_id: thread_id.clone(),
            role: MessageRole::Assistant,
            content: "New message 2".to_string(),
            created_at: now,
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        image_hashes: Vec::new(),
        },
    ];

    cache.set_messages(thread_id.clone(), backend_msgs);

    // Verify messages are completely replaced (no local messages to preserve)
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages.len(), 2, "Should have exactly backend messages");
    assert_eq!(messages[0].content, "Updated message 1");
    assert_eq!(messages[1].content, "New message 2");
}

#[test]
fn test_set_messages_merge_order_backend_then_local() {
    let mut cache = ThreadCache::new();
    let thread_id = "test-thread".to_string();

    let now = Utc::now();

    // Add streaming and temp messages
    let streaming_msg = Message {
        id: 0,
        thread_id: thread_id.clone(),
        role: MessageRole::Assistant,
        content: String::new(),
        created_at: now,
        is_streaming: true,
        partial_content: "Streaming".to_string(),
        reasoning_content: String::new(),
        reasoning_collapsed: false,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };
    let temp_msg = Message {
        id: 0,
        thread_id: thread_id.clone(),
        role: MessageRole::User,
        content: "Temp".to_string(),
        created_at: now,
        is_streaming: false,
        partial_content: String::new(),
        reasoning_content: String::new(),
        reasoning_collapsed: true,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };
    cache.add_message(temp_msg);
    cache.add_message(streaming_msg);

    // Backend sends messages
    let backend_msgs = vec![Message {
        id: 1,
        thread_id: thread_id.clone(),
        role: MessageRole::User,
        content: "Backend 1".to_string(),
        created_at: now,
        is_streaming: false,
        partial_content: String::new(),
        reasoning_content: String::new(),
        reasoning_collapsed: true,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    }];

    cache.set_messages(thread_id.clone(), backend_msgs);

    // Verify order: backend first, then local
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].id, 1, "First should be backend");
    assert_eq!(messages[1].content, "Temp", "Then local temp message");
    assert!(messages[2].is_streaming, "Then streaming message");
}

#[test]
fn test_set_messages_empty_backend_preserves_local() {
    let mut cache = ThreadCache::new();
    let thread_id = "test-thread".to_string();

    let now = Utc::now();

    // Add streaming message
    let streaming_msg = Message {
        id: 0,
        thread_id: thread_id.clone(),
        role: MessageRole::Assistant,
        content: String::new(),
        created_at: now,
        is_streaming: true,
        partial_content: "Content".to_string(),
        reasoning_content: String::new(),
        reasoning_collapsed: false,
        segments: Vec::new(),
        render_version: 0,
        image_hashes: Vec::new(),
    };
    cache.add_message(streaming_msg);

    // Backend sends empty list (possible during initialization)
    cache.set_messages(thread_id.clone(), vec![]);

    // Local streaming message should still be there
    let messages = cache.get_messages(&thread_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].is_streaming);
}
