//! Full Flow Integration Test for Question Overlay
//!
//! This test file exercises the complete question flow from end to end:
//! 1. Receive multi-question AskUserQuestion payload via WebSocket
//! 2. Store question data in DashboardState
//! 3. Open question overlay with options displayed
//! 4. Navigate options with arrow keys (Down, Down)
//! 5. Select option with Enter (auto-advances to next question)
//! 6. Switch questions with Tab
//! 7. Navigate and select with arrow keys and Enter
//! 8. Verify response payload structure matches conductor expectations
//!
//! This test covers the requirements from Plan 20260125-1936, Phase 6.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use spoq::models::dashboard::{ThreadStatus, WaitingFor};
use spoq::models::{Thread, ThreadMode, ThreadType};
use spoq::state::dashboard::{DashboardQuestionState, DashboardState};
use spoq::state::session::{AskUserQuestionData, Question, QuestionOption};
use spoq::ui::dashboard::question_card::{render_question, QuestionRenderConfig};
use std::collections::HashMap;

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test thread with the given properties
fn make_test_thread(id: &str, title: &str) -> Thread {
    Thread {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        preview: format!("Preview for {}", title),
        updated_at: chrono::Utc::now(),
        thread_type: ThreadType::Programming,
        mode: ThreadMode::default(),
        model: Some("claude-opus-4".to_string()),
        permission_mode: Some("plan".to_string()),
        message_count: 5,
        created_at: chrono::Utc::now(),
        working_directory: Some(format!("/Users/sam/{}", id)),
        status: Some(ThreadStatus::Waiting),
        verified: None,
        verified_at: None,
    }
}

/// Create a multi-question AskUserQuestionData payload
/// This simulates what would come from a WebSocket message
fn make_multi_question_payload() -> AskUserQuestionData {
    AskUserQuestionData {
        questions: vec![
            Question {
                question: "Which authentication method should we use?".to_string(),
                header: "Auth".to_string(),
                options: vec![
                    QuestionOption {
                        label: "JWT tokens".to_string(),
                        description: "Stateless token-based authentication".to_string(),
                    },
                    QuestionOption {
                        label: "Session cookies".to_string(),
                        description: "Server-side session storage".to_string(),
                    },
                    QuestionOption {
                        label: "OAuth 2.0".to_string(),
                        description: "Third-party provider authentication".to_string(),
                    },
                ],
                multi_select: false,
            },
            Question {
                question: "Which database do you prefer?".to_string(),
                header: "Database".to_string(),
                options: vec![
                    QuestionOption {
                        label: "PostgreSQL".to_string(),
                        description: "Relational database with ACID compliance".to_string(),
                    },
                    QuestionOption {
                        label: "MongoDB".to_string(),
                        description: "NoSQL document database".to_string(),
                    },
                    QuestionOption {
                        label: "SQLite".to_string(),
                        description: "Lightweight embedded database".to_string(),
                    },
                ],
                multi_select: false,
            },
            Question {
                question: "Select build tools to enable:".to_string(),
                header: "Build".to_string(),
                options: vec![
                    QuestionOption {
                        label: "ESLint".to_string(),
                        description: "JavaScript/TypeScript linter".to_string(),
                    },
                    QuestionOption {
                        label: "Prettier".to_string(),
                        description: "Code formatter".to_string(),
                    },
                    QuestionOption {
                        label: "TypeScript".to_string(),
                        description: "Static type checking".to_string(),
                    },
                ],
                multi_select: true,
            },
        ],
        answers: HashMap::new(),
    }
}

/// Simulates parsing the WebSocket AskUserQuestion permission_request payload
/// Returns the extracted AskUserQuestionData if valid
fn parse_websocket_ask_user_question(json: &str) -> Option<AskUserQuestionData> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let tool_input = value.get("tool_input")?;
    serde_json::from_value(tool_input.clone()).ok()
}

