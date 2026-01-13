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
}

/// Role of a message in a conversation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
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
}

impl StreamRequest {
    /// Create a new StreamRequest for a new thread
    pub fn new(prompt: String) -> Self {
        Self {
            prompt,
            session_id: Uuid::new_v4().to_string(),
            thread_id: None,
            reply_to: None,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_creation() {
        let thread = Thread {
            id: "thread-123".to_string(),
            title: "Test Thread".to_string(),
            preview: "Hello, world!".to_string(),
            updated_at: Utc::now(),
        };

        assert_eq!(thread.id, "thread-123");
        assert_eq!(thread.title, "Test Thread");
        assert_eq!(thread.preview, "Hello, world!");
    }

    #[test]
    fn test_thread_serialization() {
        let thread = Thread {
            id: "thread-456".to_string(),
            title: "Serialization Test".to_string(),
            preview: "Testing JSON".to_string(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&thread).expect("Failed to serialize");
        let deserialized: Thread = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(thread, deserialized);
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
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let deserialized: StreamRequest = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(request, deserialized);
    }
}
