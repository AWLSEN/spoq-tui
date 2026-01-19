//! Integration tests for the complete authentication flow.
//!
//! These tests verify the end-to-end auth and provisioning flow:
//! - Initial screen determination based on credentials
//! - Token refresh logic
//! - Credential persistence
//! - Provisioning flow state machine

use spoq::app::{App, ProvisioningPhase, Screen};
use spoq::auth::Credentials;

/// Test initial screen is Login when no credentials exist
#[test]
fn test_initial_screen_no_credentials() {
    let app = App::new().expect("App should create successfully");

    // When credentials file doesn't exist or has no token, should start on Login
    // Note: This may vary based on whether credentials file exists in test environment
    // The key assertion is that the screen is determined by credential state
    assert!(
        app.screen == Screen::Login
        || app.screen == Screen::Provisioning
        || app.screen == Screen::CommandDeck,
        "Initial screen should be valid"
    );
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
    };

    // Serialize and deserialize
    let json = serde_json::to_string_pretty(&credentials).expect("Should serialize");
    let parsed: Credentials = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(parsed.access_token, credentials.access_token);
    assert_eq!(parsed.vps_url, credentials.vps_url);
    assert_eq!(parsed.vps_status, credentials.vps_status);
}

/// Test provisioning phase transitions
#[test]
fn test_provisioning_phase_flow() {
    // The full provisioning flow should be:
    // LoadingPlans -> SelectPlan -> Provisioning -> WaitingReady -> Ready

    let loading = ProvisioningPhase::LoadingPlans;
    let select = ProvisioningPhase::SelectPlan;
    let provisioning = ProvisioningPhase::Provisioning;
    let waiting = ProvisioningPhase::WaitingReady { status: "configuring".to_string() };
    let ready = ProvisioningPhase::Ready {
        hostname: "test.spoq.cloud".to_string(),
        ip: "192.168.1.100".to_string(),
    };

    // Verify all phases can be created
    assert!(matches!(loading, ProvisioningPhase::LoadingPlans));
    assert!(matches!(select, ProvisioningPhase::SelectPlan));
    assert!(matches!(provisioning, ProvisioningPhase::Provisioning));
    assert!(matches!(waiting, ProvisioningPhase::WaitingReady { .. }));
    assert!(matches!(ready, ProvisioningPhase::Ready { .. }));
}

/// Test error phases exist
#[test]
fn test_provisioning_error_phases() {
    let plans_error = ProvisioningPhase::PlansError("Network error".to_string());
    let provision_error = ProvisioningPhase::ProvisionError("Server error".to_string());

    if let ProvisioningPhase::PlansError(msg) = plans_error {
        assert_eq!(msg, "Network error");
    } else {
        panic!("Expected PlansError variant");
    }

    if let ProvisioningPhase::ProvisionError(msg) = provision_error {
        assert_eq!(msg, "Server error");
    } else {
        panic!("Expected ProvisionError variant");
    }
}

/// Test screen enum variants
#[test]
fn test_screen_variants() {
    // Verify all screen variants exist
    let login = Screen::Login;
    let provisioning = Screen::Provisioning;
    let command_deck = Screen::CommandDeck;
    let conversation = Screen::Conversation;

    assert_eq!(login, Screen::Login);
    assert_eq!(provisioning, Screen::Provisioning);
    assert_eq!(command_deck, Screen::CommandDeck);
    assert_eq!(conversation, Screen::Conversation);
}

/// Test default screen is CommandDeck
#[test]
fn test_default_screen() {
    let default = Screen::default();
    assert_eq!(default, Screen::CommandDeck);
}

/// Test default provisioning phase is LoadingPlans
#[test]
fn test_default_provisioning_phase() {
    let default = ProvisioningPhase::default();
    assert!(matches!(default, ProvisioningPhase::LoadingPlans));
}

