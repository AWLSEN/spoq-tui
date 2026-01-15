//! SSE (Server-Sent Events) stream parser
//!
//! Parses SSE format from the Conductor backend streaming API.
//! SSE format consists of:
//! - `event: <type>` - event type line
//! - `data: <json>` - data payload line
//! - Empty line - signals end of event
//! - Lines starting with `:` - comments (ignored)

use serde::{Deserialize, Serialize};

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
}

/// Raw data payload from SSE data lines
/// Supports multiple field names that backends might use for content
/// Also captures flattened metadata fields from the backend
#[derive(Debug, Clone, Deserialize)]
struct ContentPayload {
    /// The text content - accepts "text", "content", "data", "chunk", or "token" fields
    /// Conductor uses "data" for content chunks
    #[serde(alias = "content", alias = "data", alias = "chunk", alias = "token")]
    text: Option<String>,
    /// Some backends nest content in a delta object (OpenAI style)
    #[serde(default)]
    delta: Option<DeltaPayload>,
    /// Sequence number for ordering events
    #[serde(default)]
    seq: Option<u64>,
    /// Unix timestamp in milliseconds
    #[serde(default)]
    timestamp: Option<u64>,
    /// Session ID for the current streaming session
    #[serde(default)]
    session_id: Option<String>,
    /// Thread ID this event belongs to
    #[serde(default)]
    thread_id: Option<String>,
}

/// Nested delta payload for OpenAI-style responses
#[derive(Debug, Clone, Deserialize, Default)]
struct DeltaPayload {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    data: Option<String>,
}

/// Legacy thread_info payload
#[derive(Debug, Clone, Deserialize)]
struct ThreadInfoPayload {
    thread_id: String,
    #[serde(default)]
    title: Option<String>,
}

