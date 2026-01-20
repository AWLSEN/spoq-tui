//! SSE stream parsing logic
//!
//! Contains the stateful SseParser for accumulating lines and emitting events,
//! as well as the core parsing functions.

mod content;
mod misc;
mod permissions;
mod subagent;
mod thread;
mod tools;

use crate::sse::events::{SseEvent, SseLine, SseParseError};

// Import parser functions from submodules
use content::{parse_content_event, parse_reasoning_event};
use misc::{
    parse_context_compacted_event, parse_error_event, parse_oauth_consent_event,
    parse_skills_injected_event, parse_todos_updated_event, parse_usage_event,
};
use permissions::parse_permission_request_event;
use subagent::{
    parse_subagent_completed_event, parse_subagent_progress_event, parse_subagent_started_event,
};
use thread::{
    parse_done_event, parse_message_info_event, parse_thread_info_event,
    parse_thread_updated_event, parse_user_message_saved_event,
};
use tools::{
    parse_tool_call_argument_event, parse_tool_call_start_event, parse_tool_executing_event,
    parse_tool_result_event,
};

/// Parse a single SSE line into its component type
pub fn parse_sse_line(line: &str) -> SseLine {
    if line.is_empty() {
        return SseLine::Empty;
    }

    if let Some(stripped) = line.strip_prefix(':') {
        return SseLine::Comment(stripped.trim().to_string());
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
            parse_content_event(event_type, data)
        }
        // thread_info - legacy format with thread_id field
        "thread_info" => parse_thread_info_event(event_type, data),
        // Conductor sends user_message_saved with message_id and optional thread_id
        "user_message_saved" => parse_user_message_saved_event(event_type, data),
        "message_info" => parse_message_info_event(event_type, data),
        // Conductor sends done with message_id in JSON
        "done" => parse_done_event(data),
        "ping" => Ok(SseEvent::Ping),
        "skills_injected" => parse_skills_injected_event(event_type, data),
        "oauth_consent_required" => parse_oauth_consent_event(event_type, data),
        "context_compacted" => parse_context_compacted_event(event_type, data),
        "error" => parse_error_event(event_type, data),
        "tool_call_start" => parse_tool_call_start_event(event_type, data),
        "tool_call_argument" => parse_tool_call_argument_event(event_type, data),
        "tool_executing" => parse_tool_executing_event(event_type, data),
        "tool_result" => parse_tool_result_event(event_type, data),
        "reasoning" | "thinking" => parse_reasoning_event(event_type, data),
        "permission_request" => parse_permission_request_event(event_type, data),
        "todos_updated" => parse_todos_updated_event(event_type, data),
        "subagent_started" => parse_subagent_started_event(event_type, data),
        "subagent_progress" => parse_subagent_progress_event(event_type, data),
        "subagent_completed" => parse_subagent_completed_event(event_type, data),
        "thread_updated" => parse_thread_updated_event(event_type, data),
        "usage" => parse_usage_event(event_type, data),
        // Ignore unknown events instead of erroring (more resilient)
        _ => Ok(SseEvent::Ping),
    }
}

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
    use crate::sse::events::SseEventMeta;

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
    fn test_parse_ping_event() {
        let result = parse_sse_event("ping", "{}");
        assert_eq!(result.unwrap(), SseEvent::Ping);
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
        parser
            .feed_line(r#"data: {"type":"content","data":"test"}"#)
            .unwrap();
        let event = parser.feed_line("").unwrap();
        assert!(matches!(event, Some(SseEvent::Content { .. })));
    }
}
