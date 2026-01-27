//! Tests for Round 1 of plan-20260125-1845
//!
//! OBSOLETE: These tests reference session_state.pending_permission which was removed
//! in plan-20260127-1645 Round 3 (migration to per-thread permissions).
//!
//! Tests the following changes:
//! 1. 'A' key routing based on top thread type (UserInput vs Permission)
//! 2. The new `open_ask_user_question_dialog()` method in app/permissions.rs

use spoq::app::App;
use spoq::models::dashboard::{WaitingFor, ThreadStatus};
use spoq::models::{Thread, ThreadMode, ThreadType};
use spoq::state::session::{AskUserQuestionData, Question, QuestionOption};
use spoq::state::PermissionRequest;
use std::collections::HashMap;
use std::time::Instant;
use chrono::Utc;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_app() -> App {
    let mut app = App::default();
    let (tx, _rx) = tokio::sync::mpsc::channel(10);
    app.ws_sender = Some(tx);
    app.ws_connection_state = spoq::websocket::WsConnectionState::Connected;
    app
}

fn create_test_thread(id: &str, title: &str) -> Thread {
    Thread {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        preview: String::new(),
        updated_at: Utc::now(),
        thread_type: ThreadType::Conversation,
        mode: ThreadMode::Normal,
        model: None,
        permission_mode: None,
        message_count: 0,
        created_at: Utc::now(),
        working_directory: None,
        status: None,
        verified: None,
        verified_at: None,
    }
}

fn create_bash_permission(permission_id: &str) -> PermissionRequest {
    PermissionRequest {
        permission_id: permission_id.to_string(),
        tool_name: "Bash".to_string(),
        description: "Run a command".to_string(),
        context: Some("ls -la".to_string()),
        tool_input: Some(serde_json::json!({"command": "ls -la"})),
        received_at: Instant::now(),
    }
}

fn make_test_question() -> AskUserQuestionData {
    AskUserQuestionData {
        questions: vec![Question {
            question: "Test question?".to_string(),
            header: "Test".to_string(),
            options: vec![QuestionOption {
                label: "Yes".to_string(),
                description: "Confirm".to_string(),
            }],
            multi_select: false,
        }],
        answers: HashMap::new(),
    }
}

fn setup_user_input_thread(app: &mut App, thread_id: &str, request_id: &str) {
    app.dashboard.set_pending_question(thread_id, request_id.to_string(), make_test_question());
    app.dashboard.update_thread_status(thread_id, ThreadStatus::Waiting, Some(WaitingFor::UserInput));
    app.dashboard.compute_thread_views();
}

fn setup_permission_thread(app: &mut App, thread_id: &str, perm_id: &str, tool_name: &str) {
    app.dashboard.update_thread_status(
        thread_id,
        ThreadStatus::Waiting,
        Some(WaitingFor::Permission {
            request_id: perm_id.to_string(),
            tool_name: tool_name.to_string(),
        }),
    );
    app.dashboard.compute_thread_views();
}

// ============================================================================
// Tests: open_ask_user_question_dialog()
// ============================================================================

#[test]
fn test_open_ask_user_question_dialog_opens_first_user_input_thread() {
    let mut app = create_test_app();

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    app.dashboard.add_thread(create_test_thread("t2", "Thread 2"));
    app.dashboard.add_thread(create_test_thread("t3", "Thread 3"));

    setup_user_input_thread(&mut app, "t2", "req-test");

    assert!(app.dashboard.overlay().is_none());

    let result = app.open_ask_user_question_dialog();

    assert!(result, "Should return true when dialog is opened");
    assert!(app.dashboard.overlay().is_some(), "Overlay should be open");
}

#[test]
fn test_open_ask_user_question_dialog_does_not_open_when_overlay_exists() {
    let mut app = create_test_app();

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    setup_user_input_thread(&mut app, "t1", "req-1");

    app.dashboard.expand_thread("t1", 10);
    assert!(app.dashboard.overlay().is_some());

    app.dashboard.add_thread(create_test_thread("t2", "Thread 2"));
    setup_user_input_thread(&mut app, "t2", "req-2");

    let result = app.open_ask_user_question_dialog();

    assert!(!result, "Should return false when overlay already exists");
}

#[test]
fn test_open_ask_user_question_dialog_no_user_input_threads() {
    let mut app = create_test_app();

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    setup_permission_thread(&mut app, "t1", "perm-test", "Bash");

    let result = app.open_ask_user_question_dialog();

    assert!(!result, "Should return false when no user input threads exist");
    assert!(app.dashboard.overlay().is_none());
}

#[test]
fn test_open_ask_user_question_dialog_finds_first_user_input() {
    let mut app = create_test_app();

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    app.dashboard.add_thread(create_test_thread("t2", "Thread 2"));
    app.dashboard.add_thread(create_test_thread("t3", "Thread 3"));

    setup_permission_thread(&mut app, "t1", "perm-1", "Bash");
    setup_user_input_thread(&mut app, "t2", "req-2");
    setup_user_input_thread(&mut app, "t3", "req-3");

    let result = app.open_ask_user_question_dialog();

    assert!(result);
    assert!(app.dashboard.overlay().is_some());
}

