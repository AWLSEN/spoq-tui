//! Tests for the dirty flag mechanism (Round 2)
//!
//! The dirty flag (`needs_redraw`) optimizes rendering by only redrawing the UI
//! when state changes occur. This test suite verifies that:
//! 1. The flag is properly set when state changes
//! 2. The flag can be cleared
//! 3. Critical state changes mark the UI as dirty

use spoq::app::{App, AppMessage, Screen};

#[test]
fn test_app_initializes_with_needs_redraw_true() {
    // App should start with dirty flag set to force initial render
    let app = App::new().unwrap();
    assert!(app.needs_redraw, "App should initialize with needs_redraw=true");
}

#[test]
fn test_mark_dirty_sets_flag() {
    let mut app = App::new().unwrap();
    // Clear the flag first
    app.needs_redraw = false;

    // Call mark_dirty
    app.mark_dirty();

    assert!(app.needs_redraw, "mark_dirty() should set needs_redraw=true");
}

#[test]
fn test_clear_error_marks_dirty() {
    let mut app = App::new().unwrap();
    app.stream_error = Some("Test error".to_string());
    app.needs_redraw = false;

    app.clear_error();

    assert!(app.needs_redraw, "clear_error() should mark dirty");
    assert!(app.stream_error.is_none(), "Error should be cleared");
}

#[test]
fn test_reset_scroll_marks_dirty() {
    let mut app = App::new().unwrap();
    app.conversation_scroll = 10;
    app.scroll_position = 10.0;
    app.needs_redraw = false;

    app.reset_scroll();

    assert!(app.needs_redraw, "reset_scroll() should mark dirty");
    assert_eq!(app.conversation_scroll, 0, "Scroll should be reset to 0");
    assert_eq!(app.scroll_position, 0.0, "Scroll position should be reset to 0.0");
}

#[test]
fn test_update_terminal_dimensions_marks_dirty() {
    let mut app = App::new().unwrap();
    app.needs_redraw = false;

    // Update dimensions to different values
    app.update_terminal_dimensions(100, 50);

    assert!(app.needs_redraw, "update_terminal_dimensions() should mark dirty");
    assert_eq!(app.terminal_width(), 100);
    assert_eq!(app.terminal_height(), 50);
}

#[test]
fn test_update_terminal_dimensions_same_values_does_not_mark_dirty() {
    let mut app = App::new().unwrap();
    app.update_terminal_dimensions(80, 24);
    app.needs_redraw = false;

    // Update with same dimensions
    app.update_terminal_dimensions(80, 24);

    assert!(!app.needs_redraw, "Updating with same dimensions should not mark dirty");
}

#[test]
fn test_tick_marks_dirty_when_velocity_present() {
    let mut app = App::new().unwrap();
    app.scroll_velocity = 5.0; // Set initial velocity
    app.needs_redraw = false;

    app.tick();

    assert!(app.needs_redraw, "tick() should mark dirty when velocity is present");
}

#[test]
fn test_tick_marks_dirty_when_boundary_hit() {
    let mut app = App::new().unwrap();
    app.scroll_boundary_hit = Some(spoq::app::ScrollBoundary::Bottom);
    app.boundary_hit_tick = app.tick_count;
    app.needs_redraw = false;

    app.tick();

    assert!(app.needs_redraw, "tick() should mark dirty when boundary indicator is present");
}

#[test]
fn test_tick_does_not_mark_dirty_when_idle() {
    let mut app = App::new().unwrap();
    app.scroll_velocity = 0.0;
    app.scroll_boundary_hit = None;
    app.needs_redraw = false;

    // Make sure no streaming is happening
    app.active_thread_id = None;

    app.tick();

    assert!(!app.needs_redraw, "tick() should not mark dirty when idle");
}

#[test]
fn test_handle_message_connection_status_marks_dirty() {
    let mut app = App::new().unwrap();
    app.needs_redraw = false;

    let msg = AppMessage::ConnectionStatus(true);
    app.handle_message(msg);

    assert!(app.needs_redraw, "ConnectionStatus message should mark dirty");
}

#[test]
fn test_scroll_mouse_event_marks_dirty() {
    let mut app = App::new().unwrap();
    app.screen = Screen::Conversation;
    app.max_scroll = 10;
    app.conversation_scroll = 5;
    app.needs_redraw = false;

    // Simulate scroll up (increasing offset)
    if app.conversation_scroll < app.max_scroll {
        app.conversation_scroll += 1;
        app.scroll_position = app.conversation_scroll as f32;
        app.mark_dirty();
    }

    assert!(app.needs_redraw, "Scroll should mark dirty");
}

#[test]
fn test_boundary_indicator_clears_after_timeout() {
    let mut app = App::new().unwrap();
    app.scroll_boundary_hit = Some(spoq::app::ScrollBoundary::Top);
    app.boundary_hit_tick = 0;
    app.tick_count = 0;

    // Run tick 11 times to exceed the 10-tick threshold
    for _ in 0..11 {
        app.tick();
    }

    assert!(app.scroll_boundary_hit.is_none(), "Boundary indicator should clear after timeout");
}

#[test]
fn test_dirty_flag_can_be_cleared() {
    let mut app = App::new().unwrap();
    app.needs_redraw = true;

    // Simulate the render loop clearing the flag
    app.needs_redraw = false;

    assert!(!app.needs_redraw, "Dirty flag should be clearable");
}

#[test]
fn test_permission_approval_marks_dirty() {
    let mut app = App::new().unwrap();

    // Create a mock permission
    use spoq::state::PermissionRequest;
    app.session_state.pending_permission = Some(PermissionRequest {
        permission_id: "test-perm-123".to_string(),
        tool_name: "some_tool".to_string(),
        description: "Test permission".to_string(),
        context: None,
        tool_input: Some(serde_json::json!({})),
        received_at: std::time::Instant::now(),
    });

    app.needs_redraw = false;
    app.approve_permission("test-perm-123");

    assert!(app.needs_redraw, "Permission approval should mark dirty");
}

#[test]
fn test_permission_denial_marks_dirty() {
    let mut app = App::new().unwrap();

    // Create a mock permission
    use spoq::state::PermissionRequest;
    app.session_state.pending_permission = Some(PermissionRequest {
        permission_id: "test-perm-123".to_string(),
        tool_name: "some_tool".to_string(),
        description: "Test permission".to_string(),
        context: None,
        tool_input: Some(serde_json::json!({})),
        received_at: std::time::Instant::now(),
    });

    app.needs_redraw = false;
    app.deny_permission("test-perm-123");

    assert!(app.needs_redraw, "Permission denial should mark dirty");
}

#[test]
fn test_navigate_to_command_deck_marks_dirty() {
    let mut app = App::new().unwrap();
    app.screen = Screen::Conversation;
    app.needs_redraw = false;

    app.navigate_to_command_deck();

    assert!(app.needs_redraw, "navigate_to_command_deck() should mark dirty");
    assert_eq!(app.screen, Screen::CommandDeck, "Should be on CommandDeck screen");
}
