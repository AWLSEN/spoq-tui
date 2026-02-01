//! Round 2 Dynamic Question Dialog Sizing Tests
//!
//! Tests for scroll rendering, auto-scroll on navigation, adaptive width,
//! and helper functions introduced in Round 2.
//!
//! Modified files:
//! - src/ui/dashboard/overlay.rs: adaptive width, extract helpers, scroll_offset/needs_scroll
//! - src/ui/dashboard/question_card.rs: dynamic wrapping, scroll indicators, scroll_offset
//! - src/state/dashboard.rs: auto-scroll in question_prev_option/question_next_option

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use spoq::models::dashboard::{ThreadStatus, WaitingFor};
use spoq::models::{Thread, ThreadMode, ThreadType};
use spoq::state::dashboard::DashboardState;
use spoq::state::session::{AskUserQuestionData, Question, QuestionOption};
use spoq::ui::dashboard::overlay;
use spoq::ui::dashboard::question_card::{self, QuestionRenderConfig};
use spoq::view_state::{OverlayState, RenderContext};
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

fn make_question_with_many_options(count: usize) -> AskUserQuestionData {
    let options: Vec<QuestionOption> = (0..count)
        .map(|i| QuestionOption {
            label: format!("Option {}", i + 1),
            description: format!("Description for option {}", i + 1),
        })
        .collect();

    AskUserQuestionData {
        questions: vec![Question {
            question: "Which option should I use?".to_string(),
            header: "Choice".to_string(),
            options,
            multi_select: false,
        }],
        answers: HashMap::new(),
    }
}

fn make_question_with_long_text() -> AskUserQuestionData {
    AskUserQuestionData {
        questions: vec![Question {
            question: "This is a very long question that should wrap to multiple lines when rendered in the card. It tests the dynamic wrapping behavior introduced in Round 2.".to_string(),
            header: "Long Question".to_string(),
            options: vec![
                QuestionOption {
                    label: "Short".to_string(),
                    description: "".to_string(),
                },
                QuestionOption {
                    label: "This is a very long option label that will need wrapping on narrow displays".to_string(),
                    description: "This is a very long description that will also need wrapping when displayed".to_string(),
                },
            ],
            multi_select: false,
        }],
        answers: HashMap::new(),
    }
}

fn setup_question_overlay(state: &mut DashboardState, thread_id: &str, question_data: AskUserQuestionData) {
    // Add thread
    let thread = make_test_thread(thread_id, "Test Thread");
    state.add_thread(thread);

    // Set waiting for UserInput
    state.update_thread_status(thread_id, ThreadStatus::Waiting, Some(WaitingFor::UserInput));

    // Add pending question
    state.set_pending_question(thread_id, "req-1".to_string(), question_data);

    // Expand thread to create overlay
    state.expand_thread(thread_id, 10);
}

fn get_scroll_offset(state: &DashboardState) -> Option<usize> {
    match state.overlay() {
        Some(OverlayState::Question { scroll_offset, .. }) => Some(*scroll_offset),
        _ => None,
    }
}

// ============================================================================
// Helper Functions (note: extract_question_* are private, tested indirectly)
// ============================================================================

// ============================================================================
// Calculate Height Tests (question_card.rs)
// ============================================================================

#[test]
fn test_calculate_height_simple() {
    let options = vec![
        ("Option A".to_string(), "".to_string()),
        ("Option B".to_string(), "".to_string()),
    ];
    let height = question_card::calculate_height("Short question?", &options, false, 50);

    // header(1) + blanks(3) + question(2 min) + options(2) + other(1) + help(1) = 10
    assert_eq!(height, 10);
}

#[test]
fn test_calculate_height_with_descriptions() {
    let options = vec![
        ("Option A".to_string(), "Description A".to_string()),
        ("Option B".to_string(), "Description B".to_string()),
    ];
    let height_with_desc = question_card::calculate_height("Question?", &options, false, 50);

    let options_no_desc = vec![
        ("Option A".to_string(), "".to_string()),
        ("Option B".to_string(), "".to_string()),
    ];
    let height_no_desc = question_card::calculate_height("Question?", &options_no_desc, false, 50);

    assert!(
        height_with_desc > height_no_desc,
        "Options with descriptions should be taller: {} vs {}",
        height_with_desc,
        height_no_desc
    );
}

#[test]
fn test_calculate_height_with_tabs() {
    let options = vec![("A".to_string(), "".to_string())];
    let height_no_tabs = question_card::calculate_height("Q?", &options, false, 50);
    let height_with_tabs = question_card::calculate_height("Q?", &options, true, 50);

    assert_eq!(height_with_tabs, height_no_tabs + 1);
}

#[test]
fn test_calculate_height_many_options() {
    let options: Vec<(String, String)> = (0..10)
        .map(|i| (format!("Option {}", i), format!("Description {}", i)))
        .collect();
    let height = question_card::calculate_height("Pick one:", &options, false, 50);

    assert!(
        height > 20,
        "10 options with descriptions should need many rows, got {}",
        height
    );
}

