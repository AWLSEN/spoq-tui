//! SSE payload deserialization structs
//!
//! Contains internal structs used to deserialize JSON data payloads
//! from the backend SSE stream.

use serde::Deserialize;

/// Raw data payload from SSE data lines
/// Supports multiple field names that backends might use for content
/// Also captures flattened metadata fields from the backend
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ContentPayload {
    /// The text content - accepts "text", "content", "data", "chunk", or "token" fields
    /// Conductor uses "data" for content chunks
    #[serde(alias = "content", alias = "data", alias = "chunk", alias = "token")]
    pub text: Option<String>,
    /// Some backends nest content in a delta object (OpenAI style)
    #[serde(default)]
    pub delta: Option<DeltaPayload>,
    /// Sequence number for ordering events
    #[serde(default)]
    pub seq: Option<u64>,
    /// Unix timestamp in milliseconds
    #[serde(default)]
    pub timestamp: Option<u64>,
    /// Session ID for the current streaming session
    #[serde(default)]
    pub session_id: Option<String>,
    /// Thread ID this event belongs to
    #[serde(default)]
    pub thread_id: Option<String>,
}

/// Nested delta payload for OpenAI-style responses
#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct DeltaPayload {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub data: Option<String>,
}

/// Legacy thread_info payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ThreadInfoPayload {
    pub thread_id: String,
    #[serde(default)]
    pub title: Option<String>,
}

/// Conductor's done payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DonePayload {
    pub message_id: String,
}

/// Message info payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MessageInfoPayload {
    pub message_id: i64,
}

/// Error payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ErrorPayload {
    pub message: String,
    #[serde(default)]
    pub code: Option<String>,
}

/// Skills injected payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SkillsInjectedPayload {
    pub skills: Vec<String>,
}

/// OAuth consent required payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct OAuthConsentRequiredPayload {
    pub provider: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub skill_name: Option<String>,
}

/// Context compacted payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ContextCompactedPayload {
    pub messages_removed: u32,
    pub tokens_freed: u32,
    #[serde(default)]
    pub tokens_used: Option<u32>,
    #[serde(default)]
    pub token_limit: Option<u32>,
}

/// Thread updated payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ThreadUpdatedPayload {
    pub thread_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Usage payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UsagePayload {
    pub context_window_used: u32,
    pub context_window_limit: u32,
}

/// SystemInit payload
/// Note: Conductor sends tool_count (number) not tools (array)
/// cli_session_id is the Claude CLI session, session_id is Conductor's session (from EventMeta)
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SystemInitPayload {
    /// Claude CLI session ID (distinct from EventMeta's session_id)
    pub cli_session_id: String,
    pub permission_mode: String,
    pub model: String,
    /// Number of tools available (Conductor sends tool_count, not tools array)
    pub tool_count: usize,
}

/// Rate limited payload
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RateLimitedPayload {
    pub message: String,
    pub current_account_id: String,
    #[serde(default)]
    pub next_account_id: Option<String>,
    pub retry_after_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_payload_text_field() {
        let json = r#"{"text": "Hello world"}"#;
        let payload: ContentPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.text, Some("Hello world".to_string()));
    }

    #[test]
    fn test_content_payload_content_alias() {
        let json = r#"{"content": "From content field"}"#;
        let payload: ContentPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.text, Some("From content field".to_string()));
    }

    #[test]
    fn test_content_payload_data_alias() {
        let json = r#"{"data": "From data field"}"#;
        let payload: ContentPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.text, Some("From data field".to_string()));
    }

    #[test]
    fn test_content_payload_with_metadata() {
        let json = r#"{"text": "Hello", "seq": 5, "timestamp": 1736956800000, "session_id": "abc", "thread_id": "xyz"}"#;
        let payload: ContentPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.text, Some("Hello".to_string()));
        assert_eq!(payload.seq, Some(5));
        assert_eq!(payload.timestamp, Some(1736956800000));
        assert_eq!(payload.session_id, Some("abc".to_string()));
        assert_eq!(payload.thread_id, Some("xyz".to_string()));
    }

    #[test]
    fn test_delta_payload() {
        let json = r#"{"delta": {"content": "Delta content"}}"#;
        let payload: ContentPayload = serde_json::from_str(json).unwrap();
        assert!(payload.delta.is_some());
        assert_eq!(
            payload.delta.unwrap().content,
            Some("Delta content".to_string())
        );
    }

    #[test]
    fn test_thread_info_payload() {
        let json = r#"{"thread_id": "t-123", "title": "My Thread"}"#;
        let payload: ThreadInfoPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.thread_id, "t-123");
        assert_eq!(payload.title, Some("My Thread".to_string()));
    }

    #[test]
    fn test_thread_info_payload_no_title() {
        let json = r#"{"thread_id": "t-123"}"#;
        let payload: ThreadInfoPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.thread_id, "t-123");
        assert!(payload.title.is_none());
    }

    #[test]
    fn test_error_payload() {
        let json = r#"{"message": "Something went wrong", "code": "ERR_500"}"#;
        let payload: ErrorPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.message, "Something went wrong");
        assert_eq!(payload.code, Some("ERR_500".to_string()));
    }

    #[test]
    fn test_error_payload_no_code() {
        let json = r#"{"message": "Oops"}"#;
        let payload: ErrorPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.message, "Oops");
        assert!(payload.code.is_none());
    }

    #[test]
    fn test_context_compacted_payload() {
        let json = r#"{"messages_removed": 5, "tokens_freed": 1000, "tokens_used": 2000, "token_limit": 8000}"#;
        let payload: ContextCompactedPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.messages_removed, 5);
        assert_eq!(payload.tokens_freed, 1000);
        assert_eq!(payload.tokens_used, Some(2000));
        assert_eq!(payload.token_limit, Some(8000));
    }

    #[test]
    fn test_usage_payload() {
        let json = r#"{"context_window_used": 5000, "context_window_limit": 200000}"#;
        let payload: UsagePayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.context_window_used, 5000);
        assert_eq!(payload.context_window_limit, 200000);
    }

    #[test]
    fn test_thread_updated_payload() {
        let json = r#"{"thread_id": "t-abc", "title": "New Title", "description": "New Desc"}"#;
        let payload: ThreadUpdatedPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.thread_id, "t-abc");
        assert_eq!(payload.title, Some("New Title".to_string()));
        assert_eq!(payload.description, Some("New Desc".to_string()));
    }

    #[test]
    fn test_system_init_payload() {
        let json = r#"{"cli_session_id": "sess-abc", "permission_mode": "auto", "model": "opus", "tool_count": 15}"#;
        let payload: SystemInitPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.cli_session_id, "sess-abc");
        assert_eq!(payload.permission_mode, "auto");
        assert_eq!(payload.model, "opus");
        assert_eq!(payload.tool_count, 15);
    }
}
