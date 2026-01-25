//! API Backward Compatibility Tests
//!
//! These tests ensure that the TUI client can gracefully handle API responses
//! from both older backends (without new dashboard fields) and newer backends
//! (with full dashboard support).
//!
//! This is critical for maintaining compatibility during the rollout of
//! dashboard features, where different backend versions may coexist.

use spoq::models::{Thread, ThreadListResponse, ThreadStatus, ThreadType};
use spoq::websocket::messages::WsIncomingMessage;

// ============================================================================
// Thread Deserialization - Backward Compatibility
// ============================================================================

#[test]
fn test_deserialize_thread_without_new_fields() {
    // Simulate old backend response without status, verified, verified_at fields
    let json = r#"{
        "id": "123",
        "title": "Test Thread",
        "preview": "Hello world",
        "updated_at": "2026-01-20T00:00:00Z"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    assert_eq!(thread.id, "123");
    assert_eq!(thread.title, "Test Thread");
    assert_eq!(thread.preview, "Hello world");
    // Dashboard extension fields should be None when not present
    assert!(thread.status.is_none());
    assert!(thread.verified.is_none());
    assert!(thread.verified_at.is_none());
}

#[test]
fn test_deserialize_thread_with_new_fields() {
    // New backend response with all dashboard fields
    let json = r#"{
        "id": "456",
        "title": "Dashboard Thread",
        "preview": "Working on feature",
        "updated_at": "2026-01-20T10:30:00Z",
        "status": "waiting",
        "verified": true,
        "verified_at": "2026-01-20T10:25:00Z"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    assert_eq!(thread.id, "456");
    assert_eq!(thread.title, "Dashboard Thread");
    assert_eq!(thread.status, Some(ThreadStatus::Waiting));
    assert_eq!(thread.verified, Some(true));
    assert!(thread.verified_at.is_some());
}

#[test]
fn test_deserialize_thread_with_running_status() {
    let json = r#"{
        "id": "789",
        "title": "Running Thread",
        "updated_at": "2026-01-20T00:00:00Z",
        "status": "running"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    assert_eq!(thread.status, Some(ThreadStatus::Running));
}

#[test]
fn test_deserialize_thread_with_done_status() {
    let json = r#"{
        "id": "abc",
        "title": "Completed Thread",
        "updated_at": "2026-01-20T00:00:00Z",
        "status": "done"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    assert_eq!(thread.status, Some(ThreadStatus::Done));
}

#[test]
fn test_deserialize_thread_with_error_status() {
    let json = r#"{
        "id": "def",
        "title": "Error Thread",
        "updated_at": "2026-01-20T00:00:00Z",
        "status": "error"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    assert_eq!(thread.status, Some(ThreadStatus::Error));
}

#[test]
fn test_deserialize_thread_with_idle_status() {
    let json = r#"{
        "id": "ghi",
        "title": "Idle Thread",
        "updated_at": "2026-01-20T00:00:00Z",
        "status": "idle"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    assert_eq!(thread.status, Some(ThreadStatus::Idle));
}

#[test]
fn test_existing_thread_fields_unchanged() {
    // Comprehensive test verifying all existing fields still deserialize correctly
    // Note: "last_activity" is an alias for "updated_at", can't have both
    let json = r#"{
        "id": 12345,
        "name": "Full Thread Test",
        "description": "A complete thread for testing",
        "preview": "Preview text here",
        "last_activity": "2026-01-20T15:30:00Z",
        "type": "programming",
        "model": "claude-opus-4",
        "permission_mode": "plan",
        "message_count": 42,
        "created_at": "2026-01-15T10:00:00Z",
        "working_directory": "/Users/sam/projects/myapp"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    // Verify all existing fields
    assert_eq!(thread.id, "12345"); // ID coerced to string
    assert_eq!(thread.title, "Full Thread Test"); // "name" alias
    assert_eq!(
        thread.description,
        Some("A complete thread for testing".to_string())
    );
    assert_eq!(thread.preview, "Preview text here");
    assert_eq!(thread.thread_type, ThreadType::Programming);
    assert_eq!(thread.model, Some("claude-opus-4".to_string()));
    assert_eq!(thread.permission_mode, Some("plan".to_string()));
    assert_eq!(thread.message_count, 42);
    assert_eq!(
        thread.working_directory,
        Some("/Users/sam/projects/myapp".to_string())
    );

    // Dashboard fields should be None (not present in this response)
    assert!(thread.status.is_none());
    assert!(thread.verified.is_none());
    assert!(thread.verified_at.is_none());
}

