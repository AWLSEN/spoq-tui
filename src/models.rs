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

/// Helper to deserialize nullable strings as empty string
/// Handles both missing fields and explicit null values
fn deserialize_nullable_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

/// Represents an inline error to be displayed in a thread
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorInfo {
    /// Unique identifier for this error (for dismiss tracking)
    pub id: String,
    /// Error code (e.g., "tool_execution_failed", "rate_limit_exceeded")
    pub error_code: String,
    /// Human-readable error message
    pub message: String,
    /// When the error occurred
    pub timestamp: DateTime<Utc>,
}

impl ErrorInfo {
    /// Create a new ErrorInfo with a generated ID
    pub fn new(error_code: String, message: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            error_code,
            message,
            timestamp: Utc::now(),
        }
    }
}

/// Represents a conversation thread from the backend API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Thread {
    /// Unique identifier from backend (can be string or integer)
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    /// Title derived from first message (API sends as "name")
    #[serde(default, deserialize_with = "deserialize_nullable_string", alias = "name")]
    pub title: String,
    /// Description of the thread
    #[serde(default)]
    pub description: Option<String>,
    /// Preview of the last message
    #[serde(default)]
    pub preview: String,
    /// When the thread was last updated (server sends as "last_activity")
    #[serde(default = "Utc::now", alias = "last_activity")]
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
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Status of a tool event for inline display
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolEventStatus {
    /// Tool is currently running
    Running,
    /// Tool completed successfully
    Complete,
    /// Tool failed
    Failed,
}

/// A tool event that can be displayed inline with message content
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolEvent {
    /// The tool call ID from the backend
    pub tool_call_id: String,
    /// Name of the tool (e.g., "Bash", "Read", "Glob")
    pub function_name: String,
    /// Optional display name (e.g., "Read src/main.rs" instead of just "Read")
    pub display_name: Option<String>,
    /// Current status of the tool
    pub status: ToolEventStatus,
    /// When the tool started
    pub started_at: DateTime<Utc>,
    /// When the tool completed (if complete)
    pub completed_at: Option<DateTime<Utc>>,
    /// Duration in seconds (calculated when complete)
    pub duration_secs: Option<f64>,
    /// Accumulated JSON arguments from streaming chunks
    #[serde(default)]
    pub args_json: String,
    /// Formatted display string for arguments (e.g., "Reading /src/main.rs")
    #[serde(default)]
    pub args_display: Option<String>,
    /// Truncated result preview (max ~500 chars)
    #[serde(default)]
    pub result_preview: Option<String>,
    /// Whether the result was an error
    #[serde(default)]
    pub result_is_error: bool,
}

impl ToolEvent {
    /// Create a new running tool event
    pub fn new(tool_call_id: String, function_name: String) -> Self {
        Self {
            tool_call_id,
            function_name,
            display_name: None,
            status: ToolEventStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            duration_secs: None,
            args_json: String::new(),
            args_display: None,
            result_preview: None,
            result_is_error: false,
        }
    }

    /// Mark the tool as complete
    pub fn complete(&mut self) {
        self.status = ToolEventStatus::Complete;
        self.completed_at = Some(Utc::now());
        self.duration_secs = Some((Utc::now() - self.started_at).num_milliseconds() as f64 / 1000.0);
    }

    /// Mark the tool as failed
    pub fn fail(&mut self) {
        self.status = ToolEventStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.duration_secs = Some((Utc::now() - self.started_at).num_milliseconds() as f64 / 1000.0);
    }

    /// Append a chunk of JSON arguments from streaming
    pub fn append_arg_chunk(&mut self, chunk: &str) {
        self.args_json.push_str(chunk);
    }

    /// Set the result preview, truncating if necessary
    ///
    /// # Arguments
    /// * `content` - The full result content
    /// * `is_error` - Whether the result represents an error
    pub fn set_result(&mut self, content: &str, is_error: bool) {
        const MAX_PREVIEW_LEN: usize = 500;

        self.result_is_error = is_error;

        if content.len() <= MAX_PREVIEW_LEN {
            self.result_preview = Some(content.to_string());
        } else {
            // Truncate and add ellipsis
            let truncated = &content[..MAX_PREVIEW_LEN];
            // Try to truncate at a word boundary
            let preview = if let Some(last_space) = truncated.rfind(char::is_whitespace) {
                if last_space > MAX_PREVIEW_LEN - 50 {
                    // Only use word boundary if we're not cutting off too much
                    &truncated[..last_space]
                } else {
                    truncated
                }
            } else {
                truncated
            };
            self.result_preview = Some(format!("{}...", preview));
        }
    }
}

/// A segment of message content - either text or a tool event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageSegment {
    /// Plain text content
    Text(String),
    /// An inline tool event
    ToolEvent(ToolEvent),
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
            reasoning_content: String::new(),  // Server may not provide reasoning history
            reasoning_collapsed: true,
            segments: Vec::new(),
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
    /// Reasoning/thinking content from the assistant
    #[serde(default)]
    pub reasoning_content: String,
    /// Whether the reasoning block is collapsed in the UI
    #[serde(default)]
    pub reasoning_collapsed: bool,
    /// Segments of content including inline tool events
    #[serde(default)]
    pub segments: Vec<MessageSegment>,
}