#[test]
fn test_calculate_height_long_question() {
    let long_q = "This is a very long question that should wrap to multiple lines when the available width is narrow enough to force wrapping behavior in the text rendering logic.";
    let options = vec![("A".to_string(), "".to_string())];

    let height_wide = question_card::calculate_height(long_q, &options, false, 200);
    let height_narrow = question_card::calculate_height(long_q, &options, false, 30);

    assert!(
        height_narrow > height_wide,
        "Narrow width should produce more rows: {} vs {}",
        height_narrow,
        height_wide
    );
}

// ============================================================================
// Scroll Indicator Tests
// ============================================================================

#[test]
fn test_render_with_scroll_indicators() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "Option 1".to_string(),
        "Option 2".to_string(),
        "Option 3".to_string(),
        "Option 4".to_string(),
        "Option 5".to_string(),
        "Option 6".to_string(),
        "Option 7".to_string(),
        "Option 8".to_string(),
    ];

    // Scroll offset of 2 means we've scrolled down (should show up arrow)
    let config = QuestionRenderConfig {
        question: "Select an option:",
        options: &options,
        option_descriptions: &[],
        selected_index: Some(3),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        timer_seconds: None,
        tab_headers: &[],
        current_tab: 0,
        tabs_answered: &[],
        scroll_offset: 2,
        needs_scroll: true,
    };

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            question_card::render_question(frame, area, "thread-1", "Test", "repo", &config);
        })
        .unwrap();

    // Should render without panic
}

#[test]
fn test_render_no_scroll_when_all_fit() {
    let backend = TestBackend::new(60, 25);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "Option 1".to_string(),
        "Option 2".to_string(),
        "Option 3".to_string(),
    ];

    // needs_scroll = false means all content fits
    let config = QuestionRenderConfig {
        question: "Select:",
        options: &options,
        option_descriptions: &[],
        selected_index: Some(0),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        timer_seconds: None,
        tab_headers: &[],
        current_tab: 0,
        tabs_answered: &[],
        scroll_offset: 0,
        needs_scroll: false,
    };

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 20);
            question_card::render_question(frame, area, "thread-1", "Test", "repo", &config);
        })
        .unwrap();

    // Should render without panic (no scroll indicators)
}

// ============================================================================
// Auto-Scroll Tests (dashboard.rs)
// ============================================================================

#[test]
fn test_question_next_option_auto_scrolls() {
    let mut state = DashboardState::new();
    let question_data = make_question_with_many_options(10);

    // Setup question overlay
    setup_question_overlay(&mut state, "thread-1", question_data);

    // Start at first option (scroll_offset should be 0)
    assert_eq!(get_scroll_offset(&state), Some(0));

    // Navigate down several times
    for _ in 0..7 {
        state.question_next_option();
    }

    // After navigating to option 7, scroll should have adjusted
    // (exact value depends on estimate_visible_option_count, which defaults to 6)
    let scroll = get_scroll_offset(&state);
    assert!(
        scroll.is_some(),
        "Scroll offset should be set after navigation"
    );
    assert!(
        scroll.unwrap() > 0,
        "Scroll offset should increase when navigating down past visible area"
    );
}

#[test]
fn test_question_prev_option_auto_scrolls() {
    let mut state = DashboardState::new();
    let question_data = make_question_with_many_options(10);

    setup_question_overlay(&mut state, "thread-1", question_data);

    // Navigate down to trigger scrolling
    for _ in 0..8 {
        state.question_next_option();
    }
    let scroll_after_down = get_scroll_offset(&state).unwrap();
    assert!(scroll_after_down > 0);

    // Navigate back up
    for _ in 0..8 {
        state.question_prev_option();
    }

    // Scroll should have decreased
    let scroll_after_up = get_scroll_offset(&state).unwrap();
    assert!(
        scroll_after_up < scroll_after_down,
        "Scroll should decrease when navigating up: {} vs {}",
        scroll_after_up,
        scroll_after_down
    );
}

#[test]
fn test_ensure_option_visible_keeps_selection_in_view() {
    let mut state = DashboardState::new();
    let question_data = make_question_with_many_options(20);

    setup_question_overlay(&mut state, "thread-1", question_data);

    // Directly set selection to option 15 (beyond visible area)
    if let Some(q_state) = state.question_state_mut() {
        q_state.set_current_selection(Some(15));
    }

    // Trigger ensure_option_visible via navigation
    state.ensure_option_visible(6); // visible_option_count = 6

    let scroll = get_scroll_offset(&state).unwrap();
    // With option 15 selected and 6 visible, scroll should be at least 10
    // (15 - 6 + 1 = 10)
    assert!(
        scroll >= 10,
        "Scroll offset should ensure option 15 is visible with 6 visible slots: got {}",
        scroll
    );
}

#[test]
fn test_scroll_offset_not_negative() {
    let mut state = DashboardState::new();
    let question_data = make_question_with_many_options(5);

    setup_question_overlay(&mut state, "thread-1", question_data);

    // Navigate to first option (should keep scroll at 0)
    state.question_prev_option();
    let scroll = get_scroll_offset(&state).unwrap();
    assert_eq!(scroll, 0, "Scroll should not go below 0");
}

