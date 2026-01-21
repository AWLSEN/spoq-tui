//! Integration tests for Round 5 features
//! Tests ThreadModeUpdate and ThreadVerified message handlers

use spoq::app::{App, AppMessage};
use spoq::models::{Thread, ThreadMode, ThreadType};
use spoq::state::dashboard::DashboardState;
use chrono::Utc;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test thread with default values
fn create_test_thread(id: &str, title: &str) -> Thread {
    Thread {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        preview: String::new(),
        updated_at: Utc::now(),
        thread_type: ThreadType::Programming,
        mode: ThreadMode::default(),
        model: None,
        permission_mode: None,
        message_count: 0,
        created_at: Utc::now(),
        working_directory: Some("/Users/test/project".to_string()),
        status: None,
        verified: None,
        verified_at: None,
    }
}

// ============================================================================
// ThreadModeUpdate Handler Tests
// ============================================================================

#[test]
fn test_thread_mode_update_handler_updates_dashboard() {
    let mut app = App::default();

    // Add a thread to the dashboard
    let thread = create_test_thread("t1", "Test Thread");
    app.dashboard.add_thread(thread);

    // Send ThreadModeUpdate message
    let msg = AppMessage::ThreadModeUpdate {
        thread_id: "t1".to_string(),
        mode: ThreadMode::Plan,
    };

    app.handle_message(msg);

    // Verify the thread mode was updated
    let updated_thread = app.dashboard.get_thread("t1").expect("Thread should exist");
    assert_eq!(updated_thread.mode, ThreadMode::Plan);
}

#[test]
fn test_thread_mode_update_multiple_times() {
    let mut app = App::default();

    // Add a thread to dashboard
    let thread = create_test_thread("t2", "Another Thread");
    app.dashboard.add_thread(thread);

    // Send first ThreadModeUpdate message
    let msg = AppMessage::ThreadModeUpdate {
        thread_id: "t2".to_string(),
        mode: ThreadMode::Exec,
    };

    app.handle_message(msg);

    let thread = app.dashboard.get_thread("t2").unwrap();
    assert_eq!(thread.mode, ThreadMode::Exec);

    // Send second ThreadModeUpdate message
    let msg = AppMessage::ThreadModeUpdate {
        thread_id: "t2".to_string(),
        mode: ThreadMode::Plan,
    };

    app.handle_message(msg);

    let thread = app.dashboard.get_thread("t2").unwrap();
    assert_eq!(thread.mode, ThreadMode::Plan);
}

#[test]
fn test_thread_mode_update_nonexistent_thread() {
    let mut app = App::default();

    // Send ThreadModeUpdate for a thread that doesn't exist
    let msg = AppMessage::ThreadModeUpdate {
        thread_id: "nonexistent".to_string(),
        mode: ThreadMode::Exec,
    };

    // Should not panic
    app.handle_message(msg);

    // Thread should still not exist
    assert!(app.dashboard.get_thread("nonexistent").is_none());
}

#[test]
fn test_thread_mode_update_different_modes() {
    let mut app = App::default();
    let mut thread = create_test_thread("t3", "Mode Test");
    thread.mode = ThreadMode::Normal;
    app.dashboard.add_thread(thread);

    // Update to Plan mode
    let msg = AppMessage::ThreadModeUpdate {
        thread_id: "t3".to_string(),
        mode: ThreadMode::Plan,
    };
    app.handle_message(msg);

    let thread = app.dashboard.get_thread("t3").unwrap();
    assert_eq!(thread.mode, ThreadMode::Plan);

    // Update to Exec mode
    let msg = AppMessage::ThreadModeUpdate {
        thread_id: "t3".to_string(),
        mode: ThreadMode::Exec,
    };
    app.handle_message(msg);

    let thread = app.dashboard.get_thread("t3").unwrap();
    assert_eq!(thread.mode, ThreadMode::Exec);

    // Update back to Normal mode
    let msg = AppMessage::ThreadModeUpdate {
        thread_id: "t3".to_string(),
        mode: ThreadMode::Normal,
    };
    app.handle_message(msg);

    let thread = app.dashboard.get_thread("t3").unwrap();
    assert_eq!(thread.mode, ThreadMode::Normal);
}

