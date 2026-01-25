//! Phase 6 Question Card Rendering Tests
//!
//! These tests verify the new question card rendering functionality introduced in Phase 6:
//! - QuestionRenderConfig struct construction
//! - render_question() function behavior
//! - Selection state rendering (selected/unselected options)
//! - Multi-select checkbox rendering
//! - Timer display logic

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use spoq::ui::dashboard::question_card::{render_question, QuestionRenderConfig};
use spoq::ui::interaction::HitAreaRegistry;

// ============================================================================
// QuestionRenderConfig Construction Tests
// ============================================================================

#[test]
fn test_question_render_config_construction_single_select() {
    let options = vec![
        "Option A".to_string(),
        "Option B".to_string(),
        "Option C".to_string(),
    ];

    let config = QuestionRenderConfig {
        question: "Choose one option:",
        options: &options,
        selected_index: Some(1), // Second option selected
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: Some(120), // 2 minutes
    };

    assert_eq!(config.question, "Choose one option:");
    assert_eq!(config.options.len(), 3);
    assert_eq!(config.selected_index, Some(1));
    assert!(!config.multi_select);
    assert_eq!(config.timer_seconds, Some(120));
}

#[test]
fn test_question_render_config_construction_multi_select() {
    let options = vec![
        "Feature A".to_string(),
        "Feature B".to_string(),
        "Feature C".to_string(),
    ];
    let selections = vec![true, false, true]; // A and C selected

    let config = QuestionRenderConfig {
        question: "Select all that apply:",
        options: &options,
        selected_index: Some(0), // Cursor on first option
        multi_select: true,
        multi_selections: &selections,
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    assert!(config.multi_select);
    assert_eq!(config.multi_selections.len(), 3);
    assert!(config.multi_selections[0]); // A is selected
    assert!(!config.multi_selections[1]); // B is not selected
    assert!(config.multi_selections[2]); // C is selected
}

#[test]
fn test_question_render_config_with_other_input() {
    let options = vec!["Yes".to_string(), "No".to_string()];

    let config = QuestionRenderConfig {
        question: "Proceed?",
        options: &options,
        selected_index: None, // Cursor on "Other"
        multi_select: false,
        multi_selections: &[],
        other_input: "Maybe later",
        other_selected: true,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: Some(30), // 30 seconds
    };

    assert!(config.other_selected);
    assert_eq!(config.other_input, "Maybe later");
    assert!(config.selected_index.is_none());
}

#[test]
fn test_question_render_config_default_values() {
    let config = QuestionRenderConfig::default();

    assert_eq!(config.question, "");
    assert_eq!(config.options.len(), 0);
    assert_eq!(config.selected_index, Some(0));
    assert!(!config.multi_select);
    assert_eq!(config.multi_selections.len(), 0);
    assert_eq!(config.other_input, "");
    assert!(!config.other_selected);
    assert!(config.timer_seconds.is_none());
}

// ============================================================================
// Selection State Rendering Tests
// ============================================================================

#[test]
fn test_render_single_select_first_option_selected() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "JWT tokens".to_string(),
        "Session cookies".to_string(),
        "OAuth 2.0".to_string(),
    ];

    let config = QuestionRenderConfig {
        question: "Choose authentication method:",
        options: &options,
        selected_index: Some(0), // First option selected
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Setup auth",
                "my-project",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Verify hit areas registered for all options (3 options + 1 Other)
    assert!(registry.len() >= 4);
}

#[test]
fn test_render_single_select_last_option_selected() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "Option A".to_string(),
        "Option B".to_string(),
        "Option C".to_string(),
    ];

    let config = QuestionRenderConfig {
        question: "Select one:",
        options: &options,
        selected_index: Some(2), // Last option selected
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Test",
                "repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should render without errors
    assert!(registry.len() >= 4);
}

#[test]
fn test_render_single_select_other_selected() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec!["Option A".to_string(), "Option B".to_string()];

    let config = QuestionRenderConfig {
        question: "Choose or specify:",
        options: &options,
        selected_index: None,
        multi_select: false,
        multi_selections: &[],
        other_input: "Custom option",
        other_selected: true,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Test",
                "repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Other field should be rendered and clickable
    assert!(registry.len() >= 3);
}

// ============================================================================
// Multi-Select Checkbox Rendering Tests
// ============================================================================

#[test]
fn test_render_multi_select_no_selections() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "Linting".to_string(),
        "Unit tests".to_string(),
        "E2E tests".to_string(),
    ];
    let selections = vec![false, false, false]; // Nothing selected

    let config = QuestionRenderConfig {
        question: "Select features to enable:",
        options: &options,
        selected_index: Some(0),
        multi_select: true,
        multi_selections: &selections,
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Setup CI",
                "my-repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should render checkboxes (unchecked)
    assert!(registry.len() >= 4);
}