// ============================================================================
// Adaptive Width Tests
// ============================================================================

#[test]
fn test_adaptive_width_for_question_overlay() {
    // This test verifies that Question overlays render correctly
    // (adaptive width logic is in overlay.rs but tested via rendering)
    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let question_data = make_question_with_many_options(3);
    let overlay = OverlayState::Question {
        thread_id: "t1".to_string(),
        thread_title: "Test".to_string(),
        repository: "~/repo".to_string(),
        question_data: Some(question_data),
        anchor_y: 10,
        scroll_offset: 0,
    };

    let threads = vec![];
    let aggregate = spoq::models::dashboard::Aggregate::default();
    let system_stats = spoq::view_state::SystemStats::default();
    let theme = spoq::view_state::Theme::default();
    let repos: Vec<spoq::models::GitHubRepo> = vec![];
    let ctx = RenderContext {
        threads: &threads,
        aggregate: &aggregate,
        overlay: Some(&overlay),
        system_stats: &system_stats,
        theme: &theme,
        question_state: None,
        question_timer_secs: None,
        repos: &repos,
    };

    terminal
        .draw(|frame| {
            let list_area = Rect::new(0, 0, 100, 40);
            overlay::render(frame, list_area, &overlay, &ctx);
        })
        .unwrap();

    // Should render without panic (adaptive width applied)
}

// ============================================================================
// Integration Tests with Rendering
// ============================================================================

#[test]
fn test_full_scroll_flow_with_rendering() {
    let mut state = DashboardState::new();
    let question_data = make_question_with_many_options(15);

    setup_question_overlay(&mut state, "thread-1", question_data);

    // Navigate down to middle
    for _ in 0..7 {
        state.question_next_option();
    }

    let scroll_mid = get_scroll_offset(&state).unwrap();

    // Navigate to near end
    for _ in 0..6 {
        state.question_next_option();
    }

    let scroll_end = get_scroll_offset(&state).unwrap();
    assert!(scroll_end > scroll_mid, "Scroll should increase");

    // Navigate back to start
    for _ in 0..13 {
        state.question_prev_option();
    }

    let scroll_start = get_scroll_offset(&state).unwrap();
    assert_eq!(scroll_start, 0, "Scroll should return to 0");
}

#[test]
fn test_render_with_dynamic_question_wrapping() {
    let backend = TestBackend::new(60, 25);
    let mut terminal = Terminal::new(backend).unwrap();

    let question_data = make_question_with_long_text();

    let options: Vec<String> = question_data.questions[0]
        .options
        .iter()
        .map(|o| o.label.clone())
        .collect();
    let descriptions: Vec<String> = question_data.questions[0]
        .options
        .iter()
        .map(|o| o.description.clone())
        .collect();

    let config = QuestionRenderConfig {
        question: &question_data.questions[0].question,
        options: &options,
        option_descriptions: &descriptions,
        selected_index: Some(0),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        timer_seconds: None,
        tab_headers: &[],
        current_tab: 0,
        tabs_answered: &[],
        scroll_offset: 0,
        needs_scroll: false,
    };

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 22);
            question_card::render_question(frame, area, "thread-1", "Test", "repo", &config);
        })
        .unwrap();

    // Should render without panic (dynamic wrapping applied)
}

#[test]
fn test_scroll_offset_skips_options_before_offset() {
    let backend = TestBackend::new(60, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let options = vec![
        "Option 1".to_string(),
        "Option 2".to_string(),
        "Option 3".to_string(),
        "Option 4".to_string(),
        "Option 5".to_string(),
    ];

    // With scroll_offset = 2, first two options should be skipped
    let config = QuestionRenderConfig {
        question: "Select:",
        options: &options,
        option_descriptions: &[],
        selected_index: Some(2),
        multi_select: false,
        multi_selections: &[],
        other_input: "",
        other_selected: false,
        timer_seconds: None,
        tab_headers: &[],
        current_tab: 0,
        tabs_answered: &[],
        scroll_offset: 2,
        needs_scroll: true,
    };

    terminal
        .draw(|frame| {
            let area = Rect::new(2, 1, 56, 12);
            question_card::render_question(frame, area, "thread-1", "Test", "repo", &config);
        })
        .unwrap();

    // Should render starting from Option 3 (index 2)
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_scroll_with_zero_visible_options() {
    let mut state = DashboardState::new();
    let question_data = make_question_with_many_options(3);

    setup_question_overlay(&mut state, "thread-1", question_data);

    // Call ensure_option_visible with 0 (should not panic)
    state.ensure_option_visible(0);

    // Should not crash
}

#[test]
fn test_calculate_height_zero_width() {
    let options = vec![("A".to_string(), "".to_string())];
    let height = question_card::calculate_height("Q?", &options, false, 0);

    // Should handle gracefully
    assert!(height >= 8, "Should return minimum height even with 0 width");
}

#[test]
fn test_calculate_height_empty_options() {
    let height = question_card::calculate_height("Question?", &[], false, 50);

    // header(1) + blanks(3) + question(2 min) + other(1) + help(1) = 8
    assert_eq!(height, 8);
}
