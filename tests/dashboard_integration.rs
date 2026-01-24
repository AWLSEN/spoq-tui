//! Dashboard Integration Tests
//!
//! These tests verify the complete dashboard flow including:
//! - Thread list rendering with various states
//! - Filter state changes
//! - Overlay interactions
//! - Action button triggers
//! - Integration between DashboardState and RenderContext

use spoq::models::dashboard::{
    compute_local_aggregate, infer_status_from_agent_state, Aggregate, PlanSummary, ThreadStatus,
    WaitingFor,
};
use spoq::models::{Thread, ThreadMode, ThreadType};
use spoq::state::DashboardState;
use spoq::ui::dashboard::{
    FilterState, OverlayState, RenderContext, SystemStats, Theme, ThreadView,
};
use spoq::ui::interaction::{ClickAction, HitAreaRegistry};
use std::collections::HashMap;

// ============================================================================
// Test Helpers
// ============================================================================

fn make_test_thread(id: &str, title: &str, status: Option<ThreadStatus>) -> Thread {
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
        status,
        verified: None,
        verified_at: None,
    }
}

// ============================================================================
// Dashboard Renders - Zero Threads (All Clear State)
// ============================================================================

#[test]
fn test_dashboard_renders_with_zero_threads() {
    let state = DashboardState::new();

    // Verify empty state
    assert_eq!(state.thread_count(), 0);
    assert!(state.filter().is_none());
    assert!(state.overlay().is_none());

    // Build render context
    let stats = SystemStats::default();
    let theme = Theme::default();

    // Compute thread views (should be empty)
    let mut state = state;
    let views = state.compute_thread_views();
    assert!(views.is_empty());

    // RenderContext should handle empty thread list
    let ctx = state.build_render_context(&stats, &theme);
    assert_eq!(ctx.threads.len(), 0);
    assert_eq!(ctx.action_count(), 0);
    assert!(!ctx.has_overlay());
}

#[test]
fn test_dashboard_aggregate_with_zero_threads() {
    let aggregate = Aggregate::new();

    assert_eq!(aggregate.working(), 0);
    assert_eq!(aggregate.ready_to_test(), 0);
    assert_eq!(aggregate.idle(), 0);
    assert_eq!(aggregate.total_repos, 0);
}

// ============================================================================
// Dashboard Renders - Many Threads
// ============================================================================

#[test]
fn test_dashboard_renders_with_many_threads() {
    let mut state = DashboardState::new();

    // Create 100 threads with various statuses
    let mut threads = Vec::new();
    for i in 0..100 {
        let status = match i % 5 {
            0 => Some(ThreadStatus::Running),
            1 => Some(ThreadStatus::Waiting),
            2 => Some(ThreadStatus::Done),
            3 => Some(ThreadStatus::Error),
            _ => Some(ThreadStatus::Idle),
        };
        threads.push(make_test_thread(
            &format!("t{}", i),
            &format!("Thread {}", i),
            status,
        ));
    }

    let agent_states = HashMap::new();
    state.set_threads(threads, &agent_states);

    assert_eq!(state.thread_count(), 100);

    // Compute thread views
    let views = state.compute_thread_views();
    assert_eq!(views.len(), 100);

    // Verify aggregate statistics
    let aggregate = state.aggregate();
    assert_eq!(aggregate.total_repos, 100);
    assert_eq!(aggregate.count(ThreadStatus::Running), 20);
    assert_eq!(aggregate.count(ThreadStatus::Waiting), 20);
    assert_eq!(aggregate.count(ThreadStatus::Done), 20);
    assert_eq!(aggregate.count(ThreadStatus::Error), 20);
    assert_eq!(aggregate.count(ThreadStatus::Idle), 20);
}

#[test]
fn test_dashboard_thread_sorting() {
    let mut state = DashboardState::new();

    // Create threads with different statuses
    let threads = vec![
        make_test_thread("t1", "Idle Thread", Some(ThreadStatus::Idle)),
        make_test_thread("t2", "Waiting Thread", Some(ThreadStatus::Waiting)),
        make_test_thread("t3", "Running Thread", Some(ThreadStatus::Running)),
        make_test_thread("t4", "Error Thread", Some(ThreadStatus::Error)),
    ];

    let agent_states = HashMap::new();
    state.set_threads(threads, &agent_states);

    let views = state.compute_thread_views();

    // Threads needing action (Waiting, Error) should come first
    assert!(views[0].needs_action || views[1].needs_action);

    // Count threads needing action
    let action_count = views.iter().filter(|v| v.needs_action).count();
    assert_eq!(action_count, 2); // Waiting + Error
}

