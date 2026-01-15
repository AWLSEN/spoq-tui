//! Conductor API client for backend communication.
//!
//! This module provides the HTTP client for interacting with the Conductor backend,
//! including streaming responses via Server-Sent Events (SSE).

use crate::events::SseEvent;
use crate::models::{Message, StreamRequest, Thread, ThreadDetailResponse, ThreadListResponse};
use crate::sse::{SseParseError, SseParser};
use crate::state::Task;
use futures_util::stream::{self, Stream};
use futures_util::StreamExt;
use reqwest::Client;
use std::pin::Pin;

pub const CONDUCTOR_BASE_URL: &str = "http://100.80.115.93:8000";

/// Error type for Conductor client operations
#[derive(Debug)]
pub enum ConductorError {
    /// HTTP request failed
    Http(reqwest::Error),
    /// SSE parsing failed
    SseParse(SseParseError),
    /// JSON deserialization failed
    Json(serde_json::Error),
    /// Server returned an error status
    ServerError { status: u16, message: String },
    /// Endpoint not yet implemented
    NotImplemented(String),
}

impl std::fmt::Display for ConductorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConductorError::Http(e) => write!(f, "HTTP error: {}", e),
            ConductorError::SseParse(e) => write!(f, "SSE parse error: {}", e),
            ConductorError::Json(e) => write!(f, "JSON error: {}", e),
            ConductorError::ServerError { status, message } => {
                write!(f, "Server error ({}): {}", status, message)
            }
            ConductorError::NotImplemented(endpoint) => {
                write!(f, "Endpoint not implemented: {}", endpoint)
            }
        }
    }
}

impl std::error::Error for ConductorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConductorError::Http(e) => Some(e),
            ConductorError::SseParse(e) => Some(e),
            ConductorError::Json(e) => Some(e),
            ConductorError::ServerError { .. } => None,
            ConductorError::NotImplemented(_) => None,
        }
    }
}

impl From<reqwest::Error> for ConductorError {
    fn from(e: reqwest::Error) -> Self {
        ConductorError::Http(e)
    }
}

impl From<SseParseError> for ConductorError {
    fn from(e: SseParseError) -> Self {
        ConductorError::SseParse(e)
    }
}

impl From<serde_json::Error> for ConductorError {
    fn from(e: serde_json::Error) -> Self {
        ConductorError::Json(e)
    }
}

/// Client for interacting with the Conductor backend API.
///
/// Provides methods for streaming conversations, health checks, and cancellation.
pub struct ConductorClient {
    /// Base URL for the Conductor API
    pub base_url: String,
    /// Reusable HTTP client
    client: Client,
}

impl ConductorClient {
    /// Create a new ConductorClient with the default base URL.
    pub fn new() -> Self {
        Self {
            base_url: CONDUCTOR_BASE_URL.to_string(),
            client: Client::new(),
        }
    }