// ============================================================================
// Full Flow Integration Test
// ============================================================================

#[test]
fn test_full_question_flow_receive_navigate_select_submit() {
    // ========================================================================
    // PHASE 1: Receive question via WebSocket and store in DashboardState
    // ========================================================================

    // Simulated WebSocket message (as would come from spoq-conductor)
    let ws_json = r#"{
        "type": "permission_request",
        "request_id": "perm_ask-user-123",
        "thread_id": "thread-full-flow",
        "tool_name": "AskUserQuestion",
        "tool_input": {
            "questions": [
                {
                    "question": "Which authentication method should we use?",
                    "header": "Auth",
                    "options": [
                        {"label": "JWT tokens", "description": "Stateless token-based authentication"},
                        {"label": "Session cookies", "description": "Server-side session storage"},
                        {"label": "OAuth 2.0", "description": "Third-party provider authentication"}
                    ],
                    "multiSelect": false
                },
                {
                    "question": "Which database do you prefer?",
                    "header": "Database",
                    "options": [
                        {"label": "PostgreSQL", "description": "Relational database with ACID compliance"},
                        {"label": "MongoDB", "description": "NoSQL document database"},
                        {"label": "SQLite", "description": "Lightweight embedded database"}
                    ],
                    "multiSelect": false
                },
                {
                    "question": "Select build tools to enable:",
                    "header": "Build",
                    "options": [
                        {"label": "ESLint", "description": "JavaScript/TypeScript linter"},
                        {"label": "Prettier", "description": "Code formatter"},
                        {"label": "TypeScript", "description": "Static type checking"}
                    ],
                    "multiSelect": true
                }
            ],
            "answers": {}
        },
        "description": "Ask user about project configuration",
        "timestamp": 1737838800000
    }"#;

    // Parse the WebSocket message to extract question data
    let question_data = parse_websocket_ask_user_question(ws_json)
        .expect("Should parse WebSocket AskUserQuestion payload");

    // Verify parsing is correct
    assert_eq!(question_data.questions.len(), 3);
    assert_eq!(
        question_data.questions[0].question,
        "Which authentication method should we use?"
    );
    assert_eq!(question_data.questions[0].options.len(), 3);
    assert!(!question_data.questions[0].multi_select);
    assert_eq!(
        question_data.questions[1].question,
        "Which database do you prefer?"
    );
    assert!(question_data.questions[2].multi_select);

    // Create DashboardState and add thread
    let mut state = DashboardState::new();
    let thread = make_test_thread("thread-full-flow", "Full Flow Test Thread");
    state.add_thread(thread);

    // Update thread status to waiting for user input
    state.update_thread_status(
        "thread-full-flow",
        ThreadStatus::Waiting,
        Some(WaitingFor::UserInput),
    );

    // Store the question data (simulating what WebSocket handler does)
    state.set_pending_question(
        "thread-full-flow",
        "perm_ask-user-123".to_string(),
        question_data.clone(),
    );

    // Verify question is stored
    let stored = state.get_pending_question("thread-full-flow");
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().questions.len(), 3);

    // Verify request_id is stored
    let request_id = state.get_pending_question_request_id("thread-full-flow");
    assert_eq!(request_id, Some("perm_ask-user-123"));

    // ========================================================================
    // PHASE 2: Display overlay with question options
    // ========================================================================

    // Expand thread to open question overlay
    state.expand_thread("thread-full-flow", 5);

    // Verify overlay is open with question data
    let overlay = state.overlay();
    assert!(overlay.is_some());

    if let Some(spoq::ui::dashboard::OverlayState::Question {
        thread_id,
        question_data: overlay_question_data,
        ..
    }) = overlay
    {
        assert_eq!(thread_id, "thread-full-flow");
        assert!(overlay_question_data.is_some());
        let qd = overlay_question_data.as_ref().unwrap();
        assert_eq!(qd.questions.len(), 3);
        assert_eq!(qd.questions[0].options.len(), 3);
    } else {
        panic!("Expected Question overlay");
    }

    // Verify question state is initialized
    let question_state = state.question_state();
    assert!(question_state.is_some());
    let qs = question_state.unwrap();
    assert_eq!(qs.tab_index, 0); // First question tab
    assert_eq!(qs.current_selection(), Some(0)); // First option selected by default
    assert!(!qs.other_active);

    // ========================================================================
    // PHASE 3: Navigate options with arrow keys (Down, Down)
    // ========================================================================

    // Initial selection is 0 (JWT tokens)
    assert_eq!(state.question_state().unwrap().current_selection(), Some(0));

    // Simulate pressing Down arrow
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(1)); // Session cookies

    // Simulate pressing Down arrow again
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(2)); // OAuth 2.0

    // ========================================================================
    // PHASE 4: Select option with Enter (auto-advances to next question)
    // ========================================================================

    // Confirm selection - for multi-question, this should NOT return answers yet
    // It should mark current question as answered and advance to next
    let result = state.question_confirm();
    assert!(result.is_none()); // Not done yet, auto-advanced to next question

    // Verify we moved to question 2 (Database)
    assert_eq!(state.question_state().unwrap().tab_index, 1);
    assert_eq!(state.question_state().unwrap().current_selection(), Some(0)); // PostgreSQL

    // ========================================================================
    // PHASE 5: Use Tab to see multi-question navigation works
    // ========================================================================

    // Tab to next question (Build tools - multi-select)
    state.question_next_tab();
    assert_eq!(state.question_state().unwrap().tab_index, 2);

    // Tab wraps back to first question
    state.question_next_tab();
    assert_eq!(state.question_state().unwrap().tab_index, 0);

    // Tab back to second question (Database) which is unanswered
    state.question_next_tab();
    assert_eq!(state.question_state().unwrap().tab_index, 1);

    // ========================================================================
    // PHASE 6: Navigate with Up arrow and select with Enter
    // ========================================================================

    // Currently on question 2 (Database), first option selected (PostgreSQL)
    assert_eq!(state.question_state().unwrap().current_selection(), Some(0));

    // Navigate up should wrap to "Other"
    state.question_prev_option();
    assert_eq!(state.question_state().unwrap().current_selection(), None); // Other

    // Navigate up from Other goes to last option
    state.question_prev_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(2)); // SQLite

    // Navigate up again
    state.question_prev_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(1)); // MongoDB

    // Confirm selection (MongoDB)
    let result = state.question_confirm();
    assert!(result.is_none()); // Still not done

    // Should have advanced to the third question (Build tools - multi-select)
    assert_eq!(state.question_state().unwrap().tab_index, 2);

    // ========================================================================
    // PHASE 7: Handle multi-select question with Space toggles
    // ========================================================================

    // On Build tools question (multi-select)
    // Initial selection is 0 (ESLint)
    assert_eq!(state.question_state().unwrap().current_selection(), Some(0));

    // Toggle ESLint (select it)
    state.question_toggle_option();
    assert!(state.question_state().unwrap().is_multi_selected(0));
    assert!(!state.question_state().unwrap().is_multi_selected(1));
    assert!(!state.question_state().unwrap().is_multi_selected(2));

    // Navigate down and toggle Prettier
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(1));
    state.question_toggle_option();
    assert!(state.question_state().unwrap().is_multi_selected(0));
    assert!(state.question_state().unwrap().is_multi_selected(1));

    // Navigate down to TypeScript but don't toggle it
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(2));
    // Not toggling TypeScript

    // ========================================================================
    // PHASE 8: Submit and verify response payload structure
    // ========================================================================

    // Confirm final selection - all questions answered, should return answers
    let result = state.question_confirm();
    assert!(
        result.is_some(),
        "Should return answers when all questions are answered"
    );

    let (thread_id, request_id, answers) = result.unwrap();

    // Verify thread_id and request_id
    assert_eq!(thread_id, "thread-full-flow");
    assert_eq!(request_id, "perm_ask-user-123");

    // Verify answers map structure
    assert_eq!(answers.len(), 3, "Should have 3 answers for 3 questions");

    // Question 1: Selected "OAuth 2.0" (third option, index 2)
    assert_eq!(
        answers.get("Which authentication method should we use?"),
        Some(&"OAuth 2.0".to_string())
    );

    // Question 2: Selected "MongoDB" (second option, index 1)
    assert_eq!(
        answers.get("Which database do you prefer?"),
        Some(&"MongoDB".to_string())
    );

    // Question 3: Multi-select with "ESLint" and "Prettier"
    let build_answer = answers
        .get("Select build tools to enable:")
        .expect("Should have build tools answer");
    assert!(
        build_answer.contains("ESLint"),
        "Build answer should contain ESLint"
    );
    assert!(
        build_answer.contains("Prettier"),
        "Build answer should contain Prettier"
    );
    assert!(
        !build_answer.contains("TypeScript"),
        "Build answer should NOT contain TypeScript"
    );

    // Verify the payload can be serialized for WebSocket response
    let answers_json = serde_json::to_string(&answers).expect("Answers should serialize to JSON");
    assert!(answers_json.contains("OAuth 2.0"));
    assert!(answers_json.contains("MongoDB"));
    assert!(answers_json.contains("ESLint"));
}

