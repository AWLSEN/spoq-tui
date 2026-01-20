//! Content and reasoning event parsers

use crate::sse::events::{SseEvent, SseEventMeta, SseParseError};
use crate::sse::payloads::ContentPayload;

/// Parse content event from various formats (content, text, message, chunk, delta, content_block_delta)
pub(super) fn parse_content_event(event_type: &str, data: &str) -> Result<SseEvent, SseParseError> {
    let payload: ContentPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;

    // Extract text from various possible locations in the payload
    let text = payload
        .text
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

/// Parse reasoning/thinking event
pub(super) fn parse_reasoning_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    let text = v
        .get("text")
        .or(v.get("content"))
        .or(v.get("data"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(SseEvent::Reasoning { text })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::parser::parse_sse_event;

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
}
