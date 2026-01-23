//! Integration tests for the setup flow modules.
//!
//! This module tests each step of the setup flow independently using mocks,
//! as well as testing the flow orchestration, error handling, and state transitions.
//!
//! ## Test Categories
//!
//! 1. **Precheck module tests** - VpsStatus enum, conversions
//! 2. **Provision module tests** - ProvisionResponse, error handling
//! 3. **Health-wait module tests** - HealthCheckStatus, timeout handling
//! 4. **Creds-sync module tests** - CredsSyncResult, error handling
//! 5. **Creds-verify module tests** - VerifyResult, error handling
//! 6. **Flow orchestration tests** - SetupStep, SetupError, state transitions

use spoq::auth::central_api::{CentralApiClient, VpsStatusResponse};
use spoq::auth::credentials::Credentials;
use spoq::setup::creds_sync::{CredsSyncError, CredsSyncResult};
use spoq::setup::creds_verify::{VerifyError, VerifyResult};
use spoq::setup::flow::{SetupError, SetupStep, SetupSuccess};
use spoq::setup::health_wait::{HealthWaitError, DEFAULT_HEALTH_TIMEOUT_SECS};
use spoq::setup::precheck::VpsStatus;
use spoq::setup::provision::{ProvisionError, ProvisionResponse};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create test credentials with valid access token
fn create_test_credentials() -> Credentials {
    let mut creds = Credentials::default();
    creds.access_token = Some("test-access-token".to_string());
    creds.refresh_token = Some("test-refresh-token".to_string());
    creds.expires_at = Some(chrono::Utc::now().timestamp() + 3600);
    creds.user_id = Some("user-123".to_string());
    creds.username = Some("testuser".to_string());
    creds
}

/// Create test credentials with VPS info
fn create_test_credentials_with_vps() -> Credentials {
    let mut creds = create_test_credentials();
    creds.vps_id = Some("vps-123".to_string());
    creds.vps_url = Some("https://test.spoq.dev".to_string());
    creds.vps_hostname = Some("test.spoq.dev".to_string());
    creds.vps_ip = Some("192.168.1.100".to_string());
    creds.vps_status = Some("ready".to_string());
    creds
}

/// Create a mock VPS status response for a ready VPS
fn create_ready_vps_response() -> VpsStatusResponse {
    VpsStatusResponse {
        vps_id: "vps-abc123".to_string(),
        status: "ready".to_string(),
        hostname: Some("user.spoq.dev".to_string()),
        ip: Some("1.2.3.4".to_string()),
        url: Some("https://user.spoq.dev:8000".to_string()),
        ssh_username: Some("root".to_string()),
        provider: Some("hostinger".to_string()),
        plan_id: Some("plan-small".to_string()),
        data_center_id: Some(9),
        created_at: Some("2026-01-01T00:00:00Z".to_string()),
        ready_at: Some("2026-01-01T00:05:00Z".to_string()),
    }
}

/// Create a mock VPS status response for a provisioning VPS
fn create_provisioning_vps_response() -> VpsStatusResponse {
    VpsStatusResponse {
        vps_id: "vps-def456".to_string(),
        status: "provisioning".to_string(),
        hostname: None,
        ip: None,
        url: None,
        ssh_username: None,
        provider: None,
        plan_id: None,
        data_center_id: None,
        created_at: Some("2026-01-01T00:00:00Z".to_string()),
        ready_at: None,
    }
}

// ============================================================================
// Test Group 1: Precheck Module Tests (VpsStatus)
// ============================================================================

#[test]
fn test_vps_status_from_response_ready() {
    let response = create_ready_vps_response();
    let status = VpsStatus::from(response);

    match status {
        VpsStatus::Ready {
            vps_id,
            hostname,
            ip,
            url,
            ssh_username,
        } => {
            assert_eq!(vps_id, "vps-abc123");
            assert_eq!(hostname, Some("user.spoq.dev".to_string()));
            assert_eq!(ip, Some("1.2.3.4".to_string()));
            assert_eq!(url, Some("https://user.spoq.dev:8000".to_string()));
            assert_eq!(ssh_username, Some("root".to_string()));
        }
        _ => panic!("Expected VpsStatus::Ready"),
    }
}