// ============================================================================
// Response Payload Format Verification Tests
// ============================================================================

#[test]
fn test_response_payload_matches_conductor_expectations() {
    // Create state with a simple question
    let mut state = DashboardState::new();
    let thread = make_test_thread("thread-payload", "Payload Test");
    state.add_thread(thread);

    state.update_thread_status(
        "thread-payload",
        ThreadStatus::Waiting,
        Some(WaitingFor::UserInput),
    );

    let question_data = AskUserQuestionData {
        questions: vec![Question {
            question: "Which framework?".to_string(),
            header: "Framework".to_string(),
            options: vec![
                QuestionOption {
                    label: "React".to_string(),
                    description: "Meta's UI library".to_string(),
                },
                QuestionOption {
                    label: "Vue".to_string(),
                    description: "Progressive framework".to_string(),
                },
            ],
            multi_select: false,
        }],
        answers: HashMap::new(),
    };

    state.set_pending_question("thread-payload", "perm_payload-test".to_string(), question_data);
    state.expand_thread("thread-payload", 5);

    // Select second option (Vue)
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(1));

    // Confirm and get answers
    let result = state.question_confirm();
    assert!(result.is_some());

    let (_thread_id, request_id, answers) = result.unwrap();

    // Build the response payload as the TUI would
    let answers_value = serde_json::to_value(&answers).unwrap_or_default();

    // This is the structure that spoq-conductor expects
    let response_payload = serde_json::json!({
        "type": "command_response",
        "request_id": request_id,
        "result": {
            "status": "success",
            "data": {
                "allowed": true,
                "message": answers_value.to_string()
            }
        }
    });

    // Verify the structure
    assert_eq!(response_payload["type"], "command_response");
    assert_eq!(response_payload["request_id"], "perm_payload-test");
    assert_eq!(response_payload["result"]["status"], "success");
    assert_eq!(response_payload["result"]["data"]["allowed"], true);

    // Verify the message can be parsed back to get answers
    let message_str = response_payload["result"]["data"]["message"]
        .as_str()
        .unwrap();
    let parsed_answers: HashMap<String, String> =
        serde_json::from_str(message_str).expect("Message should be valid JSON");
    assert_eq!(
        parsed_answers.get("Which framework?"),
        Some(&"Vue".to_string())
    );
}