#[test]
fn test_thread_id_as_integer() {
    // Backend may send id as integer
    let json = r#"{
        "id": 999,
        "title": "Integer ID Thread",
        "updated_at": "2026-01-20T00:00:00Z"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    assert_eq!(thread.id, "999"); // Should be converted to string
}

#[test]
fn test_thread_id_as_string() {
    // Backend may send id as string
    let json = r#"{
        "id": "uuid-1234-5678-abcd",
        "title": "String ID Thread",
        "updated_at": "2026-01-20T00:00:00Z"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    assert_eq!(thread.id, "uuid-1234-5678-abcd");
}

#[test]
fn test_thread_with_null_type() {
    // Old backends may send null for type
    let json = r#"{
        "id": "111",
        "title": "Null Type Thread",
        "updated_at": "2026-01-20T00:00:00Z",
        "type": null
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    // Should default to Conversation
    assert_eq!(thread.thread_type, ThreadType::Conversation);
}

#[test]
fn test_thread_with_normal_type_alias() {
    // Old backends may send "normal" instead of "conversation"
    let json = r#"{
        "id": "222",
        "title": "Normal Type Thread",
        "updated_at": "2026-01-20T00:00:00Z",
        "type": "normal"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    // "normal" should alias to Conversation
    assert_eq!(thread.thread_type, ThreadType::Conversation);
}

#[test]
fn test_thread_with_null_name() {
    // Backend may send null for name
    let json = r#"{
        "id": "333",
        "name": null,
        "updated_at": "2026-01-20T00:00:00Z"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize thread");

    // Should default to empty string
    assert_eq!(thread.title, "");
}

// ============================================================================
// ThreadListResponse - Backward Compatibility
// ============================================================================

#[test]
fn test_thread_list_response_basic() {
    let json = r#"{
        "threads": [
            {"id": "1", "title": "Thread 1", "updated_at": "2026-01-20T00:00:00Z"},
            {"id": "2", "title": "Thread 2", "updated_at": "2026-01-20T01:00:00Z"}
        ],
        "total": 2
    }"#;

    let response: ThreadListResponse =
        serde_json::from_str(json).expect("Failed to deserialize response");

    assert_eq!(response.threads.len(), 2);
    assert_eq!(response.total, 2);
    assert_eq!(response.threads[0].id, "1");
    assert_eq!(response.threads[1].id, "2");
}

#[test]
fn test_thread_list_response_empty() {
    let json = r#"{
        "threads": [],
        "total": 0
    }"#;

    let response: ThreadListResponse =
        serde_json::from_str(json).expect("Failed to deserialize response");

    assert!(response.threads.is_empty());
    assert_eq!(response.total, 0);
}

#[test]
fn test_thread_list_response_with_mixed_thread_data() {
    // Some threads have dashboard fields, some don't
    let json = r#"{
        "threads": [
            {
                "id": "1",
                "title": "Old Thread",
                "updated_at": "2026-01-20T00:00:00Z"
            },
            {
                "id": "2",
                "title": "New Thread",
                "updated_at": "2026-01-20T01:00:00Z",
                "status": "running",
                "verified": false
            },
            {
                "id": "3",
                "title": "Another Old Thread",
                "updated_at": "2026-01-20T02:00:00Z",
                "type": "programming"
            }
        ],
        "total": 3
    }"#;

    let response: ThreadListResponse =
        serde_json::from_str(json).expect("Failed to deserialize response");

    assert_eq!(response.threads.len(), 3);

    // First thread - no dashboard fields
    assert!(response.threads[0].status.is_none());
    assert!(response.threads[0].verified.is_none());

    // Second thread - has dashboard fields
    assert_eq!(response.threads[1].status, Some(ThreadStatus::Running));
    assert_eq!(response.threads[1].verified, Some(false));

    // Third thread - no dashboard fields, has type
    assert!(response.threads[2].status.is_none());
    assert_eq!(response.threads[2].thread_type, ThreadType::Programming);
}

// ============================================================================
// WebSocket Message - Backward Compatibility
// ============================================================================