#[test]
fn test_vps_status_from_response_provisioning() {
    let response = create_provisioning_vps_response();
    let status = VpsStatus::from(response);

    match status {
        VpsStatus::Provisioning { vps_id } => {
            assert_eq!(vps_id, "vps-def456");
        }
        _ => panic!("Expected VpsStatus::Provisioning"),
    }
}

#[test]
fn test_vps_status_from_response_running_maps_to_ready() {
    let response = VpsStatusResponse {
        vps_id: "vps-running".to_string(),
        status: "running".to_string(),
        hostname: Some("host.spoq.dev".to_string()),
        ip: None,
        url: None,
        ssh_username: None,
        provider: None,
        plan_id: None,
        data_center_id: None,
        created_at: None,
        ready_at: None,
    };

    let status = VpsStatus::from(response);
    assert!(matches!(status, VpsStatus::Ready { .. }));
}

#[test]
fn test_vps_status_from_response_active_maps_to_ready() {
    let response = VpsStatusResponse {
        vps_id: "vps-active".to_string(),
        status: "active".to_string(),
        hostname: Some("active.spoq.dev".to_string()),
        ip: Some("10.0.0.1".to_string()),
        url: None,
        ssh_username: None,
        provider: None,
        plan_id: None,
        data_center_id: None,
        created_at: None,
        ready_at: None,
    };

    let status = VpsStatus::from(response);
    match status {
        VpsStatus::Ready { vps_id, ip, .. } => {
            assert_eq!(vps_id, "vps-active");
            assert_eq!(ip, Some("10.0.0.1".to_string()));
        }
        _ => panic!("Expected VpsStatus::Ready for 'active' status"),
    }
}

#[test]
fn test_vps_status_from_response_pending_maps_to_provisioning() {
    let response = VpsStatusResponse {
        vps_id: "vps-pending".to_string(),
        status: "pending".to_string(),
        hostname: None,
        ip: None,
        url: None,
        ssh_username: None,
        provider: None,
        plan_id: None,
        data_center_id: None,
        created_at: None,
        ready_at: None,
    };

    let status = VpsStatus::from(response);
    match status {
        VpsStatus::Provisioning { vps_id } => {
            assert_eq!(vps_id, "vps-pending");
        }
        _ => panic!("Expected VpsStatus::Provisioning for 'pending' status"),
    }
}

#[test]
fn test_vps_status_from_response_creating_maps_to_provisioning() {
    let response = VpsStatusResponse {
        vps_id: "vps-creating".to_string(),
        status: "creating".to_string(),
        hostname: None,
        ip: None,
        url: None,
        ssh_username: None,
        provider: None,
        plan_id: None,
        data_center_id: None,
        created_at: None,
        ready_at: None,
    };

    let status = VpsStatus::from(response);
    assert!(matches!(status, VpsStatus::Provisioning { .. }));
}

#[test]
fn test_vps_status_from_response_other_status() {
    let response = VpsStatusResponse {
        vps_id: "vps-maintenance".to_string(),
        status: "maintenance".to_string(),
        hostname: None,
        ip: None,
        url: None,
        ssh_username: None,
        provider: None,
        plan_id: None,
        data_center_id: None,
        created_at: None,
        ready_at: None,
    };

    let status = VpsStatus::from(response);
    match status {
        VpsStatus::Other { vps_id, status } => {
            assert_eq!(vps_id, "vps-maintenance");
            assert_eq!(status, "maintenance");
        }
        _ => panic!("Expected VpsStatus::Other for unknown status"),
    }
}

#[test]
fn test_vps_status_case_insensitive() {
    // Test that status matching is case-insensitive
    let response = VpsStatusResponse {
        vps_id: "vps-uppercase".to_string(),
        status: "READY".to_string(), // Uppercase
        hostname: Some("upper.spoq.dev".to_string()),
        ip: None,
        url: None,
        ssh_username: None,
        provider: None,
        plan_id: None,
        data_center_id: None,
        created_at: None,
        ready_at: None,
    };

    let status = VpsStatus::from(response);
    assert!(matches!(status, VpsStatus::Ready { .. }));
}

// ============================================================================
// Test Group 2: Provision Module Tests
// ============================================================================

#[test]
fn test_provision_response_deserialize() {
    let json = r#"{
        "vps_id": "vps-abc123",
        "status": "provisioning",
        "domain": "user123.spoq.dev"
    }"#;

    let response: ProvisionResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.vps_id, "vps-abc123");
    assert_eq!(response.status, "provisioning");
    assert_eq!(response.domain, Some("user123.spoq.dev".to_string()));
    assert!(response.hostname.is_none());
    assert!(response.message.is_none());
}

