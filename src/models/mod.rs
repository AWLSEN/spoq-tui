pub mod dashboard;
pub mod file;
mod folder;
mod repository;
mod message;
pub mod picker;
mod request;
pub mod steering;
mod text_utils;
mod thread;
mod tools;

pub use dashboard::{
    compute_duration, compute_local_aggregate, derive_repository, infer_status_from_agent_state,
    Aggregate, PlanSummary, ThreadStatus, WaitingFor,
};
pub use file::FileEntry;
pub use folder::Folder;
pub use repository::{GitHubRepo, PrimaryLanguage};
pub use message::*;
pub use picker::*;
pub use request::PermissionMode;
pub use request::{CancelRequest, CancelResponse, ImageAttachmentPayload, StreamRequest};
pub use steering::{QueuedSteeringMessage, SteeringMessageState};
pub use text_utils::strip_thread_prefix;
pub use thread::*;
pub use tools::*;

// Re-export ThreadMode from thread module for easy access
pub use thread::ThreadMode;

use serde::{Deserialize, Deserializer};

/// Helper to deserialize id as either string or integer
pub(crate) fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct IdVisitor;

    impl<'de> Visitor<'de> for IdVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or integer")
        }

        fn visit_str<E>(self, value: &str) -> Result<String, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_string<E>(self, value: String) -> Result<String, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<String, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_u64<E>(self, value: u64) -> Result<String, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }
    }

    deserializer.deserialize_any(IdVisitor)
}

/// Helper to deserialize ThreadType with null handling
/// Returns Default (Conversation) if the field is null or missing
pub(crate) fn deserialize_thread_type<'de, D>(deserializer: D) -> Result<ThreadType, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<ThreadType>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

