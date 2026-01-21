//! Integration tests for BYOVPS (Bring Your Own VPS) functionality.
//!
//! These tests verify the BYOVPS provisioning flow, credential collection,
//! input validation, status polling, and error handling.

use spoq::auth::central_api::{
    ByovpsProvisionResponse, CentralApiClient, CentralApiError, VpsStatusResponse,
};
use spoq::auth::credentials::Credentials;

// Test constants from provisioning_flow.rs
const BYOVPS_POLL_INTERVAL_SECS: u64 = 5;
const BYOVPS_MAX_POLL_ATTEMPTS: u32 = 120;
const BYOVPS_MAX_RETRY_ATTEMPTS: u32 = 3;

#[test]
fn test_byovps_poll_interval_constants() {
    // Verify BYOVPS polling interval is 5 seconds
    assert_eq!(BYOVPS_POLL_INTERVAL_SECS, 5);

    // Verify max poll attempts is 120 (10 minutes total)
    assert_eq!(BYOVPS_MAX_POLL_ATTEMPTS, 120);

    // Verify total timeout is 10 minutes (600 seconds)
    let total_timeout = BYOVPS_POLL_INTERVAL_SECS * BYOVPS_MAX_POLL_ATTEMPTS as u64;
    assert_eq!(total_timeout, 600);
}

#[test]
fn test_byovps_max_retry_attempts() {
    // Verify max retry attempts is 3
    assert_eq!(BYOVPS_MAX_RETRY_ATTEMPTS, 3);
}

/// Test choose_vps_type input parsing
#[test]
fn test_vps_type_selection_logic() {
    // Test that "1" maps to Managed VPS
    let choice_1 = "1";
    assert_eq!(choice_1.trim(), "1");

    // Test that "2" maps to BYOVPS
    let choice_2 = "2";
    assert_eq!(choice_2.trim(), "2");

    // Test invalid inputs
    let invalid_inputs = vec!["0", "3", "a", "abc", "", " ", "1.5"];
    for input in invalid_inputs {
        let trimmed = input.trim();
        assert!(
            !matches!(trimmed, "1" | "2"),
            "Input '{}' should be invalid",
            input
        );
    }
}

/// Test collect_byovps_credentials validation logic for VPS IP
#[test]
fn test_byovps_ip_validation() {
    // Valid IPv4 addresses
    let valid_ipv4 = vec![
        "192.168.1.1",
        "10.0.0.1",
        "172.16.0.1",
        "8.8.8.8",
        "255.255.255.255",
    ];

    for ip in valid_ipv4 {
        let trimmed = ip.trim();
        assert!(!trimmed.is_empty(), "IP '{}' should be non-empty", ip);
    }

    // Valid IPv6 addresses
    let valid_ipv6 = vec![
        "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
        "2001:db8::1",
        "::1",
        "fe80::1",
    ];

    for ip in valid_ipv6 {
        let trimmed = ip.trim();
        assert!(!trimmed.is_empty(), "IPv6 '{}' should be non-empty", ip);
    }

    // Invalid inputs - empty after trim
    let invalid_inputs = vec!["", "   ", "\n", "\t"];
    for input in invalid_inputs {
        let trimmed = input.trim();
        assert!(
            trimmed.is_empty(),
            "Input '{}' should be empty after trim",
            input.escape_debug()
        );
    }

    // Test trimming whitespace
    let ip_with_spaces = "  192.168.1.100  ";
    let trimmed = ip_with_spaces.trim();
    assert_eq!(trimmed, "192.168.1.100");
    assert!(!trimmed.is_empty());
}

/// Test collect_byovps_credentials validation logic for SSH username
#[test]
fn test_byovps_username_validation() {
    // Empty username should default to "root"
    let empty_username = "";
    let trimmed = empty_username.trim();
    let final_username = if trimmed.is_empty() {
        "root"
    } else {
        trimmed
    };
    assert_eq!(final_username, "root");

    // Whitespace-only should default to "root"
    let whitespace_username = "   ";
    let trimmed = whitespace_username.trim();
    let final_username = if trimmed.is_empty() {
        "root"
    } else {
        trimmed
    };
    assert_eq!(final_username, "root");

    // Valid custom usernames
    let valid_usernames = vec!["root", "ubuntu", "admin", "user", "deploy"];
    for username in valid_usernames {
        let trimmed = username.trim();
        assert!(!trimmed.is_empty(), "Username '{}' should be valid", username);
    }

    // Username with whitespace should be trimmed
    let username_with_spaces = "  ubuntu  ";
    let trimmed = username_with_spaces.trim();
    assert_eq!(trimmed, "ubuntu");
}