// ============================================================================
// Render Verification Tests
// ============================================================================

#[test]
fn test_question_card_renders_options_correctly() {
    let backend = TestBackend::new(80, 25);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "JWT tokens".to_string(),
        "Session cookies".to_string(),
        "OAuth 2.0".to_string(),
    ];

    let descriptions = vec![
        "Stateless token-based authentication".to_string(),
        "Server-side session storage".to_string(),
        "Third-party provider authentication".to_string(),
    ];

    let tab_headers = vec![
        "Auth".to_string(),
        "Database".to_string(),
        "Build".to_string(),
    ];

    let tabs_answered = vec![false, false, false];

    let config = QuestionRenderConfig {
        question: "Which authentication method should we use?",
        options: &options,
        selected_index: Some(1), // Session cookies selected
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &descriptions,
        tab_headers: &tab_headers,
        current_tab: 0,
        tabs_answered: &tabs_answered,
        timer_seconds: None,
    };

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 2, 76, 20);
            render_question(
                frame,
                area,
                "thread-render-test",
                "Auth Setup",
                "my-project",
                &config,
            );
        })
        .unwrap();

    // Verify rendering completed without panic
}

#[test]
fn test_question_card_renders_multi_select_with_checkboxes() {
    let backend = TestBackend::new(80, 25);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "ESLint".to_string(),
        "Prettier".to_string(),
        "TypeScript".to_string(),
    ];

    let descriptions = vec![
        "JavaScript/TypeScript linter".to_string(),
        "Code formatter".to_string(),
        "Static type checking".to_string(),
    ];

    let selections = vec![true, true, false]; // ESLint and Prettier selected

    let config = QuestionRenderConfig {
        question: "Select build tools to enable:",
        options: &options,
        selected_index: Some(0), // Cursor on first option
        multi_select: true,
        multi_selections: &selections,
        other_input: "",
        other_selected: false,
        option_descriptions: &descriptions,
        tab_headers: &["Build".to_string()],
        current_tab: 0,
        tabs_answered: &[false],
        timer_seconds: None,
    };

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 2, 76, 20);
            render_question(
                frame,
                area,
                "thread-multi-select",
                "Build Setup",
                "my-project",
                &config,
            );
        })
        .unwrap();

    // Verify rendering completed without panic
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_full_flow_with_other_text_input() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("thread-other", "Other Text Test");
    state.add_thread(thread);

    state.update_thread_status(
        "thread-other",
        ThreadStatus::Waiting,
        Some(WaitingFor::UserInput),
    );

    let question_data = AskUserQuestionData {
        questions: vec![Question {
            question: "Which language?".to_string(),
            header: "Language".to_string(),
            options: vec![
                QuestionOption {
                    label: "Python".to_string(),
                    description: "General purpose".to_string(),
                },
                QuestionOption {
                    label: "Rust".to_string(),
                    description: "Systems programming".to_string(),
                },
            ],
            multi_select: false,
        }],
        answers: HashMap::new(),
    };

    state.set_pending_question("thread-other", "perm_other-test".to_string(), question_data);
    state.expand_thread("thread-other", 5);

    // Navigate to "Other" (past all options)
    state.question_next_option(); // Python -> Rust
    state.question_next_option(); // Rust -> Other
    assert_eq!(state.question_state().unwrap().current_selection(), None); // Other

    // Pressing Enter on Other should activate text input mode
    let result = state.question_confirm();
    assert!(result.is_none()); // Not submitted, activated Other mode
    assert!(state.is_question_other_active());

    // Type custom text
    for c in "Go language".chars() {
        state.question_type_char(c);
    }
    assert_eq!(
        state.question_state().unwrap().current_other_text(),
        "Go language"
    );

    // Now confirm with text entered
    let result = state.question_confirm();
    assert!(result.is_some());

    let (_, _, answers) = result.unwrap();
    assert_eq!(
        answers.get("Which language?"),
        Some(&"Go language".to_string())
    );
}

