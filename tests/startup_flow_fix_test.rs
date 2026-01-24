//! Tests for startup flow scenarios.
//!
//! NOTE: VPS state is no longer stored in credentials. VPS state is always
//! fetched from the server API. These tests focus on auth token scenarios only.
//!
//! The startup flow now:
//! 1. Loads credentials (auth tokens only)
//! 2. Checks if tokens are valid/expired
//! 3. Fetches VPS state from server API (not from local credentials)
//! 4. Takes appropriate action based on server response

use spoq::auth::credentials::{Credentials, CredentialsManager};
use tempfile::TempDir;
use serial_test::serial;

/// Scenario 1: Fresh install (no credentials.json)
#[test]
#[serial]
fn test_fresh_install_no_credentials() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = manager.load();
    assert!(!creds.has_token());
    // VPS state is now fetched from API, not stored in credentials
}

/// Scenario 2: Valid credentials (should proceed to VPS check via API)
#[test]
#[serial]
fn test_valid_credentials() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = Credentials {
        access_token: Some("valid-token".to_string()),
        refresh_token: Some("valid-refresh".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: Some("user-123".to_string()),
    };

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(!loaded.is_expired());
    assert!(loaded.is_valid());
}

/// Scenario 3: Expired token with refresh token (should refresh)
#[test]
#[serial]
fn test_expired_token_with_refresh() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = Credentials {
        access_token: Some("expired-token".to_string()),
        refresh_token: Some("valid-refresh".to_string()),
        expires_at: Some(0), // Expired
        user_id: Some("user-123".to_string()),
    };

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(loaded.is_expired()); // Should detect expiration
    assert!(!loaded.is_valid()); // Not valid because expired
    assert!(loaded.refresh_token.is_some()); // Can refresh
}

/// Scenario 4: Valid token, no refresh token
#[test]
#[serial]
fn test_valid_token_no_refresh() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = Credentials {
        access_token: Some("valid-token".to_string()),
        refresh_token: None, // No refresh token
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: Some("user-123".to_string()),
    };

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(!loaded.is_expired());
    assert!(loaded.is_valid());
    assert!(loaded.refresh_token.is_none());
}

/// Scenario 5: Expired token without refresh (needs re-auth)
#[test]
#[serial]
fn test_expired_token_no_refresh() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = Credentials {
        access_token: Some("expired-token".to_string()),
        refresh_token: None, // No refresh token
        expires_at: Some(0), // Expired
        user_id: Some("user-123".to_string()),
    };

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(loaded.is_expired());
    assert!(!loaded.is_valid());
    assert!(loaded.refresh_token.is_none()); // Can't refresh, needs re-auth
}

/// Scenario 6: No access token (needs auth)
#[test]
#[serial]
fn test_no_access_token() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = Credentials {
        access_token: None,
        refresh_token: Some("refresh-token".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: Some("user-123".to_string()),
    };

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(!loaded.has_token()); // No access token
    assert!(!loaded.is_valid()); // Not valid without token
}

/// Scenario 7: Missing expires_at (treated as expired)
#[test]
#[serial]
fn test_missing_expires_at() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: None, // Missing!
        user_id: Some("user-123".to_string()),
    };

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(loaded.has_token());
    assert!(loaded.is_expired()); // Missing expires_at = expired
    assert!(!loaded.is_valid());
}

/// Scenario 8: Empty credentials file
#[test]
#[serial]
fn test_empty_credentials() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = Credentials::default();

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(!loaded.has_token());
    assert!(loaded.is_expired());
    assert!(!loaded.is_valid());
}

/// Scenario 9: Credentials with user_id only
#[test]
#[serial]
fn test_credentials_with_user_id_only() {
    let temp_dir = TempDir::new().unwrap();
    let manager = create_test_manager(&temp_dir);

    let creds = Credentials {
        access_token: None,
        refresh_token: None,
        expires_at: None,
        user_id: Some("user-123".to_string()),
    };

    assert!(manager.save(&creds));

    let loaded = manager.load();
    assert!(!loaded.has_token());
    assert!(!loaded.is_valid());
    assert_eq!(loaded.user_id, Some("user-123".to_string()));
}

// Helper function
fn create_test_manager(temp_dir: &TempDir) -> CredentialsManager {
    // Set HOME to temp directory so CredentialsManager uses it
    std::env::set_var("HOME", temp_dir.path());
    CredentialsManager::new().expect("Failed to create credentials manager")
}