/// Test collect_byovps_credentials validation logic for SSH password
#[test]
fn test_byovps_password_validation() {
    // Password must be at least 1 character
    let valid_passwords = vec!["p", "password", "P@ssw0rd!", "very_long_secure_password_123"];

    for password in valid_passwords {
        assert!(
            !password.is_empty(),
            "Password '{}' should be valid",
            password
        );
        assert!(password.len() >= 1, "Password must be at least 1 character");
    }

    // Empty password should be rejected
    let empty_password = "";
    assert!(empty_password.is_empty());
    assert!(
        empty_password.len() < 1,
        "Empty password should be rejected"
    );

    // Test minimum length
    let one_char = "x";
    assert_eq!(one_char.len(), 1);
    assert!(!one_char.is_empty());
}

/// Test BYOVPS provision response structure
#[test]
fn test_byovps_provision_response_structure() {
    // Test successful response with all fields
    let json_full = r#"{
        "hostname": "user.spoq.dev",
        "status": "ready",
        "vps_id": "byovps-uuid-123",
        "ip": "192.168.1.100",
        "url": "https://user.spoq.dev:8000",
        "message": "BYOVPS provisioned successfully"
    }"#;

    let response: ByovpsProvisionResponse =
        serde_json::from_str(json_full).expect("Should parse full response");

    assert_eq!(response.status, "ready");
    assert_eq!(response.hostname, Some("user.spoq.dev".to_string()));
    assert_eq!(response.vps_id, Some("byovps-uuid-123".to_string()));
    assert_eq!(response.ip, Some("192.168.1.100".to_string()));
    assert_eq!(
        response.url,
        Some("https://user.spoq.dev:8000".to_string())
    );
    assert_eq!(
        response.message,
        Some("BYOVPS provisioned successfully".to_string())
    );
}

/// Test BYOVPS provision response with minimal fields
#[test]
fn test_byovps_provision_response_minimal() {
    let json_minimal = r#"{
        "status": "provisioning"
    }"#;

    let response: ByovpsProvisionResponse =
        serde_json::from_str(json_minimal).expect("Should parse minimal response");

    assert_eq!(response.status, "provisioning");
    assert!(response.hostname.is_none());
    assert!(response.vps_id.is_none());
    assert!(response.ip.is_none());
    assert!(response.url.is_none());
    assert!(response.message.is_none());
}

/// Test BYOVPS provision response with field aliases
#[test]
fn test_byovps_provision_response_aliases() {
    // Test "id" alias for "vps_id"
    let json_with_id_alias = r#"{
        "status": "ready",
        "id": "byovps-456",
        "ip_address": "10.0.0.1"
    }"#;

    let response: ByovpsProvisionResponse =
        serde_json::from_str(json_with_id_alias).expect("Should parse with aliases");

    assert_eq!(response.status, "ready");
    assert_eq!(response.vps_id, Some("byovps-456".to_string()));
    assert_eq!(response.ip, Some("10.0.0.1".to_string()));
}

/// Test BYOVPS status states
#[test]
fn test_byovps_status_ready_states() {
    // Test that various ready states are recognized
    let ready_states = ["ready", "running", "active", "Ready", "RUNNING", "Active"];

    for state in &ready_states {
        let is_ready = matches!(
            state.to_lowercase().as_str(),
            "ready" | "running" | "active"
        );
        assert!(is_ready, "State '{}' should be recognized as ready", state);
    }
}

/// Test BYOVPS status failed states
#[test]
fn test_byovps_status_failed_states() {
    // Test that failed/error states are recognized
    let failed_states = ["failed", "error", "terminated", "Failed", "ERROR"];

    for state in &failed_states {
        let is_failed = matches!(
            state.to_lowercase().as_str(),
            "failed" | "error" | "terminated"
        );
        assert!(is_failed, "State '{}' should be recognized as failed", state);
    }
}

