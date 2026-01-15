//! AppMessage enum for async communication within the application.

use crate::state::Todo;

/// Messages received from async operations (streaming, connection status)
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// A token received during streaming
    StreamToken { thread_id: String, token: String },
    /// A reasoning/thinking token received during streaming
    ReasoningToken { thread_id: String, token: String },
    /// Streaming completed successfully
    StreamComplete { thread_id: String, message_id: i64 },
    /// An error occurred during streaming
    StreamError { thread_id: String, error: String },
    /// Connection status changed
    ConnectionStatus(bool),
    /// Thread created on backend - reconcile pending ID with real ID
    ThreadCreated {
        pending_id: String,
        real_id: String,
        title: Option<String>,
    },
    /// Messages loaded for a thread
    MessagesLoaded {
        thread_id: String,
        messages: Vec<crate::models::Message>,
    },
    /// Error loading messages for a thread
    MessagesLoadError {
        thread_id: String,
        error: String,
    },
    /// Todos updated from the assistant
    TodosUpdated {
        todos: Vec<Todo>,
    },
    /// Permission request from the assistant - needs user approval
    PermissionRequested {
        permission_id: String,
        tool_name: String,
        description: String,
        tool_input: Option<serde_json::Value>,
    },
    /// Tool call started
    ToolStarted {
        tool_call_id: String,
        tool_name: String,
    },
    /// Tool is executing with display info
    ToolExecuting {
        tool_call_id: String,
        display_name: String,
    },
    /// Tool completed with result
    ToolCompleted {
        tool_call_id: String,
        success: bool,
        summary: String,
        /// Full result content for storage in ToolEvent
        result: String,
    },
    /// Tool argument chunk received
    ToolArgumentChunk {
        tool_call_id: String,
        chunk: String,
    },
    /// Skills injected into the session
    SkillsInjected {
        skills: Vec<String>,
    },
    /// OAuth consent required
    OAuthConsentRequired {
        provider: String,
        url: Option<String>,
        skill_name: Option<String>,
    },
    /// Context compacted
    ContextCompacted {
        tokens_used: Option<u32>,
        token_limit: Option<u32>,
    },
    /// Thread metadata updated
    ThreadMetadataUpdated {
        thread_id: String,
        title: Option<String>,
        description: Option<String>,
    },
}