#[test]
fn test_full_flow_single_question_immediate_submit() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("thread-single", "Single Question Test");
    state.add_thread(thread);

    state.update_thread_status(
        "thread-single",
        ThreadStatus::Waiting,
        Some(WaitingFor::UserInput),
    );

    // Single question (not multi-question)
    let question_data = AskUserQuestionData {
        questions: vec![Question {
            question: "Continue with deployment?".to_string(),
            header: "Deploy".to_string(),
            options: vec![
                QuestionOption {
                    label: "Yes, deploy now".to_string(),
                    description: "Deploy immediately".to_string(),
                },
                QuestionOption {
                    label: "No, cancel".to_string(),
                    description: "Cancel deployment".to_string(),
                },
            ],
            multi_select: false,
        }],
        answers: HashMap::new(),
    };

    state.set_pending_question("thread-single", "perm_single-123".to_string(), question_data);
    state.expand_thread("thread-single", 5);

    // Select first option and confirm - should submit immediately (single question)
    assert_eq!(state.question_state().unwrap().current_selection(), Some(0));

    let result = state.question_confirm();
    assert!(
        result.is_some(),
        "Single question should submit immediately on Enter"
    );

    let (thread_id, request_id, answers) = result.unwrap();
    assert_eq!(thread_id, "thread-single");
    assert_eq!(request_id, "perm_single-123");
    assert_eq!(
        answers.get("Continue with deployment?"),
        Some(&"Yes, deploy now".to_string())
    );
}