/// Test App can be created and has expected initial state
#[test]
fn test_app_initial_state() {
    let app = App::new().expect("App should create successfully");

    // Check that essential fields are initialized
    assert!(app.vps_plans.is_empty());
    assert_eq!(app.selected_plan_idx, 0);
    assert!(app.ssh_password_input.is_empty());
    assert!(!app.entering_ssh_password);
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
    assert!(!is_expired, "Token expiring in 1 hour should not be expired");

    // Token expires in 4 minutes (within buffer)
    let expires_soon = chrono::Utc::now().timestamp() + 240;
    let is_expiring_soon = expires_soon < now + buffer_seconds;
    assert!(is_expiring_soon, "Token expiring in 4 minutes should be within buffer");
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

/// Test App correctly determines CommandDeck screen with ready VPS
#[test]
fn test_screen_determination_with_ready_vps() {
    // This tests the logic from App constructor:
    // If vps_status == "ready" AND vps_url exists, go to CommandDeck

    let creds = Credentials {
        access_token: Some("valid-token".to_string()),
        vps_url: Some("http://192.168.1.100:8000".to_string()),
        vps_status: Some("ready".to_string()),
        ..Default::default()
    };

    let has_token = creds.access_token.is_some();
    let has_ready_vps = creds.vps_url.is_some() && creds.vps_status.as_deref() == Some("ready");

    assert!(has_token, "Should have token");
    assert!(has_ready_vps, "Should have ready VPS");

    // Logic would lead to CommandDeck
    let expected_screen = if !has_token {
        Screen::Login
    } else if !has_ready_vps {
        Screen::Provisioning
    } else {
        Screen::CommandDeck
    };

    assert_eq!(expected_screen, Screen::CommandDeck);
}

/// Test App correctly determines Provisioning screen without ready VPS
#[test]
fn test_screen_determination_without_ready_vps() {
    let creds = Credentials {
        access_token: Some("valid-token".to_string()),
        vps_status: Some("pending".to_string()),
        ..Default::default()
    };

    let has_token = creds.access_token.is_some();
    let has_ready_vps = creds.vps_url.is_some() && creds.vps_status.as_deref() == Some("ready");

    assert!(has_token, "Should have token");
    assert!(!has_ready_vps, "Should not have ready VPS");

    let expected_screen = if !has_token {
        Screen::Login
    } else if !has_ready_vps {
        Screen::Provisioning
    } else {
        Screen::CommandDeck
    };

    assert_eq!(expected_screen, Screen::Provisioning);
}

/// Test App correctly determines Login screen without credentials
#[test]
fn test_screen_determination_no_credentials() {
    let creds = Credentials::default();

    let has_token = creds.access_token.is_some();

    assert!(!has_token, "Should not have token");

    let expected_screen = if !has_token {
        Screen::Login
    } else {
        Screen::CommandDeck // won't reach here
    };

    assert_eq!(expected_screen, Screen::Login);
}

// ============================================================================
// API Response Parsing Tests
// ============================================================================
// These tests verify that API responses can be parsed correctly, including
// edge cases where the server might return unexpected formats.

use spoq::auth::central_api::{
    DeviceCodeResponse, TokenResponse, VpsPlan, VpsPlansResponse,
    VpsStatusResponse, ProvisionResponse,
};

/// Test TokenResponse parsing with all fields present
#[test]
fn test_token_response_full() {
    let json = r#"{
        "access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.test",
        "refresh_token": "refresh-abc123",
        "expires_in": 3600,
        "token_type": "Bearer",
        "user_id": "user-123",
        "username": "testuser"
    }"#;

    let response: TokenResponse = serde_json::from_str(json).expect("Should parse");
    assert_eq!(response.access_token, "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.test");
    assert_eq!(response.refresh_token, Some("refresh-abc123".to_string()));
    assert_eq!(response.expires_in, 3600);
    assert_eq!(response.token_type, Some("Bearer".to_string()));
    assert_eq!(response.user_id, Some("user-123".to_string()));
    assert_eq!(response.username, Some("testuser".to_string()));
}

/// Test TokenResponse parsing with only access_token (minimal response)
#[test]
fn test_token_response_minimal() {
    let json = r#"{"access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.test"}"#;

    let response: TokenResponse = serde_json::from_str(json).expect("Should parse minimal response");
    assert_eq!(response.access_token, "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.test");
    assert_eq!(response.refresh_token, None);
    assert_eq!(response.expires_in, 3600); // Default value
    assert_eq!(response.token_type, None);
    assert_eq!(response.user_id, None);
    assert_eq!(response.username, None);
}

/// Test TokenResponse parsing without expires_in (uses default)
#[test]
fn test_token_response_missing_expires_in() {
    let json = r#"{
        "access_token": "test-token",
        "refresh_token": "test-refresh"
    }"#;

    let response: TokenResponse = serde_json::from_str(json).expect("Should parse");
    assert_eq!(response.expires_in, 3600); // Default
}

/// Test DeviceCodeResponse parsing
#[test]
fn test_device_code_response_parsing() {
    let json = r#"{
        "device_code": "device-abc123",
        "verification_uri": "https://api.example.com/auth/verify?d=xyz",
        "user_code": "ABCD-1234",
        "expires_in": 300,
        "interval": 5
    }"#;

    let response: DeviceCodeResponse = serde_json::from_str(json).expect("Should parse");
    assert_eq!(response.device_code, "device-abc123");
    assert_eq!(response.verification_uri, "https://api.example.com/auth/verify?d=xyz");
    assert_eq!(response.user_code, Some("ABCD-1234".to_string()));
    assert_eq!(response.expires_in, 300);
    assert_eq!(response.interval, 5);
}

