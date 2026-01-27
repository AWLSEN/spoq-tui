//! Integration tests for Round 4 features
//! OBSOLETE: These tests reference session_state.pending_permission which was removed
//! in plan-20260127-1645 Round 3 (migration to per-thread permissions).
//!
//! Tests auto-initialization of question state when AskUserQuestion permission is received

use spoq::app::{App, AppMessage};
use spoq::state::PermissionRequest;
use std::time::Instant;

// ============================================================================
// AskUserQuestion Auto-Initialization Tests
// ============================================================================

/// Helper to create an AskUserQuestion permission with the specified question count
fn create_ask_user_question_permission(
    permission_id: &str,
    question_count: usize,
) -> PermissionRequest {
    let questions: Vec<serde_json::Value> = (0..question_count)
        .map(|i| {
            serde_json::json!({
                "question": format!("Question {}", i + 1),
                "header": format!("Q{}", i + 1),
                "options": [
                    {"label": format!("Option A{}", i + 1), "description": "First option"},
                    {"label": format!("Option B{}", i + 1), "description": "Second option"}
                ],
                "multiSelect": false
            })
        })
        .collect();

    PermissionRequest {
        permission_id: permission_id.to_string(),
        thread_id: None,
        tool_name: "AskUserQuestion".to_string(),
        description: "Answer questions".to_string(),
        context: None,
        tool_input: Some(serde_json::json!({
            "questions": questions,
            "answers": {}
        })),
        received_at: Instant::now(),
    }
}

#[test]
fn test_question_state_auto_initialized_on_permission_receipt() {
    let mut app = App::default();

    // Before receiving permission - question state should be empty
    assert_eq!(app.question_state.selections.len(), 0);
    assert_eq!(app.question_state.tab_index, 0);

    // Simulate receiving AskUserQuestion permission via AppMessage
    let perm = create_ask_user_question_permission("perm-auto-init", 3);
    let tool_input = perm.tool_input.clone();

    let msg = AppMessage::PermissionRequested {
        permission_id: perm.permission_id.clone(),
        thread_id: None,
        tool_name: perm.tool_name.clone(),
        description: perm.description.clone(),
        tool_input,
    };

    app.handle_message(msg);

    // After receiving permission - question state should be auto-initialized
    assert_eq!(
        app.question_state.selections.len(),
        3,
        "Should have 3 question selections initialized"
    );
    assert_eq!(app.question_state.tab_index, 0, "Should start at tab 0");

    // First option of each question should be selected by default
    assert_eq!(app.question_state.selections[0], Some(0));
    assert_eq!(app.question_state.selections[1], Some(0));
    assert_eq!(app.question_state.selections[2], Some(0));
}

#[test]
fn test_question_state_not_initialized_for_non_ask_question_permission() {
    let mut app = App::default();

    // Simulate receiving a different tool permission (e.g., Bash)
    let msg = AppMessage::PermissionRequested {
        permission_id: "perm-bash".to_string(),
        thread_id: None,
        tool_name: "Bash".to_string(),
        description: "Run a command".to_string(),
        tool_input: Some(serde_json::json!({"command": "ls -la"})),
    };

    app.handle_message(msg);

    // Question state should remain uninitialized
    assert_eq!(app.question_state.selections.len(), 0);
}

#[test]
fn test_question_state_reset_on_deny() {
    let mut app = App::default();
    let (tx, _rx) = tokio::sync::mpsc::channel(10);
    app.ws_sender = Some(tx);
    app.ws_connection_state = spoq::websocket::WsConnectionState::Connected;

    // Initialize with a permission
    let perm = create_ask_user_question_permission("perm-deny-test", 2);
    app.session_state.set_pending_permission(perm.clone());
    app.init_question_state();

    // Verify state is initialized
    assert_eq!(app.question_state.selections.len(), 2);
    assert!(app.session_state.pending_permission.is_some());

    // Deny the permission
    app.deny_permission(&perm.permission_id);

    // Question state should be reset
    assert_eq!(
        app.question_state.selections.len(),
        0,
        "Selections should be cleared after deny"
    );
    assert_eq!(app.question_state.tab_index, 0, "Tab index should be reset");
    assert!(
        !app.question_state.other_active,
        "Other mode should be deactivated"
    );
    assert!(
        app.session_state.pending_permission.is_none(),
        "Permission should be cleared"
    );
}

#[test]
fn test_question_state_initialized_exactly_once() {
    let mut app = App::default();

    let perm = create_ask_user_question_permission("perm-once", 1);
    app.session_state.set_pending_permission(perm.clone());

    // Initialize multiple times
    app.init_question_state();
    assert_eq!(app.question_state.selections.len(), 1);

    // Modify state
    app.question_state.selections[0] = Some(1);

    // Initialize again - should reinitialize (overwrite)
    app.init_question_state();

    // State should be reinitialized to default (first option selected)
    assert_eq!(app.question_state.selections.len(), 1);
    assert_eq!(
        app.question_state.selections[0],
        Some(0),
        "Should reset to first option"
    );
}

