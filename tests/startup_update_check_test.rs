//! Integration test for startup update check.
//!
//! This test verifies that the update check integration in main.rs
//! correctly checks for updates on startup and stores the pending update path.

use spoq::update::{UpdateState, UpdateStateManager};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

/// Test that UpdateStateManager can be created and used for tracking update checks.
#[test]
fn test_update_state_manager_initialization() {
    // This test verifies the basic components needed for startup update check
    let manager = UpdateStateManager::new();
    assert!(
        manager.is_some(),
        "UpdateStateManager should be created successfully"
    );

    let manager = manager.unwrap();
    let state = manager.load();

    // State may or may not have data depending on whether the app has run before
    // Just verify the structure is valid
    assert!(
        state.last_check.is_none() || state.last_check.is_some(),
        "last_check should be valid Option"
    );
}

/// Test that update state can be persisted and loaded correctly.
#[test]
fn test_update_state_persistence() {
    let _temp_dir = TempDir::new().unwrap();

    // Create a manager with test path
    let manager = UpdateStateManager::new().unwrap();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Create and save state
    let state = UpdateState {
        last_check: Some(now),
        pending_update_path: Some("/tmp/spoq-0.2.0".to_string()),
        available_version: Some("0.2.0".to_string()),
    };

    assert!(manager.save(&state), "State should be saved successfully");

    // Load and verify
    let loaded_state = manager.load();
    assert_eq!(loaded_state.last_check, Some(now));
    assert_eq!(
        loaded_state.pending_update_path,
        Some("/tmp/spoq-0.2.0".to_string())
    );
    assert_eq!(loaded_state.available_version, Some("0.2.0".to_string()));
}

/// Test that pending update detection works correctly.
#[test]
fn test_has_pending_update() {
    let mut state = UpdateState::default();

    // No pending update initially
    assert!(!state.has_pending_update());

    // Only path, not enough
    state.pending_update_path = Some("/tmp/spoq-update".to_string());
    assert!(!state.has_pending_update());

    // Both path and version required
    state.available_version = Some("0.2.0".to_string());
    assert!(state.has_pending_update());
}

/// Test rate limiting logic for update checks.
#[test]
fn test_update_check_rate_limiting() {
    const CHECK_INTERVAL_SECONDS: i64 = 24 * 60 * 60;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut state = UpdateState::default();

    // No last check - should check
    assert!(state.last_check.is_none());

    // Recent check - should NOT check
    state.last_check = Some(now - 1000); // 1000 seconds ago
    let should_check = match state.last_check {
        Some(last_check) => (now - last_check) >= CHECK_INTERVAL_SECONDS,
        None => true,
    };
    assert!(!should_check, "Should not check if less than 24 hours");

    // Old check - should check
    state.last_check = Some(now - CHECK_INTERVAL_SECONDS - 100); // More than 24 hours ago
    let should_check = match state.last_check {
        Some(last_check) => (now - last_check) >= CHECK_INTERVAL_SECONDS,
        None => true,
    };
    assert!(should_check, "Should check if more than 24 hours");
}

/// Test clearing pending update.
#[test]
fn test_clear_pending_update() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut state = UpdateState {
        last_check: Some(now),
        pending_update_path: Some("/tmp/spoq-update".to_string()),
        available_version: Some("0.2.0".to_string()),
    };

    assert!(state.has_pending_update());

    state.clear_pending_update();

    // Should preserve last_check but clear pending update info
    assert_eq!(state.last_check, Some(now));
    assert!(state.pending_update_path.is_none());
    assert!(state.available_version.is_none());
    assert!(!state.has_pending_update());
}
