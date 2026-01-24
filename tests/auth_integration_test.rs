//! Integration tests for the authentication credentials.
//!
//! NOTE: Credentials now only contain auth fields (access_token, refresh_token,
//! expires_at, user_id). VPS state is fetched from the server API.
//!
//! These tests verify credential persistence and token handling.

use spoq::app::{App, Screen};
use spoq::auth::Credentials;

/// Test initial screen is always CommandDeck (pre-flight checks handle auth)
#[test]
fn test_initial_screen_is_command_deck() {
    let app = App::new().expect("App should create successfully");

    // TUI always starts at CommandDeck since pre-flight checks handle auth
    assert_eq!(app.screen, Screen::CommandDeck);
}

/// Test that credentials struct can be created and serialized
#[test]
fn test_credentials_serialization() {
    let credentials = Credentials::default();

    // Should be serializable to JSON
    let json = serde_json::to_string(&credentials).expect("Should serialize");
    assert!(json.contains("access_token"));

    // Should be deserializable back
    let parsed: Credentials = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(parsed.access_token, credentials.access_token);
}

/// Test credentials with auth tokens
#[test]
fn test_credentials_with_auth_tokens() {
    let credentials = Credentials {
        access_token: Some("test-token".to_string()),
        refresh_token: Some("test-refresh".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: Some("user-123".to_string()),
    };

    // Serialize and deserialize
    let json = serde_json::to_string_pretty(&credentials).expect("Should serialize");
    let parsed: Credentials = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(parsed.access_token, credentials.access_token);
    assert_eq!(parsed.refresh_token, credentials.refresh_token);
    assert_eq!(parsed.expires_at, credentials.expires_at);
    assert_eq!(parsed.user_id, credentials.user_id);
}

/// Test screen enum variants
#[test]
fn test_screen_variants() {
    // Verify all screen variants exist
    let command_deck = Screen::CommandDeck;
    let conversation = Screen::Conversation;

    assert_eq!(command_deck, Screen::CommandDeck);
    assert_eq!(conversation, Screen::Conversation);
}

/// Test default screen is CommandDeck
#[test]
fn test_default_screen() {
    let default = Screen::default();
    assert_eq!(default, Screen::CommandDeck);
}

/// Test App can be created and has expected initial state
#[test]
fn test_app_initial_state() {
    let app = App::new().expect("App should create successfully");

    // Check that essential fields are initialized
    assert_eq!(app.screen, Screen::CommandDeck);
}

/// Test App has central_api configured
#[test]
fn test_app_has_central_api() {
    let app = App::new().expect("App should create successfully");

    // Central API should be configured
    assert!(app.central_api.is_some());
}

/// Test App has credentials_manager configured
#[test]
fn test_app_has_credentials_manager() {
    let app = App::new().expect("App should create successfully");

    // Credentials manager should be configured
    assert!(app.credentials_manager.is_some());
}

/// Test token expiration check logic
#[test]
fn test_token_expiration_logic() {
    // Token expires in 1 hour
    let expires_at = chrono::Utc::now().timestamp() + 3600;
    let buffer_seconds = 300; // 5 minute buffer
    let now = chrono::Utc::now().timestamp();

    // Token should be valid (not within buffer of expiration)
    let is_expired = expires_at < now + buffer_seconds;
    assert!(
        !is_expired,
        "Token expiring in 1 hour should not be expired"
    );

    // Token expires in 4 minutes (within buffer)
    let expires_soon = chrono::Utc::now().timestamp() + 240;
    let is_expiring_soon = expires_soon < now + buffer_seconds;
    assert!(
        is_expiring_soon,
        "Token expiring in 4 minutes should be within buffer"
    );
}

/// Test credentials default values
#[test]
fn test_credentials_default() {
    let creds = Credentials::default();

    // Only auth fields exist now
    assert!(creds.access_token.is_none());
    assert!(creds.refresh_token.is_none());
    assert!(creds.expires_at.is_none());
    assert!(creds.user_id.is_none());
}

/// Test credentials backward compatibility - old JSON format with only auth fields
#[test]
fn test_credentials_backward_compatibility_auth_only() {
    // JSON with only auth fields
    let json = r#"{
        "access_token": "old-access-token",
        "refresh_token": "old-refresh-token",
        "expires_at": 1999999999,
        "user_id": "user-old"
    }"#;

    // Should successfully deserialize
    let creds: Credentials = serde_json::from_str(json).expect("Should parse format");

    // All fields should be populated
    assert_eq!(creds.access_token, Some("old-access-token".to_string()));
    assert_eq!(creds.refresh_token, Some("old-refresh-token".to_string()));
    assert_eq!(creds.expires_at, Some(1999999999));
    assert_eq!(creds.user_id, Some("user-old".to_string()));
}

/// Test credentials serialization only includes auth fields
#[test]
fn test_credentials_serialization_auth_only() {
    let creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(1234567890),
        user_id: Some("user".to_string()),
    };

    let json = serde_json::to_string(&creds).expect("Should serialize");

    // Auth fields should be present
    assert!(json.contains("access_token"));
    assert!(json.contains("refresh_token"));
    assert!(json.contains("expires_at"));
    assert!(json.contains("user_id"));

    // VPS fields should NOT be present (removed from struct)
    assert!(!json.contains("vps_id"));
    assert!(!json.contains("vps_url"));
    assert!(!json.contains("vps_status"));
    assert!(!json.contains("datacenter_id"));

    // Deserialize back and verify roundtrip
    let parsed: Credentials = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(parsed.access_token, Some("token".to_string()));
}

/// Test is_valid and is_expired methods
#[test]
fn test_credentials_validity_methods() {
    // Valid credentials
    let valid_creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: Some("user".to_string()),
    };
    assert!(valid_creds.is_valid());
    assert!(!valid_creds.is_expired());

    // Expired credentials
    let expired_creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(0), // Unix epoch
        user_id: Some("user".to_string()),
    };
    assert!(!expired_creds.is_valid());
    assert!(expired_creds.is_expired());

    // No token
    let no_token = Credentials::default();
    assert!(!no_token.is_valid());
    assert!(!no_token.has_token());
}
