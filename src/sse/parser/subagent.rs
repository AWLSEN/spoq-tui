//! Subagent event parsers

use crate::sse::events::{SseEvent, SseParseError};

/// Parse subagent_started event
pub(super) fn parse_subagent_started_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::SubagentStarted {
        task_id: v
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        description: v
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        subagent_type: v
            .get("subagent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// Parse subagent_progress event
pub(super) fn parse_subagent_progress_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::SubagentProgress {
        task_id: v
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        message: v
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// Parse subagent_completed event
pub(super) fn parse_subagent_completed_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::SubagentCompleted {
        task_id: v
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        summary: v
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tool_call_count: v
            .get("tool_call_count")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32),
    })
}

#[cfg(test)]
mod tests {
    use crate::sse::events::SseEvent;
    use crate::sse::parser::{parse_sse_event, SseParser};

    #[test]
    fn test_parse_subagent_started_event() {
        let result = parse_sse_event(
            "subagent_started",
            r#"{"task_id": "task-001", "description": "Explore codebase", "subagent_type": "Explore"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::SubagentStarted {
                task_id: "task-001".to_string(),
                description: "Explore codebase".to_string(),
                subagent_type: "Explore".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_subagent_progress_event() {
        let result = parse_sse_event(
            "subagent_progress",
            r#"{"task_id": "task-002", "message": "Searching files..."}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::SubagentProgress {
                task_id: "task-002".to_string(),
                message: "Searching files...".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_subagent_completed_event() {
        let result = parse_sse_event(
            "subagent_completed",
            r#"{"task_id": "task-003", "summary": "Found 15 files", "tool_call_count": 42}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::SubagentCompleted {
                task_id: "task-003".to_string(),
                summary: "Found 15 files".to_string(),
                tool_call_count: Some(42),
            }
        );
    }

    #[test]
    fn test_parse_subagent_completed_event_without_tool_count() {
        let result = parse_sse_event(
            "subagent_completed",
            r#"{"task_id": "task-004", "summary": "Analysis complete"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::SubagentCompleted {
                task_id: "task-004".to_string(),
                summary: "Analysis complete".to_string(),
                tool_call_count: None,
            }
        );
    }

    #[test]
    fn test_parser_subagent_started_event() {
        let mut parser = SseParser::new();

        parser.feed_line("event: subagent_started").unwrap();
        parser
            .feed_line(
                r#"data: {"task_id": "t-123", "description": "Plan implementation", "subagent_type": "Plan"}"#,
            )
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::SubagentStarted {
                task_id: "t-123".to_string(),
                description: "Plan implementation".to_string(),
                subagent_type: "Plan".to_string(),
            })
        );
    }

    #[test]
    fn test_parser_subagent_progress_event() {
        let mut parser = SseParser::new();

        parser.feed_line("event: subagent_progress").unwrap();
        parser
            .feed_line(r#"data: {"task_id": "t-456", "message": "Analyzing dependencies"}"#)
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::SubagentProgress {
                task_id: "t-456".to_string(),
                message: "Analyzing dependencies".to_string(),
            })
        );
    }

    #[test]
    fn test_parser_subagent_completed_event() {
        let mut parser = SseParser::new();

        parser.feed_line("event: subagent_completed").unwrap();
        parser
            .feed_line(
                r#"data: {"task_id": "t-789", "summary": "Successfully completed analysis", "tool_call_count": 25}"#,
            )
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::SubagentCompleted {
                task_id: "t-789".to_string(),
                summary: "Successfully completed analysis".to_string(),
                tool_call_count: Some(25),
            })
        );
    }

    #[test]
    fn test_parser_subagent_completed_event_without_tool_count() {
        let mut parser = SseParser::new();

        parser.feed_line("event: subagent_completed").unwrap();
        parser
            .feed_line(r#"data: {"task_id": "t-999", "summary": "Done"}"#)
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::SubagentCompleted {
                task_id: "t-999".to_string(),
                summary: "Done".to_string(),
                tool_call_count: None,
            })
        );
    }
}
