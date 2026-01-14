use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

/// Helper to deserialize id as either string or integer
fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
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

/// Type of thread - determines UI behavior and available features
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThreadType {
    /// Standard normal thread (default)
    #[default]
    Normal,
    /// Programming-focused thread with code-specific features
    Programming,
}

/// Helper to deserialize ThreadType with null handling
/// Returns Default (Normal) if the field is null or missing
fn deserialize_thread_type<'de, D>(deserializer: D) -> Result<ThreadType, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<ThreadType>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

/// Represents a conversation thread from the backend API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Thread {
    /// Unique identifier from backend (can be string or integer)
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    /// Title derived from first message
    #[serde(default)]
    pub title: String,
    /// Preview of the last message
    #[serde(default)]
    pub preview: String,
    /// When the thread was last updated
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
    /// Type of thread (Normal or Programming)
    #[serde(default, rename = "type", deserialize_with = "deserialize_thread_type")]
    pub thread_type: ThreadType,
    /// Model used for this thread
    #[serde(default)]
    pub model: Option<String>,
    /// Permission mode for this thread
    #[serde(default)]
    pub permission_mode: Option<String>,
    /// Number of messages in this thread
    #[serde(default)]
    pub message_count: i32,
    /// When the thread was created
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

/// Response from the thread list endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadListResponse {
    /// List of threads
    pub threads: Vec<Thread>,
    /// Total number of threads available
    pub total: i32,
}

/// Response from the thread detail endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadDetailResponse {
    /// Thread ID
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    /// Type of thread
    #[serde(default, rename = "type", deserialize_with = "deserialize_thread_type")]
    pub thread_type: ThreadType,
    /// Thread name/title
    #[serde(default)]
    pub name: Option<String>,
    /// Project path for programming threads
    #[serde(default)]
    pub project_path: Option<String>,
    /// Provider used for this thread
    #[serde(default)]
    pub provider: Option<String>,
    /// Messages in this thread
    #[serde(default)]
    pub messages: Vec<ServerMessage>,
}

/// Function details within a tool call
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallFunction {
    /// Name of the function being called
    pub name: String,
    /// Arguments passed to the function (JSON string)
    #[serde(default)]
    pub arguments: String,
}

/// Represents a tool call made by the assistant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub id: String,
    /// Type of tool call (usually "function")
    #[serde(rename = "type")]
    pub call_type: String,
    /// Function details
    pub function: ToolCallFunction,
}

/// Message format from the server (different from client Message)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerMessage {
    /// Role of the message sender
    pub role: MessageRole,
    /// Content of the message (may be empty for tool calls)
    #[serde(default)]
    pub content: Option<String>,
    /// Tool calls made by the assistant
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// ID of the tool call this message responds to
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// Name for tool responses
    #[serde(default)]
    pub name: Option<String>,
}

/// Role of a message in a conversation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl ServerMessage {
    /// Convert a ServerMessage to a client Message.
    ///
    /// # Arguments
    /// * `thread_id` - The thread ID to associate with the message
    /// * `id` - The message ID to assign
    pub fn to_client_message(self, thread_id: &str, id: i64) -> Message {
        let role = match self.role {
            MessageRole::User => MessageRole::User,
            MessageRole::Assistant => MessageRole::Assistant,
            MessageRole::System => MessageRole::System,
        };

        Message {
            id,
            thread_id: thread_id.to_string(),
            role,
            content: self.content.unwrap_or_default(),
            created_at: Utc::now(),  // Server doesn't provide per-message timestamps
            is_streaming: false,
            partial_content: String::new(),
        }
    }
}

/// Represents a message within a thread from the backend API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// Message ID from backend (message_id)
    pub id: i64,
    /// ID of the thread this message belongs to
    pub thread_id: String,
    /// Role of the message sender
    pub role: MessageRole,
    /// Content of the message
    pub content: String,
    /// When the message was created
    pub created_at: DateTime<Utc>,
    /// Whether the message is currently being streamed
    #[serde(default)]
    pub is_streaming: bool,
    /// Partial content accumulated during streaming
    #[serde(default)]
    pub partial_content: String,
}

impl Message {
    /// Append a token to the partial content during streaming
    pub fn append_token(&mut self, token: &str) {
        self.partial_content.push_str(token);
    }

    /// Finalize the message by moving partial_content to content and marking as not streaming
    pub fn finalize(&mut self) {
        if self.is_streaming {
            self.content = std::mem::take(&mut self.partial_content);
            self.is_streaming = false;
        }
    }
}

/// Request structure for streaming API calls
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamRequest {
    /// The prompt/message to send
    pub prompt: String,
    /// Session ID for authentication (required by backend)
    pub session_id: String,
    /// Thread ID - None means create a new thread
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Message ID to reply to - for future stitching support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<i64>,
    /// Type of thread to create (normal or programming)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_type: Option<ThreadType>,
    /// Whether to enable plan mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_mode: Option<bool>,
}

