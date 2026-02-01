//! Tests for plan approval input handling fix
//!
//! Verifies that:
//! 1. handle_plan_approval_key_for_thread works with explicit thread_id
//! 2. submit_plan_feedback_for_thread works with explicit thread_id
//! 3. Existing delegation methods still work correctly
//! 4. Plan approval works from CommandDeck (no active_thread_id needed)

use spoq::app::App;
use spoq::models::dashboard::{PlanRequest, PlanSummary, ThreadStatus, WaitingFor};
use spoq::models::{Thread, ThreadMode, ThreadType};
use chrono::Utc;

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a test App with a properly configured WebSocket channel.
/// Returns both the App and the receiver - the receiver MUST be kept alive
/// for the duration of the test, otherwise try_send will fail with channel closed.
fn create_test_app_with_receiver() -> (App, tokio::sync::mpsc::Receiver<spoq::websocket::WsOutgoingMessage>) {
    let mut app = App::default();
    let (tx, rx) = tokio::sync::mpsc::channel(10);
    app.ws_sender = Some(tx);
    app.ws_connection_state = spoq::websocket::WsConnectionState::Connected;
    (app, rx)
}

/// Creates a test App without WebSocket (for tests that don't need send to succeed).
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

fn create_plan_summary() -> PlanSummary {
    PlanSummary::new(
        "Test Plan".to_string(),
        vec!["Phase 1: Setup".to_string(), "Phase 2: Implement".to_string()],
        5,
        Some(10000),
    )
}

fn setup_plan_approval_thread(app: &mut App, thread_id: &str, request_id: &str) {
    app.dashboard.add_thread(create_test_thread(thread_id, "Plan Thread"));
    app.dashboard.update_thread_status(
        thread_id,
        ThreadStatus::Waiting,
        Some(WaitingFor::PlanApproval {
            request_id: request_id.to_string(),
        }),
    );
    app.dashboard.set_plan_request(
        thread_id,
        PlanRequest::new(request_id.to_string(), create_plan_summary()),
    );
    app.dashboard.compute_thread_views();
}

fn setup_plan_approval_from_permission(app: &mut App, thread_id: &str, request_id: &str) {
    app.dashboard.add_thread(create_test_thread(thread_id, "Plan Thread"));
    app.dashboard.update_thread_status(
        thread_id,
        ThreadStatus::Waiting,
        Some(WaitingFor::PlanApproval {
            request_id: request_id.to_string(),
        }),
    );
    app.dashboard.set_plan_request(
        thread_id,
        PlanRequest::from_permission(request_id.to_string(), create_plan_summary()),
    );
    app.dashboard.compute_thread_views();
}

// ============================================================================
// Tests: handle_plan_approval_key_for_thread
// ============================================================================

#[test]
fn test_handle_plan_approval_key_for_thread_approve() {
    let (mut app, _rx) = create_test_app_with_receiver();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    // Verify plan request exists before approval
    assert!(app.dashboard.get_plan_request_id("t1").is_some(), "Plan request should exist after setup");
    assert!(app.ws_sender.is_some(), "WS sender should be set");
    assert_eq!(app.ws_connection_state, spoq::websocket::WsConnectionState::Connected, "WS should be connected");
    assert!(!app.dashboard.is_plan_from_permission("t1"), "Should not be from permission");

    let handled = app.handle_plan_approval_key_for_thread('y', "t1");

    assert!(handled, "'y' should be handled for plan approval");
    // Plan request should be removed after approval
    assert!(app.dashboard.get_plan_request_id("t1").is_none());
}

#[test]
fn test_handle_plan_approval_key_for_thread_reject() {
    let (mut app, _rx) = create_test_app_with_receiver();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    assert!(app.dashboard.get_plan_request_id("t1").is_some());

    let handled = app.handle_plan_approval_key_for_thread('n', "t1");

    assert!(handled, "'n' should be handled for plan rejection");
    assert!(app.dashboard.get_plan_request_id("t1").is_none());
}

#[test]
fn test_handle_plan_approval_key_for_thread_uppercase() {
    let (mut app, _rx) = create_test_app_with_receiver();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    let handled = app.handle_plan_approval_key_for_thread('Y', "t1");

    assert!(handled, "'Y' (uppercase) should be handled for plan approval");
    assert!(app.dashboard.get_plan_request_id("t1").is_none());
}

#[test]
fn test_handle_plan_approval_key_for_thread_unknown_key() {
    let mut app = create_test_app();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    let handled = app.handle_plan_approval_key_for_thread('x', "t1");

    assert!(!handled, "Unknown key 'x' should not be handled");
    // Plan request should still exist
    assert!(app.dashboard.get_plan_request_id("t1").is_some());
}

#[test]
fn test_handle_plan_approval_key_for_thread_no_plan() {
    let mut app = create_test_app();
    app.dashboard.add_thread(create_test_thread("t1", "Thread 1"));

    // No plan request set up - just a thread with no pending plan
    let handled = app.handle_plan_approval_key_for_thread('y', "t1");

    assert!(!handled, "Should return false when no plan request exists");
}

