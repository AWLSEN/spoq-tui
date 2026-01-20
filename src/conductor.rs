//! Conductor API client for backend communication.
//!
//! This module provides the HTTP client for interacting with the Conductor backend,
//! including streaming responses via Server-Sent Events (SSE).

use crate::debug::{DebugEvent, DebugEventKind, DebugEventSender, RawSseEventData};
use crate::events::SseEvent;
use crate::models::{
    Folder, FolderListResponse, Message, StreamRequest, Thread, ThreadDetailResponse,
    ThreadListResponse,
};
use crate::sse::{SseParseError, SseParser};
use crate::state::Task;
use futures_util::stream::{self, Stream};
use futures_util::StreamExt;
use reqwest::Client;
use std::pin::Pin;

/// Default URL for the Conductor API
pub const DEFAULT_CONDUCTOR_URL: &str = "http://100.85.185.33:8000";

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
    /// Optional authentication token for Bearer auth
    auth_token: Option<String>,
}

impl ConductorClient {
    /// Create a new ConductorClient with the default base URL.
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_CONDUCTOR_URL.to_string(),
            client: Client::new(),
            auth_token: None,
        }
    }

    /// Create a new ConductorClient with a custom base URL.
    pub fn with_base_url(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
            auth_token: None,
        }
    }

    /// Create a new ConductorClient with a custom URL (alias for with_base_url).
    pub fn with_url(base_url: &str) -> Self {
        Self::with_base_url(base_url.to_string())
    }

    /// Set the authentication token for Bearer auth.
    ///
    /// Returns self for method chaining.
    pub fn with_auth(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    /// Set the authentication token on an existing client.
    pub fn set_auth_token(&mut self, token: Option<String>) {
        self.auth_token = token;
    }

    /// Get the current authentication token, if set.
    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    /// Helper to add auth header to a request builder if token is set.
    fn add_auth_header(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref token) = self.auth_token {
            builder.header("Authorization", format!("Bearer {}", token))
        } else {
            builder
        }
    }

    /// Stream a conversation response from the Conductor API.
    ///
    /// Sends a POST request to `/v1/stream` and returns a stream of SSE events.
    ///
    /// # Arguments
    /// * `request` - The stream request containing the prompt and optional thread info
    /// * `debug_tx` - Optional debug event sender for emitting raw SSE events
    ///
    /// # Returns
    /// A stream of `Result<SseEvent, ConductorError>` items
    pub async fn stream(
        &self,
        request: &StreamRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<SseEvent, ConductorError>> + Send>>, ConductorError>
    {
        self.stream_with_debug(request, None).await
    }

    /// Stream a conversation response from the Conductor API with optional debug events.
    ///
    /// This is the internal implementation that supports debug event emission.
    ///
    /// # Arguments
    /// * `request` - The stream request containing the prompt and optional thread info
    /// * `debug_tx` - Optional debug event sender for emitting raw SSE events
    ///
    /// # Returns
    /// A stream of `Result<SseEvent, ConductorError>` items
    pub async fn stream_with_debug(
        &self,
        request: &StreamRequest,
        debug_tx: Option<DebugEventSender>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<SseEvent, ConductorError>> + Send>>, ConductorError>
    {
        let url = format!("{}/v1/stream", self.base_url);

        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(request);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        // Get the byte stream from the response
        let bytes_stream = response.bytes_stream();

        // Create an SSE parser and process the byte stream
        // Include debug_tx in the state tuple for emitting debug events
        // Use Vec<u8> buffer to avoid data loss when UTF-8 chars are split across TCP chunks
        let event_stream = stream::unfold(
            (bytes_stream, SseParser::new(), Vec::<u8>::new(), debug_tx),
            |(mut bytes_stream, mut parser, mut byte_buffer, debug_tx)| async move {
                loop {
                    // First, try to process any complete lines in the buffer
                    // Look for newline in the byte buffer
                    if let Some(newline_pos) = byte_buffer.iter().position(|&b| b == b'\n') {
                        // Extract the line bytes (including newline)
                        let line_bytes: Vec<u8> = byte_buffer.drain(..=newline_pos).collect();

                        // Decode to string using lossy conversion to handle edge cases
                        // where a multi-byte UTF-8 char might still be incomplete
                        let line = String::from_utf8_lossy(&line_bytes[..line_bytes.len() - 1])
                            .trim_end_matches('\r')
                            .to_string();

                        match parser.feed_line(&line) {
                            Ok(Some(sse_event)) => {
                                // Emit raw SSE debug event if debug channel is available
                                if let Some(ref tx) = debug_tx {
                                    let raw_data = RawSseEventData::new(
                                        sse_event.event_type_name(),
                                        format!("{:?}", sse_event),
                                    );
                                    let debug_event =
                                        DebugEvent::new(DebugEventKind::RawSseEvent(raw_data));
                                    let _ = tx.send(debug_event);
                                }

                                // Convert the sse::SseEvent to events::SseEvent
                                let event = convert_sse_event(sse_event);
                                return Some((
                                    Ok(event),
                                    (bytes_stream, parser, byte_buffer, debug_tx),
                                ));
                            }
                            Ok(None) => {
                                // Continue processing buffer
                                continue;
                            }
                            Err(e) => {
                                return Some((
                                    Err(ConductorError::SseParse(e)),
                                    (bytes_stream, parser, byte_buffer, debug_tx),
                                ));
                            }
                        }
                    }

                    // Need more data from the stream
                    match bytes_stream.next().await {
                        Some(Ok(chunk)) => {
                            // Append raw bytes to buffer - no UTF-8 conversion that could fail
                            byte_buffer.extend_from_slice(&chunk);
                            // Loop back to process the buffer
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(ConductorError::Http(e)),
                                (bytes_stream, parser, byte_buffer, debug_tx),
                            ));
                        }
                        None => {
                            // Stream ended - process any remaining data in buffer
                            if !byte_buffer.is_empty() {
                                let line = String::from_utf8_lossy(&byte_buffer)
                                    .trim_end_matches('\r')
                                    .to_string();
                                byte_buffer.clear();
                                match parser.feed_line(&line) {
                                    Ok(Some(sse_event)) => {
                                        // Emit raw SSE debug event if debug channel is available
                                        if let Some(ref tx) = debug_tx {
                                            let raw_data = RawSseEventData::new(
                                                sse_event.event_type_name(),
                                                format!("{:?}", sse_event),
                                            );
                                            let debug_event = DebugEvent::new(
                                                DebugEventKind::RawSseEvent(raw_data),
                                            );
                                            let _ = tx.send(debug_event);
                                        }

                                        let event = convert_sse_event(sse_event);
                                        return Some((
                                            Ok(event),
                                            (bytes_stream, parser, byte_buffer, debug_tx),
                                        ));
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        return Some((
                                            Err(ConductorError::SseParse(e)),
                                            (bytes_stream, parser, byte_buffer, debug_tx),
                                        ));
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

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        Ok(response.status().is_success())
    }

    /// Cancel an ongoing streaming session.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to cancel
    pub async fn cancel(&self, session_id: &str) -> Result<(), ConductorError> {
        let url = format!("{}/v1/cancel", self.base_url);

        let body = serde_json::json!({ "session_id": session_id });

        let builder = self.client.post(&url).json(&body);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
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

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        let data: ThreadListResponse = response.json().await?;
        Ok(data.threads)
    }

    /// Fetch all folders from the backend.
    ///
    /// # Returns
    /// A vector of folders, or an error if the request fails
    pub async fn fetch_folders(&self) -> Result<Vec<Folder>, ConductorError> {
        let url = format!("{}/v1/folders", self.base_url);
        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }
        let data: FolderListResponse = response.json().await?;
        Ok(data.folders)
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
    pub async fn fetch_thread_messages(
        &self,
        _thread_id: &str,
    ) -> Result<Vec<Message>, ConductorError> {
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

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
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

        let builder = self.client.post(&url).json(&body);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        Ok(())
    }

    /// Verify a thread via the REST endpoint.
    ///
    /// Calls `POST /v1/threads/{thread_id}/verify` to mark a thread as verified.
    /// The endpoint may return 404 if not implemented by the backend.
    ///
    /// # Returns
    /// - `Ok(true)` if the thread was successfully verified
    /// - `Ok(false)` if the response indicates verification failed
    /// - `Err(ConductorError::NotImplemented)` if the endpoint returns 404
    /// - `Err(ConductorError::ServerError)` for other errors
    pub async fn verify_thread(&self, thread_id: &str) -> Result<bool, ConductorError> {
        let url = format!("{}/v1/threads/{}/verify", self.base_url, thread_id);

        let builder = self.client.post(&url);
        let response = self.add_auth_header(builder).send().await?;

        let status = response.status();

        if status.as_u16() == 404 {
            return Err(ConductorError::NotImplemented(format!(
                "/v1/threads/{}/verify",
                thread_id
            )));
        }

        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError {
                status: status.as_u16(),
                message,
            });
        }

        // Parse the response to check if verified
        let body: serde_json::Value = response.json().await?;
        let verified = body
            .get("verified")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(verified)
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
        crate::sse::SseEvent::ThreadInfo {
            thread_id,
            title: _,
        } => {
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
        crate::sse::SseEvent::ToolCallStart {
            tool_name,
            tool_call_id,
        } => SseEvent::ToolCallStart(crate::events::ToolCallStartEvent {
            tool_name,
            tool_call_id,
        }),
        crate::sse::SseEvent::ToolCallArgument {
            tool_call_id,
            chunk,
        } => SseEvent::ToolCallArgument(crate::events::ToolCallArgumentEvent {
            tool_call_id,
            chunk,
        }),
        crate::sse::SseEvent::ToolExecuting {
            tool_call_id,
            display_name,
            url,
        } => SseEvent::ToolExecuting(crate::events::ToolExecutingEvent {
            tool_call_id,
            display_name,
            url,
        }),
        crate::sse::SseEvent::ToolResult {
            tool_call_id,
            result,
        } => SseEvent::ToolResult(crate::events::ToolResultEvent {
            tool_call_id,
            result,
        }),
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
            let todo_items: Vec<crate::events::TodoItem> =
                serde_json::from_value(todos).unwrap_or_default();
            SseEvent::TodosUpdated(crate::events::TodosUpdatedEvent { todos: todo_items })
        }
        crate::sse::SseEvent::SubagentStarted {
            task_id,
            description,
            subagent_type,
        } => SseEvent::SubagentStarted(crate::events::SubagentStartedEvent {
            task_id,
            description,
            subagent_type,
        }),
        crate::sse::SseEvent::SubagentProgress { task_id, message } => {
            SseEvent::SubagentProgress(crate::events::SubagentProgressEvent { task_id, message })
        }
        crate::sse::SseEvent::SubagentCompleted {
            task_id,
            summary,
            tool_call_count,
        } => SseEvent::SubagentCompleted(crate::events::SubagentCompletedEvent {
            task_id,
            summary,
            tool_call_count,
        }),
        crate::sse::SseEvent::ThreadUpdated {
            thread_id,
            title,
            description,
        } => SseEvent::ThreadUpdated(crate::events::ThreadUpdatedEvent {
            thread_id,
            title,
            description,
        }),
        crate::sse::SseEvent::Usage {
            context_window_used,
            context_window_limit,
        } => SseEvent::Usage(crate::events::UsageEvent {
            context_window_used,
            context_window_limit,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StreamRequest;

    #[test]
    fn test_conductor_client_new() {
        let client = ConductorClient::new();
        assert_eq!(client.base_url, DEFAULT_CONDUCTOR_URL);
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
        assert_eq!(client.base_url, DEFAULT_CONDUCTOR_URL);
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

    #[test]
    fn test_convert_sse_event_thread_updated() {
        let sse_event = crate::sse::SseEvent::ThreadUpdated {
            thread_id: "thread-123".to_string(),
            title: Some("New Title".to_string()),
            description: Some("New Description".to_string()),
        };
        let event = convert_sse_event(sse_event);
        match event {
            SseEvent::ThreadUpdated(thread_updated) => {
                assert_eq!(thread_updated.thread_id, "thread-123");
                assert_eq!(thread_updated.title, Some("New Title".to_string()));
                assert_eq!(
                    thread_updated.description,
                    Some("New Description".to_string())
                );
            }
            _ => panic!("Expected ThreadUpdated event"),
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

    #[test]
    fn test_conductor_client_with_url() {
        let client = ConductorClient::with_url("http://custom.example.com:9000");
        assert_eq!(client.base_url, "http://custom.example.com:9000");
        assert!(client.auth_token().is_none());
    }

    #[test]
    fn test_conductor_client_with_auth() {
        let client = ConductorClient::new().with_auth("my-secret-token");
        assert_eq!(client.base_url, DEFAULT_CONDUCTOR_URL);
        assert_eq!(client.auth_token(), Some("my-secret-token"));
    }

    #[test]
    fn test_conductor_client_with_url_and_auth() {
        let client = ConductorClient::with_url("http://localhost:3000").with_auth("test-token");
        assert_eq!(client.base_url, "http://localhost:3000");
        assert_eq!(client.auth_token(), Some("test-token"));
    }

    #[test]
    fn test_conductor_client_set_auth_token() {
        let mut client = ConductorClient::new();
        assert!(client.auth_token().is_none());

        client.set_auth_token(Some("new-token".to_string()));
        assert_eq!(client.auth_token(), Some("new-token"));

        client.set_auth_token(None);
        assert!(client.auth_token().is_none());
    }

    #[test]
    fn test_conductor_client_no_auth_by_default() {
        let client = ConductorClient::new();
        assert!(client.auth_token().is_none());

        let client2 = ConductorClient::with_base_url("http://example.com".to_string());
        assert!(client2.auth_token().is_none());

        let client3 = ConductorClient::default();
        assert!(client3.auth_token().is_none());
    }
}
