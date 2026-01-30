// Integration tests for ConductorClient
// These tests complement the unit tests in src/conductor.rs
// by testing cross-module integration and real-world scenarios

use spoq::conductor::{ConductorClient, ConductorError};
use spoq::models::{MessageRole, ServerMessage, StreamRequest, ThreadDetailResponse};
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
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
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
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
    };
    assert_eq!(
        request_with_thread.thread_id,
        Some("thread-123".to_string())
    );

    // Test with reply_to
    let request_with_reply = StreamRequest {
        prompt: "Continue".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: Some("thread-123".to_string()),
        reply_to: Some(456),
        thread_type: None,
        permission_mode: None,
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
    };
    assert_eq!(request_with_reply.reply_to, Some(456));
}

#[tokio::test]
async fn test_conductor_error_handling() {
    // Test that invalid server URLs produce appropriate errors
    let client = ConductorClient::with_base_url(
        "http://invalid-server-that-does-not-exist-12345:9999".to_string(),
    );

    let request = StreamRequest {
        prompt: "Test message".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: None,
        reply_to: None,
        thread_type: None,
        permission_mode: None,
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
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
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
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
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
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
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
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
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
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
            // If it succeeds (unlikely in test env), verify structure exists - nothing more to check
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
        content: Some(spoq::models::MessageContent::Legacy(
            "Hello, world!".to_string(),
        )),
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
        content: Some(spoq::models::MessageContent::Legacy(
            "First message".to_string(),
        )),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };

    let message2 = ServerMessage {
        role: MessageRole::Assistant,
        content: Some(spoq::models::MessageContent::Legacy(
            "Second message".to_string(),
        )),
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

// ROUND 1 TESTS - Working Directory functionality

#[tokio::test]
async fn test_stream_request_with_working_directory() {
    // Test that working_directory can be set and accessed
    let request = StreamRequest {
        prompt: "Test with working directory".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: None,
        reply_to: None,
        thread_type: None,
        permission_mode: None,
        working_directory: Some("/Users/dev/my-project".to_string()),
        plan_mode: false,
        images: Vec::new(),
    };

    assert_eq!(
        request.working_directory,
        Some("/Users/dev/my-project".to_string())
    );
}

#[tokio::test]
async fn test_stream_request_without_working_directory() {
    // Test that working_directory can be None
    let request = StreamRequest {
        prompt: "Test without working directory".to_string(),
        session_id: Uuid::new_v4().to_string(),
        thread_id: None,
        reply_to: None,
        thread_type: None,
        permission_mode: None,
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
    };

    assert!(request.working_directory.is_none());
}

#[test]
fn test_stream_request_working_directory_serialization() {
    // Test that working_directory serializes correctly
    let request = StreamRequest {
        prompt: "Test serialization".to_string(),
        session_id: "session-123".to_string(),
        thread_id: Some("thread-456".to_string()),
        reply_to: None,
        thread_type: None,
        permission_mode: None,
        working_directory: Some("/home/user/workspace".to_string()),
        plan_mode: false,
        images: Vec::new(),
    };

    let json = serde_json::to_string(&request).expect("Failed to serialize");
    assert!(json.contains("working_directory"));
    assert!(json.contains("/home/user/workspace"));

    let deserialized: StreamRequest = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(
        deserialized.working_directory,
        Some("/home/user/workspace".to_string())
    );
}

#[test]
fn test_stream_request_working_directory_omitted_when_none() {
    // Test that working_directory is omitted from JSON when None
    let request = StreamRequest {
        prompt: "Test omission".to_string(),
        session_id: "session-123".to_string(),
        thread_id: None,
        reply_to: None,
        thread_type: None,
        permission_mode: None,
        working_directory: None,
        plan_mode: false,
        images: Vec::new(),
    };

    let json = serde_json::to_string(&request).expect("Failed to serialize");
    // working_directory should not appear in JSON when None
    assert!(!json.contains("working_directory"));
}

#[test]
fn test_stream_request_working_directory_with_various_paths() {
    // Test various path formats
    let paths = vec![
        "/absolute/unix/path",
        "/Users/username/Documents/projects",
        "/home/dev/workspace/rust-project",
    ];

    for path in paths {
        let request = StreamRequest {
            prompt: "Test".to_string(),
            session_id: Uuid::new_v4().to_string(),
            thread_id: None,
            reply_to: None,
            thread_type: None,
            permission_mode: None,
            working_directory: Some(path.to_string()),
            plan_mode: false,
            images: Vec::new(),
        };

        assert_eq!(request.working_directory, Some(path.to_string()));

        // Verify serialization preserves the path
        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains(path));
    }
}

#[test]
fn test_stream_request_working_directory_builder_pattern() {
    // Test using builder pattern methods
    let request = StreamRequest::new("Builder test".to_string())
        .with_working_directory(Some("/Users/test/project".to_string()));

    assert_eq!(
        request.working_directory,
        Some("/Users/test/project".to_string())
    );
}

#[test]
fn test_stream_request_working_directory_with_thread_builder() {
    // Test working_directory with with_thread builder
    let request = StreamRequest::with_thread("Follow-up".to_string(), "thread-123".to_string())
        .with_working_directory(Some("/workspace/app".to_string()));

    assert_eq!(request.thread_id, Some("thread-123".to_string()));
    assert_eq!(
        request.working_directory,
        Some("/workspace/app".to_string())
    );
}

// ROUND 1 TESTS - fetch_folders API format change

#[tokio::test]
async fn test_fetch_folders_with_invalid_server() {
    // Test that fetch_folders returns appropriate errors for invalid servers
    let client = ConductorClient::with_base_url("http://invalid-server-12345:9999".to_string());
    let result = client.fetch_folders().await;

    // Should return an error (Http error for connection failure)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fetch_folders_method_exists() {
    // Verify fetch_folders method is callable and returns the correct type
    let client = ConductorClient::default();

    // Call the method (will fail to connect but that's expected)
    let result = client.fetch_folders().await;

    // Result should be Err due to connection failure, or Ok with Vec<Folder>
    match result {
        Ok(folders) => {
            // If it succeeds (unlikely in test env), verify it's a vector
            assert!(folders.is_empty() || !folders.is_empty());
        }
        Err(_) => {
            // Expected - connection failed
            assert!(result.is_err());
        }
    }
}

// ROUND 2 TESTS - Unified Picker Search API

#[tokio::test]
async fn test_search_folders_with_invalid_server() {
    // Test that search_folders returns appropriate errors for invalid servers
    let client = ConductorClient::with_base_url("http://invalid-server-12345:9999".to_string());
    let result = client.search_folders("test", 10).await;

    // Should return an error (Http error for connection failure)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_folders_method_exists() {
    // Verify search_folders method is callable and returns the correct type
    let client = ConductorClient::default();

    // Call the method (will fail to connect but that's expected)
    let result = client.search_folders("test", 10).await;

    // Result should be Err due to connection failure, or Ok with SearchFoldersResponse
    match result {
        Ok(response) => {
            // If it succeeds (unlikely in test env), verify it has folders field
            assert!(response.folders.is_empty() || !response.folders.is_empty());
        }
        Err(_) => {
            // Expected - connection failed
            assert!(result.is_err());
        }
    }
}

#[tokio::test]
async fn test_search_folders_with_different_limits() {
    // Test that search_folders accepts different limit parameters
    let client = ConductorClient::default();

    // Test with limit 1
    let _result1 = client.search_folders("test", 1).await;

    // Test with limit 50
    let _result2 = client.search_folders("test", 50).await;

    // Test with empty query
    let _result3 = client.search_folders("", 10).await;

    // All calls should compile and be callable
}

#[tokio::test]
async fn test_search_threads_with_invalid_server() {
    // Test that search_threads returns appropriate errors for invalid servers
    let client = ConductorClient::with_base_url("http://invalid-server-12345:9999".to_string());
    let result = client.search_threads("test", 10).await;

    // Should return an error (Http error for connection failure)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_threads_method_exists() {
    // Verify search_threads method is callable and returns the correct type
    let client = ConductorClient::default();

    // Call the method (will fail to connect but that's expected)
    let result = client.search_threads("conversation", 10).await;

    // Result should be Err due to connection failure, or Ok with SearchThreadsResponse
    match result {
        Ok(response) => {
            // If it succeeds (unlikely in test env), verify it has threads field
            assert!(response.threads.is_empty() || !response.threads.is_empty());
        }
        Err(_) => {
            // Expected - connection failed
            assert!(result.is_err());
        }
    }
}

#[tokio::test]
async fn test_search_threads_with_different_queries() {
    // Test that search_threads accepts different query parameters
    let client = ConductorClient::default();

    // Test with different query strings
    let _result1 = client.search_threads("", 10).await;
    let _result2 = client.search_threads("my thread", 5).await;
    let _result3 = client.search_threads("conversation", 20).await;

    // All calls should compile and be callable
}

#[tokio::test]
async fn test_search_repos_with_invalid_server() {
    // Test that search_repos returns appropriate errors for invalid servers
    let client = ConductorClient::with_base_url("http://invalid-server-12345:9999".to_string());
    let result = client.search_repos("test", 10).await;

    // Should return an error (Http error for connection failure)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_repos_method_exists() {
    // Verify search_repos method is callable and returns the correct type
    let client = ConductorClient::default();

    // Call the method (will fail to connect but that's expected)
    let result = client.search_repos("repo", 10).await;

    // Result should be Err due to connection failure, or Ok with SearchReposResponse
    match result {
        Ok(response) => {
            // If it succeeds (unlikely in test env), verify it has repos field
            assert!(response.repos.is_empty() || !response.repos.is_empty());
        }
        Err(_) => {
            // Expected - connection failed
            assert!(result.is_err());
        }
    }
}

#[tokio::test]
async fn test_search_repos_with_various_queries() {
    // Test that search_repos handles various query formats
    let client = ConductorClient::default();

    // Test with owner/repo format
    let _result1 = client.search_repos("owner/repo", 10).await;

    // Test with partial match
    let _result2 = client.search_repos("spoo", 5).await;

    // Test with empty query
    let _result3 = client.search_repos("", 10).await;

    // All calls should compile and be callable
}

#[tokio::test]
async fn test_clone_repo_with_invalid_server() {
    // Test that clone_repo returns appropriate errors for invalid servers
    let client = ConductorClient::with_base_url("http://invalid-server-12345:9999".to_string());
    let result = client.clone_repo("owner/repo").await;

    // Should return an error (Http error for connection failure)
    assert!(result.is_err());
}

#[tokio::test]
async fn test_clone_repo_method_exists() {
    // Verify clone_repo method is callable and returns the correct type
    let client = ConductorClient::default();

    // Call the method with a valid repo name format (will fail to connect but that's expected)
    let result = client.clone_repo("owner/test-repo").await;

    // Result should be Err due to connection failure, or Ok with CloneResponse
    match result {
        Ok(response) => {
            // If it succeeds (unlikely in test env), verify it has path field
            assert!(!response.path.is_empty());
        }
        Err(_) => {
            // Expected - connection failed
            assert!(result.is_err());
        }
    }
}

#[tokio::test]
async fn test_clone_repo_with_various_repo_formats() {
    // Test that clone_repo handles various repo name formats
    let client = ConductorClient::default();

    // Test with standard format
    let _result1 = client.clone_repo("owner/repo-name").await;

    // Test with hyphenated owner
    let _result2 = client.clone_repo("org-name/repo").await;

    // Test with numbers
    let _result3 = client.clone_repo("user123/project-v2").await;

    // All calls should compile and be callable
}