/// Helper to deserialize nullable strings as empty string
/// Handles both missing fields and explicit null values
pub(crate) fn deserialize_nullable_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_thread_type_default() {
        assert_eq!(ThreadType::default(), ThreadType::Conversation);
    }

    #[test]
    fn test_thread_type_variants() {
        assert_eq!(ThreadType::Conversation, ThreadType::Conversation);
        assert_eq!(ThreadType::Programming, ThreadType::Programming);
        assert_ne!(ThreadType::Conversation, ThreadType::Programming);
    }

    #[test]
    fn test_thread_type_serialization() {
        // Test Conversation serialization (lowercase for server compatibility)
        let conversation = ThreadType::Conversation;
        let json = serde_json::to_string(&conversation).expect("Failed to serialize");
        assert_eq!(json, "\"conversation\"");
        let deserialized: ThreadType = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(conversation, deserialized);

        // Test Programming serialization (lowercase for server compatibility)
        let prog = ThreadType::Programming;
        let json = serde_json::to_string(&prog).expect("Failed to serialize");
        assert_eq!(json, "\"programming\"");
        let deserialized: ThreadType = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(prog, deserialized);
    }

    #[test]
    fn test_thread_creation() {
        let thread = Thread {
            id: "thread-123".to_string(),
            title: "Test Thread".to_string(),
            description: None,
            preview: "Hello, world!".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        };

        assert_eq!(thread.id, "thread-123");
        assert_eq!(thread.title, "Test Thread");
        assert_eq!(thread.preview, "Hello, world!");
        assert_eq!(thread.thread_type, ThreadType::Conversation);
        assert_eq!(thread.mode, ThreadMode::Normal);
    }

    #[test]
    fn test_thread_creation_programming() {
        let thread = Thread {
            id: "thread-456".to_string(),
            title: "Code Review".to_string(),
            description: None,
            preview: "Let me review this code".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Programming,
            mode: crate::models::ThreadMode::default(),
            model: Some("gpt-4".to_string()),
            permission_mode: Some("auto".to_string()),
            message_count: 5,
            created_at: Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        };

        assert_eq!(thread.id, "thread-456");
        assert_eq!(thread.thread_type, ThreadType::Programming);
        assert_eq!(thread.mode, ThreadMode::Normal);
    }

    #[test]
    fn test_thread_serialization() {
        let thread = Thread {
            id: "thread-456".to_string(),
            title: "Serialization Test".to_string(),
            description: None,
            preview: "Testing JSON".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Conversation,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        };

        let json = serde_json::to_string(&thread).expect("Failed to serialize");
        let deserialized: Thread = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(thread, deserialized);
    }

    #[test]
    fn test_thread_serialization_programming() {
        let thread = Thread {
            id: "thread-789".to_string(),
            title: "Programming Thread".to_string(),
            description: None,
            preview: "Code discussion".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Programming,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        };

        let json = serde_json::to_string(&thread).expect("Failed to serialize");
        let deserialized: Thread = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(thread, deserialized);
        assert_eq!(deserialized.thread_type, ThreadType::Programming);
        assert_eq!(deserialized.mode, ThreadMode::Normal);
    }

    #[test]
    fn test_thread_deserialization_backward_compatibility() {
        // Test backward compatibility - deserialize JSON without thread_type field
        let json = r#"{
            "id": "thread-legacy",
            "title": "Legacy Thread",
            "preview": "Old format",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;

        let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(thread.id, "thread-legacy");
        assert_eq!(thread.title, "Legacy Thread");
        assert_eq!(thread.preview, "Old format");
        // Should default to Normal when thread_type is missing
        assert_eq!(thread.thread_type, ThreadType::Conversation);
    }

    #[test]
    fn test_thread_deserialization_with_thread_type() {
        // Test deserializing JSON with explicit thread_type (lowercase for server)
        let json = r#"{
            "id": "thread-new",
            "title": "New Thread",
            "preview": "New format",
            "updated_at": "2024-01-01T00:00:00Z",
            "type": "programming"
        }"#;

        let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(thread.id, "thread-new");
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    #[test]
    fn test_thread_deserialization_with_null_thread_type() {
        // Test backward compatibility - deserialize JSON with null type field
        let json = r#"{
            "id": "thread-null-type",
            "title": "Null Type Thread",
            "preview": "Thread with null type",
            "updated_at": "2024-01-01T00:00:00Z",
            "type": null
        }"#;

        let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(thread.id, "thread-null-type");
        assert_eq!(thread.title, "Null Type Thread");
        // Should default to Normal when type is null
        assert_eq!(thread.thread_type, ThreadType::Conversation);
    }

    #[test]
    fn test_thread_deserialization_with_name_field() {
        // Test that "name" field from API is mapped to "title" field in struct
        let json = r#"{
            "id": "thread-api",
            "name": "My Thread Title",
            "thread_type": "conversation",
            "project_path": "/home/user/project",
            "provider": "claude-cli"
        }"#;

        let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(thread.id, "thread-api");
        // "name" from API should map to "title" in struct
        assert_eq!(thread.title, "My Thread Title");
        assert_eq!(thread.thread_type, ThreadType::Conversation);
    }

    #[test]
    fn test_thread_deserialization_with_description() {
        // Test deserializing thread with description field
        let json = r#"{
            "id": "thread-desc",
            "name": "Thread with Description",
            "description": "This is a thread description",
            "preview": "Preview text",
            "updated_at": "2024-01-01T00:00:00Z",
            "type": "conversation"
        }"#;

        let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(thread.id, "thread-desc");
        assert_eq!(thread.title, "Thread with Description");
        assert_eq!(
            thread.description,
            Some("This is a thread description".to_string())
        );
        assert_eq!(thread.preview, "Preview text");
    }

    #[test]
    fn test_thread_deserialization_without_description() {
        // Test backward compatibility - deserialize JSON without description field
        let json = r#"{
            "id": "thread-no-desc",
            "name": "Thread without Description",
            "preview": "Preview text",
            "updated_at": "2024-01-01T00:00:00Z",
            "type": "conversation"
        }"#;

        let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(thread.id, "thread-no-desc");
        assert_eq!(thread.title, "Thread without Description");
        assert_eq!(thread.description, None);
        assert_eq!(thread.preview, "Preview text");
    }

    #[test]
    fn test_thread_type_clone() {
        let original = ThreadType::Programming;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_thread_type_debug() {
        // Verify Debug trait is implemented correctly
        let conversation = ThreadType::Conversation;
        let prog = ThreadType::Programming;
        assert_eq!(format!("{:?}", conversation), "Conversation");
        assert_eq!(format!("{:?}", prog), "Programming");
    }

    #[test]
    fn test_message_creation() {
        let message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::User,
            content: "Hello!".to_string(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };

        assert_eq!(message.id, 1);
        assert_eq!(message.thread_id, "thread-123");
        assert_eq!(message.role, MessageRole::User);
        assert_eq!(message.content, "Hello!");
        assert!(!message.is_streaming);
        assert!(message.partial_content.is_empty());
        assert!(message.reasoning_content.is_empty());
        assert!(message.reasoning_collapsed);
    }

    #[test]
    fn test_message_role_variants() {
        assert_eq!(MessageRole::User, MessageRole::User);
        assert_ne!(MessageRole::User, MessageRole::Assistant);
        assert_ne!(MessageRole::Assistant, MessageRole::System);
    }

    #[test]
    fn test_message_serialization() {
        let message = Message {
            id: 42,
            thread_id: "thread-789".to_string(),
            role: MessageRole::Assistant,
            content: "I can help with that.".to_string(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };

        let json = serde_json::to_string(&message).expect("Failed to serialize");
        let deserialized: Message = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(message, deserialized);
    }

    #[test]
    fn test_message_append_token() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.append_token("Hello");
        assert_eq!(message.partial_content, "Hello");

        message.append_token(", ");
        assert_eq!(message.partial_content, "Hello, ");

        message.append_token("world!");
        assert_eq!(message.partial_content, "Hello, world!");
    }

    #[test]
    fn test_message_append_token_creates_segments() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        // append_token should add to both partial_content AND segments
        message.append_token("Hello");
        assert_eq!(message.partial_content, "Hello");
        assert_eq!(message.segments.len(), 1);
        if let MessageSegment::Text(text) = &message.segments[0] {
            assert_eq!(text, "Hello");
        } else {
            panic!("Expected Text segment");
        }

        // Subsequent tokens should be merged into the same text segment
        message.append_token(" world");
        assert_eq!(message.partial_content, "Hello world");
        assert_eq!(message.segments.len(), 1);
        if let MessageSegment::Text(text) = &message.segments[0] {
            assert_eq!(text, "Hello world");
        } else {
            panic!("Expected Text segment");
        }
    }

    #[test]
    fn test_message_finalize() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: "Streamed content".to_string(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        assert!(message.is_streaming);
        assert_eq!(message.partial_content, "Streamed content");
        assert!(message.content.is_empty());

        message.finalize();

        assert!(!message.is_streaming);
        assert!(message.partial_content.is_empty());
        assert_eq!(message.content, "Streamed content");
    }

    #[test]
    fn test_message_finalize_when_not_streaming() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: "Original content".to_string(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: "Should not replace".to_string(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };

        message.finalize();

        // Content should remain unchanged when not streaming
        assert!(!message.is_streaming);
        assert_eq!(message.content, "Original content");
        assert_eq!(message.partial_content, "Should not replace");
    }

    #[test]
    fn test_message_streaming_workflow() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        // Simulate streaming tokens
        message.append_token("The ");
        message.append_token("quick ");
        message.append_token("brown ");
        message.append_token("fox");

        assert!(message.is_streaming);
        assert_eq!(message.partial_content, "The quick brown fox");
        assert!(message.content.is_empty());

        // Finalize the message
        message.finalize();

        assert!(!message.is_streaming);
        assert!(message.partial_content.is_empty());
        assert_eq!(message.content, "The quick brown fox");
    }

    #[test]
    fn test_message_serialization_with_streaming_fields() {
        let message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: "Partial content".to_string(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        let json = serde_json::to_string(&message).expect("Failed to serialize");
        let deserialized: Message = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(message, deserialized);
        assert!(deserialized.is_streaming);
        assert_eq!(deserialized.partial_content, "Partial content");
    }

    #[test]
    fn test_message_deserialization_without_streaming_fields() {
        // Test backward compatibility - deserialize JSON without streaming fields
        let json = r#"{
            "id": 1,
            "thread_id": "thread-123",
            "role": "user",
            "content": "Hello",
            "created_at": "2024-01-01T00:00:00Z"
        }"#;

        let message: Message = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(message.id, 1);
        assert_eq!(message.content, "Hello");
        // Default values should be applied
        assert!(!message.is_streaming);
        assert!(message.partial_content.is_empty());
    }

    #[test]
    fn test_stream_request_new() {
        let request = StreamRequest::new("Hello".to_string());

        assert_eq!(request.prompt, "Hello");
        assert!(!request.session_id.is_empty());
        assert!(request.thread_id.is_none());
        assert!(request.reply_to.is_none());
    }

    #[test]
    fn test_stream_request_with_thread() {
        let request = StreamRequest::with_thread("Hello".to_string(), "thread-123".to_string());

        assert_eq!(request.prompt, "Hello");
        assert!(!request.session_id.is_empty());
        assert_eq!(request.thread_id, Some("thread-123".to_string()));
        assert!(request.reply_to.is_none());
    }

    #[test]
    fn test_stream_request_with_reply() {
        let request = StreamRequest::with_reply("Hello".to_string(), "thread-123".to_string(), 42);

        assert_eq!(request.prompt, "Hello");
        assert!(!request.session_id.is_empty());
        assert_eq!(request.thread_id, Some("thread-123".to_string()));
        assert_eq!(request.reply_to, Some(42));
    }

    #[test]
    fn test_stream_request_serialization() {
        let request = StreamRequest {
            prompt: "Test prompt".to_string(),
            session_id: "session-abc".to_string(),
            thread_id: Some("thread-xyz".to_string()),
            reply_to: Some(100),
            thread_type: Some(ThreadType::Programming),
            permission_mode: Some(PermissionMode::Plan),
            working_directory: None,
            plan_mode: false,
            images: Vec::new(),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let deserialized: StreamRequest =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(request, deserialized);
    }

    // ============= ErrorInfo Tests =============

    #[test]
    fn test_error_info_new() {
        let error = ErrorInfo::new(
            "tool_execution_failed".to_string(),
            "File not found".to_string(),
        );

        assert_eq!(error.error_code, "tool_execution_failed");
        assert_eq!(error.message, "File not found");
        assert!(!error.id.is_empty());
        // ID should be UUID format (36 chars)
        assert_eq!(error.id.len(), 36);
    }

    #[test]
    fn test_error_info_unique_ids() {
        let error1 = ErrorInfo::new("error1".to_string(), "msg1".to_string());
        let error2 = ErrorInfo::new("error2".to_string(), "msg2".to_string());

        // Each error should have a unique ID
        assert_ne!(error1.id, error2.id);
    }

    #[test]
    fn test_error_info_serialization() {
        let error = ErrorInfo::new("test_error".to_string(), "Test message".to_string());

        let json = serde_json::to_string(&error).expect("Failed to serialize");
        let deserialized: ErrorInfo = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(error.id, deserialized.id);
        assert_eq!(error.error_code, deserialized.error_code);
        assert_eq!(error.message, deserialized.message);
    }

    #[test]
    fn test_error_info_equality() {
        let error1 = ErrorInfo {
            id: "test-id".to_string(),
            error_code: "code".to_string(),
            message: "msg".to_string(),
            timestamp: Utc::now(),
        };
        let error2 = ErrorInfo {
            id: "test-id".to_string(),
            error_code: "code".to_string(),
            message: "msg".to_string(),
            timestamp: error1.timestamp,
        };

        assert_eq!(error1, error2);
    }

    #[test]
    fn test_error_info_clone() {
        let error = ErrorInfo::new("code".to_string(), "message".to_string());
        let cloned = error.clone();

        assert_eq!(error.id, cloned.id);
        assert_eq!(error.error_code, cloned.error_code);
        assert_eq!(error.message, cloned.message);
    }

    // ============================================================================
    // Reasoning/Thinking Tests
    // ============================================================================

    #[test]
    fn test_message_append_reasoning_token() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.append_reasoning_token("Let me think...");
        assert_eq!(message.reasoning_content, "Let me think...");

        message.append_reasoning_token(" Step 1.");
        assert_eq!(message.reasoning_content, "Let me think... Step 1.");
    }

    #[test]
    fn test_message_reasoning_token_count() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: "Let me analyze this step by step".to_string(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        // "Let me analyze this step by step" = 7 words
        assert_eq!(message.reasoning_token_count(), 7);

        message.reasoning_content = String::new();
        assert_eq!(message.reasoning_token_count(), 0);
    }

    #[test]
    fn test_message_toggle_reasoning_collapsed() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: "Some reasoning".to_string(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        assert!(!message.reasoning_collapsed);
        message.toggle_reasoning_collapsed();
        assert!(message.reasoning_collapsed);
        message.toggle_reasoning_collapsed();
        assert!(!message.reasoning_collapsed);
    }

    #[test]
    fn test_message_finalize_collapses_reasoning() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: "Response content".to_string(),
            reasoning_content: "Some reasoning".to_string(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        // Reasoning should not be collapsed while streaming
        assert!(!message.reasoning_collapsed);

        message.finalize();

        // After finalize, reasoning should be collapsed by default
        assert!(message.reasoning_collapsed);
        assert!(!message.is_streaming);
        assert_eq!(message.content, "Response content");
    }

    #[test]
    fn test_message_finalize_no_reasoning_stays_uncollapsed() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: "Response content".to_string(),
            reasoning_content: String::new(), // No reasoning
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.finalize();

        // If there's no reasoning content, collapsed state shouldn't change
        // (the flag is not set to true because there's nothing to collapse)
        assert!(!message.reasoning_collapsed);
    }

    #[test]
    fn test_message_reasoning_serialization() {
        let message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: "Response".to_string(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: "Let me think about this".to_string(),
            reasoning_collapsed: true,
            segments: Vec::new(),
            render_version: 0,
        };

        let json = serde_json::to_string(&message).expect("Failed to serialize");
        let deserialized: Message = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(message.reasoning_content, deserialized.reasoning_content);
        assert_eq!(
            message.reasoning_collapsed,
            deserialized.reasoning_collapsed
        );
    }

    // ============================================================================
    // ToolEvent Tests
    // ============================================================================

    #[test]
    fn test_tool_event_new() {
        let event = ToolEvent::new("tool-123".to_string(), "Read".to_string());

        assert_eq!(event.tool_call_id, "tool-123");
        assert_eq!(event.function_name, "Read");
        assert_eq!(event.display_name, None);
        assert_eq!(event.status, ToolEventStatus::Running);
        assert!(event.completed_at.is_none());
        assert!(event.duration_secs.is_none());
    }

    #[test]
    fn test_tool_event_complete() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Bash".to_string());

        assert_eq!(event.status, ToolEventStatus::Running);
        assert!(event.completed_at.is_none());
        assert!(event.duration_secs.is_none());

        event.complete();

        assert_eq!(event.status, ToolEventStatus::Complete);
        assert!(event.completed_at.is_some());
        assert!(event.duration_secs.is_some());
    }

    #[test]
    fn test_tool_event_fail() {
        let mut event = ToolEvent::new("tool-456".to_string(), "Grep".to_string());

        assert_eq!(event.status, ToolEventStatus::Running);

        event.fail();

        assert_eq!(event.status, ToolEventStatus::Failed);
        assert!(event.completed_at.is_some());
        assert!(event.duration_secs.is_some());
    }

    #[test]
    fn test_tool_event_with_display_name() {
        let mut event = ToolEvent::new("tool-789".to_string(), "Read".to_string());
        event.display_name = Some("Read src/main.rs".to_string());

        assert_eq!(event.function_name, "Read");
        assert_eq!(event.display_name, Some("Read src/main.rs".to_string()));
    }

    #[test]
    fn test_tool_event_serialization() {
        let event = ToolEvent::new("tool-999".to_string(), "Write".to_string());

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        let deserialized: ToolEvent = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(event.tool_call_id, deserialized.tool_call_id);
        assert_eq!(event.function_name, deserialized.function_name);
        assert_eq!(event.display_name, deserialized.display_name);
        assert_eq!(event.status, deserialized.status);
    }

    #[test]
    fn test_tool_event_serialization_with_display_name() {
        let mut event = ToolEvent::new("tool-111".to_string(), "Bash".to_string());
        event.display_name = Some("cd /path && ls".to_string());

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        let deserialized: ToolEvent = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(event.display_name, deserialized.display_name);
        assert_eq!(
            deserialized.display_name,
            Some("cd /path && ls".to_string())
        );
    }

    #[test]
    fn test_tool_event_new_initializes_new_fields() {
        let event = ToolEvent::new("tool-123".to_string(), "Read".to_string());

        assert!(event.args_json.is_empty());
        assert_eq!(event.args_display, None);
        assert_eq!(event.result_preview, None);
        assert!(!event.result_is_error);
    }

    #[test]
    fn test_tool_event_append_arg_chunk() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Read".to_string());

        assert!(event.args_json.is_empty());

        event.append_arg_chunk("{\"file_path\":");
        assert_eq!(event.args_json, "{\"file_path\":");

        event.append_arg_chunk("\"/src/main.rs\"}");
        assert_eq!(event.args_json, "{\"file_path\":\"/src/main.rs\"}");
    }

    #[test]
    fn test_tool_event_append_arg_chunk_empty() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Read".to_string());

        event.append_arg_chunk("");
        assert!(event.args_json.is_empty());
    }

    #[test]
    fn test_tool_event_set_result_short() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Read".to_string());

        let content = "File contents here";
        event.set_result(content, false);

        assert_eq!(event.result_preview, Some("File contents here".to_string()));
        assert!(!event.result_is_error);
    }

    #[test]
    fn test_tool_event_set_result_error() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Read".to_string());

        event.set_result("File not found", true);

        assert_eq!(event.result_preview, Some("File not found".to_string()));
        assert!(event.result_is_error);
    }

    #[test]
    fn test_tool_event_set_result_truncates_long_content() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Bash".to_string());

        // Create content longer than 500 chars
        let long_content = "x".repeat(600);
        event.set_result(&long_content, false);

        let preview = event.result_preview.unwrap();
        assert!(preview.len() <= 503); // 500 + "..."
        assert!(preview.ends_with("..."));
        assert!(!event.result_is_error);
    }

    #[test]
    fn test_tool_event_set_result_truncates_at_word_boundary() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Bash".to_string());

        // Create content with words, where a word boundary falls within the last 50 chars of the 500-char limit
        let mut content = "word ".repeat(99); // 495 chars
        content.push_str("end"); // 498 chars total, then more
        content.push_str(" extra words that go beyond the limit and should be truncated nicely");

        event.set_result(&content, false);

        let preview = event.result_preview.unwrap();
        assert!(preview.ends_with("..."));
        // Should truncate at a word boundary
        assert!(!preview.contains(" extra"));
    }

    #[test]
    fn test_tool_event_set_result_exactly_500_chars() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Read".to_string());

        let content = "x".repeat(500);
        event.set_result(&content, false);

        let preview = event.result_preview.unwrap();
        assert_eq!(preview.len(), 500);
        assert!(!preview.ends_with("..."));
    }

    #[test]
    fn test_tool_event_set_result_501_chars() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Read".to_string());

        let content = "x".repeat(501);
        event.set_result(&content, false);

        let preview = event.result_preview.unwrap();
        assert!(preview.ends_with("..."));
        // 500 x's + "..."
        assert_eq!(preview.len(), 503);
    }

    #[test]
    fn test_tool_event_serialization_with_new_fields() {
        let mut event = ToolEvent::new("tool-123".to_string(), "Read".to_string());
        event.args_json = "{\"file_path\":\"/src/main.rs\"}".to_string();
        event.args_display = Some("Reading /src/main.rs".to_string());
        event.set_result("fn main() { }", false);

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        let deserialized: ToolEvent = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(event.args_json, deserialized.args_json);
        assert_eq!(event.args_display, deserialized.args_display);
        assert_eq!(event.result_preview, deserialized.result_preview);
        assert_eq!(event.result_is_error, deserialized.result_is_error);
    }

    #[test]
    fn test_tool_event_deserialization_without_new_fields() {
        // Test backward compatibility - deserialize JSON without new fields
        let json = r#"{
            "tool_call_id": "tool-legacy",
            "function_name": "Read",
            "display_name": null,
            "status": "Running",
            "started_at": "2024-01-01T00:00:00Z",
            "completed_at": null,
            "duration_secs": null
        }"#;

        let event: ToolEvent = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(event.tool_call_id, "tool-legacy");
        assert_eq!(event.function_name, "Read");
        // New fields should default to their empty/false values
        assert!(event.args_json.is_empty());
        assert_eq!(event.args_display, None);
        assert_eq!(event.result_preview, None);
        assert!(!event.result_is_error);
    }

    #[test]
    fn test_message_segment_text() {
        let segment = MessageSegment::Text("Hello, world!".to_string());

        if let MessageSegment::Text(text) = segment {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("Expected MessageSegment::Text");
        }
    }

    #[test]
    fn test_message_segment_tool_event() {
        let event = ToolEvent::new("tool-222".to_string(), "Glob".to_string());
        let segment = MessageSegment::ToolEvent(event);

        if let MessageSegment::ToolEvent(e) = segment {
            assert_eq!(e.tool_call_id, "tool-222");
            assert_eq!(e.function_name, "Glob");
        } else {
            panic!("Expected MessageSegment::ToolEvent");
        }
    }

    #[test]
    fn test_message_start_tool_event() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.start_tool_event("tool-333".to_string(), "Read".to_string());

        assert_eq!(message.segments.len(), 1);
        if let MessageSegment::ToolEvent(event) = &message.segments[0] {
            assert_eq!(event.tool_call_id, "tool-333");
            assert_eq!(event.function_name, "Read");
            assert_eq!(event.status, ToolEventStatus::Running);
        } else {
            panic!("Expected ToolEvent segment");
        }
    }

    #[test]
    fn test_message_complete_tool_event() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.start_tool_event("tool-444".to_string(), "Bash".to_string());
        message.complete_tool_event("tool-444");

        if let MessageSegment::ToolEvent(event) = &message.segments[0] {
            assert_eq!(event.status, ToolEventStatus::Complete);
            assert!(event.completed_at.is_some());
            assert!(event.duration_secs.is_some());
        } else {
            panic!("Expected ToolEvent segment");
        }
    }

    #[test]
    fn test_message_fail_tool_event() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.start_tool_event("tool-555".to_string(), "Write".to_string());
        message.fail_tool_event("tool-555");

        if let MessageSegment::ToolEvent(event) = &message.segments[0] {
            assert_eq!(event.status, ToolEventStatus::Failed);
            assert!(event.completed_at.is_some());
        } else {
            panic!("Expected ToolEvent segment");
        }
    }

    #[test]
    fn test_message_get_tool_event() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.start_tool_event("tool-666".to_string(), "Grep".to_string());

        let event = message.get_tool_event("tool-666");
        assert!(event.is_some());
        assert_eq!(event.unwrap().function_name, "Grep");

        let nonexistent = message.get_tool_event("tool-999");
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_message_has_running_tools() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        assert!(!message.has_running_tools());

        message.start_tool_event("tool-777".to_string(), "Read".to_string());
        assert!(message.has_running_tools());

        message.complete_tool_event("tool-777");
        assert!(!message.has_running_tools());
    }

    #[test]
    fn test_message_multiple_tool_events() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.start_tool_event("tool-1".to_string(), "Read".to_string());
        message.start_tool_event("tool-2".to_string(), "Bash".to_string());
        message.start_tool_event("tool-3".to_string(), "Write".to_string());

        assert_eq!(message.segments.len(), 3);
        assert!(message.has_running_tools());

        message.complete_tool_event("tool-1");
        message.complete_tool_event("tool-2");
        assert!(message.has_running_tools()); // tool-3 still running

        message.complete_tool_event("tool-3");
        assert!(!message.has_running_tools());
    }

    #[test]
    fn test_message_add_text_segment() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.add_text_segment("Hello".to_string());
        message.add_text_segment(" world".to_string());

        assert_eq!(message.segments.len(), 1);
        if let MessageSegment::Text(text) = &message.segments[0] {
            assert_eq!(text, "Hello world");
        } else {
            panic!("Expected Text segment");
        }
    }

    #[test]
    fn test_message_mixed_segments() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.add_text_segment("Let me check that file...".to_string());
        message.start_tool_event("tool-888".to_string(), "Read".to_string());
        message.add_text_segment("The file contains:".to_string());

        assert_eq!(message.segments.len(), 3);

        // Verify segment types
        assert!(matches!(&message.segments[0], MessageSegment::Text(_)));
        assert!(matches!(&message.segments[1], MessageSegment::ToolEvent(_)));
        assert!(matches!(&message.segments[2], MessageSegment::Text(_)));
    }

    #[test]
    fn test_message_set_tool_display_name() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        // Start a tool event
        message.start_tool_event("tool-999".to_string(), "Read".to_string());

        // Initially display_name should be None
        if let MessageSegment::ToolEvent(event) = &message.segments[0] {
            assert_eq!(event.display_name, None);
        } else {
            panic!("Expected ToolEvent segment");
        }

        // Set the display_name
        message.set_tool_display_name("tool-999", "Read src/main.rs".to_string());

        // Verify display_name was set
        if let MessageSegment::ToolEvent(event) = &message.segments[0] {
            assert_eq!(event.display_name, Some("Read src/main.rs".to_string()));
        } else {
            panic!("Expected ToolEvent segment");
        }
    }

    #[test]
    fn test_message_set_tool_display_name_multiple_tools() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        // Start multiple tool events
        message.start_tool_event("tool-1".to_string(), "Read".to_string());
        message.start_tool_event("tool-2".to_string(), "Bash".to_string());
        message.start_tool_event("tool-3".to_string(), "Write".to_string());

        // Set display_name for the second tool
        message.set_tool_display_name("tool-2", "Run tests".to_string());

        // Verify only tool-2 was updated
        if let MessageSegment::ToolEvent(event) = &message.segments[0] {
            assert_eq!(event.tool_call_id, "tool-1");
            assert_eq!(event.display_name, None);
        }
        if let MessageSegment::ToolEvent(event) = &message.segments[1] {
            assert_eq!(event.tool_call_id, "tool-2");
            assert_eq!(event.display_name, Some("Run tests".to_string()));
        }
        if let MessageSegment::ToolEvent(event) = &message.segments[2] {
            assert_eq!(event.tool_call_id, "tool-3");
            assert_eq!(event.display_name, None);
        }
    }

    #[test]
    fn test_message_set_tool_display_name_nonexistent() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        message.start_tool_event("tool-123".to_string(), "Read".to_string());

        // Try to set display_name for a nonexistent tool
        message.set_tool_display_name("tool-999", "Nonexistent".to_string());

        // Verify the existing tool was not affected
        if let MessageSegment::ToolEvent(event) = &message.segments[0] {
            assert_eq!(event.tool_call_id, "tool-123");
            assert_eq!(event.display_name, None);
        }
    }

    #[test]
    fn test_message_segments_maintain_order() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        // Simulate a realistic interleaved streaming scenario
        message.append_token("Let me ");
        message.append_token("check that file.");
        message.start_tool_event("tool-1".to_string(), "Read".to_string());
        message.complete_tool_event("tool-1");
        message.append_token("The file ");
        message.append_token("contains important data.");

        // Verify segment order: text -> tool -> text
        assert_eq!(message.segments.len(), 3);

        // First segment: text
        if let MessageSegment::Text(text) = &message.segments[0] {
            assert_eq!(text, "Let me check that file.");
        } else {
            panic!("Expected Text segment at position 0");
        }

        // Second segment: tool event
        if let MessageSegment::ToolEvent(event) = &message.segments[1] {
            assert_eq!(event.tool_call_id, "tool-1");
            assert_eq!(event.function_name, "Read");
            assert_eq!(event.status, ToolEventStatus::Complete);
        } else {
            panic!("Expected ToolEvent segment at position 1");
        }

        // Third segment: text
        if let MessageSegment::Text(text) = &message.segments[2] {
            assert_eq!(text, "The file contains important data.");
        } else {
            panic!("Expected Text segment at position 2");
        }

        // Verify partial_content accumulated correctly
        assert_eq!(
            message.partial_content,
            "Let me check that file.The file contains important data."
        );
    }

    #[test]
    fn test_message_finalize_with_segments() {
        let mut message = Message {
            id: 1,
            thread_id: "thread-123".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: true,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: false,
            segments: Vec::new(),
            render_version: 0,
        };

        // Build up content with interleaved text and tools
        message.append_token("Starting task.");
        message.start_tool_event("tool-1".to_string(), "Bash".to_string());
        message.complete_tool_event("tool-1");
        message.append_token("Task completed.");

        // Verify streaming state before finalization
        assert!(message.is_streaming);
        assert!(!message.partial_content.is_empty());
        assert!(message.content.is_empty());
        assert_eq!(message.segments.len(), 3);

        // Finalize the message
        message.finalize();

        // Verify finalized state
        assert!(!message.is_streaming);
        assert!(message.partial_content.is_empty());
        assert_eq!(message.content, "Starting task.Task completed.");

        // Segments should be preserved after finalization
        assert_eq!(message.segments.len(), 3);
    }
}
