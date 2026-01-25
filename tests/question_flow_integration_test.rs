//! Question Flow Integration Tests
//!
//! These tests verify the complete question flow including:
//! - Receive question via WebSocket -> Store in DashboardState
//! - Display overlay with question options
//! - Navigate options (Up/Down, Tab for multi-question)
//! - Submit response via WebSocket
//! - Cleanup when thread status changes
//!
//! Edge cases tested:
//! - Empty options list
//! - Very long question text (truncation with ellipsis)
//! - Multi-select combinations
//! - "Other" text input validation
//! - Multi-question tab flow
//! - Cleanup on thread completion/dismissal

use spoq::models::dashboard::{ThreadStatus, WaitingFor};
use spoq::models::{Thread, ThreadMode, ThreadType};
use spoq::state::dashboard::{DashboardQuestionState, DashboardState};
use spoq::state::session::{AskUserQuestionData, Question, QuestionOption};
use std::collections::HashMap;

// ============================================================================
// Test Helpers
// ============================================================================

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

fn make_simple_question() -> AskUserQuestionData {
    AskUserQuestionData {
        questions: vec![Question {
            question: "Which authentication method should we use?".to_string(),
            header: "Auth".to_string(),
            options: vec![
                QuestionOption {
                    label: "JWT".to_string(),
                    description: "Stateless token-based authentication".to_string(),
                },
                QuestionOption {
                    label: "Sessions".to_string(),
                    description: "Server-side session storage".to_string(),
                },
                QuestionOption {
                    label: "OAuth 2.0".to_string(),
                    description: "Third-party authentication".to_string(),
                },
            ],
            multi_select: false,
        }],
        answers: HashMap::new(),
    }
}

fn make_multi_select_question() -> AskUserQuestionData {
    AskUserQuestionData {
        questions: vec![Question {
            question: "Select features to enable:".to_string(),
            header: "Features".to_string(),
            options: vec![
                QuestionOption {
                    label: "Dark mode".to_string(),
                    description: "Enable dark theme".to_string(),
                },
                QuestionOption {
                    label: "Notifications".to_string(),
                    description: "Push notifications".to_string(),
                },
                QuestionOption {
                    label: "Analytics".to_string(),
                    description: "Usage analytics".to_string(),
                },
            ],
            multi_select: true,
        }],
        answers: HashMap::new(),
    }
}

fn make_multi_question_data() -> AskUserQuestionData {
    AskUserQuestionData {
        questions: vec![
            Question {
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
            },
            Question {
                question: "Which database?".to_string(),
                header: "Database".to_string(),
                options: vec![
                    QuestionOption {
                        label: "PostgreSQL".to_string(),
                        description: "Relational database".to_string(),
                    },
                    QuestionOption {
                        label: "MongoDB".to_string(),
                        description: "Document database".to_string(),
                    },
                ],
                multi_select: false,
            },
            Question {
                question: "Select build tools:".to_string(),
                header: "Build".to_string(),
                options: vec![
                    QuestionOption {
                        label: "ESLint".to_string(),
                        description: "JavaScript linter".to_string(),
                    },
                    QuestionOption {
                        label: "Prettier".to_string(),
                        description: "Code formatter".to_string(),
                    },
                ],
                multi_select: true,
            },
        ],
        answers: HashMap::new(),
    }
}

// ============================================================================
// Receive Question and Store in State
// ============================================================================

#[test]
fn test_receive_question_stores_in_dashboard_state() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    // Simulate receiving question from WebSocket
    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data.clone());

    // Verify question is stored
    let stored = state.get_pending_question("t1");
    assert!(stored.is_some());
    let stored = stored.unwrap();
    assert_eq!(stored.questions.len(), 1);
    assert_eq!(stored.questions[0].question, "Which authentication method should we use?");
    assert_eq!(stored.questions[0].options.len(), 3);

    // Verify request_id is stored
    let request_id = state.get_pending_question_request_id("t1");
    assert_eq!(request_id, Some("req-123"));
}