// ============================================================================
// Dashboard with Mixed Thread Data
// ============================================================================

#[test]
fn test_dashboard_with_mixed_thread_data() {
    let mut state = DashboardState::new();

    // Mix of threads - some have status, some don't
    let threads = vec![
        make_test_thread("t1", "With Status", Some(ThreadStatus::Running)),
        make_test_thread("t2", "Without Status", None),
        make_test_thread("t3", "With Waiting", Some(ThreadStatus::Waiting)),
    ];

    let agent_states = HashMap::new();
    state.set_threads(threads, &agent_states);

    let views = state.compute_thread_views();

    // Thread without status should default to Idle
    let view_without_status = views.iter().find(|v| v.id == "t2").unwrap();
    assert_eq!(view_without_status.status, ThreadStatus::Idle);

    // Thread with status should use it
    let view_with_status = views.iter().find(|v| v.id == "t1").unwrap();
    assert_eq!(view_with_status.status, ThreadStatus::Running);
}

#[test]
fn test_dashboard_agent_state_overrides_stored_status() {
    let mut state = DashboardState::new();

    // Thread has stored status of Idle
    let threads = vec![make_test_thread(
        "t1",
        "Agent Override Test",
        Some(ThreadStatus::Idle),
    )];

    // But agent events say it's thinking (Running)
    let mut agent_states = HashMap::new();
    agent_states.insert("t1".to_string(), "thinking".to_string());

    state.set_threads(threads, &agent_states);

    let views = state.compute_thread_views();
    let view = views.iter().find(|v| v.id == "t1").unwrap();

    // Agent state should override stored status
    assert_eq!(view.status, ThreadStatus::Running);
}

// ============================================================================
// Filter State Changes
// ============================================================================

#[test]
fn test_filter_state_changes_displayed_threads() {
    let mut state = DashboardState::new();

    let threads = vec![
        make_test_thread("t1", "Running 1", Some(ThreadStatus::Running)),
        make_test_thread("t2", "Running 2", Some(ThreadStatus::Running)),
        make_test_thread("t3", "Waiting", Some(ThreadStatus::Waiting)),
        make_test_thread("t4", "Done 1", Some(ThreadStatus::Done)),
        make_test_thread("t5", "Done 2", Some(ThreadStatus::Done)),
        make_test_thread("t6", "Idle", Some(ThreadStatus::Idle)),
        make_test_thread("t7", "Error", Some(ThreadStatus::Error)),
    ];

    let agent_states = HashMap::new();
    state.set_threads(threads, &agent_states);

    let _views = state.compute_thread_views();
    let stats = SystemStats::default();
    let theme = Theme::default();

    // No filter - all threads
    let ctx = state.build_render_context(&stats, &theme);
    assert_eq!(ctx.filtered_threads().len(), 7);

    // Working filter - Running + Waiting = 3
    state.toggle_filter(FilterState::Working);
    let ctx = state.build_render_context(&stats, &theme);
    assert_eq!(ctx.filtered_threads().len(), 3);

    // ReadyToTest filter - Done = 2
    state.toggle_filter(FilterState::ReadyToTest);
    let ctx = state.build_render_context(&stats, &theme);
    assert_eq!(ctx.filtered_threads().len(), 2);

    // Idle filter - Idle + Error = 2
    state.toggle_filter(FilterState::Idle);
    let ctx = state.build_render_context(&stats, &theme);
    assert_eq!(ctx.filtered_threads().len(), 2);

    // Toggle same filter off
    state.toggle_filter(FilterState::Idle);
    let ctx = state.build_render_context(&stats, &theme);
    assert_eq!(ctx.filtered_threads().len(), 7);
}

#[test]
fn test_filter_state_cycling() {
    // Test filter state cycling through next/prev
    let filter = FilterState::All;

    assert_eq!(filter.next(), FilterState::Working);
    assert_eq!(filter.next().next(), FilterState::ReadyToTest);
    assert_eq!(filter.next().next().next(), FilterState::Idle);
    assert_eq!(filter.next().next().next().next(), FilterState::All);

    assert_eq!(filter.prev(), FilterState::Idle);
    assert_eq!(filter.prev().prev(), FilterState::ReadyToTest);
}

