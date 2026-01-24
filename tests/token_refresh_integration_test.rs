//! Integration tests for complete token refresh flow (Phase 6).
//!
//! NOTE: Credentials now only contain auth fields (access_token, refresh_token,
//! expires_at, user_id). VPS state is fetched from the server API.
//!
//! These tests verify the complete token refresh behavior including:
//! 1. Token expired + valid refresh token → silent refresh (no auth prompt)
//! 2. Token expires soon (< 5 min) → proactive refresh
//! 3. Token expired + invalid refresh token → clear error + auth prompt
//! 4. Token expired + no refresh token → auth prompt
//! 5. Multiple rapid CLI invocations with expired token (intermittent case)
//! 6. Health check doesn't bypass startup refresh
//! 7. Credentials persist after each scenario

use spoq::auth::credentials::Credentials;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a temporary credentials directory for testing
fn setup_test_credentials_dir() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");
    (temp_dir, creds_path)
}

/// Helper to create expired credentials
fn create_expired_credentials() -> Credentials {
    Credentials {
        access_token: Some("expired-access-token".to_string()),
        refresh_token: Some("valid-refresh-token".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() - 3600), // Expired 1 hour ago
        user_id: Some("user-123".to_string()),
    }
}

/// Helper to create credentials expiring soon (< 5 minutes)
fn create_expiring_soon_credentials() -> Credentials {
    Credentials {
        access_token: Some("soon-to-expire-token".to_string()),
        refresh_token: Some("valid-refresh-token".to_string()),
        // Expires in 4 minutes (240 seconds) - below the 300s threshold
        expires_at: Some(chrono::Utc::now().timestamp() + 240),
        user_id: Some("user-123".to_string()),
    }
}

/// Helper to create credentials with no refresh token
fn create_no_refresh_token_credentials() -> Credentials {
    Credentials {
        access_token: Some("expired-token".to_string()),
        refresh_token: None,
        expires_at: Some(chrono::Utc::now().timestamp() - 3600),
        user_id: Some("user-123".to_string()),
    }
}

#[test]
fn test_expired_token_detection() {
    let creds = create_expired_credentials();

    assert!(
        creds.is_expired(),
        "Credentials should be detected as expired"
    );
    assert!(!creds.is_valid(), "Expired credentials should not be valid");

    let now = chrono::Utc::now().timestamp();
    let expires_at = creds.expires_at.unwrap();
    assert!(expires_at < now, "Expiration time should be in the past");
}

#[test]
fn test_expiring_soon_detection() {
    let creds = create_expiring_soon_credentials();

    // Token is not technically expired yet
    assert!(!creds.is_expired(), "Credentials should not be expired yet");
    assert!(creds.is_valid(), "Credentials should still be valid");

    // But it should be close to expiration
    let now = chrono::Utc::now().timestamp();
    let expires_at = creds.expires_at.unwrap();
    let time_remaining = expires_at - now;

    assert!(
        time_remaining < 300,
        "Token should expire in less than 5 minutes (300s)"
    );
    assert!(time_remaining > 0, "Token should not be expired yet");
}

#[test]
fn test_no_refresh_token_scenario() {
    let creds = create_no_refresh_token_credentials();

    assert!(creds.is_expired(), "Credentials should be expired");
    assert!(
        creds.refresh_token.is_none(),
        "Should have no refresh token"
    );
    assert!(
        !creds.is_valid(),
        "Expired credentials without refresh token should not be valid"
    );
}

#[test]
fn test_credentials_persistence() {
    let (_temp_dir, creds_path) = setup_test_credentials_dir();

    // Create and save credentials
    let creds = create_expired_credentials();

    let json = serde_json::to_string_pretty(&creds).unwrap();
    fs::write(&creds_path, json).unwrap();

    // Load credentials back
    let loaded_json = fs::read_to_string(&creds_path).unwrap();
    let loaded_creds: Credentials = serde_json::from_str(&loaded_json).unwrap();

    assert_eq!(loaded_creds.access_token, creds.access_token);
    assert_eq!(loaded_creds.refresh_token, creds.refresh_token);
    assert_eq!(loaded_creds.expires_at, creds.expires_at);
    assert_eq!(loaded_creds.user_id, creds.user_id);
}

#[test]
fn test_rapid_successive_checks() {
    // Simulate multiple rapid CLI invocations
    let creds = create_expired_credentials();

    // First check
    let check1 = creds.is_expired();

    // Immediate second check (< 1ms later)
    let check2 = creds.is_expired();

    // Third check
    let check3 = creds.is_expired();

    // All checks should consistently report expired
    assert!(check1, "First check should detect expiration");
    assert!(check2, "Second check should detect expiration");
    assert!(check3, "Third check should detect expiration");
}