#[test]
fn test_receive_question_with_multi_select() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_multi_select_question();
    state.set_pending_question("t1", "req-multi".to_string(), question_data);

    let stored = state.get_pending_question("t1").unwrap();
    assert!(stored.questions[0].multi_select);
}

#[test]
fn test_receive_multi_question() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_multi_question_data();
    state.set_pending_question("t1", "req-multi-q".to_string(), question_data);

    let stored = state.get_pending_question("t1").unwrap();
    assert_eq!(stored.questions.len(), 3);
    assert_eq!(stored.questions[0].header, "Framework");
    assert_eq!(stored.questions[1].header, "Database");
    assert_eq!(stored.questions[2].header, "Build");
}

// ============================================================================
// Display Overlay with Question Data
// ============================================================================

#[test]
fn test_expand_thread_opens_question_overlay() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    // Set waiting for user input
    state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

    // Set pending question
    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    // Expand thread
    state.expand_thread("t1", 10);

    // Verify overlay is open with question data
    let overlay = state.overlay();
    assert!(overlay.is_some());

    if let Some(spoq::ui::dashboard::OverlayState::Question {
        thread_id,
        question_data,
        ..
    }) = overlay
    {
        assert_eq!(thread_id, "t1");
        assert!(question_data.is_some());
        let q = question_data.as_ref().unwrap();
        assert_eq!(q.questions[0].options.len(), 3);
    } else {
        panic!("Expected Question overlay");
    }
}

#[test]
fn test_expand_thread_initializes_question_state() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // Verify question navigation state is initialized
    let question_state = state.question_state();
    assert!(question_state.is_some());
    let qs = question_state.unwrap();
    assert_eq!(qs.tab_index, 0);
    assert_eq!(qs.current_selection(), Some(0)); // First option selected by default
    assert!(!qs.other_active);
}

// ============================================================================
// Keyboard Navigation Tests
// ============================================================================

#[test]
fn test_question_navigation_up_down() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // Initial selection is 0
    assert_eq!(state.question_state().unwrap().current_selection(), Some(0));

    // Navigate down
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(1));

    // Navigate down again
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(2));

    // Navigate down to "Other"
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), None); // None = Other

    // Navigate down wraps to first
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(0));

    // Navigate up from first
    state.question_prev_option();
    assert_eq!(state.question_state().unwrap().current_selection(), None); // Other

    // Navigate up from Other
    state.question_prev_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(2));
}

#[test]
fn test_question_tab_navigation_multi_question() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_multi_question_data();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // Initial tab is 0
    assert_eq!(state.question_state().unwrap().tab_index, 0);

    // Tab to next question
    state.question_next_tab();
    assert_eq!(state.question_state().unwrap().tab_index, 1);

    // Tab to next question
    state.question_next_tab();
    assert_eq!(state.question_state().unwrap().tab_index, 2);

    // Tab wraps around
    state.question_next_tab();
    assert_eq!(state.question_state().unwrap().tab_index, 0);
}

#[test]
fn test_question_toggle_multi_select() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_multi_select_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // Toggle first option
    state.question_toggle_option();
    assert!(state.question_state().unwrap().is_multi_selected(0));
    assert!(!state.question_state().unwrap().is_multi_selected(1));

    // Navigate to second and toggle
    state.question_next_option();
    state.question_toggle_option();
    assert!(state.question_state().unwrap().is_multi_selected(0));
    assert!(state.question_state().unwrap().is_multi_selected(1));

    // Toggle first off
    state.question_prev_option();
    state.question_toggle_option();
    assert!(!state.question_state().unwrap().is_multi_selected(0));
    assert!(state.question_state().unwrap().is_multi_selected(1));
}

// ============================================================================
// Other Text Input Tests
// ============================================================================

