use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::thread::ThreadType;

/// Permission mode for Claude tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// User approval required for each tool call
    #[default]
    Default,
    /// Claude proposes changes but doesn't execute
    Plan,
    /// Auto-approve all tool calls
    BypassPermissions,
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
    /// Permission mode for tool execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<PermissionMode>,
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
            permission_mode: None,
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
            permission_mode: None,
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
            permission_mode: None,
        }
    }

    /// Set the thread type for this request (builder pattern)
    pub fn with_type(mut self, thread_type: ThreadType) -> Self {
        self.thread_type = Some(thread_type);
        self
    }

    /// Set permission mode for this request (builder pattern)
    pub fn with_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = Some(mode);
        self
    }
}
