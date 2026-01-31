//! Miscellaneous event parsers

use crate::sse::events::{SseEvent, SseParseError};
use crate::sse::payloads::{
    ContextCompactedPayload, ErrorPayload, OAuthConsentRequiredPayload, RateLimitedPayload,
    SkillsInjectedPayload, SystemInitPayload, UsagePayload,
};

/// Parse skills_injected event
pub(super) fn parse_skills_injected_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let payload: SkillsInjectedPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::SkillsInjected {
        skills: payload.skills,
    })
}

/// Parse oauth_consent_required event
pub(super) fn parse_oauth_consent_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let payload: OAuthConsentRequiredPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::OAuthConsentRequired {
        provider: payload.provider,
        url: payload.url,
        skill_name: payload.skill_name,
    })
}

/// Parse context_compacted event
pub(super) fn parse_context_compacted_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let payload: ContextCompactedPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::ContextCompacted {
        messages_removed: payload.messages_removed,
        tokens_freed: payload.tokens_freed,
        tokens_used: payload.tokens_used,
        token_limit: payload.token_limit,
    })
}

/// Parse error event
pub(super) fn parse_error_event(event_type: &str, data: &str) -> Result<SseEvent, SseParseError> {
    let payload: ErrorPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::Error {
        message: payload.message,
        code: payload.code,
    })
}

/// Parse todos_updated event
pub(super) fn parse_todos_updated_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let v: serde_json::Value =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    let todos = v
        .get("todos")
        .cloned()
        .unwrap_or(serde_json::Value::Array(vec![]));
    Ok(SseEvent::TodosUpdated { todos })
}

/// Parse usage event
pub(super) fn parse_usage_event(event_type: &str, data: &str) -> Result<SseEvent, SseParseError> {
    let payload: UsagePayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::Usage {
        context_window_used: payload.context_window_used,
        context_window_limit: payload.context_window_limit,
    })
}

/// Parse system_init event
pub(super) fn parse_system_init_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let payload: SystemInitPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::SystemInit {
        cli_session_id: payload.cli_session_id,
        permission_mode: payload.permission_mode,
        model: payload.model,
        tool_count: payload.tool_count,
    })
}

/// Parse cancelled event
pub(super) fn parse_cancelled_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    #[derive(serde::Deserialize)]
    struct CancelledPayload {
        reason: String,
    }
    let payload: CancelledPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::Cancelled {
        reason: payload.reason,
    })
}

/// Parse rate_limited event
pub(super) fn parse_rate_limited_event(
    event_type: &str,
    data: &str,
) -> Result<SseEvent, SseParseError> {
    let payload: RateLimitedPayload =
        serde_json::from_str(data).map_err(|e| SseParseError::InvalidJson {
            event_type: event_type.to_string(),
            source: e.to_string(),
        })?;
    Ok(SseEvent::RateLimited {
        message: payload.message,
        current_account_id: payload.current_account_id,
        next_account_id: payload.next_account_id,
        retry_after_secs: payload.retry_after_secs,
    })
}

#[cfg(test)]
mod tests {
    use crate::sse::events::SseEvent;
    use crate::sse::parser::{parse_sse_event, SseParser};

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
    fn test_parse_skills_injected() {
        let result = parse_sse_event(
            "skills_injected",
            r#"{"skills": ["commit", "review-pr", "pdf"]}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::SkillsInjected {
                skills: vec![
                    "commit".to_string(),
                    "review-pr".to_string(),
                    "pdf".to_string()
                ],
            }
        );
    }

    #[test]
    fn test_parse_oauth_consent() {
        let result = parse_sse_event(
            "oauth_consent_required",
            r#"{"provider": "github", "url": "https://github.com/auth", "skill_name": "github-pr"}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::OAuthConsentRequired {
                provider: "github".to_string(),
                url: Some("https://github.com/auth".to_string()),
                skill_name: Some("github-pr".to_string()),
            }
        );
    }

    #[test]
    fn test_parse_context_compacted() {
        let result = parse_sse_event(
            "context_compacted",
            r#"{"messages_removed": 10, "tokens_freed": 5000, "tokens_used": 150000, "token_limit": 200000}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::ContextCompacted {
                messages_removed: 10,
                tokens_freed: 5000,
                tokens_used: Some(150000),
                token_limit: Some(200000),
            }
        );
    }

    #[test]
    fn test_parse_todos_updated() {
        let result = parse_sse_event(
            "todos_updated",
            r#"{"todos": [{"id": 1, "text": "Task 1", "done": false}]}"#,
        );
        let event = result.unwrap();
        match event {
            SseEvent::TodosUpdated { todos } => {
                assert!(todos.is_array());
                assert_eq!(todos.as_array().unwrap().len(), 1);
            }
            _ => panic!("Expected TodosUpdated event"),
        }
    }

    #[test]
    fn test_parse_usage() {
        let result = parse_sse_event(
            "usage",
            r#"{"context_window_used": 100000, "context_window_limit": 200000}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::Usage {
                context_window_used: 100000,
                context_window_limit: 200000,
            }
        );
    }

    #[test]
    fn test_parse_system_init() {
        let result = parse_sse_event(
            "system_init",
            r#"{"cli_session_id": "sess-123", "permission_mode": "auto", "model": "opus", "tool_count": 15}"#,
        );
        assert_eq!(
            result.unwrap(),
            SseEvent::SystemInit {
                cli_session_id: "sess-123".to_string(),
                permission_mode: "auto".to_string(),
                model: "opus".to_string(),
                tool_count: 15,
            }
        );
    }

    #[test]
    fn test_parse_cancelled() {
        let result = parse_sse_event("cancelled", r#"{"reason": "user_requested"}"#);
        assert_eq!(
            result.unwrap(),
            SseEvent::Cancelled {
                reason: "user_requested".to_string(),
            }
        );
    }

    #[test]
    fn test_parser_cancelled_event() {
        let mut parser = SseParser::new();

        parser.feed_line("event: cancelled").unwrap();
        parser
            .feed_line(r#"data: {"reason": "user_requested"}"#)
            .unwrap();

        let event = parser.feed_line("").unwrap();
        assert_eq!(
            event,
            Some(SseEvent::Cancelled {
                reason: "user_requested".to_string(),
            })
        );
    }
}