#[test]
fn test_unknown_ws_message_type_returns_error() {
    // Unknown WS message types should fail to parse (serde will return error)
    // This is expected behavior - the caller should handle parse errors gracefully
    let json = r#"{
        "type": "future_unknown_message_type",
        "data": "some data"
    }"#;

    let result = serde_json::from_str::<WsIncomingMessage>(json);

    // Unknown message types will return an error, which is correct behavior
    // The application should catch this and log/ignore the unknown message
    assert!(result.is_err());
}

#[test]
fn test_ws_agent_status_backward_compat() {
    // Agent status message without new fields
    let json = r#"{
        "type": "agent_status",
        "thread_id": "thread-123",
        "state": "thinking",
        "model": "claude-opus-4",
        "timestamp": 1737331200000
    }"#;

    let msg: WsIncomingMessage = serde_json::from_str(json).expect("Failed to parse message");

    match msg {
        WsIncomingMessage::AgentStatus(status) => {
            assert_eq!(status.thread_id, "thread-123");
            assert_eq!(status.state, "thinking");
            assert_eq!(status.model, "claude-opus-4");
            assert!(status.tool.is_none());
        }
        _ => panic!("Expected AgentStatus"),
    }
}

#[test]
fn test_ws_agent_status_with_tool() {
    // Agent status message with optional tool field
    let json = r#"{
        "type": "agent_status",
        "thread_id": "thread-456",
        "state": "tool_use",
        "model": "claude-opus-4",
        "tool": "Bash",
        "timestamp": 1737331200000
    }"#;

    let msg: WsIncomingMessage = serde_json::from_str(json).expect("Failed to parse message");

    match msg {
        WsIncomingMessage::AgentStatus(status) => {
            assert_eq!(status.thread_id, "thread-456");
            assert_eq!(status.state, "tool_use");
            assert_eq!(status.tool, Some("Bash".to_string()));
        }
        _ => panic!("Expected AgentStatus"),
    }
}

#[test]
fn test_ws_connected_message() {
    let json = r#"{
        "type": "connected",
        "session_id": "session-abc-123",
        "timestamp": 1737331200000
    }"#;

    let msg: WsIncomingMessage = serde_json::from_str(json).expect("Failed to parse message");

    match msg {
        WsIncomingMessage::Connected(conn) => {
            assert_eq!(conn.session_id, "session-abc-123");
            assert_eq!(conn.timestamp, 1737331200000);
        }
        _ => panic!("Expected Connected"),
    }
}

// ============================================================================
// Thread effective_status Fallback
// ============================================================================

#[test]
fn test_thread_effective_status_uses_agent_events() {
    use std::collections::HashMap;

    let json = r#"{
        "id": "thread-1",
        "title": "Test",
        "updated_at": "2026-01-20T00:00:00Z",
        "status": "idle"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

    // Without agent events, should use stored status
    let agent_events: HashMap<String, (String, Option<String>)> = HashMap::new();
    assert_eq!(thread.effective_status(&agent_events), ThreadStatus::Idle);

    // With agent events, should use agent event state
    let mut agent_events = HashMap::new();
    agent_events.insert("thread-1".to_string(), ("thinking".to_string(), None));
    assert_eq!(
        thread.effective_status(&agent_events),
        ThreadStatus::Running
    );
}

#[test]
fn test_thread_effective_status_respects_waiting_status() {
    use std::collections::HashMap;

    let json = r#"{
        "id": "thread-waiting",
        "title": "Waiting Thread",
        "updated_at": "2026-01-20T00:00:00Z",
        "status": "waiting"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

    // Explicit Waiting status should take precedence over agent events
    let mut agent_events = HashMap::new();
    agent_events.insert("thread-waiting".to_string(), ("tool_use".to_string(), Some("Bash".to_string())));

    // Should return Waiting despite agent event saying "tool_use"
    assert_eq!(thread.effective_status(&agent_events), ThreadStatus::Waiting);

    // Should also return Waiting without agent events
    let agent_events: HashMap<String, (String, Option<String>)> = HashMap::new();
    assert_eq!(thread.effective_status(&agent_events), ThreadStatus::Waiting);
}

