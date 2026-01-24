//! Streaming-related error types.
//!
//! This module defines errors that occur during SSE stream processing,
//! message handling, and real-time communication.

use std::fmt;

/// Stream-specific error variants.
///
/// These errors represent issues with Server-Sent Events (SSE) streams,
/// message parsing, and real-time data handling.
#[derive(Debug, Clone)]
pub enum StreamError {
    /// Stream connection was lost unexpectedly.
    ConnectionLost {
        message: String,
    },

    /// Failed to parse SSE event.
    ParseError {
        event_type: String,
        message: String,
    },

    /// Unknown event type received.
    UnknownEventType {
        event_type: String,
    },

    /// Invalid JSON in stream data.
    InvalidJson {
        event_type: String,
        message: String,
    },

    /// Stream was closed by the server.
    ServerClosed {
        reason: Option<String>,
    },

    /// Stream timeout (no data received).
    Timeout {
        duration_secs: u64,
    },

    /// Backend reported an error via SSE.
    BackendError {
        code: Option<String>,
        message: String,
    },

    /// Permission was denied for an operation.
    PermissionDenied {
        tool_name: String,
        permission_id: String,
    },

    /// Permission request timed out.
    PermissionTimeout {
        permission_id: String,
    },

    /// Thread not found or invalid.
    ThreadNotFound {
        thread_id: String,
    },

    /// Message not found or invalid.
    MessageNotFound {
        message_id: i64,
    },

    /// Session was invalidated.
    SessionInvalidated,

    /// Generic stream error.
    Other {
        message: String,
    },
}

impl StreamError {
    /// Check if this error is likely transient and can be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            StreamError::ConnectionLost { .. }
                | StreamError::Timeout { .. }
                | StreamError::ServerClosed { .. }
        )
    }

    /// Check if the stream should be reconnected.
    pub fn should_reconnect(&self) -> bool {
        matches!(
            self,
            StreamError::ConnectionLost { .. }
                | StreamError::Timeout { .. }
                | StreamError::ServerClosed { reason: None }
        )
    }

    /// Get a user-friendly error message.
    pub fn user_message(&self) -> String {
        match self {
            StreamError::ConnectionLost { .. } => {
                "Connection to the server was lost. Attempting to reconnect...".to_string()
            }
            StreamError::ParseError { event_type, .. } => {
                format!("Failed to process server message ({}). Please try again.", event_type)
            }
            StreamError::UnknownEventType { event_type } => {
                format!("Received unknown message type: {}. Your client may need to be updated.", event_type)
            }
            StreamError::InvalidJson { .. } => {
                "Received invalid data from server. Please try again.".to_string()
            }
            StreamError::ServerClosed { reason } => {
                match reason {
                    Some(r) => format!("Server closed the connection: {}", r),
                    None => "Server closed the connection.".to_string(),
                }
            }
            StreamError::Timeout { duration_secs } => {
                format!(
                    "No response from server for {} seconds. The connection may have been lost.",
                    duration_secs
                )
            }
            StreamError::BackendError { message, .. } => {
                format!("Server error: {}", message)
            }
            StreamError::PermissionDenied { tool_name, .. } => {
                format!("Permission denied for tool '{}'.", tool_name)
            }
            StreamError::PermissionTimeout { .. } => {
                "Permission request timed out. Please try again.".to_string()
            }
            StreamError::ThreadNotFound { thread_id } => {
                format!("Thread '{}' was not found.", thread_id)
            }
            StreamError::MessageNotFound { message_id } => {
                format!("Message {} was not found.", message_id)
            }
            StreamError::SessionInvalidated => {
                "Your session was invalidated. Please start a new conversation.".to_string()
            }
            StreamError::Other { message } => {
                format!("Stream error: {}", message)
            }
        }
    }

    /// Get a short error code for logging.
    pub fn error_code(&self) -> &'static str {
        match self {
            StreamError::ConnectionLost { .. } => "E_STREAM_CONN",
            StreamError::ParseError { .. } => "E_STREAM_PARSE",
            StreamError::UnknownEventType { .. } => "E_STREAM_UNKNOWN",
            StreamError::InvalidJson { .. } => "E_STREAM_JSON",
            StreamError::ServerClosed { .. } => "E_STREAM_CLOSED",
            StreamError::Timeout { .. } => "E_STREAM_TIMEOUT",
            StreamError::BackendError { .. } => "E_STREAM_BACKEND",
            StreamError::PermissionDenied { .. } => "E_STREAM_PERM",
            StreamError::PermissionTimeout { .. } => "E_STREAM_PERM_TO",
            StreamError::ThreadNotFound { .. } => "E_STREAM_THREAD",
            StreamError::MessageNotFound { .. } => "E_STREAM_MSG",
            StreamError::SessionInvalidated => "E_STREAM_SESSION",
            StreamError::Other { .. } => "E_STREAM_OTHER",
        }
    }
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamError::ConnectionLost { message } => {
                write!(f, "Stream connection lost: {}", message)
            }
            StreamError::ParseError { event_type, message } => {
                write!(f, "Failed to parse {} event: {}", event_type, message)
            }
            StreamError::UnknownEventType { event_type } => {
                write!(f, "Unknown event type: {}", event_type)
            }
            StreamError::InvalidJson { event_type, message } => {
                write!(f, "Invalid JSON for {} event: {}", event_type, message)
            }
            StreamError::ServerClosed { reason } => {
                match reason {
                    Some(r) => write!(f, "Server closed stream: {}", r),
                    None => write!(f, "Server closed stream"),
                }
            }
            StreamError::Timeout { duration_secs } => {
                write!(f, "Stream timeout after {} seconds", duration_secs)
            }
            StreamError::BackendError { code, message } => {
                match code {
                    Some(c) => write!(f, "Backend error [{}]: {}", c, message),
                    None => write!(f, "Backend error: {}", message),
                }
            }
            StreamError::PermissionDenied { tool_name, permission_id } => {
                write!(f, "Permission denied for '{}' ({})", tool_name, permission_id)
            }
            StreamError::PermissionTimeout { permission_id } => {
                write!(f, "Permission timeout for {}", permission_id)
            }
            StreamError::ThreadNotFound { thread_id } => {
                write!(f, "Thread not found: {}", thread_id)
            }
            StreamError::MessageNotFound { message_id } => {
                write!(f, "Message not found: {}", message_id)
            }
            StreamError::SessionInvalidated => {
                write!(f, "Session invalidated")
            }
            StreamError::Other { message } => {
                write!(f, "Stream error: {}", message)
            }
        }
    }
}