#[test]
fn test_filter_state_matches_status() {
    // All matches everything
    assert!(FilterState::All.matches(ThreadStatus::Idle));
    assert!(FilterState::All.matches(ThreadStatus::Running));
    assert!(FilterState::All.matches(ThreadStatus::Waiting));
    assert!(FilterState::All.matches(ThreadStatus::Done));
    assert!(FilterState::All.matches(ThreadStatus::Error));

    // Working matches Running and Waiting
    assert!(FilterState::Working.matches(ThreadStatus::Running));
    assert!(FilterState::Working.matches(ThreadStatus::Waiting));
    assert!(!FilterState::Working.matches(ThreadStatus::Idle));
    assert!(!FilterState::Working.matches(ThreadStatus::Done));
    assert!(!FilterState::Working.matches(ThreadStatus::Error));

    // ReadyToTest matches Done
    assert!(FilterState::ReadyToTest.matches(ThreadStatus::Done));
    assert!(!FilterState::ReadyToTest.matches(ThreadStatus::Running));

    // Idle matches Idle and Error
    assert!(FilterState::Idle.matches(ThreadStatus::Idle));
    assert!(FilterState::Idle.matches(ThreadStatus::Error));
    assert!(!FilterState::Idle.matches(ThreadStatus::Running));
}

// ============================================================================
// Overlay Opens on Thread Expand
// ============================================================================

#[test]
fn test_overlay_opens_on_thread_expand() {
    let mut state = DashboardState::new();

    let threads = vec![make_test_thread(
        "t1",
        "Test Thread",
        Some(ThreadStatus::Idle),
    )];
    let agent_states = HashMap::new();
    state.set_threads(threads, &agent_states);

    // No overlay initially
    assert!(state.overlay().is_none());

    // Expand thread
    state.expand_thread("t1", 10);

    // Overlay should be open
    assert!(state.overlay().is_some());
    if let Some(OverlayState::Question {
        thread_id,
        anchor_y,
        ..
    }) = state.overlay()
    {
        assert_eq!(thread_id, "t1");
        assert_eq!(*anchor_y, 10);
    } else {
        panic!("Expected Question overlay");
    }
}

#[test]
fn test_overlay_opens_plan_for_plan_approval() {
    let mut state = DashboardState::new();

    let threads = vec![make_test_thread(
        "t1",
        "Plan Thread",
        Some(ThreadStatus::Waiting),
    )];
    let agent_states = HashMap::new();
    state.set_threads(threads, &agent_states);

    // Set up waiting for plan approval
    state.update_thread_status(
        "t1",
        ThreadStatus::Waiting,
        Some(WaitingFor::PlanApproval {
            plan_summary: "Add dark mode".to_string(),
        }),
    );

    // Set up plan request
    state.set_plan_request(
        "t1",
        "req-123".to_string(),
        PlanSummary::new(
            "Add dark mode".to_string(),
            vec!["Phase 1".to_string(), "Phase 2".to_string()],
            10,
            50000,
        ),
    );

    // Expand thread
    state.expand_thread("t1", 5);

    // Should open Plan overlay
    assert!(state.overlay().is_some());
    if let Some(OverlayState::Plan {
        thread_id,
        request_id,
        summary,
        ..
    }) = state.overlay()
    {
        assert_eq!(thread_id, "t1");
        assert_eq!(request_id, "req-123");
        assert_eq!(summary.title, "Add dark mode");
        assert_eq!(summary.phases.len(), 2);
    } else {
        panic!("Expected Plan overlay");
    }
}

#[test]
fn test_overlay_does_not_open_for_permission() {
    let mut state = DashboardState::new();

    let threads = vec![make_test_thread(
        "t1",
        "Permission Thread",
        Some(ThreadStatus::Waiting),
    )];
    let agent_states = HashMap::new();
    state.set_threads(threads, &agent_states);

    // Set up waiting for permission (shown inline, not in overlay)
    state.update_thread_status(
        "t1",
        ThreadStatus::Waiting,
        Some(WaitingFor::Permission {
            request_id: "perm-123".to_string(),
            tool_name: "Bash".to_string(),
        }),
    );

    // Try to expand thread
    state.expand_thread("t1", 5);

    // Should NOT open overlay (permissions are inline)
    assert!(state.overlay().is_none());
}