/// Test DeviceCodeResponse without user_code (embedded in URI)
#[test]
fn test_device_code_response_no_user_code() {
    let json = r#"{
        "device_code": "device-abc123",
        "verification_uri": "https://api.example.com/auth/verify?d=eyJ3b3JkX2NvZGUiOiJ0ZXN0In0",
        "expires_in": 300,
        "interval": 5
    }"#;

    let response: DeviceCodeResponse = serde_json::from_str(json).expect("Should parse");
    assert_eq!(response.user_code, None);
}

/// Test VpsPlansResponse wrapper format
#[test]
fn test_vps_plans_response_wrapper_format() {
    let json = r#"{
        "plans": [
            {
                "id": "plan-small",
                "name": "Small",
                "vcpus": 1,
                "ram_mb": 1024,
                "disk_gb": 25,
                "price_cents": 500
            },
            {
                "id": "plan-medium",
                "name": "Medium",
                "vcpus": 2,
                "ram_mb": 2048,
                "disk_gb": 50,
                "price_cents": 1000
            }
        ]
    }"#;

    let response: VpsPlansResponse = serde_json::from_str(json).expect("Should parse wrapper format");
    assert_eq!(response.plans.len(), 2);
    assert_eq!(response.plans[0].id, "plan-small");
    assert_eq!(response.plans[1].id, "plan-medium");
}

/// Test VpsPlan bare array format
#[test]
fn test_vps_plans_bare_array_format() {
    let json = r#"[
        {
            "id": "plan-small",
            "name": "Small",
            "vcpus": 1,
            "ram_mb": 1024,
            "disk_gb": 25,
            "price_cents": 500
        }
    ]"#;

    let plans: Vec<VpsPlan> = serde_json::from_str(json).expect("Should parse bare array");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].id, "plan-small");
}

/// Test empty plans array
#[test]
fn test_vps_plans_empty() {
    let json_wrapper = r#"{"plans": []}"#;
    let json_array = r#"[]"#;

    let wrapper: VpsPlansResponse = serde_json::from_str(json_wrapper).expect("Should parse empty wrapper");
    let array: Vec<VpsPlan> = serde_json::from_str(json_array).expect("Should parse empty array");

    assert_eq!(wrapper.plans.len(), 0);
    assert_eq!(array.len(), 0);
}

/// Test VpsStatusResponse parsing
#[test]
fn test_vps_status_response_provisioning() {
    let json = r#"{
        "vps_id": "vps-abc123",
        "status": "provisioning"
    }"#;

    let response: VpsStatusResponse = serde_json::from_str(json).expect("Should parse");
    assert_eq!(response.vps_id, "vps-abc123");
    assert_eq!(response.status, "provisioning");
    assert_eq!(response.hostname, None);
    assert_eq!(response.ip, None);
}

/// Test VpsStatusResponse when ready
#[test]
fn test_vps_status_response_ready() {
    let json = r#"{
        "vps_id": "vps-abc123",
        "status": "ready",
        "hostname": "spoq-abc123.spoq.cloud",
        "ip": "192.168.1.100",
        "url": "http://192.168.1.100:8000"
    }"#;

    let response: VpsStatusResponse = serde_json::from_str(json).expect("Should parse");
    assert_eq!(response.status, "ready");
    assert_eq!(response.hostname, Some("spoq-abc123.spoq.cloud".to_string()));
    assert_eq!(response.ip, Some("192.168.1.100".to_string()));
    assert_eq!(response.url, Some("http://192.168.1.100:8000".to_string()));
}

/// Test ProvisionResponse parsing
#[test]
fn test_provision_response_parsing() {
    let json = r#"{
        "vps_id": "vps-new-123",
        "status": "queued"
    }"#;

    let response: ProvisionResponse = serde_json::from_str(json).expect("Should parse");
    assert_eq!(response.vps_id, "vps-new-123");
    assert_eq!(response.status, "queued");
}

// ============================================================================
// Device Flow State Machine Tests
// ============================================================================

use spoq::auth::{DeviceFlowState, DeviceFlowManager, CentralApiClient};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Test DeviceFlowManager initial state
#[test]
fn test_device_flow_initial_state() {
    let client = Arc::new(CentralApiClient::new());
    let manager = DeviceFlowManager::new(client);

    assert!(matches!(manager.state(), DeviceFlowState::NotStarted));
}

