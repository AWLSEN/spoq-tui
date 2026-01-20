//! Permission request event parser

use crate::sse::events::{SseEvent, SseParseError};

/// Parse permission_request event
pub(super) fn parse_permission_request_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::PermissionRequest {
        permission_id: v
            .get("permission_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tool_name: v
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        description: v
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tool_call_id: v
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(String::from),
        tool_input: v.get("tool_input").cloned(),
    })
}

#[cfg(test)]
mod tests {
    use crate::sse::events::SseEvent;
    use crate::sse::parser::parse_sse_event;

    #[test]
    fn test_parse_permission_request() {
        let result = parse_sse_event(
            "permission_request",
            r#"{"permission_id": "perm-123", "tool_name": "execute_bash", "description": "Run shell command"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::PermissionRequest {
                permission_id: "perm-123".to_string(),
                tool_name: "execute_bash".to_string(),
                description: "Run shell command".to_string(),
                tool_call_id: None,
                tool_input: None,
            }
        );
    }

    #[test]
    fn test_parse_permission_request_with_tool_call_id() {
        let result = parse_sse_event(
            "permission_request",
            r#"{"permission_id": "perm-456", "tool_name": "write_file", "description": "Write to file", "tool_call_id": "call-789"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::PermissionRequest {
                permission_id: "perm-456".to_string(),
                tool_name: "write_file".to_string(),
                description: "Write to file".to_string(),
                tool_call_id: Some("call-789".to_string()),
                tool_input: None,
            }
        );
    }

    #[test]
    fn test_parse_permission_request_with_tool_input() {
        let result = parse_sse_event(
            "permission_request",
            r#"{"permission_id": "perm-abc", "tool_name": "edit_file", "description": "Edit", "tool_input": {"path": "/tmp/test.txt"}}"#,
        );
        let event = result.unwrap();
        match event {
            SseEvent::PermissionRequest {
                permission_id,
                tool_name,
                description,
                tool_call_id,
                tool_input,
            } => {
                assert_eq!(permission_id, "perm-abc");
                assert_eq!(tool_name, "edit_file");
                assert_eq!(description, "Edit");
                assert_eq!(tool_call_id, None);
                assert!(tool_input.is_some());
                let input = tool_input.unwrap();
                assert_eq!(
                    input.get("path").and_then(|v| v.as_str()),
                    Some("/tmp/test.txt")
                );
            }
            _ => panic!("Expected PermissionRequest event"),
        }
    }
}