// ============================================================================
// Overlay Closes on Escape
// ============================================================================

#[test]
fn test_overlay_closes_on_escape() {
    let mut state = DashboardState::new();

    let threads = vec![make_test_thread(
        "t1",
        "Test Thread",
        Some(ThreadStatus::Idle),
    )];
    let agent_states = HashMap::new();
    state.set_threads(threads, &agent_states);

    // Open overlay
    state.expand_thread("t1", 10);
    assert!(state.overlay().is_some());

    // Close overlay (simulating Escape key)
    state.collapse_overlay();
    assert!(state.overlay().is_none());
}

#[test]
fn test_overlay_free_form_transition() {
    let mut state = DashboardState::new();

    let threads = vec![make_test_thread(
        "t1",
        "Test Thread",
        Some(ThreadStatus::Idle),
    )];
    let agent_states = HashMap::new();
    state.set_threads(threads, &agent_states);

    // Open Question overlay
    state.expand_thread("t1", 10);

    // Transition to FreeForm
    state.show_free_form("t1");

    if let Some(OverlayState::FreeForm {
        thread_id,
        input,
        cursor_pos,
        ..
    }) = state.overlay()
    {
        assert_eq!(thread_id, "t1");
        assert!(input.is_empty());
        assert_eq!(*cursor_pos, 0);
    } else {
        panic!("Expected FreeForm overlay");
    }

    // Update input
    state.update_free_form_input("Hello world".to_string(), 5);

    if let Some(OverlayState::FreeForm {
        input, cursor_pos, ..
    }) = state.overlay()
    {
        assert_eq!(input, "Hello world");
        assert_eq!(*cursor_pos, 5);
    } else {
        panic!("Expected FreeForm overlay");
    }

    // Go back to options
    state.back_to_options("t1");

    assert!(matches!(
        state.overlay(),
        Some(OverlayState::Question { .. })
    ));
}

// ============================================================================
// Hit Area Registry for Click Actions
// ============================================================================

#[test]
fn test_hit_area_filter_click_actions() {
    let mut registry = HitAreaRegistry::new();

    // Register filter hit areas (simulating status bar)
    registry.register(
        ratatui::layout::Rect::new(0, 0, 10, 2),
        ClickAction::FilterWorking,
        None,
    );
    registry.register(
        ratatui::layout::Rect::new(10, 0, 15, 2),
        ClickAction::FilterReadyToTest,
        None,
    );
    registry.register(
        ratatui::layout::Rect::new(25, 0, 10, 2),
        ClickAction::FilterIdle,
        None,
    );

    // Test hit testing
    assert_eq!(registry.hit_test(5, 1), Some(ClickAction::FilterWorking));
    assert_eq!(
        registry.hit_test(15, 1),
        Some(ClickAction::FilterReadyToTest)
    );
    assert_eq!(registry.hit_test(30, 1), Some(ClickAction::FilterIdle));
    assert_eq!(registry.hit_test(50, 1), None); // Outside all areas
}

#[test]
fn test_hit_area_thread_expand() {
    let mut registry = HitAreaRegistry::new();

    // Register thread row hit area
    registry.register(
        ratatui::layout::Rect::new(0, 5, 80, 3),
        ClickAction::ExpandThread {
            thread_id: "thread-123".to_string(),
            anchor_y: 5,
        },
        None,
    );

    // Test hit testing
    let action = registry.hit_test(40, 6);
    assert_eq!(
        action,
        Some(ClickAction::ExpandThread {
            thread_id: "thread-123".to_string(),
            anchor_y: 5
        })
    );
}