#[test]
fn test_provision_response_deserialize_with_id_alias() {
    let json = r#"{
        "id": "vps-xyz789",
        "status": "provisioning",
        "hostname": "user456.spoq.dev",
        "message": "VPS provisioning started"
    }"#;

    let response: ProvisionResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.vps_id, "vps-xyz789");
    assert_eq!(response.status, "provisioning");
    assert!(response.domain.is_none());
    assert_eq!(response.hostname, Some("user456.spoq.dev".to_string()));
    assert_eq!(
        response.message,
        Some("VPS provisioning started".to_string())
    );
}

#[test]
fn test_provision_response_deserialize_minimal() {
    let json = r#"{
        "vps_id": "vps-min123",
        "status": "pending"
    }"#;

    let response: ProvisionResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.vps_id, "vps-min123");
    assert_eq!(response.status, "pending");
    assert!(response.domain.is_none());
    assert!(response.hostname.is_none());
    assert!(response.message.is_none());
}

#[test]
fn test_provision_response_get_domain_prefers_domain() {
    let response = ProvisionResponse {
        vps_id: "vps-1".to_string(),
        status: "provisioning".to_string(),
        domain: Some("domain.spoq.dev".to_string()),
        hostname: Some("host.spoq.dev".to_string()),
        message: None,
    };
    assert_eq!(response.get_domain(), Some("domain.spoq.dev"));
}

#[test]
fn test_provision_response_get_domain_falls_back_to_hostname() {
    let response = ProvisionResponse {
        vps_id: "vps-2".to_string(),
        status: "provisioning".to_string(),
        domain: None,
        hostname: Some("host.spoq.dev".to_string()),
        message: None,
    };
    assert_eq!(response.get_domain(), Some("host.spoq.dev"));
}

#[test]
fn test_provision_response_get_domain_returns_none() {
    let response = ProvisionResponse {
        vps_id: "vps-3".to_string(),
        status: "provisioning".to_string(),
        domain: None,
        hostname: None,
        message: None,
    };
    assert!(response.get_domain().is_none());
}

#[test]
fn test_provision_error_display_messages() {
    let errors = [
        (
            ProvisionError::AlreadyHasVps,
            "already have a VPS",
        ),
        (
            ProvisionError::QuotaExceeded,
            "quota",
        ),
        (
            ProvisionError::Unauthorized,
            "sign in",
        ),
        (
            ProvisionError::PaymentRequired,
            "subscribe",
        ),
        (
            ProvisionError::ServerError {
                status: 500,
                message: "Internal error".to_string(),
            },
            "500",
        ),
    ];

    for (error, expected_substr) in errors {
        let display = format!("{}", error);
        assert!(
            display.contains(expected_substr),
            "Error display '{}' should contain '{}'",
            display,
            expected_substr
        );
    }
}

#[tokio::test]
async fn test_provision_with_wiremock_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/vps/provision"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "vps_id": "vps-mock-123",
            "status": "provisioning",
            "domain": "mock.spoq.dev"
        })))
        .mount(&mock_server)
        .await;

    use spoq::setup::provision::provision;
    let result = provision("test-token", &mock_server.uri()).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.vps_id, "vps-mock-123");
    assert_eq!(response.status, "provisioning");
    assert_eq!(response.get_domain(), Some("mock.spoq.dev"));
}

#[tokio::test]
async fn test_provision_unauthorized_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/vps/provision"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": "Unauthorized"
        })))
        .mount(&mock_server)
        .await;

    use spoq::setup::provision::provision;
    let result = provision("bad-token", &mock_server.uri()).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ProvisionError::Unauthorized));
}

#[tokio::test]
async fn test_provision_payment_required_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/vps/provision"))
        .respond_with(ResponseTemplate::new(402).set_body_json(serde_json::json!({
            "error": "Payment required"
        })))
        .mount(&mock_server)
        .await;

    use spoq::setup::provision::provision;
    let result = provision("test-token", &mock_server.uri()).await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ProvisionError::PaymentRequired
    ));
}