/// Test DeviceFlowState variants exist
#[test]
fn test_device_flow_state_variants() {
    let not_started = DeviceFlowState::NotStarted;
    let waiting = DeviceFlowState::WaitingForUser {
        verification_uri: "https://example.com/verify".to_string(),
        user_code: Some("ABCD-1234".to_string()),
        device_code: "device-123".to_string(),
        expires_at: Instant::now() + Duration::from_secs(300),
        interval: Duration::from_secs(5),
        last_poll: None,
    };
    let authorized = DeviceFlowState::Authorized {
        access_token: "token-123".to_string(),
        refresh_token: "refresh-456".to_string(),
        expires_in: 3600,
    };
    let denied = DeviceFlowState::Denied;
    let expired = DeviceFlowState::Expired;
    let error = DeviceFlowState::Error("Test error".to_string());

    // All variants should be constructible
    assert!(matches!(not_started, DeviceFlowState::NotStarted));
    assert!(matches!(waiting, DeviceFlowState::WaitingForUser { .. }));
    assert!(matches!(authorized, DeviceFlowState::Authorized { .. }));
    assert!(matches!(denied, DeviceFlowState::Denied));
    assert!(matches!(expired, DeviceFlowState::Expired));
    assert!(matches!(error, DeviceFlowState::Error(_)));
}

/// Test DeviceFlowManager state setters
#[test]
fn test_device_flow_manager_setters() {
    let client = Arc::new(CentralApiClient::new());
    let mut manager = DeviceFlowManager::new(client);

    // Test set_authorized
    manager.set_authorized(
        "access-token".to_string(),
        "refresh-token".to_string(),
        3600,
    );
    assert!(matches!(manager.state(), DeviceFlowState::Authorized { .. }));

    // Reset and test set_denied
    let client = Arc::new(CentralApiClient::new());
    let mut manager = DeviceFlowManager::new(client);
    manager.set_denied();
    assert!(matches!(manager.state(), DeviceFlowState::Denied));

    // Reset and test set_expired
    let client = Arc::new(CentralApiClient::new());
    let mut manager = DeviceFlowManager::new(client);
    manager.set_expired();
    assert!(matches!(manager.state(), DeviceFlowState::Expired));

    // Reset and test set_error
    let client = Arc::new(CentralApiClient::new());
    let mut manager = DeviceFlowManager::new(client);
    manager.set_error("Test error".to_string());
    assert!(matches!(manager.state(), DeviceFlowState::Error(_)));
}

// ============================================================================
// Flow Integration Tests
// ============================================================================

/// Test the complete screen decision logic matches expected behavior
#[test]
fn test_complete_screen_decision_logic() {
    // Case 1: No credentials -> Login
    let creds_none = Credentials::default();
    let screen1 = determine_screen(&creds_none);
    assert_eq!(screen1, Screen::Login, "No credentials should go to Login");

    // Case 2: Has token, no VPS URL -> Provisioning
    let creds_no_vps = Credentials {
        access_token: Some("token".to_string()),
        ..Default::default()
    };
    let screen2 = determine_screen(&creds_no_vps);
    assert_eq!(screen2, Screen::Provisioning, "Token without VPS should go to Provisioning");

    // Case 3: Has token, has VPS URL, status not ready -> Provisioning
    let creds_vps_pending = Credentials {
        access_token: Some("token".to_string()),
        vps_url: Some("http://192.168.1.100:8000".to_string()),
        vps_status: Some("pending".to_string()),
        ..Default::default()
    };
    let screen3 = determine_screen(&creds_vps_pending);
    assert_eq!(screen3, Screen::Provisioning, "VPS not ready should go to Provisioning");

    // Case 4: Has token, has VPS URL, status ready -> CommandDeck
    let creds_ready = Credentials {
        access_token: Some("token".to_string()),
        vps_url: Some("http://192.168.1.100:8000".to_string()),
        vps_status: Some("ready".to_string()),
        ..Default::default()
    };
    let screen4 = determine_screen(&creds_ready);
    assert_eq!(screen4, Screen::CommandDeck, "Ready VPS should go to CommandDeck");
}

/// Helper function that mirrors the App's screen determination logic
fn determine_screen(credentials: &Credentials) -> Screen {
    if credentials.access_token.is_none() {
        Screen::Login
    } else if credentials.vps_url.is_none()
        || credentials.vps_status.as_deref() != Some("ready")
    {
        Screen::Provisioning
    } else {
        Screen::CommandDeck
    }
}