// ============================================================================
// Tests: 'A' Key Routing Based on Top Thread Type
// ============================================================================

#[test]
fn test_handle_permission_key_a_ignores_when_top_thread_is_user_input() {
    let mut app = create_test_app();

    app.session_state.set_pending_permission(create_bash_permission("perm-bash"));

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    setup_user_input_thread(&mut app, "t1", "req-1");

    if let Some((_, wf)) = app.dashboard.get_top_needs_action_thread() {
        assert!(matches!(wf, WaitingFor::UserInput));
    } else {
        panic!("Should have top thread");
    }

    let handled = app.handle_permission_key('a');

    assert!(!handled, "'A' key should be ignored when top thread is UserInput");
    assert!(app.session_state.pending_permission.is_some());
}

#[test]
fn test_handle_permission_key_a_allows_when_top_thread_is_permission() {
    let mut app = create_test_app();

    app.session_state.set_pending_permission(create_bash_permission("perm-bash"));

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    setup_permission_thread(&mut app, "t1", "perm-bash", "Bash");

    if let Some((_, wf)) = app.dashboard.get_top_needs_action_thread() {
        assert!(matches!(wf, WaitingFor::Permission { .. }));
    } else {
        panic!("Should have top thread");
    }

    let handled = app.handle_permission_key('a');

    assert!(handled, "'A' key should be handled when top thread is Permission");
    assert!(app.session_state.pending_permission.is_none());
    assert!(app.session_state.allowed_tools.contains("Bash"));
}

#[test]
fn test_handle_permission_key_a_allows_when_no_top_thread() {
    let mut app = create_test_app();

    app.session_state.set_pending_permission(create_bash_permission("perm-bash"));

    assert!(app.dashboard.get_top_needs_action_thread().is_none());

    let handled = app.handle_permission_key('a');

    assert!(handled, "'A' key should be handled when no top thread exists");
    assert!(app.session_state.pending_permission.is_none());
    assert!(app.session_state.allowed_tools.contains("Bash"));
}

#[test]
fn test_handle_permission_key_y_ignores_when_top_thread_is_user_input() {
    let mut app = create_test_app();

    app.session_state.set_pending_permission(create_bash_permission("perm-bash"));

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    setup_user_input_thread(&mut app, "t1", "req-1");

    let handled = app.handle_permission_key('y');

    assert!(!handled, "'Y' key should be ignored when top thread is UserInput");
    assert!(app.session_state.pending_permission.is_some());
}

#[test]
fn test_handle_permission_key_n_ignores_when_top_thread_is_user_input() {
    let mut app = create_test_app();

    app.session_state.set_pending_permission(create_bash_permission("perm-bash"));

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    setup_user_input_thread(&mut app, "t1", "req-1");

    let handled = app.handle_permission_key('n');

    assert!(!handled, "'N' key should be ignored when top thread is UserInput");
    assert!(app.session_state.pending_permission.is_some());
}

#[test]
fn test_handle_permission_key_y_works_when_top_thread_is_permission() {
    let mut app = create_test_app();

    app.session_state.set_pending_permission(create_bash_permission("perm-bash"));

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    setup_permission_thread(&mut app, "t1", "perm-bash", "Bash");

    let handled = app.handle_permission_key('y');

    assert!(handled, "'Y' key should be handled when top thread is Permission");
    assert!(app.session_state.pending_permission.is_none());
}

#[test]
fn test_handle_permission_key_uppercase_variants() {
    let mut app = create_test_app();

    app.session_state.set_pending_permission(create_bash_permission("perm-1"));

    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));
    setup_user_input_thread(&mut app, "t1", "req-1");

    let handled = app.handle_permission_key('A');
    assert!(!handled);
    assert!(app.session_state.pending_permission.is_some());
}

#[test]
fn test_full_flow_a_key_with_mixed_threads() {
    let mut app = create_test_app();

    app.session_state.set_pending_permission(create_bash_permission("perm-bash"));

    // Add one thread that needs user input - this will be the top needs-action thread
    app.dashboard.add_thread(create_test_thread("t1", "User Input Thread"));
    setup_user_input_thread(&mut app, "t1", "req-1");

    // Verify it's a UserInput thread
    if let Some((thread_id, wf)) = app.dashboard.get_top_needs_action_thread() {
        assert_eq!(thread_id, "t1");
        assert!(matches!(wf, WaitingFor::UserInput));

        // Press 'A' - should be ignored because top thread is UserInput
        let handled = app.handle_permission_key('a');
        assert!(!handled, "'A' should be ignored when top thread is UserInput");
        assert!(app.session_state.pending_permission.is_some(), "Permission should still be pending");
    } else {
        panic!("Should have a top needs-action thread");
    }
}
