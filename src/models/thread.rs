use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{deserialize_id, deserialize_nullable_string, deserialize_thread_type, ServerMessage};

/// Type of thread - determines UI behavior and available features
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThreadType {
    /// Standard conversation thread (default)
    /// Accepts "normal" from server for backward compatibility
    #[default]
    #[serde(alias = "normal")]
    Conversation,
    /// Programming-focused thread with code-specific features
    Programming,
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
    #[serde(
        default,
        deserialize_with = "deserialize_nullable_string",
        alias = "name"
    )]
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
    /// Working directory for this thread (programming threads)
    #[serde(default)]
    pub working_directory: Option<String>,
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