impl Message {
    /// Append a token to the partial content during streaming
    pub fn append_token(&mut self, token: &str) {
        self.partial_content.push_str(token);
        self.add_text_segment(token.to_string());
    }

    /// Append a token to the reasoning content during streaming
    pub fn append_reasoning_token(&mut self, token: &str) {
        self.reasoning_content.push_str(token);
    }

    /// Finalize the message by moving partial_content to content and marking as not streaming
    pub fn finalize(&mut self) {
        if self.is_streaming {
            self.content = std::mem::take(&mut self.partial_content);
            self.is_streaming = false;
            // Collapse reasoning by default when message is finalized
            if !self.reasoning_content.is_empty() {
                self.reasoning_collapsed = true;
            }
        }
    }

    /// Toggle the reasoning collapsed state
    pub fn toggle_reasoning_collapsed(&mut self) {
        self.reasoning_collapsed = !self.reasoning_collapsed;
    }

    /// Count tokens in the reasoning content (approximation using whitespace)
    pub fn reasoning_token_count(&self) -> usize {
        // Simple approximation: split on whitespace and count
        self.reasoning_content.split_whitespace().count()
    }

    /// Add a text segment to the message
    pub fn add_text_segment(&mut self, text: String) {
        // If the last segment is text, append to it instead of creating a new one
        if let Some(MessageSegment::Text(last_text)) = self.segments.last_mut() {
            last_text.push_str(&text);
        } else if !text.is_empty() {
            self.segments.push(MessageSegment::Text(text));
        }
    }

    /// Start a new tool event
    pub fn start_tool_event(&mut self, tool_call_id: String, function_name: String) {
        let event = ToolEvent::new(tool_call_id, function_name);
        self.segments.push(MessageSegment::ToolEvent(event));
    }

    /// Complete a tool event by its tool_call_id
    pub fn complete_tool_event(&mut self, tool_call_id: &str) {
        for segment in &mut self.segments {
            if let MessageSegment::ToolEvent(event) = segment {
                if event.tool_call_id == tool_call_id {
                    event.complete();
                    break;
                }
            }
        }
    }

    /// Fail a tool event by its tool_call_id
    pub fn fail_tool_event(&mut self, tool_call_id: &str) {
        for segment in &mut self.segments {
            if let MessageSegment::ToolEvent(event) = segment {
                if event.tool_call_id == tool_call_id {
                    event.fail();
                    break;
                }
            }
        }
    }

    /// Get a tool event by its tool_call_id
    pub fn get_tool_event(&self, tool_call_id: &str) -> Option<&ToolEvent> {
        for segment in &self.segments {
            if let MessageSegment::ToolEvent(event) = segment {
                if event.tool_call_id == tool_call_id {
                    return Some(event);
                }
            }
        }
        None
    }

    /// Check if there are any running tools
    pub fn has_running_tools(&self) -> bool {
        self.segments.iter().any(|s| {
            matches!(s, MessageSegment::ToolEvent(e) if e.status == ToolEventStatus::Running)
        })
    }

    /// Set the display_name for a tool event by its tool_call_id
    pub fn set_tool_display_name(&mut self, tool_call_id: &str, display_name: String) {
        for segment in &mut self.segments {
            if let MessageSegment::ToolEvent(event) = segment {
                if event.tool_call_id == tool_call_id {
                    event.display_name = Some(display_name);
                    break;
                }
            }
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

    /// Set the thread type for this request (builder pattern)
    pub fn with_type(mut self, thread_type: ThreadType) -> Self {
        self.thread_type = Some(thread_type);
        self
    }

    /// Set plan mode for this request (builder pattern)
    pub fn with_plan_mode(mut self, plan_mode: bool) -> Self {
        self.plan_mode = Some(plan_mode);
        self
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
            description: None,
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
            description: None,
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
            description: None,
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
            description: None,
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
    fn test_thread_deserialization_with_name_field() {
        // Test that "name" field from API is mapped to "title" field in struct
        let json = r#"{
            "id": "thread-api",
            "name": "My Thread Title",
            "thread_type": "normal",
            "project_path": "/home/user/project",
            "provider": "claude-cli"
        }"#;

        let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(thread.id, "thread-api");
        // "name" from API should map to "title" in struct
        assert_eq!(thread.title, "My Thread Title");
        assert_eq!(thread.thread_type, ThreadType::Normal);
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
            "type": "normal"
        }"#;

        let thread: Thread = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(thread.id, "thread-desc");
        assert_eq!(thread.title, "Thread with Description");
        assert_eq!(thread.description, Some("This is a thread description".to_string()));
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
            "type": "normal"
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
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
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

    // ============= ErrorInfo Tests =============

    #[test]
    fn test_error_info_new() {
        let error = ErrorInfo::new("tool_execution_failed".to_string(), "File not found".to_string());

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
        };

        let json = serde_json::to_string(&message).expect("Failed to serialize");
        let deserialized: Message = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(message.reasoning_content, deserialized.reasoning_content);
        assert_eq!(message.reasoning_collapsed, deserialized.reasoning_collapsed);
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
        assert_eq!(deserialized.display_name, Some("cd /path && ls".to_string()));
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
