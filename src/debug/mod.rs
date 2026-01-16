//! Debug event types and channel for the debug server.
//!
//! This module provides structured debug events that capture all relevant
//! debugging information during SSE stream processing. Events are broadcast
//! via a tokio broadcast channel for consumption by the debug server.

mod events;
mod html;
mod server;

pub use events::*;
pub use server::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    #[test]
    fn test_state_snapshot_default() {
        use crate::debug::events::StateSnapshot;

        let snapshot = StateSnapshot::default();
        assert_eq!(snapshot.threads_count, 0);
        assert_eq!(snapshot.messages_count, 0);
        assert_eq!(snapshot.active_subagents, 0);
        assert!(!snapshot.is_streaming);
        assert!(snapshot.active_tools.is_empty());
        assert_eq!(snapshot.session_id, None);
        assert_eq!(snapshot.thread_id, None);
    }

    #[test]
    fn test_state_snapshot_with_subagents() {
        use crate::debug::events::StateSnapshot;

        let snapshot = StateSnapshot {
            threads_count: 2,
            messages_count: 10,
            is_streaming: true,
            active_tools: vec![],
            active_subagents: 3,
            session_id: Some("session-123".to_string()),
            thread_id: Some("thread-456".to_string()),
        };

        assert_eq!(snapshot.active_subagents, 3);
        assert!(snapshot.is_streaming);
    }

    #[test]
    fn test_state_snapshot_serialization() {
        use crate::debug::events::StateSnapshot;

        let snapshot = StateSnapshot {
            threads_count: 1,
            messages_count: 5,
            is_streaming: false,
            active_tools: vec![],
            active_subagents: 2,
            session_id: Some("test-session".to_string()),
            thread_id: None,
        };

        let json = serde_json::to_string(&snapshot).expect("Failed to serialize");
        assert!(json.contains("\"active_subagents\":2"));
        assert!(json.contains("\"threads_count\":1"));
        assert!(json.contains("\"messages_count\":5"));
    }

    #[test]
    fn test_subagent_tracker_state_change() {
        let data = StateChangeData::new(
            StateType::SubagentTracker,
            "Subagent registered",
            "active: 1, task: task-123",
        );
        let event = DebugEvent::new(DebugEventKind::StateChange(data));

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\"state_type\":\"subagent_tracker\""));
        assert!(json.contains("\"description\":\"Subagent registered\""));
        assert!(json.contains("active: 1"));
    }
}