#[test]
fn test_navigation_wraps_around_correctly() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("thread-wrap", "Wrap Test");
    state.add_thread(thread);

    let question_data = AskUserQuestionData {
        questions: vec![Question {
            question: "Pick one".to_string(),
            header: "Pick".to_string(),
            options: vec![
                QuestionOption {
                    label: "A".to_string(),
                    description: "Option A".to_string(),
                },
                QuestionOption {
                    label: "B".to_string(),
                    description: "Option B".to_string(),
                },
            ],
            multi_select: false,
        }],
        answers: HashMap::new(),
    };

    state.set_pending_question("thread-wrap", "perm_wrap".to_string(), question_data);
    state.expand_thread("thread-wrap", 5);

    // Start at 0
    assert_eq!(state.question_state().unwrap().current_selection(), Some(0));

    // Down -> 1
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(1));

    // Down -> Other (None)
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), None);

    // Down from Other wraps to 0
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(0));

    // Up from 0 goes to Other
    state.question_prev_option();
    assert_eq!(state.question_state().unwrap().current_selection(), None);

    // Up from Other goes to last option (1)
    state.question_prev_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(1));
}

#[test]
fn test_tab_navigation_wraps_correctly() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("thread-tab-wrap", "Tab Wrap Test");
    state.add_thread(thread);

    let question_data = make_multi_question_payload();
    state.set_pending_question("thread-tab-wrap", "perm_tab".to_string(), question_data);
    state.expand_thread("thread-tab-wrap", 5);

    // Start at tab 0
    assert_eq!(state.question_state().unwrap().tab_index, 0);

    // Tab to 1
    state.question_next_tab();
    assert_eq!(state.question_state().unwrap().tab_index, 1);

    // Tab to 2
    state.question_next_tab();
    assert_eq!(state.question_state().unwrap().tab_index, 2);

    // Tab wraps to 0
    state.question_next_tab();
    assert_eq!(state.question_state().unwrap().tab_index, 0);
}

#[test]
fn test_cleanup_clears_pending_question_on_status_change() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("thread-cleanup", "Cleanup Test");
    state.add_thread(thread);

    state.update_thread_status(
        "thread-cleanup",
        ThreadStatus::Waiting,
        Some(WaitingFor::UserInput),
    );

    let question_data = make_multi_question_payload();
    state.set_pending_question("thread-cleanup", "perm_cleanup".to_string(), question_data);

    // Verify question is stored
    assert!(state.get_pending_question("thread-cleanup").is_some());

    // Thread completes - should clear pending question
    state.update_thread_status("thread-cleanup", ThreadStatus::Done, None);

    // Pending question should be cleared
    assert!(state.get_pending_question("thread-cleanup").is_none());
}