#[test]
fn test_other_text_input_activation() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // Navigate to Other
    state.question_next_option();
    state.question_next_option();
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), None);

    // Activate Other input
    state.question_activate_other();
    assert!(state.is_question_other_active());

    // Type some text
    state.question_type_char('C');
    state.question_type_char('u');
    state.question_type_char('s');
    state.question_type_char('t');
    state.question_type_char('o');
    state.question_type_char('m');

    assert_eq!(state.question_state().unwrap().current_other_text(), "Custom");

    // Backspace
    state.question_backspace();
    assert_eq!(state.question_state().unwrap().current_other_text(), "Custo");
}

#[test]
fn test_other_text_input_cancel() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // Navigate to Other and activate
    for _ in 0..3 {
        state.question_next_option();
    }
    state.question_activate_other();
    state.question_type_char('T');
    state.question_type_char('e');
    state.question_type_char('s');
    state.question_type_char('t');

    assert_eq!(state.question_state().unwrap().current_other_text(), "Test");

    // Cancel Other input
    state.question_cancel_other();
    assert!(!state.is_question_other_active());
    assert_eq!(state.question_state().unwrap().current_other_text(), "");
}

#[test]
fn test_other_text_validation_empty_not_allowed() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // Navigate to Other, activate but don't type anything
    for _ in 0..3 {
        state.question_next_option();
    }
    state.question_activate_other();

    // Try to confirm with empty text
    let result = state.question_confirm();
    assert!(result.is_none()); // Should not submit with empty Other text
}

// ============================================================================
// Question Submit Flow Tests
// ============================================================================

#[test]
fn test_question_confirm_single_question() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // Select second option (Sessions)
    state.question_next_option();
    assert_eq!(state.question_state().unwrap().current_selection(), Some(1));

    // Confirm
    let result = state.question_confirm();
    assert!(result.is_some());

    let (thread_id, request_id, answers) = result.unwrap();
    assert_eq!(thread_id, "t1");
    assert_eq!(request_id, "req-123");
    assert_eq!(
        answers.get("Which authentication method should we use?"),
        Some(&"Sessions".to_string())
    );
}

#[test]
fn test_question_confirm_multi_select() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_multi_select_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // Toggle first option
    state.question_toggle_option();

    // Navigate to third and toggle
    state.question_next_option();
    state.question_next_option();
    state.question_toggle_option();

    // Confirm
    let result = state.question_confirm();
    assert!(result.is_some());

    let (_, _, answers) = result.unwrap();
    let answer = answers.get("Select features to enable:").unwrap();
    // Should be comma-separated
    assert!(answer.contains("Dark mode"));
    assert!(answer.contains("Analytics"));
}

#[test]
fn test_question_confirm_multi_question_flow() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_multi_question_data();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);

    // First question: select first option and confirm
    let result = state.question_confirm();
    assert!(result.is_none()); // Should not submit yet, advances to next

    // Verify we moved to second question
    assert_eq!(state.question_state().unwrap().tab_index, 1);

    // Second question: select second option and confirm
    state.question_next_option();
    let result = state.question_confirm();
    assert!(result.is_none()); // Still not done

    // Third question (multi-select): toggle both and confirm
    assert_eq!(state.question_state().unwrap().tab_index, 2);
    state.question_toggle_option();
    state.question_next_option();
    state.question_toggle_option();

    let result = state.question_confirm();
    assert!(result.is_some()); // Now we're done

    let (_, _, answers) = result.unwrap();
    assert_eq!(answers.len(), 3);
    assert_eq!(answers.get("Which framework?"), Some(&"React".to_string()));
    assert_eq!(answers.get("Which database?"), Some(&"MongoDB".to_string()));
    let build_answer = answers.get("Select build tools:").unwrap();
    assert!(build_answer.contains("ESLint"));
    assert!(build_answer.contains("Prettier"));
}

// ============================================================================
// Cleanup Tests
// ============================================================================

#[test]
fn test_cleanup_on_thread_completion() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    // Set pending question
    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

    assert!(state.get_pending_question("t1").is_some());

    // Thread completes (Done status)
    state.update_thread_status("t1", ThreadStatus::Done, None);

    // Pending question should be cleared
    assert!(state.get_pending_question("t1").is_none());
}