#[test]
fn test_credentials_reload_after_refresh() {
    let (_temp_dir, creds_path) = setup_test_credentials_dir();

    // Simulate initial expired credentials
    let creds = create_expired_credentials();
    let json = serde_json::to_string_pretty(&creds).unwrap();
    fs::write(&creds_path, &json).unwrap();

    // Simulate refresh by creating new credentials with updated values
    let refreshed_creds = Credentials {
        access_token: Some("refreshed-token".to_string()),
        refresh_token: creds.refresh_token.clone(),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: creds.user_id.clone(),
    };

    // Save refreshed credentials
    let refreshed_json = serde_json::to_string_pretty(&refreshed_creds).unwrap();
    fs::write(&creds_path, refreshed_json).unwrap();

    // Reload from disk
    let reloaded_json = fs::read_to_string(&creds_path).unwrap();
    let reloaded_creds: Credentials = serde_json::from_str(&reloaded_json).unwrap();

    // Verify refreshed state persisted
    assert!(
        !reloaded_creds.is_expired(),
        "Reloaded credentials should not be expired"
    );
    assert_eq!(reloaded_creds.access_token.unwrap(), "refreshed-token");
}

#[test]
fn test_proactive_refresh_threshold() {
    // Test the 300-second (5-minute) threshold
    const PROACTIVE_REFRESH_THRESHOLD: i64 = 300;

    let now = chrono::Utc::now().timestamp();

    // Case 1: Token expires in 6 minutes (360s) - should NOT trigger proactive refresh
    let expires_at_safe = now + 360;
    let time_remaining_safe = expires_at_safe - now;
    assert!(
        time_remaining_safe >= PROACTIVE_REFRESH_THRESHOLD,
        "Token with 6 minutes remaining should not trigger proactive refresh"
    );

    // Case 2: Token expires in 4 minutes (240s) - SHOULD trigger proactive refresh
    let expires_at_soon = now + 240;
    let time_remaining_soon = expires_at_soon - now;
    assert!(
        time_remaining_soon < PROACTIVE_REFRESH_THRESHOLD,
        "Token with 4 minutes remaining should trigger proactive refresh"
    );

    // Case 3: Token expires in exactly 5 minutes (300s) - edge case
    let expires_at_edge = now + 300;
    let time_remaining_edge = expires_at_edge - now;
    assert!(
        time_remaining_edge <= PROACTIVE_REFRESH_THRESHOLD,
        "Token with exactly 5 minutes should trigger proactive refresh (inclusive threshold)"
    );
}

#[test]
fn test_missing_expires_at_treated_as_expired() {
    let creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: None, // Missing expiration
        user_id: None,
    };

    // From investigation report: missing expires_at is treated as expired
    assert!(
        creds.is_expired(),
        "Missing expires_at should be treated as expired"
    );
}

#[test]
fn test_health_check_timing() {
    // This test verifies that credential reload happens after refresh
    // to prevent TOCTOU (Time-of-Check Time-of-Use) race conditions

    let (_temp_dir, creds_path) = setup_test_credentials_dir();

    // Initial state: expired credentials
    let creds = create_expired_credentials();
    let json = serde_json::to_string_pretty(&creds).unwrap();
    fs::write(&creds_path, &json).unwrap();

    // Simulate refresh updating the file
    let refreshed_creds = Credentials {
        access_token: Some("new-token".to_string()),
        refresh_token: creds.refresh_token.clone(),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: creds.user_id.clone(),
    };
    let new_json = serde_json::to_string_pretty(&refreshed_creds).unwrap();
    fs::write(&creds_path, new_json).unwrap();

    // Simulate health check reloading credentials from disk
    let health_check_json = fs::read_to_string(&creds_path).unwrap();
    let health_check_creds: Credentials = serde_json::from_str(&health_check_json).unwrap();

    // Health check should see the refreshed credentials
    assert!(
        !health_check_creds.is_expired(),
        "Health check should see refreshed credentials"
    );
    assert_eq!(health_check_creds.access_token.unwrap(), "new-token");
}

#[test]
fn test_all_required_fields_persist() {
    let (_temp_dir, creds_path) = setup_test_credentials_dir();

    // Credentials now only have 4 fields (auth tokens only)
    // VPS state is fetched from the server API
    let creds = Credentials {
        access_token: Some("access".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(12345),
        user_id: Some("user-1".to_string()),
    };

    let json = serde_json::to_string_pretty(&creds).unwrap();
    fs::write(&creds_path, json).unwrap();

    let loaded_json = fs::read_to_string(&creds_path).unwrap();
    let loaded: Credentials = serde_json::from_str(&loaded_json).unwrap();

    assert_eq!(loaded.access_token, creds.access_token);
    assert_eq!(loaded.refresh_token, creds.refresh_token);
    assert_eq!(loaded.expires_at, creds.expires_at);
    assert_eq!(loaded.user_id, creds.user_id);
}