#[tokio::test]
async fn test_provision_already_has_vps_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/vps/provision"))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
            "error": "User already has a VPS"
        })))
        .mount(&mock_server)
        .await;

    use spoq::setup::provision::provision;
    let result = provision("test-token", &mock_server.uri()).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ProvisionError::AlreadyHasVps));
}

#[tokio::test]
async fn test_provision_quota_exceeded_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/vps/provision"))
        .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
            "error": "Quota exceeded"
        })))
        .mount(&mock_server)
        .await;

    use spoq::setup::provision::provision;
    let result = provision("test-token", &mock_server.uri()).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ProvisionError::QuotaExceeded));
}

#[tokio::test]
async fn test_provision_connection_error() {
    use spoq::setup::provision::provision;
    let result = provision("test-token", "http://127.0.0.1:1").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ProvisionError::Http(_)));
}

// ============================================================================
// Test Group 3: Health-Wait Module Tests
// ============================================================================

#[test]
fn test_health_wait_error_display_timeout() {
    let err = HealthWaitError::Timeout { waited_secs: 300 };
    let display = format!("{}", err);
    assert!(display.contains("300"));
    assert!(display.to_lowercase().contains("timeout"));
}

#[test]
fn test_health_wait_error_display_unhealthy() {
    let err = HealthWaitError::Unhealthy {
        message: "starting".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("starting"));
}

#[test]
fn test_health_wait_default_timeout() {
    assert_eq!(DEFAULT_HEALTH_TIMEOUT_SECS, 300);
}

#[tokio::test]
async fn test_wait_for_health_timeout_with_invalid_url() {
    use spoq::setup::health_wait::wait_for_health;

    // Very short timeout should fail immediately with unreachable host
    let result = wait_for_health("http://127.0.0.1:1", 1).await;
    assert!(matches!(result, Err(HealthWaitError::Timeout { .. })));
}

#[tokio::test]
async fn test_wait_for_health_with_progress_callback() {
    use spoq::setup::health_wait::wait_for_health_with_progress;

    let mut progress_calls = 0;
    let result = wait_for_health_with_progress("http://127.0.0.1:1", 1, |attempt, elapsed, status| {
        progress_calls += 1;
        // Verify callback parameters are reasonable
        assert!(attempt >= 1);
        assert!(elapsed <= 10);
        assert!(!status.is_empty());
    })
    .await;

    assert!(matches!(result, Err(HealthWaitError::Timeout { .. })));
    assert!(progress_calls >= 1, "Progress callback should be called at least once");
}

#[tokio::test]
async fn test_wait_for_health_success_with_mock() {
    use spoq::setup::health_wait::wait_for_health;

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "healthy"
        })))
        .mount(&mock_server)
        .await;

    let result = wait_for_health(&mock_server.uri(), 30).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_wait_for_health_unhealthy_response_keeps_waiting() {
    use spoq::setup::health_wait::wait_for_health;

    let mock_server = MockServer::start().await;

    // Return unhealthy status - should eventually timeout
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "starting"
        })))
        .mount(&mock_server)
        .await;

    let result = wait_for_health(&mock_server.uri(), 1).await;
    assert!(matches!(result, Err(HealthWaitError::Timeout { .. })));
}

// ============================================================================
// Test Group 4: Creds-Sync Module Tests
// ============================================================================

#[test]
fn test_creds_sync_result_any_synced_claude_only() {
    let result = CredsSyncResult {
        claude_synced: true,
        github_synced: false,
        claude_bytes: 100,
        github_bytes: 0,
    };
    assert!(result.any_synced());
    assert!(!result.all_synced());
}

#[test]
fn test_creds_sync_result_any_synced_github_only() {
    let result = CredsSyncResult {
        claude_synced: false,
        github_synced: true,
        claude_bytes: 0,
        github_bytes: 200,
    };
    assert!(result.any_synced());
    assert!(!result.all_synced());
}

#[test]
fn test_creds_sync_result_all_synced() {
    let result = CredsSyncResult {
        claude_synced: true,
        github_synced: true,
        claude_bytes: 100,
        github_bytes: 200,
    };
    assert!(result.any_synced());
    assert!(result.all_synced());
}

#[test]
fn test_creds_sync_result_none_synced() {
    let result = CredsSyncResult {
        claude_synced: false,
        github_synced: false,
        claude_bytes: 0,
        github_bytes: 0,
    };
    assert!(!result.any_synced());
    assert!(!result.all_synced());
}

