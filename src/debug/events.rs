//! Debug event data types.
//!
//! This module contains the data structures for various debug event types
//! that are broadcast via the debug channel.

use serde::{Deserialize, Serialize};

/// Raw SSE event data as received from the conductor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSseEventData {
    /// The raw event type from SSE (e.g., "content", "tool_call_start")
    pub event_type: String,
    /// The raw JSON payload
    pub payload: String,
    /// Sequence number if present
    pub seq: Option<u64>,
}

impl RawSseEventData {
    /// Create a new raw SSE event.
    pub fn new(event_type: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            payload: payload.into(),
            seq: None,
        }
    }

    /// Create a new raw SSE event with sequence number.
    pub fn with_seq(
        event_type: impl Into<String>,
        payload: impl Into<String>,
        seq: u64,
    ) -> Self {
        Self {
            event_type: event_type.into(),
            payload: payload.into(),
            seq: Some(seq),
        }
    }
}

/// Processed event data after conversion to AppMessage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedEventData {
    /// The type of AppMessage that was generated
    pub message_type: String,
    /// Summary of the message content (truncated for large payloads)
    pub summary: String,
    /// Cumulative token count (for token events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u64>,
    /// Tokens per second rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_second: Option<f64>,
    /// Latency in milliseconds since last event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl ProcessedEventData {
    /// Create a new processed event.
    pub fn new(message_type: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            message_type: message_type.into(),
            summary: summary.into(),
            token_count: None,
            tokens_per_second: None,
            latency_ms: None,
        }
    }

    /// Create a new processed event with statistics.
    pub fn with_stats(
        message_type: impl Into<String>,
        summary: impl Into<String>,
        token_count: Option<u64>,
        tokens_per_second: Option<f64>,
        latency_ms: Option<u64>,
    ) -> Self {
        Self {
            message_type: message_type.into(),
            summary: summary.into(),
            token_count,
            tokens_per_second,
            latency_ms,
        }
    }
}

/// State change data capturing updates to cache or app state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeData {
    /// The type of state that changed
    pub state_type: StateType,
    /// Description of the change
    pub description: String,
    /// Previous value (if applicable, may be truncated)
    pub previous: Option<String>,
    /// New value (may be truncated)
    pub current: String,
}

impl StateChangeData {
    /// Create a new state change event.
    pub fn new(
        state_type: StateType,
        description: impl Into<String>,
        current: impl Into<String>,
    ) -> Self {
        Self {
            state_type,
            description: description.into(),
            previous: None,
            current: current.into(),
        }
    }

    /// Create a new state change event with previous value.
    pub fn with_previous(
        state_type: StateType,
        description: impl Into<String>,
        previous: impl Into<String>,
        current: impl Into<String>,
    ) -> Self {
        Self {
            state_type,
            description: description.into(),
            previous: Some(previous.into()),
            current: current.into(),
        }
    }
}

/// Types of state that can change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateType {
    /// Thread cache update
    ThreadCache,
    /// Message cache update
    MessageCache,
    /// Tool tracker update
    ToolTracker,
    /// Session state update
    SessionState,
    /// Subagent tracker update
    SubagentTracker,
    /// Todos update
    Todos,
}

/// Stream lifecycle event data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamLifecycleData {
    /// The lifecycle phase
    pub phase: StreamPhase,
    /// Additional details about the phase
    pub details: Option<String>,
}

impl StreamLifecycleData {
    /// Create a new stream lifecycle event.
    pub fn new(phase: StreamPhase) -> Self {
        Self {
            phase,
            details: None,
        }
    }

    /// Create a new stream lifecycle event with details.
    pub fn with_details(phase: StreamPhase, details: impl Into<String>) -> Self {
        Self {
            phase,
            details: Some(details.into()),
        }
    }
}

/// Phases of a stream's lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamPhase {
    /// Stream connection initiated
    Connecting,
    /// Stream connection established
    Connected,
    /// Stream completed normally
    Completed,
    /// Stream disconnected (may be temporary)
    Disconnected,
    /// Stream permanently closed
    Closed,
}

/// Error data for debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    /// Error code if available
    pub code: Option<String>,
    /// Error message
    pub message: String,
    /// Source of the error
    pub source: ErrorSource,
}

impl ErrorData {
    /// Create a new error event.
    pub fn new(source: ErrorSource, message: impl Into<String>) -> Self {
        Self {
            code: None,
            message: message.into(),
            source,
        }
    }

    /// Create a new error event with error code.
    pub fn with_code(
        source: ErrorSource,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: Some(code.into()),
            message: message.into(),
            source,
        }
    }
}

/// Source of an error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSource {
    /// Error from SSE connection
    SseConnection,
    /// Error parsing SSE event
    SseParsing,
    /// Error from conductor API
    ConductorApi,
    /// Error in app state processing
    AppState,
    /// Error from cache operations
    Cache,
}

/// Snapshot of the current application state for the debug dashboard.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Number of threads in the cache
    pub threads_count: usize,
    /// Number of messages across all threads
    pub messages_count: usize,
    /// Whether the app is currently streaming
    pub is_streaming: bool,
    /// List of currently active tools
    pub active_tools: Vec<ActiveToolInfo>,
    /// Current session ID if any
    pub session_id: Option<String>,
    /// Current thread ID if any
    pub thread_id: Option<String>,
}

/// Information about an active tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveToolInfo {
    /// Tool call ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Whether the tool is currently running
    pub is_running: bool,
}
