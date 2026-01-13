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

/// Typed SSE events from the Conductor API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    /// Content chunk for streaming text
    Content {
        text: String,
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
#[derive(Debug, Clone, Deserialize)]
struct ContentPayload {
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ThreadInfoPayload {
    thread_id: String,
    #[serde(default)]
    title: Option<String>,
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
        "content" => {
            let payload: ContentPayload = serde_json::from_str(data)
                .map_err(|e| SseParseError::InvalidJson {
                    event_type: event_type.to_string(),
                    source: e.to_string(),
                })?;
            Ok(SseEvent::Content { text: payload.text })
        }
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
        "done" => Ok(SseEvent::Done),
        "ping" => Ok(SseEvent::Ping),
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
        _ => Err(SseParseError::UnknownEventType(event_type.to_string())),
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

        let event_type = self.current_event_type.take();
        let data = self.data_buffer.join("\n");
        self.data_buffer.clear();

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
                text: "Hello world".to_string()
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
        let result = parse_sse_event("unknown_type", "{}");
        assert!(matches!(result, Err(SseParseError::UnknownEventType(_))));
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
                text: "Hello".to_string()
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
                text: "First".to_string()
            })
        );

        // Second event
        parser.feed_line("event: content").unwrap();
        parser.feed_line(r#"data: {"text": "Second"}"#).unwrap();
        let event2 = parser.feed_line("").unwrap();
        assert_eq!(
            event2,
            Some(SseEvent::Content {
                text: "Second".to_string()
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
                text: "Hello".to_string()
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
}