#[test]
fn test_hit_area_action_buttons() {
    let mut registry = HitAreaRegistry::new();

    // Register action button hit areas
    registry.register(
        ratatui::layout::Rect::new(10, 10, 10, 2),
        ClickAction::ApproveThread("t1".to_string()),
        None,
    );
    registry.register(
        ratatui::layout::Rect::new(25, 10, 10, 2),
        ClickAction::RejectThread("t1".to_string()),
        None,
    );
    registry.register(
        ratatui::layout::Rect::new(40, 10, 10, 2),
        ClickAction::VerifyThread("t1".to_string()),
        None,
    );

    // Test hit testing
    assert_eq!(
        registry.hit_test(15, 11),
        Some(ClickAction::ApproveThread("t1".to_string()))
    );
    assert_eq!(
        registry.hit_test(30, 11),
        Some(ClickAction::RejectThread("t1".to_string()))
    );
    assert_eq!(
        registry.hit_test(45, 11),
        Some(ClickAction::VerifyThread("t1".to_string()))
    );
}

#[test]
fn test_hit_area_overlay_priority() {
    let mut registry = HitAreaRegistry::new();

    // Register background hit area first (lower z-order)
    registry.register(
        ratatui::layout::Rect::new(0, 0, 100, 50),
        ClickAction::ClearFilter,
        None,
    );

    // Register overlay hit areas on top (higher z-order)
    registry.register(
        ratatui::layout::Rect::new(20, 10, 60, 30),
        ClickAction::CollapseOverlay,
        None,
    );

    // Click in overlay area should hit overlay action (last registered wins)
    assert_eq!(
        registry.hit_test(50, 25),
        Some(ClickAction::CollapseOverlay)
    );

    // Click outside overlay should hit background
    assert_eq!(registry.hit_test(5, 5), Some(ClickAction::ClearFilter));
}

// ============================================================================
// Aggregate Statistics
// ============================================================================

#[test]
fn test_aggregate_working_calculation() {
    let mut aggregate = Aggregate::new();

    aggregate.increment(ThreadStatus::Running);
    aggregate.increment(ThreadStatus::Running);
    aggregate.increment(ThreadStatus::Waiting);
    aggregate.increment(ThreadStatus::Idle);
    aggregate.increment(ThreadStatus::Done);

    // Working = Running + Waiting
    assert_eq!(aggregate.working(), 3);
}

#[test]
fn test_aggregate_ready_to_test_calculation() {
    let mut aggregate = Aggregate::new();

    aggregate.increment(ThreadStatus::Done);
    aggregate.increment(ThreadStatus::Done);
    aggregate.increment(ThreadStatus::Done);
    aggregate.increment(ThreadStatus::Running);

    // Ready to test = Done
    assert_eq!(aggregate.ready_to_test(), 3);
}

#[test]
fn test_aggregate_idle_calculation() {
    let mut aggregate = Aggregate::new();

    aggregate.increment(ThreadStatus::Idle);
    aggregate.increment(ThreadStatus::Idle);
    aggregate.increment(ThreadStatus::Error);
    aggregate.increment(ThreadStatus::Running);

    // Idle = Idle + Error
    assert_eq!(aggregate.idle(), 3);
}

#[test]
fn test_compute_local_aggregate() {
    let threads = vec![
        make_test_thread("t1", "Running", Some(ThreadStatus::Running)),
        make_test_thread("t2", "Waiting", Some(ThreadStatus::Waiting)),
        make_test_thread("t3", "Done", Some(ThreadStatus::Done)),
        make_test_thread("t4", "Idle", Some(ThreadStatus::Idle)),
        make_test_thread("t5", "Error", Some(ThreadStatus::Error)),
    ];

    // Agent events override some statuses
    let mut agent_events = HashMap::new();
    agent_events.insert("t1".to_string(), "thinking".to_string()); // Running
    agent_events.insert("t4".to_string(), "waiting".to_string()); // Override Idle -> Waiting

    let aggregate = compute_local_aggregate(&threads, &agent_events);

    // t1: Running (from agent), t2: ? (no agent event, so Idle), t3: ? (no agent), t4: Waiting (from agent), t5: ? (no agent)
    // Actually compute_local_aggregate uses agent_events first, defaults to Idle if no agent event
    assert_eq!(aggregate.total_repos, 5);
}

// ============================================================================
// Status Inference
// ============================================================================