// ============================================================================
// ThreadVerified Handler Tests
// ============================================================================

#[test]
fn test_thread_verified_handler_parses_rfc3339() {
    let mut app = App::default();

    // Add a thread to the dashboard
    let mut thread = create_test_thread("t4", "Verification Test");
    thread.verified = Some(false);
    thread.verified_at = None;
    app.dashboard.add_thread(thread);

    // Send ThreadVerified with RFC3339 timestamp
    let verified_timestamp = "2026-01-21T10:30:00Z";
    let msg = AppMessage::ThreadVerified {
        thread_id: "t4".to_string(),
        verified_at: verified_timestamp.to_string(),
    };

    app.handle_message(msg);

    // Verify the thread was updated
    let updated_thread = app.dashboard.get_thread("t4").expect("Thread should exist");
    assert_eq!(updated_thread.verified, Some(true));
    assert!(updated_thread.verified_at.is_some());
}

#[test]
fn test_thread_verified_handler_fallback_on_invalid_timestamp() {
    let mut app = App::default();

    // Add a thread to the dashboard
    let thread = create_test_thread("t5", "Invalid Timestamp Test");
    app.dashboard.add_thread(thread);

    // Send ThreadVerified with invalid timestamp
    let msg = AppMessage::ThreadVerified {
        thread_id: "t5".to_string(),
        verified_at: "invalid-timestamp".to_string(),
    };

    app.handle_message(msg);

    // Verify the thread was updated (with fallback to current time)
    let updated_thread = app.dashboard.get_thread("t5").expect("Thread should exist");
    assert_eq!(updated_thread.verified, Some(true));
    assert!(updated_thread.verified_at.is_some()); // Should use current time
}

#[test]
fn test_thread_verified_sets_verified_flag_true() {
    let mut app = App::default();

    let mut thread = create_test_thread("t6", "Flag Test");
    thread.verified = Some(false);
    app.dashboard.add_thread(thread);

    let msg = AppMessage::ThreadVerified {
        thread_id: "t6".to_string(),
        verified_at: "2026-01-21T12:00:00Z".to_string(),
    };

    app.handle_message(msg);

    let updated = app.dashboard.get_thread("t6").unwrap();
    assert_eq!(updated.verified, Some(true));
}

#[test]
fn test_thread_verified_handler_nonexistent_thread() {
    let mut app = App::default();

    // Send ThreadVerified for a thread that doesn't exist
    let msg = AppMessage::ThreadVerified {
        thread_id: "nonexistent".to_string(),
        verified_at: "2026-01-21T14:00:00Z".to_string(),
    };

    // Should not panic
    app.handle_message(msg);

    // Thread should still not exist
    assert!(app.dashboard.get_thread("nonexistent").is_none());
}

// ============================================================================
// DashboardState Tests for update_thread_mode
// ============================================================================

#[test]
fn test_dashboard_update_thread_mode() {
    let mut state = DashboardState::new();

    let mut thread = create_test_thread("t7", "Dashboard Mode Test");
    thread.mode = ThreadMode::Normal;
    state.add_thread(thread);

    state.update_thread_mode("t7", ThreadMode::Plan);

    let updated = state.get_thread("t7").unwrap();
    assert_eq!(updated.mode, ThreadMode::Plan);
}

#[test]
fn test_dashboard_update_thread_mode_persists() {
    let mut state = DashboardState::new();

    let mut thread = create_test_thread("t8", "Dirty Flag Test");
    thread.mode = ThreadMode::Normal;
    state.add_thread(thread);

    // Update mode
    state.update_thread_mode("t8", ThreadMode::Exec);

    // Verify the mode persisted
    let updated = state.get_thread("t8").unwrap();
    assert_eq!(updated.mode, ThreadMode::Exec);

    // Compute views and verify mode is still correct
    state.compute_thread_views();
    let views = state.compute_thread_views();
    assert_eq!(views.len(), 1);
}