/// Conductor's done payload
#[derive(Debug, Clone, Deserialize)]
struct DonePayload {
    message_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MessageInfoPayload {
    message_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ErrorPayload {
    message: String,
    #[serde(default)]
    code: Option<String>,
}

/// Parse a single SSE line into its component type
pub fn parse_sse_line(line: &str) -> SseLine {
    if line.is_empty() {
        return SseLine::Empty;
    }

    if line.starts_with(':') {
        return SseLine::Comment(line[1..].trim().to_string());
    }

    if let Some(rest) = line.strip_prefix("event:") {
        return SseLine::Event(rest.trim().to_string());
    }

    if let Some(rest) = line.strip_prefix("data:") {
        return SseLine::Data(rest.trim().to_string());
    }

    // Unknown line format - treat as comment
    SseLine::Comment(line.to_string())
}

/// Parse SSE event type and data into a typed SseEvent
pub fn parse_sse_event(event_type: &str, data: &str) -> Result<SseEvent, SseParseError> {
    match event_type {
        // Support various event type names for content
        "content" | "text" | "message" | "chunk" | "delta" | "content_block_delta" => {
            let payload: ContentPayload = serde_json::from_str(data)
                .map_err(|e| SseParseError::InvalidJson {
                    event_type: event_type.to_string(),
                    source: e.to_string(),
                })?;

            // Extract text from various possible locations in the payload
            let text = payload.text
                .or_else(|| payload.delta.as_ref().and_then(|d| d.content.clone()))
                .or_else(|| payload.delta.as_ref().and_then(|d| d.text.clone()))
                .or_else(|| payload.delta.as_ref().and_then(|d| d.data.clone()))
                .unwrap_or_default();

            // Extract metadata from flattened fields
            let meta = SseEventMeta {
                seq: payload.seq,
                timestamp: payload.timestamp,
                session_id: payload.session_id,
                thread_id: payload.thread_id,
            };

            Ok(SseEvent::Content { text, meta })
        }
        // thread_info - legacy format with thread_id field
        "thread_info" => {
            let payload: ThreadInfoPayload = serde_json::from_str(data)
                .map_err(|e| SseParseError::InvalidJson {
                    event_type: event_type.to_string(),
                    source: e.to_string(),
                })?;
            Ok(SseEvent::ThreadInfo {
                thread_id: payload.thread_id,
                title: payload.title,
            })
        }
        // Conductor sends user_message_saved with message_id and optional thread_id
        // Parse to Value first to handle potential duplicate fields (serde rejects duplicates)
        "user_message_saved" => {
            let v: serde_json::Value = serde_json::from_str(data)
                .map_err(|e| SseParseError::InvalidJson {
                    event_type: event_type.to_string(),
                    source: e.to_string(),
                })?;

            let message_id = v.get("message_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let thread_id = v.get("thread_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| message_id.to_string());

            Ok(SseEvent::ThreadInfo {
                thread_id,
                title: None,
            })
        }
        "message_info" => {
            let payload: MessageInfoPayload = serde_json::from_str(data)
                .map_err(|e| SseParseError::InvalidJson {
                    event_type: event_type.to_string(),
                    source: e.to_string(),
                })?;
            Ok(SseEvent::MessageInfo {
                message_id: payload.message_id,
            })
        }
        // Conductor sends done with message_id in JSON
        "done" => {
            // Try to parse message_id from JSON, fall back to Done without data
            if let Ok(payload) = serde_json::from_str::<DonePayload>(data) {
                Ok(SseEvent::MessageInfo {
                    message_id: payload.message_id.parse().unwrap_or(0),
                })
            } else {
                Ok(SseEvent::Done)
            }
        }
        "ping" => Ok(SseEvent::Ping),
        // Conductor sends skills_injected - ignore it
        "skills_injected" => Ok(SseEvent::Ping), // Treat as no-op
        "error" => {
            let payload: ErrorPayload = serde_json::from_str(data)
                .map_err(|e| SseParseError::InvalidJson {
                    event_type: event_type.to_string(),
                    source: e.to_string(),
                })?;
            Ok(SseEvent::Error {
                message: payload.message,
                code: payload.code,
            })
        }
        // Ignore unknown events instead of erroring (more resilient)
        _ => Ok(SseEvent::Ping)
    }
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

/// Stateful SSE parser that accumulates lines and emits complete events
#[derive(Debug, Default)]
pub struct SseParser {
    /// Current event type being accumulated
    current_event_type: Option<String>,
    /// Accumulated data lines (SSE allows multiple data: lines)
    data_buffer: Vec<String>,
}

impl SseParser {
    /// Create a new SSE parser
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a line to the parser, potentially returning a complete event
    ///
    /// Returns:
    /// - `Ok(Some(event))` - A complete event was parsed
    /// - `Ok(None)` - Line was consumed but event is incomplete
    /// - `Err(error)` - Parse error occurred
    pub fn feed_line(&mut self, line: &str) -> Result<Option<SseEvent>, SseParseError> {
        let parsed = parse_sse_line(line);

        match parsed {
            SseLine::Event(event_type) => {
                self.current_event_type = Some(event_type);
                Ok(None)
            }
            SseLine::Data(data) => {
                self.data_buffer.push(data);
                Ok(None)
            }
            SseLine::Empty => {
                // Empty line signals end of event - try to emit
                self.try_emit_event()
            }
            SseLine::Comment(_) => {
                // Comments are ignored
                Ok(None)
            }
        }
    }

    /// Try to emit a complete event from accumulated state
    fn try_emit_event(&mut self) -> Result<Option<SseEvent>, SseParseError> {
        // If we have no event type or data, nothing to emit
        if self.current_event_type.is_none() && self.data_buffer.is_empty() {
            return Ok(None);
        }

        let mut event_type = self.current_event_type.take();
        let data = self.data_buffer.join("\n");
        self.data_buffer.clear();

        // If no explicit event type, try to extract from JSON "type" field
        // Conductor sends: data: {"type":"content","data":"hello",...}
        if event_type.is_none() && !data.is_empty() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(t) = json.get("type").and_then(|v| v.as_str()) {
                    event_type = Some(t.to_string());
                }
            }
        }

        match event_type {
            Some(et) => {
                // Events like 'done' and 'ping' may not have data
                if data.is_empty() && (et == "done" || et == "ping") {
                    parse_sse_event(&et, "{}").map(Some)
                } else if data.is_empty() {
                    Err(SseParseError::MissingData { event_type: et })
                } else {
                    parse_sse_event(&et, &data).map(Some)
                }
            }
            None => {
                // Data without event type - treat as content by default
                if !data.is_empty() {
                    parse_sse_event("content", &data).map(Some)
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Reset the parser state
    pub fn reset(&mut self) {
        self.current_event_type = None;
        self.data_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for parse_sse_line

    #[test]
    fn test_parse_empty_line() {
        assert_eq!(parse_sse_line(""), SseLine::Empty);
    }

    #[test]
    fn test_parse_comment_line() {
        assert_eq!(
            parse_sse_line(": this is a comment"),
            SseLine::Comment("this is a comment".to_string())
        );
        assert_eq!(
            parse_sse_line(":no space"),
            SseLine::Comment("no space".to_string())
        );
    }

    #[test]
    fn test_parse_event_line() {
        assert_eq!(
            parse_sse_line("event: content"),
            SseLine::Event("content".to_string())
        );
        assert_eq!(
            parse_sse_line("event:content"),
            SseLine::Event("content".to_string())
        );
        assert_eq!(
            parse_sse_line("event:   thread_info  "),
            SseLine::Event("thread_info".to_string())
        );
    }

    #[test]
    fn test_parse_data_line() {
        assert_eq!(
            parse_sse_line("data: {\"text\": \"hello\"}"),
            SseLine::Data("{\"text\": \"hello\"}".to_string())
        );
        assert_eq!(
            parse_sse_line("data:{\"x\":1}"),
            SseLine::Data("{\"x\":1}".to_string())
        );
    }

    #[test]
    fn test_parse_unknown_line() {
        // Unknown lines are treated as comments
        assert_eq!(
            parse_sse_line("unknown: something"),
            SseLine::Comment("unknown: something".to_string())
        );
    }

    // Tests for parse_sse_event

    #[test]
    fn test_parse_content_event() {
        let result = parse_sse_event("content", r#"{"text": "Hello world"}"#);
        assert_eq!(
            result.unwrap(),
            SseEvent::Content {
                text: "Hello world".to_string(),
                meta: SseEventMeta::default(),
            }
        );
    }

    #[test]
    fn test_parse_content_event_with_content_field() {
        // Some backends use "content" instead of "text"
        let result = parse_sse_event("content", r#"{"content": "From content field"}"#);
        assert_eq!(
            result.unwrap(),
            SseEvent::Content {
                text: "From content field".to_string(),
                meta: SseEventMeta::default(),
            }
        );
    }

    #[test]
    fn test_parse_content_event_with_delta_field() {
        // OpenAI-style nested delta.content
        let result = parse_sse_event("content", r#"{"delta": {"content": "From delta"}}"#);
        assert_eq!(
            result.unwrap(),
            SseEvent::Content {
                text: "From delta".to_string(),
                meta: SseEventMeta::default(),
            }
        );
    }

    #[test]
    fn test_parse_content_event_with_extra_fields() {
        // Backend may send extra fields we don't care about
        let result = parse_sse_event(
            "content",
            r#"{"text": "Hello", "id": 123, "model": "claude", "extra": "ignored"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::Content {
                text: "Hello".to_string(),
                meta: SseEventMeta::default(),
            }
        );
    }

    #[test]
    fn test_parse_content_event_empty_when_no_text() {
        // If no text field found, should return empty string (not error)
        let result = parse_sse_event("content", r#"{"other_field": "value"}"#);
        assert_eq!(
            result.unwrap(),
            SseEvent::Content {
                text: "".to_string(),
                meta: SseEventMeta::default(),
            }
        );
    }

    #[test]
    fn test_parse_thread_info_event() {
        let result = parse_sse_event(
            "thread_info",
            r#"{"thread_id": "abc-123", "title": "My Thread"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ThreadInfo {
                thread_id: "abc-123".to_string(),
                title: Some("My Thread".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_thread_info_without_title() {
        let result = parse_sse_event("thread_info", r#"{"thread_id": "abc-123"}"#);
        assert_eq!(
            result.unwrap(),
            SseEvent::ThreadInfo {
                thread_id: "abc-123".to_string(),
                title: None,
            }
        );
    }

    #[test]
    fn test_parse_message_info_event() {
        let result = parse_sse_event("message_info", r#"{"message_id": 42}"#);
        assert_eq!(result.unwrap(), SseEvent::MessageInfo { message_id: 42 });
    }

    #[test]
    fn test_parse_done_event() {
        let result = parse_sse_event("done", "{}");
        assert_eq!(result.unwrap(), SseEvent::Done);
    }

    #[test]
    fn test_parse_ping_event() {
        let result = parse_sse_event("ping", "{}");
        assert_eq!(result.unwrap(), SseEvent::Ping);
    }

    #[test]
    fn test_parse_error_event() {
        let result = parse_sse_event(
            "error",
            r#"{"message": "Something went wrong", "code": "ERR_500"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::Error {
                message: "Something went wrong".to_string(),
                code: Some("ERR_500".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_error_event_without_code() {
        let result = parse_sse_event("error", r#"{"message": "Oops"}"#);
        assert_eq!(
            result.unwrap(),
            SseEvent::Error {
                message: "Oops".to_string(),
                code: None,
            }
        );
    }

    #[test]
    fn test_parse_unknown_event_type() {
        // Unknown events are now ignored (return Ping) for resilience
        let result = parse_sse_event("unknown_type", "{}");
        assert!(matches!(result, Ok(SseEvent::Ping)));
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_sse_event("content", "not json");
        assert!(matches!(result, Err(SseParseError::InvalidJson { .. })));
    }

    // Tests for SseParser

    #[test]
    fn test_parser_simple_event() {
        let mut parser = SseParser::new();

        assert!(parser.feed_line("event: content").unwrap().is_none());
        assert!(parser
            .feed_line(r#"data: {"text": "Hello"}"#)
            .unwrap()
            .is_none());

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::Content {
                text: "Hello".to_string(),
                meta: SseEventMeta::default(),
            })
        );
    }

    #[test]
    fn test_parser_multiple_events() {
        let mut parser = SseParser::new();

        // First event
        parser.feed_line("event: content").unwrap();
        parser.feed_line(r#"data: {"text": "First"}"#).unwrap();
        let event1 = parser.feed_line("").unwrap();
        assert_eq!(
            event1,
            Some(SseEvent::Content {
                text: "First".to_string(),
                meta: SseEventMeta::default(),
            })
        );

        // Second event
        parser.feed_line("event: content").unwrap();
        parser.feed_line(r#"data: {"text": "Second"}"#).unwrap();
        let event2 = parser.feed_line("").unwrap();
        assert_eq!(
            event2,
            Some(SseEvent::Content {
                text: "Second".to_string(),
                meta: SseEventMeta::default(),
            })
        );
    }

    #[test]
    fn test_parser_done_event_no_data() {
        let mut parser = SseParser::new();

        parser.feed_line("event: done").unwrap();
        let event = parser.feed_line("").unwrap();
        assert_eq!(event, Some(SseEvent::Done));
    }

    #[test]
    fn test_parser_ping_event_no_data() {
        let mut parser = SseParser::new();

        parser.feed_line("event: ping").unwrap();
        let event = parser.feed_line("").unwrap();
        assert_eq!(event, Some(SseEvent::Ping));
    }

    #[test]
    fn test_parser_ignores_comments() {
        let mut parser = SseParser::new();

        parser.feed_line(": keepalive").unwrap();
        parser.feed_line("event: content").unwrap();
        parser.feed_line(": another comment").unwrap();
        parser.feed_line(r#"data: {"text": "Hello"}"#).unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::Content {
                text: "Hello".to_string(),
                meta: SseEventMeta::default(),
            })
        );
    }

    #[test]
    fn test_parser_multiple_data_lines() {
        let mut parser = SseParser::new();

        parser.feed_line("event: content").unwrap();
        // Multi-line data gets concatenated with newlines
        // This forms: {"text": "line1\nline2"}
        parser.feed_line(r#"data: {"text": "line1"#).unwrap();
        parser.feed_line(r#"data: line2"}"#).unwrap();

        // Concatenated with newline: {"text": "line1\nline2"}
        // This is actually invalid JSON (unescaped newline in string)
        let result = parser.feed_line("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parser_reset() {
        let mut parser = SseParser::new();

        parser.feed_line("event: content").unwrap();
        parser.feed_line(r#"data: {"text": "Hello"}"#).unwrap();

        parser.reset();

        // After reset, empty line should not emit anything
        let event = parser.feed_line("").unwrap();
        assert!(event.is_none());
    }

    #[test]
    fn test_parser_missing_data_error() {
        let mut parser = SseParser::new();

        parser.feed_line("event: content").unwrap();
        // No data line, just empty line

        let result = parser.feed_line("");
        assert!(matches!(result, Err(SseParseError::MissingData { .. })));
    }

    #[test]
    fn test_parser_thread_info_event() {
        let mut parser = SseParser::new();

        parser.feed_line("event: thread_info").unwrap();
        parser
            .feed_line(r#"data: {"thread_id": "t-123", "title": "Test"}"#)
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::ThreadInfo {
                thread_id: "t-123".to_string(),
                title: Some("Test".to_string()),
            })
        );
    }

    #[test]
    fn test_parser_error_event() {
        let mut parser = SseParser::new();

        parser.feed_line("event: error").unwrap();
        parser
            .feed_line(r#"data: {"message": "Rate limited", "code": "429"}"#)
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::Error {
                message: "Rate limited".to_string(),
                code: Some("429".to_string()),
            })
        );
    }

    #[test]
    fn test_parse_user_message_saved_with_duplicate_fields() {
        // Backend may send JSON with duplicate field names
        // serde_json::from_str to struct would reject this, but parsing to Value handles it
        // Note: serde_json::Value uses the last occurrence of duplicate keys
        let data_with_duplicate = r#"{"message_id": 42, "thread_id": "thread-123", "thread_id": "thread-456"}"#;

        let result = parse_sse_event("user_message_saved", data_with_duplicate);

        // Should successfully parse (serde_json::Value uses last occurrence)
        assert!(result.is_ok());
        let event = result.unwrap();

        // Should be ThreadInfo with thread_id extracted (last occurrence)
        match event {
            SseEvent::ThreadInfo { thread_id, title } => {
                assert_eq!(thread_id, "thread-456");
                assert_eq!(title, None);
            }
            _ => panic!("Expected ThreadInfo event"),
        }
    }

    #[test]
    fn test_parse_user_message_saved_without_thread_id() {
        // When thread_id is not present, should fall back to message_id
        let data = r#"{"message_id": 789}"#;

        let result = parse_sse_event("user_message_saved", data);
        assert!(result.is_ok());

        let event = result.unwrap();
        match event {
            SseEvent::ThreadInfo { thread_id, title } => {
                assert_eq!(thread_id, "789");
                assert_eq!(title, None);
            }
            _ => panic!("Expected ThreadInfo event"),
        }
    }

    #[test]
    fn test_parse_user_message_saved_with_thread_id() {
        // Normal case with thread_id present
        let data = r#"{"message_id": 123, "thread_id": "my-thread"}"#;

        let result = parse_sse_event("user_message_saved", data);
        assert!(result.is_ok());

        let event = result.unwrap();
        match event {
            SseEvent::ThreadInfo { thread_id, title } => {
                assert_eq!(thread_id, "my-thread");
                assert_eq!(title, None);
            }
            _ => panic!("Expected ThreadInfo event"),
        }
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

    // Integration test simulating real SSE stream
    #[test]
    fn test_parser_realistic_stream() {
        let mut parser = SseParser::new();
        let mut events = Vec::new();

        // Simulate a realistic SSE stream from conductor
        let stream_lines = [
            ": connected",
            "",
            "event: thread_info",
            r#"data: {"thread_id": "thread-abc-123"}"#,
            "",
            "event: message_info",
            r#"data: {"message_id": 1}"#,
            "",
            "event: content",
            r#"data: {"text": "Hello, "}"#,
            "",
            "event: content",
            r#"data: {"text": "world!"}"#,
            "",
            "event: done",
            "",
        ];

        for line in stream_lines {
            if let Ok(Some(event)) = parser.feed_line(line) {
                events.push(event);
            }
        }

        assert_eq!(events.len(), 5);
        assert!(matches!(events[0], SseEvent::ThreadInfo { .. }));
        assert!(matches!(events[1], SseEvent::MessageInfo { .. }));
        assert!(matches!(events[2], SseEvent::Content { .. }));
        assert!(matches!(events[3], SseEvent::Content { .. }));
        assert_eq!(events[4], SseEvent::Done);
    }

    #[test]
    fn test_parse_backend_flattened_format() {
        // Test the exact format the backend sends:
        // data: {"type":"content","seq":5,"timestamp":1736956800000,"session_id":"abc123","thread_id":"thread_456","data":"Hello"}\n\n
        // Note: Backend uses "data" field for content, not "text"
        let json = r#"{"type":"content","seq":5,"timestamp":1736956800000,"session_id":"abc123","thread_id":"thread_456","data":"Hello"}"#;

        let result = parse_sse_event("content", json);
        let event = result.unwrap();

        match event {
            SseEvent::Content { text, meta } => {
                assert_eq!(text, "Hello");
                assert_eq!(meta.seq, Some(5));
                assert_eq!(meta.timestamp, Some(1736956800000));
                assert_eq!(meta.session_id, Some("abc123".to_string()));
                assert_eq!(meta.thread_id, Some("thread_456".to_string()));
            }
            _ => panic!("Expected Content event"),
        }
    }

    #[test]
    fn test_parse_backend_stream_with_metadata() {
        // Simulate a realistic backend stream with flattened metadata
        let mut parser = SseParser::new();

        // Backend sends: data: {json}\n\n (no event: line, type is in JSON)
        parser.feed_line(r#"data: {"type":"content","seq":1,"timestamp":1736956800000,"session_id":"sess-abc","thread_id":"thread-123","data":"Hello "}"#).unwrap();
        let event1 = parser.feed_line("").unwrap().unwrap();

        parser.feed_line(r#"data: {"type":"content","seq":2,"timestamp":1736956800001,"session_id":"sess-abc","thread_id":"thread-123","data":"world!"}"#).unwrap();
        let event2 = parser.feed_line("").unwrap().unwrap();

        // Verify first chunk
        match event1 {
            SseEvent::Content { text, meta } => {
                assert_eq!(text, "Hello ");
                assert_eq!(meta.seq, Some(1));
            }
            _ => panic!("Expected Content event"),
        }

        // Verify second chunk with incremented seq
        match event2 {
            SseEvent::Content { text, meta } => {
                assert_eq!(text, "world!");
                assert_eq!(meta.seq, Some(2));
            }
            _ => panic!("Expected Content event"),
        }
    }

    #[test]
    fn test_keepalive_comment_handling() {
        // Backend sends `: comment` lines every 15 seconds as keep-alive
        let mut parser = SseParser::new();

        // Keep-alive should be ignored
        assert!(parser.feed_line(": keep-alive").unwrap().is_none());
        assert!(parser.feed_line(":").unwrap().is_none());

        // But content should still parse
        parser.feed_line(r#"data: {"type":"content","data":"test"}"#).unwrap();
        let event = parser.feed_line("").unwrap();
        assert!(matches!(event, Some(SseEvent::Content { .. })));
    }
}
