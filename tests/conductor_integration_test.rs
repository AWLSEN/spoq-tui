// Integration tests for ConductorClient
// These tests complement the unit tests in src/conductor.rs
// by testing cross-module integration and real-world scenarios

use spoq::conductor::{ConductorClient, ConductorError};
use spoq::models::StreamRequest;
use uuid::Uuid;

#[tokio::test]
async fn test_conductor_client_creation_and_configuration() {
    // Test that we can create a client with default settings
    let client = ConductorClient::default();
    // Default URL should be set (actual value may vary based on configuration)
    assert!(!client.base_url.is_empty());
    assert!(client.base_url.starts_with("http://") || client.base_url.starts_with("https://"));

    // Test that we can create a client with custom base URL
    let custom_client = ConductorClient::with_base_url("http://custom:8080".to_string());
    assert_eq!(custom_client.base_url, "http://custom:8080");
}

#[tokio::test]
async fn test_stream_request_construction() {
    // Test basic stream request creation
    let request = StreamRequest {
        prompt: "Hello, world!".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: None,
        reply_to: None,
        thread_type: None,
        plan_mode: None,
    };
    assert_eq!(request.prompt, "Hello, world!");
    assert!(!request.session_id.is_empty());
    assert_eq!(request.thread_id, None);
    assert_eq!(request.reply_to, None);

    // Test with thread ID
    let request_with_thread = StreamRequest {
        prompt: "Follow-up question".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: Some("thread-123".to_string()),
        reply_to: None,
        thread_type: None,
        plan_mode: None,
    };
    assert_eq!(request_with_thread.thread_id, Some("thread-123".to_string()));

    // Test with reply_to
    let request_with_reply = StreamRequest {
        prompt: "Continue".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: Some("thread-123".to_string()),
        reply_to: Some(456),
        thread_type: None,
        plan_mode: None,
    };
    assert_eq!(request_with_reply.reply_to, Some(456));
}

#[tokio::test]
async fn test_conductor_error_handling() {
    // Test that invalid server URLs produce appropriate errors
    let client = ConductorClient::with_base_url("http://invalid-server-that-does-not-exist-12345:9999".to_string());

    let request = StreamRequest {
        prompt: "Test message".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: None,
        reply_to: None,
        thread_type: None,
        plan_mode: None,
    };
    let result = client.stream(&request).await;

    assert!(result.is_err());
    // Connection failure will produce an Http error
    if let Err(ConductorError::Http(_)) = result {
        // Expected error type for connection failures
    } else {
        // Accept any error type since connection errors can vary
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn test_conductor_client_methods_exist() {
    // This test ensures all required methods are present and callable
    let client = ConductorClient::new();

    // Verify base_url field exists
    let _url = &client.base_url;

    // Verify we can create requests
    let request = StreamRequest {
        prompt: "test".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: None,
        reply_to: None,
        thread_type: None,
        plan_mode: None,
    };

    // Verify stream method exists (will fail to connect but that's expected)
    let _result = client.stream(&request).await;
}

#[test]
fn test_conductor_error_display_formatting() {
    // Test error display for different error types
    let error1 = ConductorError::ServerError {
        status: 500,
        message: "Internal server error".to_string(),
    };
    let display1 = format!("{}", error1);
    assert!(display1.contains("500") || display1.contains("Server"));

    // Test that we can format the error (actual implementation may vary)
    let display_debug = format!("{:?}", error1);
    assert!(!display_debug.is_empty());
}

#[tokio::test]
async fn test_stream_request_with_different_configurations() {
    let client = ConductorClient::default();

    // Test with minimal request - validates the request can be created
    let request1 = StreamRequest {
        prompt: "Hello".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: None,
        reply_to: None,
        thread_type: None,
        plan_mode: None,
    };
    // Validate request structure
    assert_eq!(request1.prompt, "Hello");
    assert!(!request1.session_id.is_empty());

    // Test with thread_id - validates the request can be created
    let request2 = StreamRequest {
        prompt: "Hello".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: Some("thread-123".to_string()),
        reply_to: None,
        thread_type: None,
        plan_mode: None,
    };
    assert_eq!(request2.thread_id, Some("thread-123".to_string()));

    // Test with all parameters - validates the request can be created
    let request3 = StreamRequest {
        prompt: "Hello".to_string(),
        session_id: "session-789".to_string(),
        thread_id: Some("thread-123".to_string()),
        reply_to: Some(456),
        thread_type: None,
        plan_mode: None,
    };
    assert_eq!(request3.session_id, "session-789");
    assert_eq!(request3.reply_to, Some(456));
}
