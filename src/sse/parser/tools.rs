//! Tool-related event parsers

use crate::sse::events::{SseEvent, SseParseError};

/// Parse tool_call_start event
pub(super) fn parse_tool_call_start_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::ToolCallStart {
        tool_name: v
            .get("function")
            .or(v.get("tool_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tool_call_id: v
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// Parse tool_call_argument event
pub(super) fn parse_tool_call_argument_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::ToolCallArgument {
        tool_call_id: v
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        chunk: v
            .get("chunk")
            .or(v.get("argument_chunk"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// Parse tool_executing event
pub(super) fn parse_tool_executing_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::ToolExecuting {
        tool_call_id: v
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        display_name: v
            .get("display_name")
            .or(v.get("function"))
            .and_then(|v| v.as_str())
            .map(String::from),
        url: v.get("url").and_then(|v| v.as_str()).map(String::from),
    })
}

/// Parse tool_result event
pub(super) fn parse_tool_result_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    let result = v
        .get("result")
        .map(|r| {
            if r.is_string() {
                r.as_str().unwrap().to_string()
            } else {
                r.to_string()
            }
        })
        .unwrap_or_default();
    Ok(SseEvent::ToolResult {
        tool_call_id: v
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        result,
    })
}

#[cfg(test)]
mod tests {
    use crate::sse::events::SseEvent;
    use crate::sse::parser::parse_sse_event;

    #[test]
    fn test_parse_tool_call_start() {
        let result = parse_sse_event(
            "tool_call_start",
            r#"{"function": "read_file", "tool_call_id": "call-123"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ToolCallStart {
                tool_name: "read_file".to_string(),
                tool_call_id: "call-123".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_tool_call_start_with_tool_name() {
        let result = parse_sse_event(
            "tool_call_start",
            r#"{"tool_name": "write_file", "tool_call_id": "call-456"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ToolCallStart {
                tool_name: "write_file".to_string(),
                tool_call_id: "call-456".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_tool_call_argument() {
        let result = parse_sse_event(
            "tool_call_argument",
            r#"{"tool_call_id": "call-123", "chunk": "{\"path\":"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ToolCallArgument {
                tool_call_id: "call-123".to_string(),
                chunk: "{\"path\":".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_tool_call_argument_with_argument_chunk() {
        let result = parse_sse_event(
            "tool_call_argument",
            r#"{"tool_call_id": "call-789", "argument_chunk": "partial"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ToolCallArgument {
                tool_call_id: "call-789".to_string(),
                chunk: "partial".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_tool_executing() {
        let result = parse_sse_event(
            "tool_executing",
            r#"{"tool_call_id": "call-123", "display_name": "Reading file", "url": "https://example.com"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ToolExecuting {
                tool_call_id: "call-123".to_string(),
                display_name: Some("Reading file".to_string()),
                url: Some("https://example.com".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_tool_executing_minimal() {
        let result = parse_sse_event("tool_executing", r#"{"tool_call_id": "call-456"}"#);
        assert_eq!(
            result.unwrap(),
            SseEvent::ToolExecuting {
                tool_call_id: "call-456".to_string(),
                display_name: None,
                url: None,
            }
        );
    }

    #[test]
    fn test_parse_tool_result_string() {
        let result = parse_sse_event(
            "tool_result",
            r#"{"tool_call_id": "call-123", "result": "File contents here"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ToolResult {
                tool_call_id: "call-123".to_string(),
                result: "File contents here".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_tool_result_object() {
        let result = parse_sse_event(
            "tool_result",
            r#"{"tool_call_id": "call-456", "result": {"key": "value"}}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ToolResult {
                tool_call_id: "call-456".to_string(),
                result: r#"{"key":"value"}"#.to_string(),
            }
        );
    }
}