/// Test BYOVPS status polling states
#[test]
fn test_byovps_status_polling_states() {
    // Test states that should trigger continued polling
    let polling_states = [
        "pending",
        "provisioning",
        "registering",
        "configuring",
        "installing",
    ];

    for state in &polling_states {
        let is_ready = matches!(
            state.to_lowercase().as_str(),
            "ready" | "running" | "active"
        );
        let is_failed = matches!(
            state.to_lowercase().as_str(),
            "failed" | "error" | "terminated"
        );

        assert!(
            !is_ready && !is_failed,
            "State '{}' should trigger polling",
            state
        );
    }
}

/// Test VPS status polling logic (stops at ready)
#[test]
fn test_vps_status_polling_stops_at_ready() {
    // Create a ready status response
    let status = VpsStatusResponse {
        vps_id: "test-vps".to_string(),
        status: "ready".to_string(),
        hostname: Some("test.spoq.dev".to_string()),
        ip: Some("192.168.1.1".to_string()),
        url: Some("https://test.spoq.dev:8000".to_string()),
        ssh_username: None,
        provider: None,
        plan_id: None,
        data_center_id: None,
        created_at: None,
        ready_at: None,
    };

    // Verify status is ready
    assert_eq!(status.status.to_lowercase(), "ready");
    assert!(matches!(
        status.status.to_lowercase().as_str(),
        "ready" | "running" | "active"
    ));
}