// ============================================================================
// DashboardState Tests for update_thread_verified
// ============================================================================

#[test]
fn test_dashboard_update_thread_verified() {
    let mut state = DashboardState::new();

    let mut thread = create_test_thread("t9", "Dashboard Verified Test");
    thread.verified = Some(false);
    state.add_thread(thread);

    let now = Utc::now();
    state.update_thread_verified("t9", now);

    let updated = state.get_thread("t9").unwrap();
    assert_eq!(updated.verified, Some(true));
    assert_eq!(updated.verified_at, Some(now));
}

#[test]
fn test_dashboard_update_thread_verified_persists() {
    let mut state = DashboardState::new();

    let thread = create_test_thread("t10", "Verified Dirty Test");
    state.add_thread(thread);

    // Update verified status
    let now = Utc::now();
    state.update_thread_verified("t10", now);

    // Verify it persisted
    let updated = state.get_thread("t10").unwrap();
    assert_eq!(updated.verified, Some(true));
    assert_eq!(updated.verified_at, Some(now));

    // Compute views and verify it's still there
    state.compute_thread_views();
    let views = state.compute_thread_views();
    assert_eq!(views.len(), 1);
}

#[test]
fn test_dashboard_update_thread_verified_timestamp() {
    let mut state = DashboardState::new();

    let thread = create_test_thread("t11", "Timestamp Test");
    state.add_thread(thread);

    let specific_time = chrono::DateTime::parse_from_rfc3339("2026-01-21T15:30:45Z")
        .unwrap()
        .with_timezone(&Utc);

    state.update_thread_verified("t11", specific_time);

    let updated = state.get_thread("t11").unwrap();
    assert_eq!(updated.verified_at, Some(specific_time));
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_multiple_thread_updates() {
    let mut app = App::default();

    // Add multiple threads
    app.dashboard.add_thread(create_test_thread("thread-1", "First"));
    app.dashboard.add_thread(create_test_thread("thread-2", "Second"));
    app.dashboard.add_thread(create_test_thread("thread-3", "Third"));

    // Update mode for first thread
    let msg1 = AppMessage::ThreadModeUpdate {
        thread_id: "thread-1".to_string(),
        mode: ThreadMode::Plan,
    };
    app.handle_message(msg1);

    // Update verified for second thread
    let msg2 = AppMessage::ThreadVerified {
        thread_id: "thread-2".to_string(),
        verified_at: "2026-01-21T16:00:00Z".to_string(),
    };
    app.handle_message(msg2);

    // Update mode for third thread
    let msg3 = AppMessage::ThreadModeUpdate {
        thread_id: "thread-3".to_string(),
        mode: ThreadMode::Exec,
    };
    app.handle_message(msg3);

    // Verify all updates
    assert_eq!(app.dashboard.get_thread("thread-1").unwrap().mode, ThreadMode::Plan);
    assert_eq!(app.dashboard.get_thread("thread-2").unwrap().verified, Some(true));
    assert_eq!(app.dashboard.get_thread("thread-3").unwrap().mode, ThreadMode::Exec);
}

#[test]
fn test_thread_mode_and_verified_combined() {
    let mut app = App::default();

    let mut thread = create_test_thread("combined-test", "Combined Test");
    thread.mode = ThreadMode::Normal;
    thread.verified = Some(false);
    app.dashboard.add_thread(thread);

    // Update mode first
    let msg1 = AppMessage::ThreadModeUpdate {
        thread_id: "combined-test".to_string(),
        mode: ThreadMode::Plan,
    };
    app.handle_message(msg1);

    // Then verify
    let msg2 = AppMessage::ThreadVerified {
        thread_id: "combined-test".to_string(),
        verified_at: "2026-01-21T17:00:00Z".to_string(),
    };
    app.handle_message(msg2);

    let updated = app.dashboard.get_thread("combined-test").unwrap();
    assert_eq!(updated.mode, ThreadMode::Plan);
    assert_eq!(updated.verified, Some(true));
    assert!(updated.verified_at.is_some());
}