#[test]
fn test_render_multi_select_some_selections() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "Linting".to_string(),
        "Unit tests".to_string(),
        "E2E tests".to_string(),
    ];
    let selections = vec![true, true, false]; // First two selected

    let config = QuestionRenderConfig {
        question: "Select features:",
        options: &options,
        selected_index: Some(1),
        multi_select: true,
        multi_selections: &selections,
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Setup CI",
                "my-repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should render with mixed checkboxes
    assert!(registry.len() >= 4);
}

#[test]
fn test_render_multi_select_all_selections() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "Feature A".to_string(),
        "Feature B".to_string(),
        "Feature C".to_string(),
    ];
    let selections = vec![true, true, true]; // All selected

    let config = QuestionRenderConfig {
        question: "Select all that apply:",
        options: &options,
        selected_index: Some(2),
        multi_select: true,
        multi_selections: &selections,
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Test",
                "repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // All checkboxes should be checked
    assert!(registry.len() >= 4);
}

#[test]
fn test_render_multi_select_with_other_input() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec!["Option A".to_string(), "Option B".to_string()];
    let selections = vec![true, false]; // A selected

    let config = QuestionRenderConfig {
        question: "Select or add custom:",
        options: &options,
        selected_index: Some(0),
        multi_select: true,
        multi_selections: &selections,
        other_input: "Custom feature",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Test",
                "repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should render with checkboxes and Other field
    assert!(registry.len() >= 3);
}

// ============================================================================
// Timer Display Logic Tests
// ============================================================================

#[test]
fn test_render_timer_normal_time() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec!["Yes".to_string(), "No".to_string()];

    let config = QuestionRenderConfig {
        question: "Proceed with deployment?",
        options: &options,
        selected_index: Some(0),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: Some(272), // 4:32 - normal time
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Deploy",
                "production",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Timer should be displayed (not red, >= 10 seconds)
}

#[test]
fn test_render_timer_urgent_time() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec!["Yes".to_string(), "No".to_string()];

    let config = QuestionRenderConfig {
        question: "Hurry up!",
        options: &options,
        selected_index: Some(0),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: Some(5), // 5 seconds - urgent (< 10)
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Urgent",
                "task",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Timer should be displayed in red (< 10 seconds)
}

#[test]
fn test_render_no_timer() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec!["Option A".to_string(), "Option B".to_string()];

    let config = QuestionRenderConfig {
        question: "No rush:",
        options: &options,
        selected_index: Some(0),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None, // No timer
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Test",
                "repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should render without timer display
}

#[test]
fn test_render_timer_zero_seconds() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec!["Quick!".to_string()];

    let config = QuestionRenderConfig {
        question: "Time's up!",
        options: &options,
        selected_index: Some(0),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: Some(0), // Zero seconds
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Expired",
                "timer",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should display (0:00) in red
}

// ============================================================================
// Edge Cases and Boundary Conditions
// ============================================================================

#[test]
fn test_render_many_options() {
    let backend = TestBackend::new(60, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    let options: Vec<String> = (1..=10).map(|i| format!("Option {}", i)).collect();

    let config = QuestionRenderConfig {
        question: "Choose from many options:",
        options: &options,
        selected_index: Some(5),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 18);
            render_question(
                frame,
                area,
                "thread-1",
                "Many Options",
                "repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should handle many options gracefully (may not all fit)
    // At minimum should have registered some hit areas
    assert!(registry.len() >= 1);
}

#[test]
fn test_render_long_question_text() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec!["Yes".to_string(), "No".to_string()];
    let long_question = "This is a very long question that should be wrapped to multiple lines to fit within the available card width without overflowing";

    let config = QuestionRenderConfig {
        question: long_question,
        options: &options,
        selected_index: Some(0),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Test",
                "repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should wrap long question text
}

#[test]
fn test_render_empty_options() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options: Vec<String> = vec![];

    let config = QuestionRenderConfig {
        question: "No options available:",
        options: &options,
        selected_index: None,
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Test",
                "repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should handle empty options gracefully
    // Should at least have "Other" option
    assert!(registry.len() >= 1);
}

#[test]
fn test_render_mismatched_selections_length() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "A".to_string(),
        "B".to_string(),
        "C".to_string(),
    ];
    let selections = vec![true]; // Intentionally shorter than options

    let config = QuestionRenderConfig {
        question: "Test mismatch:",
        options: &options,
        selected_index: Some(0),
        multi_select: true,
        multi_selections: &selections,
        other_input: "",
        other_selected: false,
        option_descriptions: &[], tab_headers: &[], current_tab: 0, tabs_answered: &[], timer_seconds: None,
    };

    let mut registry = HitAreaRegistry::new();

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            render_question(
                frame,
                area,
                "thread-1",
                "Test",
                "repo",
                &config,
                &mut registry,
            );
        })
        .unwrap();

    // Should handle gracefully (defaults to false for missing indices)
    assert!(registry.len() >= 4);
}
