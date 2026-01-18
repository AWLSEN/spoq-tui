//! SSE event types and definitions
//!
//! Contains the SseEvent enum with all possible event variants from the
//! Conductor backend streaming API.

use serde::{Deserialize, Serialize};

/// Metadata included with SSE events from the backend.
/// Backend sends these fields flattened at root level of each event.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SseEventMeta {
    /// Sequence number for ordering events (auto-increments per event)
    pub seq: Option<u64>,
    /// Unix timestamp in milliseconds
    pub timestamp: Option<u64>,
    /// Session ID for the current streaming session
    pub session_id: Option<String>,
    /// Thread ID this event belongs to
    pub thread_id: Option<String>,
}

/// Typed SSE events from the Conductor API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    /// Content chunk for streaming text
    Content {
        text: String,
        #[serde(skip)]
        meta: SseEventMeta,
    },
    /// Thread metadata when a new thread is created or identified
    ThreadInfo {
        thread_id: String,
        #[serde(default)]
        title: Option<String>,
    },
    /// Message metadata
    MessageInfo {
        message_id: i64,
    },
    /// Stream completed successfully
    Done,
    /// Error from the backend
    Error {
        message: String,
        #[serde(default)]
        code: Option<String>,
    },
    /// Heartbeat/keepalive
    Ping,
    /// Skills injected event
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
        messages_removed: u32,
        tokens_freed: u32,
        tokens_used: Option<u32>,
        token_limit: Option<u32>,
    },
    /// Tool call started
    ToolCallStart {
        tool_name: String,
        tool_call_id: String,
    },
    /// Tool call argument chunk
    ToolCallArgument {
        tool_call_id: String,
        chunk: String,
    },
    /// Tool executing with display info
    ToolExecuting {
        tool_call_id: String,
        display_name: Option<String>,
        url: Option<String>,
    },
    /// Tool result
    ToolResult {
        tool_call_id: String,
        result: String,
    },
    /// Reasoning/thinking content
    Reasoning {
        text: String,
    },
    /// Permission request
    PermissionRequest {
        permission_id: String,
        tool_name: String,
        description: String,
        tool_call_id: Option<String>,
        tool_input: Option<serde_json::Value>,
    },
    /// Todos updated
    TodosUpdated {
        todos: serde_json::Value,
    },
    /// Subagent started
    SubagentStarted {
        task_id: String,
        description: String,
        subagent_type: String,
    },
    /// Subagent progress update
    SubagentProgress {
        task_id: String,
        message: String,
    },
    /// Subagent completed
    SubagentCompleted {
        task_id: String,
        summary: String,
        tool_call_count: Option<u32>,
    },
    /// Thread updated - when thread metadata is changed
    ThreadUpdated {
        thread_id: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        description: Option<String>,
    },
    /// Usage information - context window usage
    Usage {
        context_window_used: u32,
        context_window_limit: u32,
    },
}

impl SseEvent {
    /// Returns the event type name as a string for debugging purposes.
    pub fn event_type_name(&self) -> &'static str {
        match self {
            SseEvent::Content { .. } => "content",
            SseEvent::ThreadInfo { .. } => "thread_info",
            SseEvent::MessageInfo { .. } => "message_info",
            SseEvent::Done => "done",
            SseEvent::Error { .. } => "error",
            SseEvent::Ping => "ping",
            SseEvent::SkillsInjected { .. } => "skills_injected",
            SseEvent::OAuthConsentRequired { .. } => "oauth_consent_required",
            SseEvent::ContextCompacted { .. } => "context_compacted",
            SseEvent::ToolCallStart { .. } => "tool_call_start",
            SseEvent::ToolCallArgument { .. } => "tool_call_argument",
            SseEvent::ToolExecuting { .. } => "tool_executing",
            SseEvent::ToolResult { .. } => "tool_result",
            SseEvent::Reasoning { .. } => "reasoning",
            SseEvent::PermissionRequest { .. } => "permission_request",
            SseEvent::TodosUpdated { .. } => "todos_updated",
            SseEvent::SubagentStarted { .. } => "subagent_started",
            SseEvent::SubagentProgress { .. } => "subagent_progress",
            SseEvent::SubagentCompleted { .. } => "subagent_completed",
            SseEvent::ThreadUpdated { .. } => "thread_updated",
            SseEvent::Usage { .. } => "usage",
        }
    }
}

/// Represents a parsed SSE line
#[derive(Debug, Clone, PartialEq)]
pub enum SseLine {
    /// Event type declaration (e.g., "event: content")
    Event(String),
    /// Data payload (e.g., "data: {\"text\": \"hello\"}")
    Data(String),
    /// Empty line - signals end of event
    Empty,
    /// Comment line (starts with ':')
    Comment(String),
}

/// Errors that can occur during SSE parsing
#[derive(Debug, Clone, PartialEq)]
pub enum SseParseError {
    /// Unknown event type received
    UnknownEventType(String),
    /// Invalid JSON in data payload
    InvalidJson {
        event_type: String,
        source: String,
    },
    /// Missing data for event
    MissingData {
        event_type: String,
    },
}

impl std::fmt::Display for SseParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SseParseError::UnknownEventType(t) => write!(f, "Unknown SSE event type: {}", t),
            SseParseError::InvalidJson { event_type, source } => {
                write!(f, "Invalid JSON for event '{}': {}", event_type, source)
            }
            SseParseError::MissingData { event_type } => {
                write!(f, "Missing data for event type: {}", event_type)
            }
        }
    }
}

impl std::error::Error for SseParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_event_type_name() {
        assert_eq!(
            SseEvent::Content {
                text: "".to_string(),
                meta: SseEventMeta::default(),
            }
            .event_type_name(),
            "content"
        );
        assert_eq!(SseEvent::Done.event_type_name(), "done");
        assert_eq!(SseEvent::Ping.event_type_name(), "ping");
        assert_eq!(
            SseEvent::ThreadInfo {
                thread_id: "".to_string(),
                title: None,
            }
            .event_type_name(),
            "thread_info"
        );
    }

    #[test]
    fn test_sse_parse_error_display() {
        let err = SseParseError::UnknownEventType("foo".to_string());
        assert_eq!(format!("{}", err), "Unknown SSE event type: foo");

        let err = SseParseError::InvalidJson {
            event_type: "content".to_string(),
            source: "expected value".to_string(),
        };
        assert!(format!("{}", err).contains("Invalid JSON"));

        let err = SseParseError::MissingData {
            event_type: "content".to_string(),
        };
        assert!(format!("{}", err).contains("Missing data"));
    }

    #[test]
    fn test_sse_event_meta_default() {
        let meta = SseEventMeta::default();
        assert!(meta.seq.is_none());
        assert!(meta.timestamp.is_none());
        assert!(meta.session_id.is_none());
        assert!(meta.thread_id.is_none());
    }
}
