use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a conversation thread from the backend API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Thread {
    /// Unique identifier from backend
    pub id: String,
    /// Title derived from first message
    pub title: String,
    /// Preview of the last message
    pub preview: String,
    /// When the thread was last updated
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
}

/// Request structure for streaming API calls
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamRequest {
    /// The prompt/message to send
    pub prompt: String,
    /// Optional session ID for authentication
    pub session_id: Option<String>,
    /// Thread ID - None means create a new thread
    pub thread_id: Option<String>,
    /// Message ID to reply to - for future stitching support
    pub reply_to: Option<i64>,
}

impl StreamRequest {
    /// Create a new StreamRequest for a new thread
    pub fn new(prompt: String) -> Self {
        Self {
            prompt,
            session_id: None,
            thread_id: None,
            reply_to: None,
        }
    }

    /// Create a StreamRequest for an existing thread
    pub fn with_thread(prompt: String, thread_id: String) -> Self {
        Self {
            prompt,
            session_id: None,
            thread_id: Some(thread_id),
            reply_to: None,
        }
    }

    /// Create a StreamRequest as a reply to a specific message
    pub fn with_reply(prompt: String, thread_id: String, reply_to: i64) -> Self {
        Self {
            prompt,
            session_id: None,
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
        };

        assert_eq!(message.id, 1);
        assert_eq!(message.thread_id, "thread-123");
        assert_eq!(message.role, MessageRole::User);
        assert_eq!(message.content, "Hello!");
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
        };

        let json = serde_json::to_string(&message).expect("Failed to serialize");
        let deserialized: Message = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(message, deserialized);
    }

    #[test]
    fn test_stream_request_new() {
        let request = StreamRequest::new("Hello".to_string());

        assert_eq!(request.prompt, "Hello");
        assert!(request.session_id.is_none());
        assert!(request.thread_id.is_none());
        assert!(request.reply_to.is_none());
    }

    #[test]
    fn test_stream_request_with_thread() {
        let request = StreamRequest::with_thread("Hello".to_string(), "thread-123".to_string());

        assert_eq!(request.prompt, "Hello");
        assert!(request.session_id.is_none());
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
        assert!(request.session_id.is_none());
        assert_eq!(request.thread_id, Some("thread-123".to_string()));
        assert_eq!(request.reply_to, Some(42));
    }

    #[test]
    fn test_stream_request_serialization() {
        let request = StreamRequest {
            prompt: "Test prompt".to_string(),
            session_id: Some("session-abc".to_string()),
            thread_id: Some("thread-xyz".to_string()),
            reply_to: Some(100),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let deserialized: StreamRequest = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(request, deserialized);
    }
}