    /// Create a new ConductorClient with a custom base URL.
    pub fn with_base_url(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    /// Stream a conversation response from the Conductor API.
    ///
    /// Sends a POST request to `/v1/stream` and returns a stream of SSE events.
    ///
    /// # Arguments
    /// * `request` - The stream request containing the prompt and optional thread info
    ///
    /// # Returns
    /// A stream of `Result<SseEvent, ConductorError>` items
    pub async fn stream(
        &self,
        request: &StreamRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<SseEvent, ConductorError>> + Send>>, ConductorError>
    {
        let url = format!("{}/v1/stream", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        // Get the byte stream from the response
        let bytes_stream = response.bytes_stream();

        // Create an SSE parser and process the byte stream
        let event_stream = stream::unfold(
            (bytes_stream, SseParser::new(), String::new()),
            |(mut bytes_stream, mut parser, mut buffer)| async move {
                loop {
                    // First, try to process any complete lines in the buffer
                    if let Some(newline_pos) = buffer.find('\n') {
                        let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                        buffer = buffer[newline_pos + 1..].to_string();

                        match parser.feed_line(&line) {
                            Ok(Some(sse_event)) => {
                                // Convert the sse::SseEvent to events::SseEvent
                                let event = convert_sse_event(sse_event);
                                return Some((Ok(event), (bytes_stream, parser, buffer)));
                            }
                            Ok(None) => {
                                // Continue processing buffer
                                continue;
                            }
                            Err(e) => {
                                return Some((Err(ConductorError::SseParse(e)), (bytes_stream, parser, buffer)));
                            }
                        }
                    }

                    // Need more data from the stream
                    match bytes_stream.next().await {
                        Some(Ok(chunk)) => {
                            // Append new data to buffer
                            if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                                buffer.push_str(&text);
                            }
                            // Loop back to process the buffer
                        }
                        Some(Err(e)) => {
                            return Some((Err(ConductorError::Http(e)), (bytes_stream, parser, buffer)));
                        }
                        None => {
                            // Stream ended - process any remaining data in buffer
                            if !buffer.is_empty() {
                                let line = buffer.trim_end_matches('\r').to_string();
                                buffer.clear();
                                match parser.feed_line(&line) {
                                    Ok(Some(sse_event)) => {
                                        let event = convert_sse_event(sse_event);
                                        return Some((Ok(event), (bytes_stream, parser, buffer)));
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        return Some((Err(ConductorError::SseParse(e)), (bytes_stream, parser, buffer)));
                                    }
                                }
                            }
                            return None;
                        }
                    }
                }
            },
        );

        Ok(Box::pin(event_stream))
    }

    /// Check if the Conductor API is healthy and reachable.
    ///
    /// # Returns
    /// `true` if the health endpoint returns 200 OK, `false` otherwise
    pub async fn health_check(&self) -> Result<bool, ConductorError> {
        let url = format!("{}/v1/health", self.base_url);

        let response = self.client.get(&url).send().await?;

        Ok(response.status().is_success())
    }

    /// Cancel an ongoing streaming session.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to cancel
    pub async fn cancel(&self, session_id: &str) -> Result<(), ConductorError> {
        let url = format!("{}/v1/cancel", self.base_url);

        let body = serde_json::json!({ "session_id": session_id });

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        Ok(())
    }

    /// Fetch all threads from the backend.
    ///
    /// # Returns
    /// A vector of threads, or an error if the request fails
    pub async fn fetch_threads(&self) -> Result<Vec<Thread>, ConductorError> {
        let url = format!("{}/v1/threads", self.base_url);

        let response = self.client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        let data: ThreadListResponse = response.json().await?;
        Ok(data.threads)
    }

    /// Fetch all tasks from the backend.
    ///
    /// TODO: Expected endpoint: GET /v1/tasks
    ///
    /// # Returns
    /// A vector of tasks, or an error if the request fails
    pub async fn fetch_tasks(&self) -> Result<Vec<Task>, ConductorError> {
        // Stub: return empty vec for now
        Ok(Vec::new())
    }

    /// Fetch messages for a specific thread from the backend.
    ///
    /// TODO: Expected endpoint: GET /v1/threads/{id}/messages
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the thread to fetch messages for
    ///
    /// # Returns
    /// A vector of messages for the specified thread, or an error if the request fails
    pub async fn fetch_thread_messages(&self, _thread_id: &str) -> Result<Vec<Message>, ConductorError> {
        // Stub: return empty vec for now
        Ok(Vec::new())
    }

    /// Fetch a thread with its messages from the backend.
    ///
    /// GET /v1/threads/{id}?include_messages=true
    pub async fn fetch_thread_with_messages(
        &self,
        thread_id: &str,
    ) -> Result<ThreadDetailResponse, ConductorError> {
        let url = format!(
            "{}/v1/threads/{}?include_messages=true",
            self.base_url, thread_id
        );

        let response = self.client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        let data: ThreadDetailResponse = response.json().await?;
        Ok(data)
    }

    /// Get a thread by ID (stub - will implement with REST API)
    #[allow(dead_code)]
    pub fn get_thread(&self, _thread_id: &str) -> Option<Thread> {
        // Stub: return None for now
        None
    }

    /// Get recent messages (stub - will implement with REST API)
    #[allow(dead_code)]
    pub fn get_recent_messages(&self) -> Vec<Message> {
        // Stub: return empty vec for now
        Vec::new()
    }

    /// Respond to a permission request from the assistant.
    ///
    /// POST /v1/permissions/{permission_id}
    ///
    /// # Arguments
    /// * `permission_id` - The ID of the permission request
    /// * `approved` - Whether to approve (true) or deny (false) the permission
    ///
    /// # Returns
    /// Ok(()) on success, or an error if the request fails
    pub async fn respond_to_permission(
        &self,
        permission_id: &str,
        approved: bool,
    ) -> Result<(), ConductorError> {
        let url = format!("{}/v1/permissions/{}", self.base_url, permission_id);

        let body = serde_json::json!({
            "approved": approved
        });

        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        Ok(())
    }
}

impl Default for ConductorClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert sse::SseEvent to events::SseEvent
///
/// The sse module has a simpler SseEvent type used during parsing,
/// while events module has the full typed event structure.
fn convert_sse_event(event: crate::sse::SseEvent) -> SseEvent {
    match event {
        crate::sse::SseEvent::Content { text, meta } => {
            SseEvent::Content(crate::events::ContentEvent {
                text,
                meta: crate::events::EventMeta {
                    seq: meta.seq,
                    timestamp: meta.timestamp,
                    session_id: meta.session_id,
                    thread_id: meta.thread_id,
                },
            })
        }
        crate::sse::SseEvent::ThreadInfo { thread_id, title: _ } => {
            // Map to UserMessageSaved as a proxy for thread info
            SseEvent::UserMessageSaved(crate::events::UserMessageSavedEvent {
                message_id: String::new(),
                thread_id,
            })
        }
        crate::sse::SseEvent::MessageInfo { message_id } => {
            SseEvent::Done(crate::events::DoneEvent {
                message_id: message_id.to_string(),
            })
        }
        crate::sse::SseEvent::Done => SseEvent::Done(crate::events::DoneEvent {
            message_id: String::new(),
        }),
        crate::sse::SseEvent::Error { message, code } => {
            SseEvent::Error(crate::events::ErrorEvent { message, code })
        }
        crate::sse::SseEvent::Ping => {
            // Ping/keepalive - emit empty content that will be filtered
            SseEvent::Content(crate::events::ContentEvent {
                text: String::new(),
                meta: crate::events::EventMeta::default(),
            })
        }
        crate::sse::SseEvent::SkillsInjected { skills } => {
            SseEvent::SkillsInjected(crate::events::SkillsInjectedEvent { skills })
        }
        crate::sse::SseEvent::OAuthConsentRequired {
            provider,
            url,
            skill_name,
        } => SseEvent::OAuthConsentRequired(crate::events::OAuthConsentRequiredEvent {
            provider,
            url,
            skill_name,
        }),
        crate::sse::SseEvent::ContextCompacted {
            messages_removed,
            tokens_freed,
            tokens_used,
            token_limit,
        } => SseEvent::ContextCompacted(crate::events::ContextCompactedEvent {
            messages_removed,
            tokens_freed,
            tokens_used,
            token_limit,
        }),
        crate::sse::SseEvent::ToolCallStart { tool_name, tool_call_id } => {
            SseEvent::ToolCallStart(crate::events::ToolCallStartEvent {
                tool_name,
                tool_call_id,
            })
        }
        crate::sse::SseEvent::ToolCallArgument { tool_call_id, chunk } => {
            SseEvent::ToolCallArgument(crate::events::ToolCallArgumentEvent {
                tool_call_id,
                chunk,
            })
        }
        crate::sse::SseEvent::ToolExecuting { tool_call_id, display_name, url } => {
            SseEvent::ToolExecuting(crate::events::ToolExecutingEvent {
                tool_call_id,
                display_name,
                url,
            })
        }
        crate::sse::SseEvent::ToolResult { tool_call_id, result } => {
            SseEvent::ToolResult(crate::events::ToolResultEvent {
                tool_call_id,
                result,
            })
        }
        crate::sse::SseEvent::Reasoning { text } => {
            SseEvent::Reasoning(crate::events::ReasoningEvent { text })
        }
        crate::sse::SseEvent::PermissionRequest {
            permission_id,
            tool_name,
            description,
            tool_call_id,
            tool_input,
        } => SseEvent::PermissionRequest(crate::events::PermissionRequestEvent {
            permission_id,
            tool_name,
            description,
            tool_call_id,
            tool_input,
        }),
        crate::sse::SseEvent::TodosUpdated { todos } => {
            // Parse todos from Value to Vec<TodoItem>
            let todo_items: Vec<crate::events::TodoItem> = serde_json::from_value(todos)
                .unwrap_or_default();
            SseEvent::TodosUpdated(crate::events::TodosUpdatedEvent { todos: todo_items })
        }
        crate::sse::SseEvent::Subagent { subagent_type, data } => {
            SseEvent::Subagent(crate::events::SubagentEvent {
                subagent_type,
                data,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StreamRequest;

    #[test]
    fn test_conductor_client_new() {
        let client = ConductorClient::new();
        assert_eq!(client.base_url, CONDUCTOR_BASE_URL);
    }

    #[test]
    fn test_conductor_client_with_base_url() {
        let custom_url = "http://localhost:8080".to_string();
        let client = ConductorClient::with_base_url(custom_url.clone());
        assert_eq!(client.base_url, custom_url);
    }

    #[test]
    fn test_conductor_client_default() {
        let client = ConductorClient::default();
        assert_eq!(client.base_url, CONDUCTOR_BASE_URL);
    }

    #[test]
    fn test_get_thread_returns_none() {
        let client = ConductorClient::new();
        let result = client.get_thread("test-id");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_recent_messages_returns_empty() {
        let client = ConductorClient::new();
        let messages = client.get_recent_messages();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_stream_request_creation() {
        let request = StreamRequest::new("test".to_string());
        assert_eq!(request.prompt, "test");
    }

    #[test]
    fn test_conductor_error_display() {
        let err = ConductorError::ServerError {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("500"));
        assert!(display.contains("Internal Server Error"));
    }

    #[test]
    fn test_conductor_error_from_sse_parse() {
        let sse_err = SseParseError::UnknownEventType("test".to_string());
        let err: ConductorError = sse_err.into();
        assert!(matches!(err, ConductorError::SseParse(_)));
    }

    #[test]
    fn test_convert_sse_event_content() {
        let sse_event = crate::sse::SseEvent::Content {
            text: "Hello".to_string(),
            meta: crate::sse::SseEventMeta {
                seq: Some(5),
                timestamp: Some(1736956800000),
                session_id: Some("sess-123".to_string()),
                thread_id: Some("thread-456".to_string()),
            },
        };
        let event = convert_sse_event(sse_event);
        match event {
            SseEvent::Content(content) => {
                assert_eq!(content.text, "Hello");
                assert_eq!(content.meta.seq, Some(5));
                assert_eq!(content.meta.timestamp, Some(1736956800000));
            }
            _ => panic!("Expected Content event"),
        }
    }

    #[test]
    fn test_convert_sse_event_done() {
        let sse_event = crate::sse::SseEvent::Done;
        let event = convert_sse_event(sse_event);
        assert!(matches!(event, SseEvent::Done(_)));
    }

    #[test]
    fn test_convert_sse_event_error() {
        let sse_event = crate::sse::SseEvent::Error {
            message: "Test error".to_string(),
            code: Some("ERR001".to_string()),
        };
        let event = convert_sse_event(sse_event);
        match event {
            SseEvent::Error(err) => {
                assert_eq!(err.message, "Test error");
                assert_eq!(err.code, Some("ERR001".to_string()));
            }
            _ => panic!("Expected Error event"),
        }
    }

    // Async tests for HTTP methods
    #[tokio::test]
    async fn test_health_check_with_invalid_server() {
        // Use an invalid URL that will fail to connect
        let client = ConductorClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.health_check().await;
        // Should fail with HTTP error since server doesn't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancel_with_invalid_server() {
        let client = ConductorClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.cancel("test-session").await;
        // Should fail with HTTP error since server doesn't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stream_with_invalid_server() {
        let client = ConductorClient::with_base_url("http://127.0.0.1:1".to_string());
        let request = StreamRequest::new("test prompt".to_string());
        let result = client.stream(&request).await;
        // Should fail with HTTP error since server doesn't exist
        assert!(result.is_err());
    }

    #[test]
    fn test_conductor_error_not_implemented_display() {
        let err = ConductorError::NotImplemented("/v1/test/endpoint".to_string());
        let display = format!("{}", err);
        assert!(display.contains("not implemented"));
        assert!(display.contains("/v1/test/endpoint"));
    }

}