#[test]
fn test_thread_effective_status_without_stored_status() {
    use std::collections::HashMap;

    // Thread without status field (old backend)
    let json = r#"{
        "id": "thread-old",
        "title": "Old Thread",
        "updated_at": "2026-01-20T00:00:00Z"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

    // Without agent events, should default to Idle
    let agent_events: HashMap<String, (String, Option<String>)> = HashMap::new();
    assert_eq!(thread.effective_status(&agent_events), ThreadStatus::Idle);

    // With waiting agent event
    let mut agent_events = HashMap::new();
    agent_events.insert(
        "thread-old".to_string(),
        ("awaiting_permission".to_string(), None),
    );
    assert_eq!(
        thread.effective_status(&agent_events),
        ThreadStatus::Waiting
    );
}

#[test]
fn test_thread_needs_action() {
    use std::collections::HashMap;

    let json = r#"{
        "id": "thread-action",
        "title": "Action Thread",
        "updated_at": "2026-01-20T00:00:00Z"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

    // Idle thread doesn't need action
    let agent_events: HashMap<String, (String, Option<String>)> = HashMap::new();
    assert!(!thread.needs_action(&agent_events));

    // Waiting thread needs action
    let mut agent_events = HashMap::new();
    agent_events.insert("thread-action".to_string(), ("waiting".to_string(), None));
    assert!(thread.needs_action(&agent_events));

    // Error thread needs action
    agent_events.insert("thread-action".to_string(), ("error".to_string(), None));
    assert!(thread.needs_action(&agent_events));

    // Running thread doesn't need action
    agent_events.insert("thread-action".to_string(), ("running".to_string(), None));
    assert!(!thread.needs_action(&agent_events));
}

// ============================================================================
// Thread display helpers
// ============================================================================

#[test]
fn test_thread_display_repository() {
    let json = r#"{
        "id": "thread-repo",
        "title": "Repo Thread",
        "updated_at": "2026-01-20T00:00:00Z",
        "working_directory": "/Users/sam/projects/myapp"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

    // Should convert to display-friendly format
    assert_eq!(thread.display_repository(), "~/projects/myapp");
}

#[test]
fn test_thread_display_repository_without_working_directory() {
    let json = r#"{
        "id": "thread-no-repo",
        "title": "No Repo Thread",
        "updated_at": "2026-01-20T00:00:00Z"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

    // Should return empty string
    assert_eq!(thread.display_repository(), "");
}

#[test]
fn test_thread_display_duration() {
    let json = r#"{
        "id": "thread-duration",
        "title": "Duration Thread",
        "updated_at": "2026-01-20T00:00:00Z"
    }"#;

    let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

    // Should return a duration string (format depends on current time)
    let duration = thread.display_duration();
    assert!(!duration.is_empty());
    // Duration should contain a number and a unit
    assert!(
        duration.contains('s')
            || duration.contains('m')
            || duration.contains('h')
            || duration.contains('d')
            || duration.contains('<')
    );
}

// ============================================================================
// Conductor Error - NotImplemented for fallback handling
// ============================================================================

#[test]
fn test_conductor_error_not_implemented() {
    use spoq::conductor::ConductorError;

    let error = ConductorError::NotImplemented("/v1/threads/123/verify".to_string());

    let display = format!("{}", error);
    assert!(display.contains("not implemented"));
    assert!(display.contains("/v1/threads/123/verify"));
}

// ============================================================================
// ThreadStatus serialization roundtrip
// ============================================================================

#[test]
fn test_thread_status_serialization_roundtrip() {
    let statuses = vec![
        ThreadStatus::Idle,
        ThreadStatus::Running,
        ThreadStatus::Waiting,
        ThreadStatus::Done,
        ThreadStatus::Error,
    ];

    for status in statuses {
        let json = serde_json::to_string(&status).expect("Failed to serialize");
        let deserialized: ThreadStatus =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(status, deserialized);
    }
}

#[test]
fn test_thread_status_snake_case_format() {
    // Verify snake_case format is used for serialization
    assert_eq!(
        serde_json::to_string(&ThreadStatus::Idle).unwrap(),
        "\"idle\""
    );
    assert_eq!(
        serde_json::to_string(&ThreadStatus::Running).unwrap(),
        "\"running\""
    );
    assert_eq!(
        serde_json::to_string(&ThreadStatus::Waiting).unwrap(),
        "\"waiting\""
    );
    assert_eq!(
        serde_json::to_string(&ThreadStatus::Done).unwrap(),
        "\"done\""
    );
    assert_eq!(
        serde_json::to_string(&ThreadStatus::Error).unwrap(),
        "\"error\""
    );
}