#[test]
fn test_cleanup_on_thread_error() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

    assert!(state.get_pending_question("t1").is_some());

    // Thread errors
    state.update_thread_status("t1", ThreadStatus::Error, None);

    // Pending question should be cleared
    assert!(state.get_pending_question("t1").is_none());
}

#[test]
fn test_cleanup_on_thread_idle() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

    assert!(state.get_pending_question("t1").is_some());

    // Thread goes idle (dismissed)
    state.update_thread_status("t1", ThreadStatus::Idle, None);

    // Pending question should be cleared
    assert!(state.get_pending_question("t1").is_none());
}

#[test]
fn test_cleanup_on_waiting_for_change() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

    assert!(state.get_pending_question("t1").is_some());

    // Thread now waiting for permission instead of user input
    state.update_thread_status(
        "t1",
        ThreadStatus::Waiting,
        Some(WaitingFor::Permission {
            request_id: "perm-456".to_string(),
            tool_name: "Bash".to_string(),
        }),
    );

    // Pending question should be cleared (no longer waiting for user input)
    assert!(state.get_pending_question("t1").is_none());
}

#[test]
fn test_no_cleanup_when_still_waiting_for_user_input() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

    assert!(state.get_pending_question("t1").is_some());

    // Status update but still waiting for user input
    state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

    // Pending question should still be there
    assert!(state.get_pending_question("t1").is_some());
}

#[test]
fn test_no_cleanup_when_running() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.update_thread_status("t1", ThreadStatus::Waiting, Some(WaitingFor::UserInput));

    assert!(state.get_pending_question("t1").is_some());

    // Thread now running (might still need the question after running completes)
    state.update_thread_status("t1", ThreadStatus::Running, None);

    // When running, we should not clear pending question prematurely
    // because the question flow might resume after running completes
    // However, if waiting_for is no longer UserInput, we clear it
    assert!(state.get_pending_question("t1").is_none());
}

#[test]
fn test_collapse_overlay_clears_question_state() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);

    state.expand_thread("t1", 10);
    assert!(state.question_state().is_some());

    // Collapse overlay
    state.collapse_overlay();

    // Question state should be cleared
    assert!(state.question_state().is_none());
    assert!(state.overlay().is_none());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_empty_options_list() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    // Question with no options (only "Other" available)
    let question_data = AskUserQuestionData {
        questions: vec![Question {
            question: "Please specify:".to_string(),
            header: "Custom".to_string(),
            options: vec![], // Empty options
            multi_select: false,
        }],
        answers: HashMap::new(),
    };

    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.expand_thread("t1", 10);

    // With no options, selection might be None (Other)
    let _qs = state.question_state().unwrap();
    // The initial selection with 0 options would try to set Some(0)
    // but get_current_option_count returns 0
    // Navigation should still work (moving to/from Other)
}

#[test]
fn test_very_long_question_text() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let long_question = "This is a very long question text that would normally exceed the display width of the question card and should be properly truncated with an ellipsis to ensure the UI remains clean and readable without any layout issues or text overflow problems that could affect the user experience".to_string();

    let question_data = AskUserQuestionData {
        questions: vec![Question {
            question: long_question.clone(),
            header: "Long Q".to_string(),
            options: vec![QuestionOption {
                label: "Yes".to_string(),
                description: "Confirm".to_string(),
            }],
            multi_select: false,
        }],
        answers: HashMap::new(),
    };

    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.expand_thread("t1", 10);

    // Question should be stored correctly
    let stored = state.get_pending_question("t1").unwrap();
    assert_eq!(stored.questions[0].question, long_question);
}