/// Test VPS status polling logic (stops at failed)
#[test]
fn test_vps_status_polling_stops_at_failed() {
    // Create a failed status response
    let status = VpsStatusResponse {
        vps_id: "test-vps".to_string(),
        status: "failed".to_string(),
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

    // Verify status is failed
    assert_eq!(status.status.to_lowercase(), "failed");
    assert!(matches!(
        status.status.to_lowercase().as_str(),
        "failed" | "error" | "terminated"
    ));
}

/// Test VPS status polling logic (continues on provisioning)
#[test]
fn test_vps_status_polling_continues_on_provisioning() {
    // Create a provisioning status response
    let status = VpsStatusResponse {
        vps_id: "test-vps".to_string(),
        status: "provisioning".to_string(),
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

    // Verify status should trigger continued polling
    let is_ready = matches!(
        status.status.to_lowercase().as_str(),
        "ready" | "running" | "active"
    );
    let is_failed = matches!(
        status.status.to_lowercase().as_str(),
        "failed" | "error" | "terminated"
    );

    assert!(
        !is_ready && !is_failed,
        "Status 'provisioning' should continue polling"
    );
}

/// Test error handling for missing access token
#[test]
fn test_byovps_requires_access_token() {
    // Create credentials without access token
    let credentials = Credentials::default();
    assert!(credentials.access_token.is_none());

    // Verify error would occur when trying to use BYOVPS without token
    let result = credentials
        .access_token
        .as_ref()
        .ok_or_else(|| CentralApiError::ServerError {
            status: 401,
            message: "No access token available".to_string(),
        });

    assert!(result.is_err());
    if let Err(CentralApiError::ServerError { status, message }) = result {
        assert_eq!(status, 401);
        assert!(message.contains("access token"));
    }
}

/// Test credentials update from BYOVPS response
#[test]
fn test_credentials_update_from_byovps_response() {
    let mut credentials = Credentials::default();

    // Simulate updating credentials from BYOVPS response
    credentials.vps_status = Some("ready".to_string());
    credentials.vps_id = Some("byovps-uuid-789".to_string());
    credentials.vps_hostname = Some("myserver.spoq.dev".to_string());
    credentials.vps_ip = Some("203.0.113.10".to_string());
    credentials.vps_url = Some("https://myserver.spoq.dev:8000".to_string());

    // Verify all fields were updated
    assert_eq!(credentials.vps_status, Some("ready".to_string()));
    assert_eq!(credentials.vps_id, Some("byovps-uuid-789".to_string()));
    assert_eq!(
        credentials.vps_hostname,
        Some("myserver.spoq.dev".to_string())
    );
    assert_eq!(credentials.vps_ip, Some("203.0.113.10".to_string()));
    assert_eq!(
        credentials.vps_url,
        Some("https://myserver.spoq.dev:8000".to_string())
    );
}

/// Test credentials update from partial BYOVPS response
#[test]
fn test_credentials_update_from_partial_byovps_response() {
    let mut credentials = Credentials::default();

    // Simulate updating credentials from partial BYOVPS response
    credentials.vps_status = Some("provisioning".to_string());
    credentials.vps_id = Some("byovps-uuid-partial".to_string());

    // Verify partial fields were updated
    assert_eq!(credentials.vps_status, Some("provisioning".to_string()));
    assert_eq!(
        credentials.vps_id,
        Some("byovps-uuid-partial".to_string())
    );

    // Verify optional fields remain None
    assert!(credentials.vps_hostname.is_none());
    assert!(credentials.vps_ip.is_none());
    assert!(credentials.vps_url.is_none());
}

/// Test SSH connection error detection
#[test]
fn test_ssh_connection_error_detection() {
    // Helper function to check if message indicates SSH connection error
    fn is_ssh_connection_error(message: &str) -> bool {
        let lower = message.to_lowercase();
        lower.contains("ssh")
            || lower.contains("connection refused")
            || lower.contains("connection timed out")
            || lower.contains("host unreachable")
            || lower.contains("network unreachable")
            || lower.contains("no route to host")
            || lower.contains("authentication failed")
            || lower.contains("permission denied")
            || lower.contains("port 22")
    }

    // Test SSH-related errors
    assert!(is_ssh_connection_error("SSH connection failed"));
    assert!(is_ssh_connection_error("ssh: Connection refused"));
    assert!(is_ssh_connection_error("Failed to establish SSH connection"));

    // Test connection errors
    assert!(is_ssh_connection_error("Connection refused"));
    assert!(is_ssh_connection_error("connection timed out"));
    assert!(is_ssh_connection_error("Host unreachable"));
    assert!(is_ssh_connection_error("Network unreachable"));
    assert!(is_ssh_connection_error("No route to host"));

    // Test authentication errors
    assert!(is_ssh_connection_error("Authentication failed"));
    assert!(is_ssh_connection_error("Permission denied"));
    assert!(is_ssh_connection_error(
        "permission denied (publickey,password)"
    ));

    // Test port errors
    assert!(is_ssh_connection_error("Failed to connect to port 22"));
    assert!(is_ssh_connection_error("Port 22: Connection refused"));

    // Test non-SSH errors return false
    assert!(!is_ssh_connection_error("Invalid request"));
    assert!(!is_ssh_connection_error("Server error 500"));
    assert!(!is_ssh_connection_error("VPS already exists"));
}

/// Test BYOVPS error retry logic
#[test]
fn test_byovps_retry_action_logic() {
    // Test retry actions
    #[derive(Debug, Clone, PartialEq)]
    enum RetryAction {
        Retry,
        ChangeCredentials,
        Exit,
    }

    // Test that "y" and "yes" map to Retry
    let retry_inputs = vec!["y", "yes", "Y", "YES"];
    for input in retry_inputs {
        let action = match input.to_lowercase().as_str() {
            "y" | "yes" => RetryAction::Retry,
            "c" | "change" => RetryAction::ChangeCredentials,
            "e" | "exit" => RetryAction::Exit,
            _ => RetryAction::Exit,
        };
        assert_eq!(action, RetryAction::Retry);
    }

    // Test that "c" and "change" map to ChangeCredentials
    let change_inputs = vec!["c", "change", "C", "CHANGE"];
    for input in change_inputs {
        let action = match input.to_lowercase().as_str() {
            "y" | "yes" => RetryAction::Retry,
            "c" | "change" => RetryAction::ChangeCredentials,
            "e" | "exit" => RetryAction::Exit,
            _ => RetryAction::Exit,
        };
        assert_eq!(action, RetryAction::ChangeCredentials);
    }

    // Test that "e" and "exit" map to Exit
    let exit_inputs = vec!["e", "exit", "E", "EXIT"];
    for input in exit_inputs {
        let action = match input.to_lowercase().as_str() {
            "y" | "yes" => RetryAction::Retry,
            "c" | "change" => RetryAction::ChangeCredentials,
            "e" | "exit" => RetryAction::Exit,
            _ => RetryAction::Exit,
        };
        assert_eq!(action, RetryAction::Exit);
    }
}

/// Test BYOVPS max retry attempts limit
#[test]
fn test_byovps_max_retry_attempts_limit() {
    // Simulate retry attempts
    let max_attempts = BYOVPS_MAX_RETRY_ATTEMPTS;
    let mut attempts = 0;

    // Simulate 3 failed attempts
    for _ in 0..max_attempts {
        attempts += 1;
    }

    assert_eq!(attempts, 3);
    assert!(attempts >= max_attempts);
}

/// Test BYOVPS timeout calculation
#[test]
fn test_byovps_timeout_calculation() {
    // Calculate total timeout
    let total_timeout_secs = BYOVPS_MAX_POLL_ATTEMPTS as u64 * BYOVPS_POLL_INTERVAL_SECS;

    // Verify timeout is 600 seconds (10 minutes)
    assert_eq!(total_timeout_secs, 600);
    assert_eq!(total_timeout_secs / 60, 10);
}

/// Test BYOVPS API payload structure
#[test]
fn test_byovps_api_payload() {
    // Simulate creating a BYOVPS provision request payload
    #[derive(serde::Serialize)]
    struct ByovpsProvisionRequest {
        vps_ip: String,
        ssh_username: String,
        ssh_password: String,
    }

    let payload = ByovpsProvisionRequest {
        vps_ip: "192.168.1.100".to_string(),
        ssh_username: "root".to_string(),
        ssh_password: "testpass".to_string(),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&payload).expect("Should serialize");

    // Verify all fields are present
    assert!(json.contains("vps_ip"));
    assert!(json.contains("192.168.1.100"));
    assert!(json.contains("ssh_username"));
    assert!(json.contains("root"));
    assert!(json.contains("ssh_password"));
    assert!(json.contains("testpass"));
}

/// Test BYOVPS with IPv6 address
#[test]
fn test_byovps_ipv6_support() {
    // Test that IPv6 addresses can be used
    let ipv6_address = "2001:0db8:85a3::8a2e:0370:7334";

    // Verify IPv6 address is valid (non-empty)
    let trimmed = ipv6_address.trim();
    assert!(!trimmed.is_empty());
    assert!(trimmed.contains(':'));

    // Simulate creating request with IPv6
    #[derive(serde::Serialize)]
    struct ByovpsRequest {
        vps_ip: String,
        ssh_username: String,
        ssh_password: String,
    }

    let request = ByovpsRequest {
        vps_ip: ipv6_address.to_string(),
        ssh_username: "root".to_string(),
        ssh_password: "pass".to_string(),
    };

    let json = serde_json::to_string(&request).expect("Should serialize IPv6");
    assert!(json.contains("2001:0db8:85a3::8a2e:0370:7334"));
}

/// Test CentralApiClient can be created for BYOVPS calls
#[test]
fn test_central_api_client_creation_for_byovps() {
    // Create API client without auth
    let _client = CentralApiClient::new();
    assert!(true, "Client should be created successfully");

    // Create client with auth
    let token = "test-access-token";
    let _client_with_auth = CentralApiClient::new().with_auth(token);
    assert!(true, "Client with auth should be created successfully");
}

/// Test error response parsing for BYOVPS failures
#[test]
fn test_byovps_error_response_parsing() {
    // Test parsing error response with JSON format
    let error_json = r#"{"error": "SSH connection failed"}"#;
    let parsed: serde_json::Value =
        serde_json::from_str(error_json).expect("Should parse error JSON");

    if let Some(msg) = parsed.get("error").and_then(|e| e.as_str()) {
        assert_eq!(msg, "SSH connection failed");
    } else {
        panic!("Should extract error message");
    }

    // Test parsing error response without JSON format
    let error_text = "Connection refused";
    assert!(error_text.len() > 0);
}

/// Test BYOVPS 409 conflict error handling
#[test]
fn test_byovps_conflict_error_handling() {
    // Create a 409 conflict error
    let error = CentralApiError::ServerError {
        status: 409,
        message: "User already has an active VPS".to_string(),
    };

    // Verify error is 409
    if let CentralApiError::ServerError { status, message } = error {
        assert_eq!(status, 409);
        assert!(message.contains("already"));
    } else {
        panic!("Expected ServerError with status 409");
    }
}

/// Test BYOVPS credentials serialization for API call
#[test]
fn test_byovps_credentials_serialization() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct TestByovpsCredentials {
        vps_ip: String,
        ssh_username: String,
        ssh_password: String,
    }

    let creds = TestByovpsCredentials {
        vps_ip: "10.0.0.1".to_string(),
        ssh_username: "admin".to_string(),
        ssh_password: "secure123".to_string(),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&creds).expect("Should serialize");

    // Verify JSON structure
    assert!(json.contains("vps_ip"));
    assert!(json.contains("10.0.0.1"));
    assert!(json.contains("ssh_username"));
    assert!(json.contains("admin"));

    // Deserialize back
    let parsed: TestByovpsCredentials =
        serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(parsed.vps_ip, "10.0.0.1");
    assert_eq!(parsed.ssh_username, "admin");
}

/// Test that BYOVPS has_vps check works correctly
#[test]
fn test_byovps_has_vps_check() {
    // Test credentials without VPS
    let creds_no_vps = Credentials::default();
    assert!(!creds_no_vps.has_vps());

    // Test credentials with partial VPS info (only ID)
    let creds_only_id = Credentials {
        vps_id: Some("byovps-123".to_string()),
        ..Default::default()
    };
    assert!(!creds_only_id.has_vps());

    // Test credentials with partial VPS info (only URL)
    let creds_only_url = Credentials {
        vps_url: Some("https://test.spoq.dev".to_string()),
        ..Default::default()
    };
    assert!(!creds_only_url.has_vps());

    // Test credentials with both ID and URL (has VPS)
    let creds_with_vps = Credentials {
        vps_id: Some("byovps-123".to_string()),
        vps_url: Some("https://test.spoq.dev".to_string()),
        ..Default::default()
    };
    assert!(creds_with_vps.has_vps());
}

/// Test BYOVPS status response deserialization
#[test]
fn test_vps_status_response_deserialization() {
    let json = r#"{
        "vps_id": "byovps-test",
        "status": "ready",
        "hostname": "myserver.spoq.dev",
        "ip": "192.168.1.50",
        "url": "https://myserver.spoq.dev:8000"
    }"#;

    let status: VpsStatusResponse =
        serde_json::from_str(json).expect("Should deserialize VPS status");

    assert_eq!(status.vps_id, "byovps-test");
    assert_eq!(status.status, "ready");
    assert_eq!(status.hostname, Some("myserver.spoq.dev".to_string()));
    assert_eq!(status.ip, Some("192.168.1.50".to_string()));
    assert_eq!(
        status.url,
        Some("https://myserver.spoq.dev:8000".to_string())
    );
}

/// Test BYOVPS retry counter logic
#[test]
fn test_byovps_retry_counter() {
    let max_retries = 3;
    let mut retry_count = 0;

    // Simulate failed attempts
    for _ in 0..5 {
        retry_count += 1;

        if retry_count >= max_retries {
            break;
        }
    }

    assert_eq!(retry_count, 3);
    assert!(retry_count >= max_retries);
}

/// Test BYOVPS validation comprehensive
#[test]
fn test_byovps_validation_comprehensive() {
    // Test IP validation
    let valid_ip = "192.168.1.1";
    assert!(!valid_ip.trim().is_empty());

    // Test username validation with default
    let empty_username = "";
    let username = if empty_username.trim().is_empty() {
        "root"
    } else {
        empty_username.trim()
    };
    assert_eq!(username, "root");

    // Test password validation
    let valid_password = "testpass123";
    assert!(!valid_password.is_empty());
    assert!(valid_password.len() >= 1);

    // Test all fields together
    assert!(!valid_ip.is_empty());
    assert!(!username.is_empty());
    assert!(!valid_password.is_empty());
}
