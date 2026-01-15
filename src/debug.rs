//! Debug event types and channel for the debug server.
//!
//! This module provides structured debug events that capture all relevant
//! debugging information during SSE stream processing. Events are broadcast
//! via a tokio broadcast channel for consumption by the debug server.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::broadcast;

/// Type alias for the debug event sender.
pub type DebugEventSender = broadcast::Sender<DebugEvent>;

/// Create a new debug event channel with the specified capacity.
///
/// Returns both the sender and receiver. The sender can be cloned
/// to allow multiple producers, and the receiver can be resubscribed
/// to allow multiple consumers.
pub fn create_debug_channel(capacity: usize) -> (DebugEventSender, broadcast::Receiver<DebugEvent>) {
    broadcast::channel(capacity)
}

/// A debug event capturing internal system state for debugging.
///
/// All events include common metadata (timestamp, thread_id, session_id)
/// along with event-specific data.
#[derive(Debug, Clone, Serialize)]
pub struct DebugEvent {
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// Thread ID associated with this event (if applicable)
    pub thread_id: Option<String>,
    /// Session ID associated with this event (if applicable)
    pub session_id: Option<String>,
    /// The specific event data
    pub event: DebugEventKind,
}

impl DebugEvent {
    /// Create a new debug event with the current timestamp.
    pub fn new(event: DebugEventKind) -> Self {
        Self {
            timestamp: Utc::now(),
            thread_id: None,
            session_id: None,
            event,
        }
    }

    /// Create a new debug event with thread and session context.
    pub fn with_context(
        event: DebugEventKind,
        thread_id: Option<String>,
        session_id: Option<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            thread_id,
            session_id,
            event,
        }
    }
}

/// The specific kind of debug event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DebugEventKind {
    /// Raw SSE event received from conductor
    RawSseEvent(RawSseEventData),
    /// Processed event after conversion to AppMessage
    ProcessedEvent(ProcessedEventData),
    /// State change in the cache or app state
    StateChange(StateChangeData),
    /// Stream lifecycle event (start/end)
    StreamLifecycle(StreamLifecycleData),
    /// Error that occurred during processing
    Error(ErrorData),
}

/// Raw SSE event data as received from the conductor.
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct ProcessedEventData {
    /// The type of AppMessage that was generated
    pub message_type: String,
    /// Summary of the message content (truncated for large payloads)
    pub summary: String,
}

impl ProcessedEventData {
    /// Create a new processed event.
    pub fn new(message_type: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            message_type: message_type.into(),
            summary: summary.into(),
        }
    }
}