#[test]
fn test_auto_approve_skips_question_initialization() {
    let mut app = App::default();
    let (tx, _rx) = tokio::sync::mpsc::channel(10);
    app.ws_sender = Some(tx);
    app.ws_connection_state = spoq::websocket::WsConnectionState::Connected;

    // Mark AskUserQuestion as always allowed
    app.session_state.allow_tool("AskUserQuestion".to_string());

    // Simulate receiving AskUserQuestion permission
    let perm = create_ask_user_question_permission("perm-auto-approve", 2);
    let tool_input = perm.tool_input.clone();

    let msg = AppMessage::PermissionRequested {
        permission_id: perm.permission_id.clone(),
        thread_id: None,
        tool_name: perm.tool_name.clone(),
        description: perm.description.clone(),
        tool_input,
    };

    app.handle_message(msg);

    // Question state should NOT be initialized (auto-approved, not shown to user)
    assert_eq!(
        app.question_state.selections.len(),
        0,
        "Should not initialize for auto-approved permissions"
    );
    assert!(
        app.session_state.pending_permission.is_none(),
        "Permission should be auto-approved and cleared"
    );
}

#[test]
fn test_question_state_with_multi_select_questions() {
    let mut app = App::default();

    let perm = PermissionRequest {
        permission_id: "perm-multi".to_string(),
        thread_id: None,
        tool_name: "AskUserQuestion".to_string(),
        description: "Answer questions".to_string(),
        context: None,
        tool_input: Some(serde_json::json!({
            "questions": [
                {
                    "question": "Single select question",
                    "header": "Q1",
                    "options": [
                        {"label": "A", "description": ""},
                        {"label": "B", "description": ""}
                    ],
                    "multiSelect": false
                },
                {
                    "question": "Multi select question",
                    "header": "Q2",
                    "options": [
                        {"label": "X", "description": ""},
                        {"label": "Y", "description": ""},
                        {"label": "Z", "description": ""}
                    ],
                    "multiSelect": true
                }
            ],
            "answers": {}
        })),
        received_at: Instant::now(),
    };

    app.session_state.set_pending_permission(perm);
    app.init_question_state();

    // Verify initialization for mixed question types
    assert_eq!(app.question_state.selections.len(), 2);
    assert_eq!(app.question_state.multi_selections.len(), 2);

    // First question (single-select) should have first option selected
    assert_eq!(app.question_state.selections[0], Some(0));

    // Second question (multi-select) should have first option selected but nothing toggled
    assert_eq!(app.question_state.selections[1], Some(0));
    assert_eq!(app.question_state.multi_selections[1].len(), 3);
    assert!(!app.question_state.multi_selections[1][0]);
    assert!(!app.question_state.multi_selections[1][1]);
    assert!(!app.question_state.multi_selections[1][2]);
}

#[test]
fn test_is_ask_user_question_pending_after_auto_init() {
    let mut app = App::default();

    // Send permission message
    let perm = create_ask_user_question_permission("perm-check", 1);
    let tool_input = perm.tool_input.clone();

    let msg = AppMessage::PermissionRequested {
        permission_id: perm.permission_id.clone(),
        thread_id: None,
        tool_name: perm.tool_name.clone(),
        description: perm.description.clone(),
        tool_input,
    };

    app.handle_message(msg);

    // Should detect AskUserQuestion is pending
    assert!(
        app.is_ask_user_question_pending(),
        "Should detect AskUserQuestion after auto-initialization"
    );
}

// ============================================================================
// Redundant Initialization Check Removal Tests
// ============================================================================

#[test]
fn test_main_does_not_redundantly_check_initialization() {
    // This test verifies that the code compiles without the redundant check
    // The actual verification is that main.rs compiles and doesn't have the check anymore
    // We test the behavior indirectly through the handler tests above

    let mut app = App::default();

    // Simulate the flow that would happen in main
    let perm = create_ask_user_question_permission("perm-main-flow", 1);
    let tool_input = perm.tool_input.clone();

    // This is what happens in main's event loop (line 220-280 in main.rs)
    // Permission is received -> handler auto-initializes -> UI ready
    let msg = AppMessage::PermissionRequested {
        permission_id: perm.permission_id.clone(),
        thread_id: None,
        tool_name: perm.tool_name.clone(),
        description: perm.description.clone(),
        tool_input,
    };

    app.handle_message(msg);

    // No redundant check needed - state is ready
    assert!(app.is_ask_user_question_pending());
    assert_eq!(app.question_state.selections.len(), 1);
}