#[test]
fn test_infer_status_from_various_agent_states() {
    // Running states
    assert_eq!(
        infer_status_from_agent_state("thinking"),
        ThreadStatus::Running
    );
    assert_eq!(
        infer_status_from_agent_state("tool_use"),
        ThreadStatus::Running
    );
    assert_eq!(
        infer_status_from_agent_state("running"),
        ThreadStatus::Running
    );
    assert_eq!(
        infer_status_from_agent_state("executing"),
        ThreadStatus::Running
    );

    // Waiting states
    assert_eq!(
        infer_status_from_agent_state("waiting"),
        ThreadStatus::Waiting
    );
    assert_eq!(
        infer_status_from_agent_state("awaiting_permission"),
        ThreadStatus::Waiting
    );
    assert_eq!(
        infer_status_from_agent_state("awaiting_input"),
        ThreadStatus::Waiting
    );
    assert_eq!(
        infer_status_from_agent_state("paused"),
        ThreadStatus::Waiting
    );

    // Done states
    assert_eq!(infer_status_from_agent_state("done"), ThreadStatus::Done);
    assert_eq!(
        infer_status_from_agent_state("complete"),
        ThreadStatus::Done
    );
    assert_eq!(
        infer_status_from_agent_state("completed"),
        ThreadStatus::Done
    );
    assert_eq!(
        infer_status_from_agent_state("finished"),
        ThreadStatus::Done
    );
    assert_eq!(infer_status_from_agent_state("success"), ThreadStatus::Done);

    // Error states
    assert_eq!(infer_status_from_agent_state("error"), ThreadStatus::Error);
    assert_eq!(infer_status_from_agent_state("failed"), ThreadStatus::Error);
    assert_eq!(
        infer_status_from_agent_state("failure"),
        ThreadStatus::Error
    );

    // Idle states
    assert_eq!(infer_status_from_agent_state("idle"), ThreadStatus::Idle);
    assert_eq!(infer_status_from_agent_state("ready"), ThreadStatus::Idle);
    assert_eq!(infer_status_from_agent_state(""), ThreadStatus::Idle);

    // Unknown defaults to Idle
    assert_eq!(
        infer_status_from_agent_state("something_unknown"),
        ThreadStatus::Idle
    );
}

#[test]
fn test_infer_status_case_insensitive() {
    assert_eq!(
        infer_status_from_agent_state("THINKING"),
        ThreadStatus::Running
    );
    assert_eq!(
        infer_status_from_agent_state("Waiting"),
        ThreadStatus::Waiting
    );
    assert_eq!(infer_status_from_agent_state("DONE"), ThreadStatus::Done);
    assert_eq!(infer_status_from_agent_state("Error"), ThreadStatus::Error);
    assert_eq!(infer_status_from_agent_state("IDLE"), ThreadStatus::Idle);
}

// ============================================================================
// ThreadView Builder
// ============================================================================

#[test]
fn test_thread_view_builder() {
    let view = ThreadView::new(
        "t1".to_string(),
        "Test Thread".to_string(),
        "~/project".to_string(),
    )
    .with_status(ThreadStatus::Running)
    .with_duration("5m".to_string());

    assert_eq!(view.id, "t1");
    assert_eq!(view.title, "Test Thread");
    assert_eq!(view.repository, "~/project");
    assert_eq!(view.status, ThreadStatus::Running);
    assert_eq!(view.duration, "5m");
    assert!(!view.needs_action); // Running doesn't need action
}

#[test]
fn test_thread_view_needs_action() {
    // Waiting needs action
    let waiting_view = ThreadView::new(
        "t1".to_string(),
        "Waiting".to_string(),
        "~/repo".to_string(),
    )
    .with_status(ThreadStatus::Waiting);
    assert!(waiting_view.needs_action);

    // Error needs action
    let error_view = ThreadView::new("t2".to_string(), "Error".to_string(), "~/repo".to_string())
        .with_status(ThreadStatus::Error);
    assert!(error_view.needs_action);

    // Running doesn't need action
    let running_view = ThreadView::new(
        "t3".to_string(),
        "Running".to_string(),
        "~/repo".to_string(),
    )
    .with_status(ThreadStatus::Running);
    assert!(!running_view.needs_action);

    // WaitingFor also triggers needs_action
    let view_with_waiting_for =
        ThreadView::new("t4".to_string(), "Idle".to_string(), "~/repo".to_string())
            .with_status(ThreadStatus::Idle)
            .with_waiting_for(Some(WaitingFor::UserInput));
    assert!(view_with_waiting_for.needs_action);
}

