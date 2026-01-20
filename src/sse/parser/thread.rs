//! Thread-related event parsers

use crate::sse::events::{SseEvent, SseParseError};
use crate::sse::payloads::{
    DonePayload, MessageInfoPayload, ThreadInfoPayload, ThreadUpdatedPayload,
};

/// Parse thread_info event (legacy format with thread_id field)
pub(super) fn parse_thread_info_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let payload: ThreadInfoPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::ThreadInfo {
        thread_id: payload.thread_id,
        title: payload.title,
    })
}

/// Parse user_message_saved event (Conductor sends message_id and optional thread_id)
pub(super) fn parse_user_message_saved_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    // Parse to Value first to handle potential duplicate fields (serde rejects duplicates)
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;

    let message_id = v.get("message_id").and_then(|v| v.as_i64()).unwrap_or(0);

    let thread_id = v
        .get("thread_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| message_id.to_string());

    Ok(SseEvent::ThreadInfo {
        thread_id,
        title: None,
    })
}

/// Parse message_info event
pub(super) fn parse_message_info_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let payload: MessageInfoPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::MessageInfo {
        message_id: payload.message_id,
    })
}

/// Parse done event (Conductor sends message_id in JSON)
pub(super) fn parse_done_event(data: &str) -> Result<SseEvent, SseParseError> {
    // Try to parse message_id from JSON, fall back to Done without data
    if let Ok(payload) = serde_json::from_str::<DonePayload>(data) {
        Ok(SseEvent::MessageInfo {
            message_id: payload.message_id.parse().unwrap_or(0),
        })
    } else {
        Ok(SseEvent::Done)
    }
}

/// Parse thread_updated event
pub(super) fn parse_thread_updated_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let payload: ThreadUpdatedPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::ThreadUpdated {
        thread_id: payload.thread_id,
        title: payload.title,
        description: payload.description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::parser::{parse_sse_event, SseParser};

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
    fn test_parse_user_message_saved_with_duplicate_fields() {
        // Backend may send JSON with duplicate field names
        // serde_json::from_str to struct would reject this, but parsing to Value handles it
        // Note: serde_json::Value uses the last occurrence of duplicate keys
        let data_with_duplicate =
            r#"{"message_id": 42, "thread_id": "thread-123", "thread_id": "thread-456"}"#;

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

    // Tests for thread_updated event

    #[test]
    fn test_parse_thread_updated_with_all_fields() {
        let result = parse_sse_event(
            "thread_updated",
            r#"{"thread_id": "thread-uuid-123", "title": "New Title", "description": "New Description"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ThreadUpdated {
                thread_id: "thread-uuid-123".to_string(),
                title: Some("New Title".to_string()),
                description: Some("New Description".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_thread_updated_with_only_thread_id() {
        let result = parse_sse_event("thread_updated", r#"{"thread_id": "thread-456"}"#);
        assert_eq!(
            result.unwrap(),
            SseEvent::ThreadUpdated {
                thread_id: "thread-456".to_string(),
                title: None,
                description: None,
            }
        );
    }

    #[test]
    fn test_parse_thread_updated_with_title_only() {
        let result = parse_sse_event(
            "thread_updated",
            r#"{"thread_id": "thread-789", "title": "Updated Title"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ThreadUpdated {
                thread_id: "thread-789".to_string(),
                title: Some("Updated Title".to_string()),
                description: None,
            }
        );
    }

    #[test]
    fn test_parse_thread_updated_with_description_only() {
        let result = parse_sse_event(
            "thread_updated",
            r#"{"thread_id": "thread-abc", "description": "New description text"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ThreadUpdated {
                thread_id: "thread-abc".to_string(),
                title: None,
                description: Some("New description text".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_thread_updated_with_empty_strings() {
        let result = parse_sse_event(
            "thread_updated",
            r#"{"thread_id": "thread-xyz", "title": "", "description": ""}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ThreadUpdated {
                thread_id: "thread-xyz".to_string(),
                title: Some("".to_string()),
                description: Some("".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_thread_updated_invalid_json() {
        let result = parse_sse_event("thread_updated", "not json");
        assert!(matches!(result, Err(SseParseError::InvalidJson { .. })));
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
    fn test_parser_thread_updated_event() {
        let mut parser = SseParser::new();

        parser.feed_line("event: thread_updated").unwrap();
        parser
            .feed_line(
                r#"data: {"thread_id": "t-999", "title": "Test Thread", "description": "Test Desc"}"#,
            )
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::ThreadUpdated {
                thread_id: "t-999".to_string(),
                title: Some("Test Thread".to_string()),
                description: Some("Test Desc".to_string()),
            })
        );
    }

    #[test]
    fn test_parser_thread_updated_event_minimal() {
        let mut parser = SseParser::new();

        parser.feed_line("event: thread_updated").unwrap();
        parser
            .feed_line(r#"data: {"thread_id": "minimal-thread"}"#)
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::ThreadUpdated {
                thread_id: "minimal-thread".to_string(),
                title: None,
                description: None,
            })
        );
    }
}