#[test]
fn test_handle_plan_approval_key_for_thread_nonexistent_thread() {
    let mut app = create_test_app();

    let handled = app.handle_plan_approval_key_for_thread('y', "nonexistent");

    assert!(!handled, "Should return false for nonexistent thread");
}

#[test]
fn test_handle_plan_approval_key_for_thread_from_permission_approve() {
    let (mut app, _rx) = create_test_app_with_receiver();
    setup_plan_approval_from_permission(&mut app, "t1", "perm-plan-1");

    let handled = app.handle_plan_approval_key_for_thread('y', "t1");

    assert!(handled, "'y' should be handled for permission-based plan approval");
    assert!(app.dashboard.get_plan_request_id("t1").is_none());
}

#[test]
fn test_handle_plan_approval_key_for_thread_from_permission_reject() {
    let (mut app, _rx) = create_test_app_with_receiver();
    setup_plan_approval_from_permission(&mut app, "t1", "perm-plan-1");

    let handled = app.handle_plan_approval_key_for_thread('n', "t1");

    assert!(handled, "'n' should be handled for permission-based plan rejection");
    assert!(app.dashboard.get_plan_request_id("t1").is_none());
}

// ============================================================================
// Tests: handle_plan_approval_key delegation
// ============================================================================

#[test]
fn test_handle_plan_approval_key_delegates_with_active_thread() {
    let (mut app, _rx) = create_test_app_with_receiver();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    // Set active_thread_id (as if on Conversation screen)
    app.active_thread_id = Some("t1".to_string());

    let handled = app.handle_permission_key('y');

    // handle_permission_key -> no pending permission -> handle_plan_approval_key -> delegates
    assert!(handled, "'y' should be handled via delegation chain");
    assert!(app.dashboard.get_plan_request_id("t1").is_none());
}

#[test]
fn test_handle_plan_approval_key_fails_without_active_thread() {
    let mut app = create_test_app();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    // No active_thread_id set (CommandDeck scenario)
    app.active_thread_id = None;

    // handle_permission_key -> no pending permission -> handle_plan_approval_key
    // -> active_thread_id is None -> returns false
    let handled = app.handle_permission_key('y');

    assert!(!handled, "'y' should NOT be handled without active_thread_id (this is the bug we fixed)");
    // Plan request should still exist (not handled)
    assert!(app.dashboard.get_plan_request_id("t1").is_some());
}

// ============================================================================
// Tests: submit_plan_feedback_for_thread
// ============================================================================

#[test]
fn test_submit_plan_feedback_for_thread() {
    let (mut app, _rx) = create_test_app_with_receiver();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    // Set up feedback state
    if let Some(state) = app.dashboard.get_plan_approval_state_mut("t1") {
        state.feedback_active = true;
        state.feedback_text = "Please add more tests".to_string();
    }

    app.submit_plan_feedback_for_thread("t1");

    // After feedback submission, plan request should be removed
    assert!(app.dashboard.get_plan_request_id("t1").is_none());
    // Feedback state should be cleared
    if let Some(state) = app.dashboard.get_plan_approval_state("t1") {
        assert!(!state.feedback_active);
        assert!(state.feedback_text.is_empty());
    }
}

#[test]
fn test_submit_plan_feedback_for_thread_empty_feedback() {
    let mut app = create_test_app();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    // Set up empty feedback
    if let Some(state) = app.dashboard.get_plan_approval_state_mut("t1") {
        state.feedback_active = true;
        state.feedback_text = String::new();
    }

    app.submit_plan_feedback_for_thread("t1");

    // Empty feedback should not submit -- plan request should still exist
    assert!(app.dashboard.get_plan_request_id("t1").is_some());
}

#[test]
fn test_submit_plan_feedback_delegates_with_active_thread() {
    let (mut app, _rx) = create_test_app_with_receiver();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");
    app.active_thread_id = Some("t1".to_string());

    if let Some(state) = app.dashboard.get_plan_approval_state_mut("t1") {
        state.feedback_active = true;
        state.feedback_text = "Needs improvement".to_string();
    }

    app.submit_plan_feedback();

    assert!(app.dashboard.get_plan_request_id("t1").is_none());
}

// ============================================================================
// Tests: get_top_needs_action_thread returns PlanApproval
// ============================================================================

#[test]
fn test_plan_approval_thread_is_needs_action() {
    let mut app = create_test_app();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    let result = app.dashboard.get_top_needs_action_thread();

    assert!(result.is_some(), "Plan approval thread should be needs-action");
    let (thread_id, waiting_for) = result.unwrap();
    assert_eq!(thread_id, "t1");
    assert!(matches!(waiting_for, WaitingFor::PlanApproval { .. }));
}

#[test]
fn test_plan_approval_key_for_thread_uses_needs_action_thread_id() {
    let (mut app, _rx) = create_test_app_with_receiver();
    setup_plan_approval_thread(&mut app, "t1", "req-plan-1");

    // Simulate what main.rs does: get thread_id from needs-action, then use it
    let (thread_id, _) = app.dashboard.get_top_needs_action_thread().unwrap();

    let handled = app.handle_plan_approval_key_for_thread('y', &thread_id);

    assert!(handled, "Should handle 'y' using thread_id from get_top_needs_action_thread");
    assert!(app.dashboard.get_plan_request_id("t1").is_none());
}
