//! Thread mode API endpoint tests using wiremock.
//!
//! These tests verify that the ConductorClient correctly calls the
//! PUT /v1/threads/{id}/mode and PUT /v1/threads/{id}/permission endpoints.

use spoq::conductor::{ConductorClient, ConductorError};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper to create a mock server with auth header expected.
async fn setup_authenticated_server() -> MockServer {
    let mock_server = MockServer::start().await;
    mock_server
}

/// Helper to create a thread ID for testing.
fn test_thread_id() -> String {
    "test-thread-123".to_string()
}

/// Helper to create a test token.
fn test_token() -> String {
    "test-auth-token".to_string()
}

#[tokio::test]
async fn test_update_thread_mode_success() {
    let mock_server = setup_authenticated_server().await;
    let thread_id = test_thread_id();

    // Mock the PUT /v1/threads/{id}/mode endpoint
    Mock::given(method("PUT"))
        .and(path(format!("/v1/threads/{}/mode", thread_id)))
        .and(header("Authorization", format!("Bearer {}", test_token())))
        .and(body_json(serde_json::json!({"mode": "plan"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "thread_id": thread_id,
                "previous_mode": "normal",
                "new_mode": "plan"
            }))
        )
        .mount(&mock_server)
        .await;

    // Create client with mock server URL and auth token
    let client = ConductorClient::with_url(&mock_server.uri()).with_auth(&test_token());

    // Call update_thread_mode
    let result = client.update_thread_mode(&thread_id, "plan").await;

    // Verify success
    assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result);
}

#[tokio::test]
async fn test_update_thread_permission_success() {
    let mock_server = setup_authenticated_server().await;
    let thread_id = test_thread_id();

    // Mock the PUT /v1/threads/{id}/permission endpoint
    Mock::given(method("PUT"))
        .and(path(format!("/v1/threads/{}/permission", thread_id)))
        .and(header("Authorization", format!("Bearer {}", test_token())))
        .and(body_json(serde_json::json!({"mode": "execution"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "previous_mode": "default",
                "new_mode": "execution"
            }))
        )
        .mount(&mock_server)
        .await;

    // Create client with mock server URL and auth token
    let client = ConductorClient::with_url(&mock_server.uri()).with_auth(&test_token());

    // Call update_thread_permission
    let result = client.update_thread_permission(&thread_id, "execution").await;

    // Verify success
    assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result);
}

#[tokio::test]
async fn test_update_thread_mode_not_found() {
    let mock_server = setup_authenticated_server().await;
    let thread_id = "nonexistent-thread".to_string();

    // Mock 404 response
    Mock::given(method("PUT"))
        .and(path(format!("/v1/threads/{}/mode", thread_id)))
        .and(header("Authorization", format!("Bearer {}", test_token())))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": "Thread not found"
            }))
        )
        .mount(&mock_server)
        .await;

    let client = ConductorClient::with_url(&mock_server.uri()).with_auth(&test_token());

    let result = client.update_thread_mode(&thread_id, "plan").await;

    assert!(result.is_err());
    match result {
        Err(ConductorError::ServerError { status, .. }) => {
            assert_eq!(status, 404);
        }
        _ => panic!("Expected ServerError with status 404"),
    }
}

#[tokio::test]
async fn test_update_thread_permission_invalid_mode() {
    let mock_server = setup_authenticated_server().await;
    let thread_id = test_thread_id();

    // Mock 400 response for invalid mode
    Mock::given(method("PUT"))
        .and(path(format!("/v1/threads/{}/permission", thread_id)))
        .and(header("Authorization", format!("Bearer {}", test_token())))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "Invalid mode"
            }))
        )
        .mount(&mock_server)
        .await;

    let client = ConductorClient::with_url(&mock_server.uri()).with_auth(&test_token());

    let result = client.update_thread_permission(&thread_id, "invalid").await;

    assert!(result.is_err());
    match result {
        Err(ConductorError::ServerError { status, .. }) => {
            assert_eq!(status, 400);
        }
        _ => panic!("Expected ServerError with status 400"),
    }
}

#[tokio::test]
async fn test_update_thread_mode_auth_required() {
    let mock_server = setup_authenticated_server().await;
    let thread_id = test_thread_id();

    // Mock 401 response (unauthorized)
    Mock::given(method("PUT"))
        .and(path(format!("/v1/threads/{}/mode", thread_id)))
        .respond_with(
            ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Unauthorized"
            }))
        )
        .mount(&mock_server)
        .await;

    // Create client WITHOUT auth token
    let client = ConductorClient::with_url(&mock_server.uri());

    let result = client.update_thread_mode(&thread_id, "plan").await;

    assert!(result.is_err());
    match result {
        Err(ConductorError::ServerError { status, .. }) => {
            assert_eq!(status, 401);
        }
        _ => panic!("Expected ServerError with status 401"),
    }
}

#[tokio::test]
async fn test_update_both_endpoints_sequentially() {
    let mock_server = setup_authenticated_server().await;
    let thread_id = test_thread_id();

    // Mock the PUT /v1/threads/{id}/mode endpoint
    Mock::given(method("PUT"))
        .and(path(format!("/v1/threads/{}/mode", thread_id)))
        .and(header("Authorization", format!("Bearer {}", test_token())))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "new_mode": "plan"
            }))
        )
        .mount(&mock_server)
        .await;

    // Mock the PUT /v1/threads/{id}/permission endpoint
    Mock::given(method("PUT"))
        .and(path(format!("/v1/threads/{}/permission", thread_id)))
        .and(header("Authorization", format!("Bearer {}", test_token())))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "new_mode": "plan"
            }))
        )
        .mount(&mock_server)
        .await;

    let client = ConductorClient::with_url(&mock_server.uri()).with_auth(&test_token());

    // Call both endpoints sequentially
    let mode_result = client.update_thread_mode(&thread_id, "plan").await;
    let perm_result = client.update_thread_permission(&thread_id, "plan").await;

    // Verify both succeeded
    assert!(mode_result.is_ok(), "update_thread_mode failed: {:?}", mode_result);
    assert!(perm_result.is_ok(), "update_thread_permission failed: {:?}", perm_result);
}

#[tokio::test]
async fn test_update_thread_mode_server_error() {
    let mock_server = setup_authenticated_server().await;
    let thread_id = test_thread_id();

    // Mock 500 response
    Mock::given(method("PUT"))
        .and(path(format!("/v1/threads/{}/mode", thread_id)))
        .and(header("Authorization", format!("Bearer {}", test_token())))
        .respond_with(
            ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "error": "Internal server error"
            }))
        )
        .mount(&mock_server)
        .await;

    let client = ConductorClient::with_url(&mock_server.uri()).with_auth(&test_token());

    let result = client.update_thread_mode(&thread_id, "plan").await;

    assert!(result.is_err());
    match result {
        Err(ConductorError::ServerError { status, .. }) => {
            assert_eq!(status, 500);
        }
        _ => panic!("Expected ServerError with status 500"),
    }
}