impl StreamRequest {
    /// Create a new StreamRequest for a new thread
    pub fn new(prompt: String) -> Self {
        Self {
            prompt,
            session_id: Uuid::new_v4().to_string(),
            thread_id: None,
            reply_to: None,
            thread_type: None,
            plan_mode: None,
        }
    }

    /// Create a StreamRequest for an existing thread
    #[allow(dead_code)]
    pub fn with_thread(prompt: String, thread_id: String) -> Self {
        Self {
            prompt,
            session_id: Uuid::new_v4().to_string(),
            thread_id: Some(thread_id),
            reply_to: None,
            thread_type: None,
            plan_mode: None,
        }
    }

    /// Create a StreamRequest as a reply to a specific message
    #[allow(dead_code)]
    pub fn with_reply(prompt: String, thread_id: String, reply_to: i64) -> Self {
        Self {
            prompt,
            session_id: Uuid::new_v4().to_string(),
            thread_id: Some(thread_id),
            reply_to: Some(reply_to),
            thread_type: None,
            plan_mode: None,
        }
    }
}

/// Request structure for programming stream API calls.
///
/// Similar to StreamRequest but with additional fields for programming mode
/// options like plan mode and permission bypassing. Used with the
/// `/v1/programming/stream` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgrammingStreamRequest {
    /// The thread ID to continue (required for programming streams)
    pub thread_id: String,
    /// The prompt/message content to send
    pub content: String,
    /// Whether to enable plan mode for this request
    #[serde(default)]
    pub plan_mode: bool,
    /// Whether to bypass permission prompts
    #[serde(default)]
    pub bypass_permissions: bool,
    /// Session ID for authentication
    pub session_id: String,
}

impl ProgrammingStreamRequest {
    /// Create a new ProgrammingStreamRequest with default options.
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the thread to continue
    /// * `content` - The message content to send
    pub fn new(thread_id: String, content: String) -> Self {
        Self {
            thread_id,
            content,
            plan_mode: false,
            bypass_permissions: false,
            session_id: Uuid::new_v4().to_string(),
        }
    }

    /// Create a ProgrammingStreamRequest with plan mode enabled.
    #[allow(dead_code)]
    pub fn with_plan_mode(thread_id: String, content: String) -> Self {
        Self {
            thread_id,
            content,
            plan_mode: true,
            bypass_permissions: false,
            session_id: Uuid::new_v4().to_string(),
        }
    }

    /// Create a ProgrammingStreamRequest with bypass permissions enabled.
    #[allow(dead_code)]
    pub fn with_bypass_permissions(thread_id: String, content: String) -> Self {
        Self {
            thread_id,
            content,
            plan_mode: false,
            bypass_permissions: true,
            session_id: Uuid::new_v4().to_string(),
        }
    }

    /// Create a ProgrammingStreamRequest with all options.
    #[allow(dead_code)]
    pub fn with_options(
        thread_id: String,
        content: String,
        plan_mode: bool,
        bypass_permissions: bool,
    ) -> Self {
        Self {
            thread_id,
            content,
            plan_mode,
            bypass_permissions,
            session_id: Uuid::new_v4().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_type_default() {
        assert_eq!(ThreadType::default(), ThreadType::Normal);
    }

    #[test]
    fn test_thread_type_variants() {
        assert_eq!(ThreadType::Normal, ThreadType::Normal);
        assert_eq!(ThreadType::Programming, ThreadType::Programming);
        assert_ne!(ThreadType::Normal, ThreadType::Programming);
    }

    #[test]
    fn test_thread_type_serialization() {
        // Test Normal serialization (lowercase for server compatibility)
        let normal = ThreadType::Normal;
        let json = serde_json::to_string(&normal).expect("Failed to serialize");
        assert_eq!(json, "\"normal\"");
        let deserialized: ThreadType = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(normal, deserialized);

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
            preview: "Hello, world!".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
        };

        assert_eq!(thread.id, "thread-123");
        assert_eq!(thread.title, "Test Thread");
        assert_eq!(thread.preview, "Hello, world!");
        assert_eq!(thread.thread_type, ThreadType::Normal);
    }

    #[test]
    fn test_thread_creation_programming() {
        let thread = Thread {
            id: "thread-456".to_string(),
            title: "Code Review".to_string(),
            preview: "Let me review this code".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Programming,
            model: Some("gpt-4".to_string()),
            permission_mode: Some("auto".to_string()),
            message_count: 5,
            created_at: Utc::now(),
        };

        assert_eq!(thread.id, "thread-456");
        assert_eq!(thread.thread_type, ThreadType::Programming);
    }

    #[test]
    fn test_thread_serialization() {
        let thread = Thread {
            id: "thread-456".to_string(),
            title: "Serialization Test".to_string(),
            preview: "Testing JSON".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Normal,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
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
            preview: "Code discussion".to_string(),
            updated_at: Utc::now(),
            thread_type: ThreadType::Programming,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&thread).expect("Failed to serialize");
        let deserialized: Thread = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(thread, deserialized);
        assert_eq!(deserialized.thread_type, ThreadType::Programming);
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
        assert_eq!(thread.thread_type, ThreadType::Normal);
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
        assert_eq!(thread.thread_type, ThreadType::Normal);
    }

    #[test]
    fn test_thread_type_clone() {
        let original = ThreadType::Programming;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_thread_type_debug() {
        // Verify Debug trait is implemented correctly
        let normal = ThreadType::Normal;
        let prog = ThreadType::Programming;
        assert_eq!(format!("{:?}", normal), "Normal");
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
        };

        assert_eq!(message.id, 1);
        assert_eq!(message.thread_id, "thread-123");
        assert_eq!(message.role, MessageRole::User);
        assert_eq!(message.content, "Hello!");
        assert!(!message.is_streaming);
        assert!(message.partial_content.is_empty());
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
        };

        message.append_token("Hello");
        assert_eq!(message.partial_content, "Hello");

        message.append_token(", ");
        assert_eq!(message.partial_content, "Hello, ");

        message.append_token("world!");
        assert_eq!(message.partial_content, "Hello, world!");
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
            "role": "User",
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
        let request = StreamRequest::with_reply(
            "Hello".to_string(),
            "thread-123".to_string(),
            42,
        );

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
            plan_mode: Some(true),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let deserialized: StreamRequest = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(request, deserialized);
    }

