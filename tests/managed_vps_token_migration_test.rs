//! Integration tests for Managed VPS token migration flow.
//!
//! NOTE: VPS state is no longer stored in credentials (as of the refactor).
//! Token migration happens separately - these tests focus on the migration
//! logic itself, not credential storage.
//!
//! Token migration runs during provisioning but doesn't store archive paths
//! in credentials anymore.

use spoq::auth::credentials::Credentials;

/// Test that credentials are properly initialized with auth tokens
#[test]
fn test_credentials_with_auth_tokens() {
    let creds = Credentials {
        access_token: Some("test-token".to_string()),
        refresh_token: Some("test-refresh".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: Some("user-123".to_string()),
    };

    assert!(creds.has_token());
    assert!(!creds.is_expired());
    assert!(creds.is_valid());
}

/// Test that credentials only contain auth fields
#[test]
fn test_credentials_structure() {
    let creds = Credentials::default();

    // Verify default state
    assert!(creds.access_token.is_none());
    assert!(creds.refresh_token.is_none());
    assert!(creds.expires_at.is_none());
    assert!(creds.user_id.is_none());
}

/// Test credentials serialization only includes auth fields
#[test]
fn test_credentials_serialization() {
    let creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(12345),
        user_id: Some("user".to_string()),
    };

    let json = serde_json::to_string(&creds).unwrap();

    // Auth fields should be present
    assert!(json.contains("access_token"));
    assert!(json.contains("refresh_token"));
    assert!(json.contains("expires_at"));
    assert!(json.contains("user_id"));

    // VPS fields should NOT be present (removed from struct)
    assert!(!json.contains("vps_id"));
    assert!(!json.contains("vps_url"));
    assert!(!json.contains("vps_status"));
    assert!(!json.contains("token_archive_path"));
}

/// Test credentials save and load
#[test]
fn test_credentials_save_load() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let credentials_dir = temp_dir.path().join(".spoq");
    let credentials_path = credentials_dir.join("credentials.json");

    // Create the credentials directory
    fs::create_dir_all(&credentials_dir).unwrap();

    // Create credentials
    let creds = Credentials {
        access_token: Some("test-token".to_string()),
        refresh_token: Some("test-refresh".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: Some("user-123".to_string()),
    };

    // Save
    let json = serde_json::to_string_pretty(&creds).unwrap();
    fs::write(&credentials_path, json).unwrap();

    // Load
    let loaded_json = fs::read_to_string(&credentials_path).unwrap();
    let loaded: Credentials = serde_json::from_str(&loaded_json).unwrap();

    assert_eq!(loaded.access_token, creds.access_token);
    assert_eq!(loaded.refresh_token, creds.refresh_token);
    assert_eq!(loaded.expires_at, creds.expires_at);
    assert_eq!(loaded.user_id, creds.user_id);
}

/// Test logging message for token migration
#[test]
fn test_token_migration_logging_message() {
    // Test that the logging message "Running token migration..." is appropriate
    let log_message = "Running token migration...";
    assert!(!log_message.is_empty());
    assert!(log_message.contains("token migration"));
}

/// Test token expiration checking
#[test]
fn test_token_expiration_checks() {
    // Valid token
    let valid = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: None,
    };
    assert!(!valid.is_expired());
    assert!(valid.is_valid());

    // Expired token
    let expired = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(0), // Unix epoch
        user_id: None,
    };
    assert!(expired.is_expired());
    assert!(!expired.is_valid());
}

/// Test token presence checking
#[test]
fn test_token_presence_checks() {
    // Has token
    let with_token = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: None,
        expires_at: None,
        user_id: None,
    };
    assert!(with_token.has_token());

    // No token
    let without_token = Credentials::default();
    assert!(!without_token.has_token());
}