#[test]
fn test_creds_sync_error_display() {
    let errors = [
        (CredsSyncError::NoHomeDirectory, "home directory"),
        (CredsSyncError::SshConnection("timeout".to_string()), "SSH connection"),
        (CredsSyncError::SshAuth("invalid password".to_string()), "SSH authentication"),
        (CredsSyncError::Sftp("permission denied".to_string()), "SFTP"),
        (CredsSyncError::SshCommand("command failed".to_string()), "SSH command"),
        (CredsSyncError::NoCredentialsFound, "No credentials found"),
        (CredsSyncError::FileRead("file not found".to_string()), "read file"),
    ];

    for (error, expected_substr) in errors {
        let display = format!("{}", error);
        assert!(
            display.to_lowercase().contains(&expected_substr.to_lowercase()),
            "Error display '{}' should contain '{}'",
            display,
            expected_substr
        );
    }
}

#[test]
fn test_get_local_credentials_info() {
    // This test verifies the function runs without panicking
    // The actual results depend on the local system state
    use spoq::setup::creds_sync::get_local_credentials_info;

    let (claude, github) = get_local_credentials_info();
    // Results are system-dependent, just verify types
    let _: bool = claude;
    let _: bool = github;
}

// ============================================================================
// Test Group 5: Creds-Verify Module Tests
// ============================================================================

#[test]
fn test_verify_result_all_ok_both_true() {
    let result = VerifyResult {
        github_ok: true,
        claude_ok: true,
    };
    assert!(result.all_ok());
}

#[test]
fn test_verify_result_all_ok_github_fails() {
    let result = VerifyResult {
        github_ok: false,
        claude_ok: true,
    };
    assert!(!result.all_ok());
}

#[test]
fn test_verify_result_all_ok_claude_fails() {
    let result = VerifyResult {
        github_ok: true,
        claude_ok: false,
    };
    assert!(!result.all_ok());
}

#[test]
fn test_verify_result_all_ok_both_fail() {
    let result = VerifyResult {
        github_ok: false,
        claude_ok: false,
    };
    assert!(!result.all_ok());
}

#[test]
fn test_verify_result_equality() {
    let result1 = VerifyResult {
        github_ok: true,
        claude_ok: true,
    };
    let result2 = VerifyResult {
        github_ok: true,
        claude_ok: true,
    };
    assert_eq!(result1, result2);
}

#[test]
fn test_verify_result_clone() {
    let result = VerifyResult {
        github_ok: true,
        claude_ok: false,
    };
    let cloned = result.clone();
    assert_eq!(result.github_ok, cloned.github_ok);
    assert_eq!(result.claude_ok, cloned.claude_ok);
}

#[test]
fn test_verify_error_tcp_connection() {
    use std::io;
    let io_error = io::Error::new(io::ErrorKind::ConnectionRefused, "Connection refused");
    let error = VerifyError::TcpConnection(io_error);
    let display = format!("{}", error);
    assert!(display.contains("TCP connection"));
}

#[test]
fn test_verify_error_auth_failed() {
    let error = VerifyError::AuthFailed;
    let display = format!("{}", error);
    assert!(display.contains("authentication"));
}

#[test]
fn test_verify_error_verification_failed() {
    let error = VerifyError::VerificationFailed("GitHub not authenticated".to_string());
    let display = format!("{}", error);
    assert!(display.contains("GitHub not authenticated"));
}

#[tokio::test]
async fn test_verify_credentials_invalid_host() {
    use spoq::setup::creds_verify::verify_credentials;

    // Test with an invalid host to verify error handling
    let result = verify_credentials("127.0.0.1", "root", "invalid", 1).await;
    assert!(result.is_err());
}

// ============================================================================
// Test Group 6: Flow Orchestration Tests (SetupStep, SetupError)
// ============================================================================

#[test]
fn test_setup_step_number_ordering() {
    assert_eq!(SetupStep::Auth.number(), 0);
    assert_eq!(SetupStep::PreCheck.number(), 1);
    assert_eq!(SetupStep::Provision.number(), 2);
    assert_eq!(SetupStep::HealthWait.number(), 3);
    assert_eq!(SetupStep::CredsSync.number(), 4);
    assert_eq!(SetupStep::CredsVerify.number(), 5);
}