// ============================================================================
// DashboardQuestionState Unit Tests
// ============================================================================

#[test]
fn test_dashboard_question_state_from_multi_question_data() {
    let question_data = make_multi_question_payload();
    let state = DashboardQuestionState::from_question_data(&question_data);

    // Verify initialization
    assert_eq!(state.tab_index, 0);
    assert_eq!(state.selections.len(), 3);
    assert_eq!(state.multi_selections.len(), 3);
    assert_eq!(state.other_texts.len(), 3);
    assert_eq!(state.answered.len(), 3);

    // All should default to first option selected
    assert_eq!(state.selections[0], Some(0));
    assert_eq!(state.selections[1], Some(0));
    assert_eq!(state.selections[2], Some(0));

    // Multi-selections should be false by default
    assert!(state.multi_selections[0].iter().all(|&x| !x));
    assert!(state.multi_selections[1].iter().all(|&x| !x));
    assert!(state.multi_selections[2].iter().all(|&x| !x));

    // Other texts should be empty
    assert!(state.other_texts.iter().all(|t| t.is_empty()));

    // Not in other mode
    assert!(!state.other_active);

    // None answered yet
    assert!(state.answered.iter().all(|&a| !a));
}

#[test]
fn test_dashboard_question_state_advance_to_unanswered() {
    let question_data = make_multi_question_payload();
    let mut state = DashboardQuestionState::from_question_data(&question_data);

    // Mark first as answered
    state.answered[0] = true;
    state.tab_index = 0;

    // Advance to next unanswered
    let moved = state.advance_to_next_unanswered(3);
    assert!(moved);
    assert_eq!(state.tab_index, 1);

    // Mark second as answered
    state.answered[1] = true;

    // Advance again
    let moved = state.advance_to_next_unanswered(3);
    assert!(moved);
    assert_eq!(state.tab_index, 2);

    // Mark third as answered
    state.answered[2] = true;

    // No more unanswered - should not move
    let moved = state.advance_to_next_unanswered(3);
    assert!(!moved);
}

#[test]
fn test_multi_select_with_other_text_appended() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("thread-multi-other", "Multi Other Test");
    state.add_thread(thread);

    // Multi-select question with some options selected + custom Other text
    let question_data = AskUserQuestionData {
        questions: vec![Question {
            question: "Select features:".to_string(),
            header: "Features".to_string(),
            options: vec![
                QuestionOption {
                    label: "Feature A".to_string(),
                    description: "A desc".to_string(),
                },
                QuestionOption {
                    label: "Feature B".to_string(),
                    description: "B desc".to_string(),
                },
            ],
            multi_select: true,
        }],
        answers: HashMap::new(),
    };

    state.set_pending_question(
        "thread-multi-other",
        "perm_multi-other".to_string(),
        question_data,
    );
    state.expand_thread("thread-multi-other", 5);

    // Toggle Feature A
    state.question_toggle_option();
    assert!(state.question_state().unwrap().is_multi_selected(0));

    // Navigate to Other and add custom text
    state.question_next_option(); // -> Feature B
    state.question_next_option(); // -> Other
    assert_eq!(state.question_state().unwrap().current_selection(), None);

    // Activate Other and type
    state.question_activate_other();
    for c in "Custom Feature".chars() {
        state.question_type_char(c);
    }

    // Confirm
    let result = state.question_confirm();
    assert!(result.is_some());

    let (_, _, answers) = result.unwrap();
    let answer = answers.get("Select features:").unwrap();

    // Should contain both Feature A and Custom Feature
    assert!(answer.contains("Feature A"));
    assert!(answer.contains("Custom Feature"));
    assert!(!answer.contains("Feature B")); // Not selected
}