/// State change data capturing updates to cache or app state.
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_event_serialization() {
        let event = DebugEvent::new(DebugEventKind::RawSseEvent(RawSseEventData::new(
            "content",
            r#"{"text": "Hello"}"#,
        )));

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"type\":\"raw_sse_event\""));
        assert!(json.contains("\"event_type\":\"content\""));
        assert!(json.contains("\"timestamp\""));
    }

    #[test]
    fn test_debug_event_with_context() {
        let event = DebugEvent::with_context(
            DebugEventKind::StreamLifecycle(StreamLifecycleData::new(StreamPhase::Connected)),
            Some("thread-123".to_string()),
            Some("session-456".to_string()),
        );

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"thread_id\":\"thread-123\""));
        assert!(json.contains("\"session_id\":\"session-456\""));
        assert!(json.contains("\"type\":\"stream_lifecycle\""));
        assert!(json.contains("\"phase\":\"connected\""));
    }

    #[test]
    fn test_raw_sse_event_serialization() {
        let data = RawSseEventData::with_seq("tool_call_start", r#"{"tool_name": "Bash"}"#, 42);
        let event = DebugEvent::new(DebugEventKind::RawSseEvent(data));

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"seq\":42"));
        assert!(json.contains("\"event_type\":\"tool_call_start\""));
    }

    #[test]
    fn test_processed_event_serialization() {
        let data = ProcessedEventData::new("StreamToken", "token: 'Hello'");
        let event = DebugEvent::new(DebugEventKind::ProcessedEvent(data));

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"type\":\"processed_event\""));
        assert!(json.contains("\"message_type\":\"StreamToken\""));
        assert!(json.contains("\"summary\":\"token: 'Hello'\""));
    }

    #[test]
    fn test_state_change_serialization() {
        let data = StateChangeData::with_previous(
            StateType::ThreadCache,
            "Thread title updated",
            "Old Title",
            "New Title",
        );
        let event = DebugEvent::new(DebugEventKind::StateChange(data));

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"type\":\"state_change\""));
        assert!(json.contains("\"state_type\":\"thread_cache\""));
        assert!(json.contains("\"previous\":\"Old Title\""));
        assert!(json.contains("\"current\":\"New Title\""));
    }

    #[test]
    fn test_state_change_without_previous() {
        let data = StateChangeData::new(StateType::Todos, "Todos updated", "3 items");
        let event = DebugEvent::new(DebugEventKind::StateChange(data));

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"state_type\":\"todos\""));
        assert!(json.contains("\"previous\":null"));
    }

    #[test]
    fn test_stream_lifecycle_serialization() {
        let phases = vec![
            StreamPhase::Connecting,
            StreamPhase::Connected,
            StreamPhase::Completed,
            StreamPhase::Disconnected,
            StreamPhase::Closed,
        ];

        for phase in phases {
            let data = StreamLifecycleData::new(phase.clone());
            let event = DebugEvent::new(DebugEventKind::StreamLifecycle(data));
            let json = serde_json::to_string(&event).expect("Failed to serialize");
            assert!(json.contains("\"type\":\"stream_lifecycle\""));
        }
    }

    #[test]
    fn test_stream_lifecycle_with_details() {
        let data = StreamLifecycleData::with_details(
            StreamPhase::Disconnected,
            "Connection timeout after 30s",
        );
        let event = DebugEvent::new(DebugEventKind::StreamLifecycle(data));

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"details\":\"Connection timeout after 30s\""));
    }

    #[test]
    fn test_error_serialization() {
        let data = ErrorData::with_code(
            ErrorSource::SseParsing,
            "PARSE_ERROR",
            "Failed to parse JSON",
        );
        let event = DebugEvent::new(DebugEventKind::Error(data));

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"source\":\"sse_parsing\""));
        assert!(json.contains("\"code\":\"PARSE_ERROR\""));
        assert!(json.contains("\"message\":\"Failed to parse JSON\""));
    }

    #[test]
    fn test_error_without_code() {
        let data = ErrorData::new(ErrorSource::ConductorApi, "Connection refused");
        let event = DebugEvent::new(DebugEventKind::Error(data));

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"source\":\"conductor_api\""));
        assert!(json.contains("\"code\":null"));
    }

    #[test]
    fn test_error_sources_serialization() {
        let sources = vec![
            ErrorSource::SseConnection,
            ErrorSource::SseParsing,
            ErrorSource::ConductorApi,
            ErrorSource::AppState,
            ErrorSource::Cache,
        ];

        for source in sources {
            let data = ErrorData::new(source, "test error");
            let event = DebugEvent::new(DebugEventKind::Error(data));
            let json = serde_json::to_string(&event).expect("Failed to serialize");
            assert!(json.contains("\"source\":"));
        }
    }

    #[test]
    fn test_state_types_serialization() {
        let types = vec![
            StateType::ThreadCache,
            StateType::MessageCache,
            StateType::ToolTracker,
            StateType::SessionState,
            StateType::SubagentTracker,
            StateType::Todos,
        ];

        for state_type in types {
            let data = StateChangeData::new(state_type, "test", "value");
            let event = DebugEvent::new(DebugEventKind::StateChange(data));
            let json = serde_json::to_string(&event).expect("Failed to serialize");
            assert!(json.contains("\"state_type\":"));
        }
    }

    #[test]
    fn test_debug_event_clone() {
        let event = DebugEvent::with_context(
            DebugEventKind::RawSseEvent(RawSseEventData::new("content", "{}")),
            Some("thread-1".to_string()),
            Some("session-1".to_string()),
        );

        let cloned = event.clone();
        assert_eq!(event.thread_id, cloned.thread_id);
        assert_eq!(event.session_id, cloned.session_id);
    }

    #[test]
    fn test_debug_event_debug_trait() {
        let event = DebugEvent::new(DebugEventKind::StreamLifecycle(StreamLifecycleData::new(
            StreamPhase::Connecting,
        )));

        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("DebugEvent"));
        assert!(debug_str.contains("StreamLifecycle"));
    }

    #[test]
    fn test_create_debug_channel() {
        let (tx, mut rx) = create_debug_channel(16);

        // Send an event
        let event = DebugEvent::new(DebugEventKind::StreamLifecycle(StreamLifecycleData::new(
            StreamPhase::Connected,
        )));
        tx.send(event.clone()).expect("Failed to send");

        // Receive and verify
        let received = rx.try_recv().expect("Failed to receive");
        assert!(matches!(
            received.event,
            DebugEventKind::StreamLifecycle(_)
        ));
    }

    #[test]
    fn test_channel_multiple_subscribers() {
        let (tx, _rx1) = create_debug_channel(16);
        let mut rx2 = tx.subscribe();

        let event = DebugEvent::new(DebugEventKind::Error(ErrorData::new(
            ErrorSource::AppState,
            "test",
        )));
        tx.send(event).expect("Failed to send");

        // Second subscriber should receive the event
        let received = rx2.try_recv().expect("Failed to receive");
        assert!(matches!(received.event, DebugEventKind::Error(_)));
    }
}