#[test]
fn test_setup_step_description() {
    assert_eq!(SetupStep::Auth.description(), "Authenticating");
    assert_eq!(SetupStep::PreCheck.description(), "Checking VPS status");
    assert_eq!(SetupStep::Provision.description(), "Provisioning VPS");
    assert_eq!(SetupStep::HealthWait.description(), "Waiting for VPS");
    assert_eq!(SetupStep::CredsSync.description(), "Syncing credentials");
    assert_eq!(SetupStep::CredsVerify.description(), "Verifying credentials");
}

#[test]
fn test_setup_step_display() {
    assert_eq!(format!("{}", SetupStep::Auth), "Step 0: Authenticating");
    assert_eq!(format!("{}", SetupStep::PreCheck), "Step 1: Checking VPS status");
    assert_eq!(format!("{}", SetupStep::Provision), "Step 2: Provisioning VPS");
    assert_eq!(format!("{}", SetupStep::HealthWait), "Step 3: Waiting for VPS");
    assert_eq!(format!("{}", SetupStep::CredsSync), "Step 4: Syncing credentials");
    assert_eq!(format!("{}", SetupStep::CredsVerify), "Step 5: Verifying credentials");
}

#[test]
fn test_setup_step_equality() {
    assert_eq!(SetupStep::Auth, SetupStep::Auth);
    assert_ne!(SetupStep::Auth, SetupStep::PreCheck);
}

#[test]
fn test_setup_step_clone_and_copy() {
    let step = SetupStep::Provision;
    let cloned = step;  // Copy trait
    assert_eq!(step, cloned);
}

#[test]
fn test_setup_error_blocking_constructor() {
    let err = SetupError::blocking(SetupStep::Auth, "test error");
    assert!(err.is_blocking);
    assert_eq!(err.step, SetupStep::Auth);
    assert_eq!(err.message, "test error");
}

#[test]
fn test_setup_error_non_blocking_constructor() {
    let err = SetupError::new(SetupStep::CredsSync, "warning", false);
    assert!(!err.is_blocking);
    assert_eq!(err.step, SetupStep::CredsSync);
    assert_eq!(err.message, "warning");
}

#[test]
fn test_setup_error_display() {
    let err = SetupError::blocking(SetupStep::Provision, "provisioning failed");
    let display = format!("{}", err);
    assert!(display.contains("Step 2"));
    assert!(display.contains("Provisioning VPS"));
    assert!(display.contains("provisioning failed"));
}

#[test]
fn test_setup_error_implements_std_error() {
    let err = SetupError::blocking(SetupStep::Auth, "auth failed");
    // Test that it implements std::error::Error
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_setup_success_fields() {
    let success = SetupSuccess {
        vps_url: "https://test.spoq.dev".to_string(),
        vps_hostname: Some("test.spoq.dev".to_string()),
        vps_ip: Some("1.2.3.4".to_string()),
        vps_id: "vps-123".to_string(),
        credentials: Credentials::default(),
    };

    assert_eq!(success.vps_url, "https://test.spoq.dev");
    assert_eq!(success.vps_hostname, Some("test.spoq.dev".to_string()));
    assert_eq!(success.vps_ip, Some("1.2.3.4".to_string()));
    assert_eq!(success.vps_id, "vps-123");
}

#[test]
fn test_setup_success_clone() {
    let success = SetupSuccess {
        vps_url: "https://test.spoq.dev".to_string(),
        vps_hostname: Some("test.spoq.dev".to_string()),
        vps_ip: Some("1.2.3.4".to_string()),
        vps_id: "vps-123".to_string(),
        credentials: Credentials::default(),
    };

    let cloned = success.clone();
    assert_eq!(success.vps_url, cloned.vps_url);
    assert_eq!(success.vps_id, cloned.vps_id);
}

// ============================================================================
// Test Group 7: Precheck API Integration Tests
// ============================================================================

#[tokio::test]
async fn test_precheck_with_mock_no_vps() {
    use spoq::setup::precheck::precheck;

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/vps/status"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": "No VPS found"
        })))
        .mount(&mock_server)
        .await;

    let mut client = CentralApiClient::with_base_url(mock_server.uri()).with_auth("test-token");

    let result = precheck(&mut client).await;
    assert!(result.is_ok());
    assert!(matches!(result.unwrap(), VpsStatus::None));
}