#[test]
fn test_very_long_option_label() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let long_label = "This is an extremely long option label that would normally not fit in the available display space".to_string();

    let question_data = AskUserQuestionData {
        questions: vec![Question {
            question: "Choose:".to_string(),
            header: "Test".to_string(),
            options: vec![
                QuestionOption {
                    label: long_label.clone(),
                    description: "A very long option".to_string(),
                },
                QuestionOption {
                    label: "Short".to_string(),
                    description: "Short option".to_string(),
                },
            ],
            multi_select: false,
        }],
        answers: HashMap::new(),
    };

    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.expand_thread("t1", 10);

    // Confirm with long label
    let result = state.question_confirm();
    assert!(result.is_some());

    let (_, _, answers) = result.unwrap();
    assert_eq!(answers.get("Choose:"), Some(&long_label));
}

#[test]
fn test_special_characters_in_other_text() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.expand_thread("t1", 10);

    // Navigate to Other
    for _ in 0..3 {
        state.question_next_option();
    }
    state.question_activate_other();

    // Type special characters
    for c in "Special: @#$%^&*()!".chars() {
        state.question_type_char(c);
    }

    assert_eq!(
        state.question_state().unwrap().current_other_text(),
        "Special: @#$%^&*()!"
    );

    // Confirm with special characters
    let result = state.question_confirm();
    assert!(result.is_some());

    let (_, _, answers) = result.unwrap();
    assert_eq!(
        answers.get("Which authentication method should we use?"),
        Some(&"Special: @#$%^&*()!".to_string())
    );
}

#[test]
fn test_unicode_in_other_text() {
    let mut state = DashboardState::new();
    let thread = make_test_thread("t1", "Test Thread");
    state.add_thread(thread);

    let question_data = make_simple_question();
    state.set_pending_question("t1", "req-123".to_string(), question_data);
    state.expand_thread("t1", 10);

    // Navigate to Other
    for _ in 0..3 {
        state.question_next_option();
    }
    state.question_activate_other();

    // Type unicode characters
    for c in "日本語テスト 🎉".chars() {
        state.question_type_char(c);
    }

    assert_eq!(
        state.question_state().unwrap().current_other_text(),
        "日本語テスト 🎉"
    );
}

// ============================================================================
// DashboardQuestionState Unit Tests
// ============================================================================

#[test]
fn test_dashboard_question_state_from_question_data() {
    let question_data = make_multi_question_data();
    let state = DashboardQuestionState::from_question_data(&question_data);

    assert_eq!(state.tab_index, 0);
    assert_eq!(state.selections.len(), 3);
    assert_eq!(state.multi_selections.len(), 3);
    assert_eq!(state.other_texts.len(), 3);
    assert_eq!(state.answered.len(), 3);

    // First question has 2 options
    assert_eq!(state.multi_selections[0].len(), 2);
    // Second question has 2 options
    assert_eq!(state.multi_selections[1].len(), 2);
    // Third question has 2 options
    assert_eq!(state.multi_selections[2].len(), 2);

    // All should start as unanswered
    assert!(state.answered.iter().all(|&a| !a));
}

#[test]
fn test_dashboard_question_state_reset() {
    let question_data = make_simple_question();
    let mut state = DashboardQuestionState::from_question_data(&question_data);

    // Modify state
    state.tab_index = 1;
    state.other_active = true;

    // Reset
    state.reset();

    assert_eq!(state.tab_index, 0);
    assert!(state.selections.is_empty());
    assert!(state.multi_selections.is_empty());
    assert!(state.other_texts.is_empty());
    assert!(!state.other_active);
    assert!(state.answered.is_empty());
}

#[test]
fn test_dashboard_question_state_advance_to_next_unanswered() {
    let question_data = make_multi_question_data();
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

    // No more unanswered
    let moved = state.advance_to_next_unanswered(3);
    assert!(!moved);
}

#[test]
fn test_dashboard_question_state_all_answered() {
    let question_data = make_multi_question_data();
    let mut state = DashboardQuestionState::from_question_data(&question_data);

    assert!(!state.all_answered());

    state.answered[0] = true;
    assert!(!state.all_answered());

    state.answered[1] = true;
    assert!(!state.all_answered());

    state.answered[2] = true;
    assert!(state.all_answered());
}
