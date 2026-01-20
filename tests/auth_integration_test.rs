//! Integration tests for the authentication credentials.
//!
//! These tests verify credential persistence and token handling.
//! Note: Login and Provisioning screens are now handled by pre-flight CLI checks,
//! so the TUI always starts at CommandDeck.

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

/// Test credentials with full VPS info
#[test]
fn test_credentials_with_vps_info() {
    let credentials = Credentials {
        access_token: Some("test-token".to_string()),
        refresh_token: Some("test-refresh".to_string()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        user_id: Some("user-123".to_string()),
        username: Some("testuser".to_string()),
        vps_id: Some("vps-abc123".to_string()),
        vps_url: Some("http://192.168.1.100:8000".to_string()),
        vps_hostname: Some("spoq-abc123.spoq.cloud".to_string()),
        vps_ip: Some("192.168.1.100".to_string()),
        vps_status: Some("ready".to_string()),
        datacenter_id: Some(1),
    };

    // Serialize and deserialize
    let json = serde_json::to_string_pretty(&credentials).expect("Should serialize");
    let parsed: Credentials = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(parsed.access_token, credentials.access_token);
    assert_eq!(parsed.vps_url, credentials.vps_url);
    assert_eq!(parsed.vps_status, credentials.vps_status);
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

    assert!(creds.access_token.is_none());
    assert!(creds.refresh_token.is_none());
    assert!(creds.expires_at.is_none());
    assert!(creds.user_id.is_none());
    assert!(creds.username.is_none());
    assert!(creds.vps_id.is_none());
    assert!(creds.vps_url.is_none());
    assert!(creds.vps_hostname.is_none());
    assert!(creds.vps_ip.is_none());
    assert!(creds.vps_status.is_none());
    assert!(creds.datacenter_id.is_none());
}

/// Test VPS ready status detection
#[test]
fn test_vps_ready_status() {
    let creds_pending = Credentials {
        vps_status: Some("pending".to_string()),
        ..Default::default()
    };

    let creds_ready = Credentials {
        vps_status: Some("ready".to_string()),
        ..Default::default()
    };

    assert_ne!(creds_pending.vps_status.as_deref(), Some("ready"));
    assert_eq!(creds_ready.vps_status.as_deref(), Some("ready"));
}

/// Test credentials backward compatibility - old JSON format without datacenter_id
#[test]
fn test_credentials_backward_compatibility_no_datacenter() {
    // Simulate old credentials file format (pre-API v2)
    let old_json = r#"{
        "access_token": "old-access-token",
        "refresh_token": "old-refresh-token",
        "expires_at": 1999999999,
        "user_id": "user-old",
        "username": "olduser",
        "vps_id": "vps-old",
        "vps_url": "https://old.spoq.dev",
        "vps_hostname": "old.spoq.dev",
        "vps_ip": "10.0.0.1",
        "vps_status": "ready"
    }"#;

    // Should successfully deserialize without datacenter_id field
    let creds: Credentials = serde_json::from_str(old_json).expect("Should parse old format");

    // All fields should be populated
    assert_eq!(creds.access_token, Some("old-access-token".to_string()));
    assert_eq!(creds.refresh_token, Some("old-refresh-token".to_string()));
    assert_eq!(creds.expires_at, Some(1999999999));
    assert_eq!(creds.user_id, Some("user-old".to_string()));
    assert_eq!(creds.username, Some("olduser".to_string()));
    assert_eq!(creds.vps_id, Some("vps-old".to_string()));
    assert_eq!(creds.vps_url, Some("https://old.spoq.dev".to_string()));
    assert_eq!(creds.vps_hostname, Some("old.spoq.dev".to_string()));
    assert_eq!(creds.vps_ip, Some("10.0.0.1".to_string()));
    assert_eq!(creds.vps_status, Some("ready".to_string()));

    // datacenter_id should default to None
    assert_eq!(creds.datacenter_id, None);
}

/// Test credentials with new datacenter_id field
#[test]
fn test_credentials_with_datacenter_id() {
    let new_json = r#"{
        "access_token": "new-token",
        "refresh_token": "new-refresh",
        "expires_at": 2999999999,
        "user_id": "user-new",
        "username": "newuser",
        "vps_id": "vps-new",
        "vps_url": "https://new.spoq.dev",
        "vps_hostname": "new.spoq.dev",
        "vps_ip": "192.168.1.100",
        "vps_status": "running",
        "datacenter_id": 9
    }"#;

    let creds: Credentials = serde_json::from_str(new_json).expect("Should parse new format");

    // datacenter_id should be present
    assert_eq!(creds.datacenter_id, Some(9));

    // Other fields should also be correct
    assert_eq!(creds.access_token, Some("new-token".to_string()));
    assert_eq!(creds.vps_status, Some("running".to_string()));
}

/// Test credentials serialization includes datacenter_id
#[test]
fn test_credentials_serialization_with_datacenter() {
    let creds = Credentials {
        access_token: Some("token".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(1234567890),
        user_id: Some("user".to_string()),
        username: Some("testuser".to_string()),
        vps_id: Some("vps".to_string()),
        vps_url: Some("https://vps.example.com".to_string()),
        vps_hostname: Some("hostname".to_string()),
        vps_ip: Some("1.2.3.4".to_string()),
        vps_status: Some("ready".to_string()),
        datacenter_id: Some(42),
    };

    let json = serde_json::to_string(&creds).expect("Should serialize");

    // Should contain datacenter_id in output
    assert!(json.contains("datacenter_id"));
    assert!(json.contains("42"));

    // Deserialize back and verify roundtrip
    let parsed: Credentials = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(parsed.datacenter_id, Some(42));
}

/// Test that VPS is considered configured when both id and url are present
#[test]
fn test_vps_has_vps_requires_both_id_and_url() {
    // Missing both - no VPS
    let creds_none = Credentials::default();
    assert!(!creds_none.has_vps());

    // Only vps_id - no VPS
    let creds_id_only = Credentials {
        vps_id: Some("vps-123".to_string()),
        ..Default::default()
    };
    assert!(!creds_id_only.has_vps());

    // Only vps_url - no VPS
    let creds_url_only = Credentials {
        vps_url: Some("https://vps.example.com".to_string()),
        ..Default::default()
    };
    assert!(!creds_url_only.has_vps());

    // Both present - has VPS
    let creds_both = Credentials {
        vps_id: Some("vps-123".to_string()),
        vps_url: Some("https://vps.example.com".to_string()),
        ..Default::default()
    };
    assert!(creds_both.has_vps());
}