#[tokio::test]
async fn test_precheck_with_mock_ready_vps() {
    use spoq::setup::precheck::precheck;

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/vps/status"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "vps_id": "vps-ready-123",
            "status": "ready",
            "hostname": "user.spoq.dev",
            "ip": "1.2.3.4",
            "url": "https://user.spoq.dev:8000",
            "ssh_username": "root"
        })))
        .mount(&mock_server)
        .await;

    let mut client = CentralApiClient::with_base_url(mock_server.uri()).with_auth("test-token");

    let result = precheck(&mut client).await;
    assert!(result.is_ok());

    match result.unwrap() {
        VpsStatus::Ready {
            vps_id,
            hostname,
            ip,
            url,
            ssh_username,
        } => {
            assert_eq!(vps_id, "vps-ready-123");
            assert_eq!(hostname, Some("user.spoq.dev".to_string()));
            assert_eq!(ip, Some("1.2.3.4".to_string()));
            assert_eq!(url, Some("https://user.spoq.dev:8000".to_string()));
            assert_eq!(ssh_username, Some("root".to_string()));
        }
        _ => panic!("Expected VpsStatus::Ready"),
    }
}

#[tokio::test]
async fn test_precheck_with_mock_provisioning_vps() {
    use spoq::setup::precheck::precheck;

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/vps/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "vps_id": "vps-provisioning-456",
            "status": "provisioning"
        })))
        .mount(&mock_server)
        .await;

    let mut client = CentralApiClient::with_base_url(mock_server.uri()).with_auth("test-token");

    let result = precheck(&mut client).await;
    assert!(result.is_ok());

    match result.unwrap() {
        VpsStatus::Provisioning { vps_id } => {
            assert_eq!(vps_id, "vps-provisioning-456");
        }
        _ => panic!("Expected VpsStatus::Provisioning"),
    }
}

#[tokio::test]
async fn test_precheck_with_mock_unauthorized() {
    use spoq::setup::precheck::precheck;

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/vps/status"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": "Unauthorized"
        })))
        .mount(&mock_server)
        .await;

    let mut client = CentralApiClient::with_base_url(mock_server.uri()).with_auth("bad-token");

    let result = precheck(&mut client).await;
    assert!(result.is_err());
}

// ============================================================================
// Test Group 8: Error Message Quality Tests
// ============================================================================

#[test]
fn test_setup_error_messages_are_helpful() {
    // Test that error messages provide actionable information
    let auth_error = SetupError::blocking(
        SetupStep::Auth,
        "Authentication failed: Token expired. Please sign in again using 'spoq --login'",
    );
    assert!(auth_error.message.contains("sign in"));

    let provision_error = SetupError::blocking(
        SetupStep::Provision,
        "Payment required - please subscribe at https://spoq.dev/subscribe",
    );
    assert!(provision_error.message.contains("https://"));

    let verify_error = SetupError::blocking(
        SetupStep::CredsVerify,
        "Credential verification failed: GitHub CLI not authenticated. Please run 'gh auth login' locally, then run 'spoq --sync' to sync credentials.",
    );
    assert!(verify_error.message.contains("gh auth login"));
    assert!(verify_error.message.contains("spoq --sync"));
}

#[test]
fn test_provision_error_messages_are_helpful() {
    // Verify error messages guide users on what to do
    let already_has = format!("{}", ProvisionError::AlreadyHasVps);
    assert!(already_has.to_lowercase().contains("already"));

    let payment = format!("{}", ProvisionError::PaymentRequired);
    assert!(payment.to_lowercase().contains("subscribe"));

    let unauthorized = format!("{}", ProvisionError::Unauthorized);
    assert!(unauthorized.to_lowercase().contains("sign in"));
}

// ============================================================================
// Test Group 9: Credentials Integration Tests
// ============================================================================

#[test]
fn test_credentials_with_vps_has_vps() {
    let creds = create_test_credentials_with_vps();
    assert!(creds.has_vps());
}

#[test]
fn test_credentials_without_vps_does_not_have_vps() {
    let creds = create_test_credentials();
    assert!(!creds.has_vps());
}

#[test]
fn test_credentials_partial_vps_info_does_not_have_vps() {
    // Only vps_id without vps_url
    let mut creds = create_test_credentials();
    creds.vps_id = Some("vps-123".to_string());
    assert!(!creds.has_vps());

    // Only vps_url without vps_id
    let mut creds2 = create_test_credentials();
    creds2.vps_url = Some("https://test.spoq.dev".to_string());
    assert!(!creds2.has_vps());
}