impl std::error::Error for StreamError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_lost_is_retryable() {
        let err = StreamError::ConnectionLost {
            message: "socket closed".to_string(),
        };
        assert!(err.is_retryable());
        assert!(err.should_reconnect());
        assert_eq!(err.error_code(), "E_STREAM_CONN");
    }

    #[test]
    fn test_timeout_is_retryable() {
        let err = StreamError::Timeout { duration_secs: 30 };
        assert!(err.is_retryable());
        assert!(err.should_reconnect());
        assert_eq!(err.error_code(), "E_STREAM_TIMEOUT");
    }

    #[test]
    fn test_server_closed_without_reason_should_reconnect() {
        let err = StreamError::ServerClosed { reason: None };
        assert!(err.is_retryable());
        assert!(err.should_reconnect());
        assert_eq!(err.error_code(), "E_STREAM_CLOSED");
    }

    #[test]
    fn test_server_closed_with_reason_should_not_reconnect() {
        let err = StreamError::ServerClosed {
            reason: Some("session ended".to_string()),
        };
        assert!(err.is_retryable());
        assert!(!err.should_reconnect());
    }

    #[test]
    fn test_parse_error_not_retryable() {
        let err = StreamError::ParseError {
            event_type: "content".to_string(),
            message: "unexpected EOF".to_string(),
        };
        assert!(!err.is_retryable());
        assert!(!err.should_reconnect());
        assert_eq!(err.error_code(), "E_STREAM_PARSE");
    }

    #[test]
    fn test_unknown_event_type_not_retryable() {
        let err = StreamError::UnknownEventType {
            event_type: "future_event".to_string(),
        };
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_STREAM_UNKNOWN");
    }

    #[test]
    fn test_backend_error() {
        let err = StreamError::BackendError {
            code: Some("rate_limit".to_string()),
            message: "Too many requests".to_string(),
        };
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_STREAM_BACKEND");
        assert!(err.user_message().contains("Too many requests"));
    }

    #[test]
    fn test_permission_denied() {
        let err = StreamError::PermissionDenied {
            tool_name: "Bash".to_string(),
            permission_id: "perm-123".to_string(),
        };
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_STREAM_PERM");
        assert!(err.user_message().contains("Bash"));
    }

    #[test]
    fn test_permission_timeout() {
        let err = StreamError::PermissionTimeout {
            permission_id: "perm-456".to_string(),
        };
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_STREAM_PERM_TO");
    }

    #[test]
    fn test_thread_not_found() {
        let err = StreamError::ThreadNotFound {
            thread_id: "thread-abc".to_string(),
        };
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_STREAM_THREAD");
        assert!(err.user_message().contains("thread-abc"));
    }

    #[test]
    fn test_message_not_found() {
        let err = StreamError::MessageNotFound { message_id: 42 };
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_STREAM_MSG");
        assert!(err.user_message().contains("42"));
    }

    #[test]
    fn test_session_invalidated() {
        let err = StreamError::SessionInvalidated;
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_STREAM_SESSION");
    }

    #[test]
    fn test_display_format() {
        let err = StreamError::BackendError {
            code: Some("E001".to_string()),
            message: "Operation failed".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("E001"));
        assert!(display.contains("Operation failed"));
    }

    #[test]
    fn test_user_message_formats() {
        let err_timeout = StreamError::Timeout { duration_secs: 60 };
        assert!(err_timeout.user_message().contains("60 seconds"));

        let err_unknown = StreamError::UnknownEventType {
            event_type: "new_event".to_string(),
        };
        assert!(err_unknown.user_message().contains("new_event"));
    }
}