    // ProgrammingStreamRequest tests

    #[test]
    fn test_programming_stream_request_new() {
        let request = ProgrammingStreamRequest::new(
            "thread-123".to_string(),
            "Help me write a function".to_string(),
        );

        assert_eq!(request.thread_id, "thread-123");
        assert_eq!(request.content, "Help me write a function");
        assert!(!request.plan_mode);
        assert!(!request.bypass_permissions);
        assert!(!request.session_id.is_empty());
    }

    #[test]
    fn test_programming_stream_request_with_plan_mode() {
        let request = ProgrammingStreamRequest::with_plan_mode(
            "thread-456".to_string(),
            "Plan the implementation".to_string(),
        );

        assert_eq!(request.thread_id, "thread-456");
        assert_eq!(request.content, "Plan the implementation");
        assert!(request.plan_mode);
        assert!(!request.bypass_permissions);
        assert!(!request.session_id.is_empty());
    }

    #[test]
    fn test_programming_stream_request_with_bypass_permissions() {
        let request = ProgrammingStreamRequest::with_bypass_permissions(
            "thread-789".to_string(),
            "Execute without prompts".to_string(),
        );

        assert_eq!(request.thread_id, "thread-789");
        assert_eq!(request.content, "Execute without prompts");
        assert!(!request.plan_mode);
        assert!(request.bypass_permissions);
        assert!(!request.session_id.is_empty());
    }

    #[test]
    fn test_programming_stream_request_with_options() {
        let request = ProgrammingStreamRequest::with_options(
            "thread-abc".to_string(),
            "Full options request".to_string(),
            true,
            true,
        );

        assert_eq!(request.thread_id, "thread-abc");
        assert_eq!(request.content, "Full options request");
        assert!(request.plan_mode);
        assert!(request.bypass_permissions);
        assert!(!request.session_id.is_empty());
    }

    #[test]
    fn test_programming_stream_request_serialization() {
        let request = ProgrammingStreamRequest {
            thread_id: "thread-xyz".to_string(),
            content: "Test content".to_string(),
            plan_mode: true,
            bypass_permissions: false,
            session_id: "session-123".to_string(),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let deserialized: ProgrammingStreamRequest =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_programming_stream_request_serialization_all_fields() {
        let request = ProgrammingStreamRequest {
            thread_id: "thread-full".to_string(),
            content: "Full test".to_string(),
            plan_mode: true,
            bypass_permissions: true,
            session_id: "session-full".to_string(),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");

        // Verify all fields are present in JSON
        assert!(json.contains("\"thread_id\":\"thread-full\""));
        assert!(json.contains("\"content\":\"Full test\""));
        assert!(json.contains("\"plan_mode\":true"));
        assert!(json.contains("\"bypass_permissions\":true"));
        assert!(json.contains("\"session_id\":\"session-full\""));

        let deserialized: ProgrammingStreamRequest =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_programming_stream_request_deserialization_with_defaults() {
        // Test deserializing JSON without optional bool fields (should default to false)
        let json = r#"{
            "thread_id": "thread-minimal",
            "content": "Minimal request",
            "session_id": "session-minimal"
        }"#;

        let request: ProgrammingStreamRequest =
            serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(request.thread_id, "thread-minimal");
        assert_eq!(request.content, "Minimal request");
        assert_eq!(request.session_id, "session-minimal");
        // Default values should be applied
        assert!(!request.plan_mode);
        assert!(!request.bypass_permissions);
    }

    #[test]
    fn test_programming_stream_request_unique_session_ids() {
        let request1 = ProgrammingStreamRequest::new(
            "thread-1".to_string(),
            "Request 1".to_string(),
        );
        let request2 = ProgrammingStreamRequest::new(
            "thread-2".to_string(),
            "Request 2".to_string(),
        );

        // Each request should have a unique session ID
        assert_ne!(request1.session_id, request2.session_id);
    }
}