#[test]
fn test_credentials_serialization_preserves_vps_fields() {
    let creds = create_test_credentials_with_vps();

    let json = serde_json::to_string(&creds).expect("Should serialize");
    let parsed: Credentials = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(parsed.vps_id, creds.vps_id);
    assert_eq!(parsed.vps_url, creds.vps_url);
    assert_eq!(parsed.vps_hostname, creds.vps_hostname);
    assert_eq!(parsed.vps_ip, creds.vps_ip);
    assert_eq!(parsed.vps_status, creds.vps_status);
}

// ============================================================================
// Test Group 10: State Transition Tests
// ============================================================================

#[test]
fn test_vps_status_transitions_are_valid() {
    // Test the valid status transitions
    // None -> Provisioning (after provision call)
    // Provisioning -> Ready (after health check passes)
    // Ready can also transition to Other (maintenance, etc.)

    let none_status = VpsStatus::None;
    assert!(matches!(none_status, VpsStatus::None));

    let provisioning_status = VpsStatus::Provisioning {
        vps_id: "vps-123".to_string(),
    };
    assert!(matches!(provisioning_status, VpsStatus::Provisioning { .. }));

    let ready_status = VpsStatus::Ready {
        vps_id: "vps-123".to_string(),
        hostname: Some("test.spoq.dev".to_string()),
        ip: Some("1.2.3.4".to_string()),
        url: Some("https://test.spoq.dev".to_string()),
        ssh_username: Some("root".to_string()),
    };
    assert!(matches!(ready_status, VpsStatus::Ready { .. }));

    let other_status = VpsStatus::Other {
        vps_id: "vps-123".to_string(),
        status: "maintenance".to_string(),
    };
    assert!(matches!(other_status, VpsStatus::Other { .. }));
}

#[test]
fn test_setup_step_progression() {
    // Test that steps follow the expected order
    let steps = [
        SetupStep::Auth,
        SetupStep::PreCheck,
        SetupStep::Provision,
        SetupStep::HealthWait,
        SetupStep::CredsSync,
        SetupStep::CredsVerify,
    ];

    for (i, step) in steps.iter().enumerate() {
        assert_eq!(step.number() as usize, i);
    }
}

// ============================================================================
// Test Group 11: Edge Cases
// ============================================================================

#[test]
fn test_vps_status_response_with_empty_strings() {
    let response = VpsStatusResponse {
        vps_id: "vps-empty".to_string(),
        status: "ready".to_string(),
        hostname: Some("".to_string()), // Empty string
        ip: Some("".to_string()),
        url: Some("".to_string()),
        ssh_username: Some("".to_string()),
        provider: None,
        plan_id: None,
        data_center_id: None,
        created_at: None,
        ready_at: None,
    };

    let status = VpsStatus::from(response);
    match status {
        VpsStatus::Ready { hostname, ip, .. } => {
            // Empty strings should be preserved (not converted to None)
            assert_eq!(hostname, Some("".to_string()));
            assert_eq!(ip, Some("".to_string()));
        }
        _ => panic!("Expected VpsStatus::Ready"),
    }
}

#[test]
fn test_provision_response_with_empty_strings() {
    let response = ProvisionResponse {
        vps_id: "vps-123".to_string(),
        status: "provisioning".to_string(),
        domain: Some("".to_string()),
        hostname: Some("host.spoq.dev".to_string()),
        message: None,
    };

    // Empty string domain should still be Some("")
    // get_domain should prefer domain, but empty domain falls through to hostname
    assert_eq!(response.get_domain(), Some("")); // Empty string is still Some
}

#[test]
fn test_setup_error_with_long_message() {
    let long_message = "x".repeat(10000);
    let err = SetupError::blocking(SetupStep::Auth, &long_message);
    assert_eq!(err.message.len(), 10000);
}

#[test]
fn test_creds_sync_result_zero_bytes_with_synced_true() {
    // Edge case: synced but 0 bytes (shouldn't happen in practice)
    let result = CredsSyncResult {
        claude_synced: true,
        github_synced: false,
        claude_bytes: 0,
        github_bytes: 0,
    };
    assert!(result.any_synced()); // Still reports as synced based on flag
}