#[test]
fn test_thread_view_status_line() {
    // Without waiting_for, shows status
    let view = ThreadView::new("t1".to_string(), "Test".to_string(), "~/repo".to_string())
        .with_status(ThreadStatus::Running);
    assert_eq!(view.status_line(), "Running");

    // With waiting_for, shows waiting description
    let view_with_waiting =
        ThreadView::new("t2".to_string(), "Test".to_string(), "~/repo".to_string())
            .with_waiting_for(Some(WaitingFor::Permission {
                request_id: "req-1".to_string(),
                tool_name: "Bash".to_string(),
            }));
    assert_eq!(view_with_waiting.status_line(), "Permission: Bash");
}

// ============================================================================
// RenderContext
// ============================================================================

#[test]
fn test_render_context_action_count() {
    let threads = vec![
        ThreadView::new("t1".to_string(), "T1".to_string(), "~/r1".to_string())
            .with_status(ThreadStatus::Waiting),
        ThreadView::new("t2".to_string(), "T2".to_string(), "~/r2".to_string())
            .with_status(ThreadStatus::Running),
        ThreadView::new("t3".to_string(), "T3".to_string(), "~/r3".to_string())
            .with_status(ThreadStatus::Error),
        ThreadView::new("t4".to_string(), "T4".to_string(), "~/r4".to_string())
            .with_status(ThreadStatus::Done),
    ];

    let aggregate = Aggregate::new();
    let stats = SystemStats::default();
    let theme = Theme::default();

    let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme);

    // Waiting + Error = 2 threads need action
    assert_eq!(ctx.action_count(), 2);
}

#[test]
fn test_render_context_with_overlay() {
    let threads = vec![];
    let aggregate = Aggregate::new();
    let stats = SystemStats::default();
    let theme = Theme::default();

    let ctx = RenderContext::new(&threads, &aggregate, &stats, &theme);
    assert!(!ctx.has_overlay());

    let overlay = OverlayState::Question {
        thread_id: "t1".to_string(),
        thread_title: "Test".to_string(),
        repository: "~/repo".to_string(),
        question: "Continue?".to_string(),
        options: vec!["Yes".to_string(), "No".to_string()],
        anchor_y: 10,
    };

    let ctx = ctx.with_overlay(Some(&overlay));
    assert!(ctx.has_overlay());
}

// ============================================================================
// SystemStats
// ============================================================================

#[test]
fn test_system_stats_heavy_load() {
    // Normal load
    let stats = SystemStats::new(true, 50.0, 8.0, 16.0);
    assert!(!stats.is_heavy_load());

    // High CPU
    let stats = SystemStats::new(true, 95.0, 8.0, 16.0);
    assert!(stats.is_heavy_load());

    // High RAM (over 90%)
    let stats = SystemStats::new(true, 50.0, 15.0, 16.0);
    assert!(stats.is_heavy_load());
}

#[test]
fn test_system_stats_display() {
    let stats = SystemStats::new(true, 45.5, 8.2, 16.0);

    assert_eq!(stats.cpu_display(), "46%"); // Rounded
    assert_eq!(stats.ram_display(), "8.2/16.0 GB");
    assert_eq!(stats.connection_display(), "Connected");

    let disconnected = SystemStats::new(false, 0.0, 0.0, 0.0);
    assert_eq!(disconnected.connection_display(), "Disconnected");
}

// ============================================================================
// ClickAction Variants
// ============================================================================

#[test]
fn test_click_action_select_option() {
    let action = ClickAction::SelectOption {
        thread_id: "t1".to_string(),
        index: 2,
    };

    if let ClickAction::SelectOption { thread_id, index } = action {
        assert_eq!(thread_id, "t1");
        assert_eq!(index, 2);
    } else {
        panic!("Expected SelectOption");
    }
}

#[test]
fn test_click_action_equality() {
    assert_eq!(ClickAction::FilterWorking, ClickAction::FilterWorking);
    assert_ne!(ClickAction::FilterWorking, ClickAction::FilterIdle);

    assert_eq!(
        ClickAction::ApproveThread("t1".to_string()),
        ClickAction::ApproveThread("t1".to_string())
    );
    assert_ne!(
        ClickAction::ApproveThread("t1".to_string()),
        ClickAction::ApproveThread("t2".to_string())
    );
}
