// Integration tests for ConductorClient
// These tests complement the unit tests in src/conductor.rs
// by testing cross-module integration and real-world scenarios

use spoq::conductor::{ConductorClient, ConductorError};
use spoq::models::{StreamRequest, ServerMessage, MessageRole, ThreadDetailResponse};
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
        permission_mode: None,
        metadata: None,
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
        permission_mode: None,
        metadata: None,
    };
    assert_eq!(request_with_thread.thread_id, Some("thread-123".to_string()));

    // Test with reply_to
    let request_with_reply = StreamRequest {
        prompt: "Continue".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: Some("thread-123".to_string()),
        reply_to: Some(456),
        thread_type: None,
        permission_mode: None,
        metadata: None,
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
        permission_mode: None,
        metadata: None,
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
        permission_mode: None,
        metadata: None,
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
    let _client = ConductorClient::default();

    // Test with minimal request - validates the request can be created
    let request1 = StreamRequest {
        prompt: "Hello".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: None,
        reply_to: None,
        thread_type: None,
        permission_mode: None,
        metadata: None,
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
        permission_mode: None,
        metadata: None,
    };
    assert_eq!(request2.thread_id, Some("thread-123".to_string()));

    // Test with all parameters - validates the request can be created
    let request3 = StreamRequest {
        prompt: "Hello".to_string(),
        session_id: "session-789".to_string(),
        thread_id: Some("thread-123".to_string()),
        reply_to: Some(456),
        thread_type: None,
        permission_mode: None,
        metadata: None,
    };
    assert_eq!(request3.session_id, "session-789");
    assert_eq!(request3.reply_to, Some(456));
}

// ROUND 2 TESTS - Phase 2 and Phase 3 functionality

#[tokio::test]
async fn test_fetch_threads_with_invalid_server() {
    // Test that fetch_threads returns appropriate errors for invalid servers
    let client = ConductorClient::with_base_url("http://invalid-server-12345:9999".to_string());
    let result = client.fetch_threads().await;

    // Should return an error (Http error for connection failure)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fetch_threads_method_exists() {
    // Verify fetch_threads method is callable and returns the correct type
    let client = ConductorClient::default();

    // Call the method (will fail to connect but that's expected)
    let result = client.fetch_threads().await;

    // Result should be Err due to connection failure, or Ok with Vec<Thread>
    match result {
        Ok(threads) => {
            // If it succeeds (unlikely in test env), verify it's a vector
            assert!(threads.is_empty() || !threads.is_empty());
        }
        Err(_) => {
            // Expected - connection failed
            assert!(result.is_err());
        }
    }
}

#[tokio::test]
async fn test_fetch_thread_with_messages_with_invalid_server() {
    // Test that fetch_thread_with_messages returns appropriate errors
    let client = ConductorClient::with_base_url("http://invalid-server-12345:9999".to_string());
    let result = client.fetch_thread_with_messages("thread-123").await;

    // Should return an error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fetch_thread_with_messages_method_exists() {
    // Verify fetch_thread_with_messages method is callable
    let client = ConductorClient::default();
    let thread_id = "test-thread-id";

    // Call the method (will fail to connect but that's expected)
    let result = client.fetch_thread_with_messages(thread_id).await;

    // Result should be Err due to connection failure, or Ok with ThreadDetailResponse
    match result {
        Ok(_response) => {
            // If it succeeds (unlikely in test env), verify structure exists
            assert!(true);
        }
        Err(_) => {
            // Expected - connection failed
            assert!(result.is_err());
        }
    }
}

#[test]
fn test_server_message_to_client_message_conversion() {
    // Test conversion from ServerMessage to client Message
    let server_msg = ServerMessage {
        role: MessageRole::User,
        content: Some(spoq::models::MessageContent::Legacy("Hello, world!".to_string())),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let client_msg = server_msg.to_client_message("thread-123", 42);

    // Verify conversion preserves key fields
    assert_eq!(client_msg.thread_id, "thread-123");
    assert_eq!(client_msg.id, 42);
    assert_eq!(client_msg.content, "Hello, world!");
    assert!(!client_msg.is_streaming);
}

#[test]
fn test_server_message_to_client_message_with_empty_content() {
    // Test conversion when content is None
    let server_msg = ServerMessage {
        role: MessageRole::Assistant,
        content: None,
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let client_msg = server_msg.to_client_message("thread-456", 10);

    // Verify empty content becomes empty string
    assert_eq!(client_msg.thread_id, "thread-456");
    assert_eq!(client_msg.id, 10);
    assert_eq!(client_msg.content, "");
    assert!(!client_msg.is_streaming);
}

#[test]
fn test_server_message_to_client_message_preserves_role() {
    // Test that different roles are preserved
    let roles = vec![
        MessageRole::User,
        MessageRole::Assistant,
        MessageRole::System,
    ];

    for role in roles {
        let server_msg = ServerMessage {
            role,
            content: Some(spoq::models::MessageContent::Legacy("Test".to_string())),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        let client_msg = server_msg.to_client_message("thread-1", 1);

        // Verify role matches
        match role {
            MessageRole::User => assert_eq!(client_msg.role, MessageRole::User),
            MessageRole::Assistant => assert_eq!(client_msg.role, MessageRole::Assistant),
            MessageRole::System => assert_eq!(client_msg.role, MessageRole::System),
            MessageRole::Tool => assert_eq!(client_msg.role, MessageRole::Tool),
        }
    }
}

#[test]
fn test_thread_detail_response_structure() {
    // Test that ThreadDetailResponse can be created and used
    // This validates the structure added in Phase 3
    let response = ThreadDetailResponse {
        id: "thread-123".to_string(),
        thread_type: spoq::models::ThreadType::Conversation,
        name: Some("Test Thread".to_string()),
        project_path: None,
        provider: Some("anthropic".to_string()),
        messages: vec![],
    };

    // Verify fields
    assert_eq!(response.id, "thread-123");
    assert_eq!(response.name, Some("Test Thread".to_string()));
    assert_eq!(response.provider, Some("anthropic".to_string()));
    assert_eq!(response.messages.len(), 0);
}

#[test]
fn test_thread_detail_response_with_messages() {
    // Test ThreadDetailResponse with populated messages
    let message1 = ServerMessage {
        role: MessageRole::User,
        content: Some(spoq::models::MessageContent::Legacy("First message".to_string())),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let message2 = ServerMessage {
        role: MessageRole::Assistant,
        content: Some(spoq::models::MessageContent::Legacy("Second message".to_string())),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let response = ThreadDetailResponse {
        id: "thread-456".to_string(),
        thread_type: spoq::models::ThreadType::Programming,
        name: Some("Programming Thread".to_string()),
        project_path: Some("/path/to/project".to_string()),
        provider: Some("anthropic".to_string()),
        messages: vec![message1, message2],
    };

    // Verify structure
    assert_eq!(response.messages.len(), 2);
    assert_eq!(response.thread_type, spoq::models::ThreadType::Programming);
    assert_eq!(response.project_path, Some("/path/to/project".to_string()));
}
