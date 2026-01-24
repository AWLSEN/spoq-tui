//! Unified error type for the Spoq application.
//!
//! This module defines the main `SpoqError` enum that unifies all error types
//! in the application, providing consistent error handling, categorization,
//! and user messaging.

use std::fmt;

use super::auth::AuthError;
use super::category::ErrorCategory;
use super::context::ErrorContext;
use super::network::NetworkError;
use super::stream::StreamError;
use super::system::SystemError;
use super::ui::UiError;

/// Unified error type for the Spoq application.
///
/// `SpoqError` consolidates all domain-specific error types into a single
/// enum, enabling:
/// - Consistent error handling across the application
/// - Uniform categorization and retry logic
/// - User-friendly error messages
/// - Optional context attachment for debugging
#[derive(Debug)]
pub enum SpoqError {
    /// Network-related errors (connections, HTTP, timeouts).
    Network(NetworkError),

    /// Authentication/authorization errors.
    Auth(AuthError),

    /// Stream/SSE processing errors.
    Stream(StreamError),

    /// UI/terminal errors.
    Ui(UiError),

    /// System/filesystem errors.
    System(SystemError),

    /// Wrapped error with additional context.
    WithContext {
        error: Box<SpoqError>,
        context: ErrorContext,
    },
}

impl SpoqError {
    /// Get the category of this error.
    pub fn category(&self) -> ErrorCategory {
        match self {
            SpoqError::Network(_) => ErrorCategory::Network,
            SpoqError::Auth(err) => {
                // Some auth errors are user-actionable
                if err.requires_reauth() {
                    ErrorCategory::Auth
                } else {
                    ErrorCategory::User
                }
            }
            SpoqError::Stream(err) => {
                // Classify stream errors based on their nature
                match err {
                    StreamError::ConnectionLost { .. } | StreamError::Timeout { .. } => {
                        ErrorCategory::Network
                    }
                    StreamError::BackendError { .. } | StreamError::ServerClosed { .. } => {
                        ErrorCategory::Server
                    }
                    StreamError::PermissionDenied { .. }
                    | StreamError::PermissionTimeout { .. } => ErrorCategory::User,
                    StreamError::ParseError { .. }
                    | StreamError::InvalidJson { .. }
                    | StreamError::UnknownEventType { .. } => ErrorCategory::Client,
                    _ => ErrorCategory::Server,
                }
            }
            SpoqError::Ui(err) => {
                if err.is_recoverable() {
                    ErrorCategory::User
                } else {
                    ErrorCategory::System
                }
            }
            SpoqError::System(_) => ErrorCategory::System,
            SpoqError::WithContext { error, .. } => error.category(),
        }
    }

    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            SpoqError::Network(err) => err.is_retryable(),
            SpoqError::Auth(err) => err.is_recoverable(),
            SpoqError::Stream(err) => err.is_retryable(),
            SpoqError::Ui(_) => false,
            SpoqError::System(err) => err.is_transient(),
            SpoqError::WithContext { error, .. } => error.is_retryable(),
        }
    }

    /// Get a user-friendly error message.
    pub fn user_message(&self) -> String {
        match self {
            SpoqError::Network(err) => err.user_message(),
            SpoqError::Auth(err) => err.user_message(),
            SpoqError::Stream(err) => err.user_message(),
            SpoqError::Ui(err) => err.user_message(),
            SpoqError::System(err) => err.user_message(),
            SpoqError::WithContext { error, context } => {
                format!("{}\n\nContext: {}", error.user_message(), context)
            }
        }
    }

    /// Get a short error code for logging.
    pub fn error_code(&self) -> &'static str {
        match self {
            SpoqError::Network(err) => err.error_code(),
            SpoqError::Auth(err) => err.error_code(),
            SpoqError::Stream(err) => err.error_code(),
            SpoqError::Ui(err) => err.error_code(),
            SpoqError::System(err) => err.error_code(),
            SpoqError::WithContext { error, .. } => error.error_code(),
        }
    }

    /// Attach context to this error.
    pub fn with_context(self, ctx: ErrorContext) -> Self {
        SpoqError::WithContext {
            error: Box::new(self),
            context: ctx,
        }
    }

    /// Get the context if this error has one attached.
    pub fn context(&self) -> Option<&ErrorContext> {
        match self {
            SpoqError::WithContext { context, .. } => Some(context),
            _ => None,
        }
    }

    /// Get the inner error without context.
    pub fn inner(&self) -> &SpoqError {
        match self {
            SpoqError::WithContext { error, .. } => error.inner(),
            _ => self,
        }
    }

    /// Get the recovery hint for this error.
    pub fn recovery_hint(&self) -> &'static str {
        self.category().recovery_hint()
    }

    /// Check if this error requires re-authentication.
    pub fn requires_reauth(&self) -> bool {
        match self {
            SpoqError::Auth(err) => err.requires_reauth(),
            SpoqError::Network(NetworkError::HttpStatus { status: 401, .. }) => true,
            SpoqError::WithContext { error, .. } => error.requires_reauth(),
            _ => false,
        }
    }
}

impl fmt::Display for SpoqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpoqError::Network(err) => write!(f, "{}", err),
            SpoqError::Auth(err) => write!(f, "{}", err),
            SpoqError::Stream(err) => write!(f, "{}", err),
            SpoqError::Ui(err) => write!(f, "{}", err),
            SpoqError::System(err) => write!(f, "{}", err),
            SpoqError::WithContext { error, context } => {
                write!(f, "{} ({})", error, context)
            }
        }
    }
}

impl std::error::Error for SpoqError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SpoqError::Network(err) => Some(err),
            SpoqError::Auth(err) => Some(err),
            SpoqError::Stream(err) => Some(err),
            SpoqError::Ui(err) => Some(err),
            SpoqError::System(err) => Some(err),
            SpoqError::WithContext { error, .. } => error.source(),
        }
    }
}

// ============================================================================
// From implementations for automatic error conversion
// ============================================================================

impl From<NetworkError> for SpoqError {
    fn from(err: NetworkError) -> Self {
        SpoqError::Network(err)
    }
}

impl From<AuthError> for SpoqError {
    fn from(err: AuthError) -> Self {
        SpoqError::Auth(err)
    }
}

impl From<StreamError> for SpoqError {
    fn from(err: StreamError) -> Self {
        SpoqError::Stream(err)
    }
}

impl From<UiError> for SpoqError {
    fn from(err: UiError) -> Self {
        SpoqError::Ui(err)
    }
}

impl From<SystemError> for SpoqError {
    fn from(err: SystemError) -> Self {
        SpoqError::System(err)
    }
}

// ============================================================================
// From implementations for external error types
// ============================================================================

impl From<std::io::Error> for SpoqError {
    fn from(err: std::io::Error) -> Self {
        use super::system::classify_io_error;
        SpoqError::System(classify_io_error(err, None, "I/O operation"))
    }
}

impl From<serde_json::Error> for SpoqError {
    fn from(err: serde_json::Error) -> Self {
        SpoqError::Stream(StreamError::InvalidJson {
            event_type: "unknown".to_string(),
            message: err.to_string(),
        })
    }
}

impl From<reqwest::Error> for SpoqError {
    fn from(err: reqwest::Error) -> Self {
        let url = err
            .url()
            .map(|u| u.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        SpoqError::Network(super::network::classify_reqwest_error(&err, &url))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::path::PathBuf;

    #[test]
    fn test_network_error_category() {
        let err = SpoqError::Network(NetworkError::ConnectionFailed {
            url: "https://example.com".to_string(),
            message: "refused".to_string(),
        });
        assert_eq!(err.category(), ErrorCategory::Network);
    }

    #[test]
    fn test_auth_error_category() {
        let err = SpoqError::Auth(AuthError::TokenExpired);
        assert_eq!(err.category(), ErrorCategory::Auth);

        let err_denied = SpoqError::Auth(AuthError::AccessDenied { resource: None });
        assert_eq!(err_denied.category(), ErrorCategory::User);
    }

    #[test]
    fn test_stream_error_category() {
        let err_conn = SpoqError::Stream(StreamError::ConnectionLost {
            message: "lost".to_string(),
        });
        assert_eq!(err_conn.category(), ErrorCategory::Network);

        let err_backend = SpoqError::Stream(StreamError::BackendError {
            code: None,
            message: "error".to_string(),
        });
        assert_eq!(err_backend.category(), ErrorCategory::Server);

        let err_perm = SpoqError::Stream(StreamError::PermissionDenied {
            tool_name: "Bash".to_string(),
            permission_id: "perm-1".to_string(),
        });
        assert_eq!(err_perm.category(), ErrorCategory::User);

        let err_parse = SpoqError::Stream(StreamError::ParseError {
            event_type: "content".to_string(),
            message: "invalid".to_string(),
        });
        assert_eq!(err_parse.category(), ErrorCategory::Client);
    }

    #[test]
    fn test_ui_error_category() {
        let err_recoverable = SpoqError::Ui(UiError::RenderFailed {
            component: "list".to_string(),
            message: "oops".to_string(),
        });
        assert_eq!(err_recoverable.category(), ErrorCategory::User);

        let err_fatal = SpoqError::Ui(UiError::TerminalInitFailed {
            message: "no tty".to_string(),
        });
        assert_eq!(err_fatal.category(), ErrorCategory::System);
    }

    #[test]
    fn test_system_error_category() {
        let err = SpoqError::System(SystemError::FileNotFound {
            path: PathBuf::from("/tmp/test"),
        });
        assert_eq!(err.category(), ErrorCategory::System);
    }

    #[test]
    fn test_is_retryable() {
        let net_err = SpoqError::Network(NetworkError::Timeout {
            operation: "connect".to_string(),
            duration_secs: 30,
        });
        assert!(net_err.is_retryable());

        let auth_err = SpoqError::Auth(AuthError::AccessDenied { resource: None });
        assert!(!auth_err.is_retryable());

        let stream_err = SpoqError::Stream(StreamError::ConnectionLost {
            message: "lost".to_string(),
        });
        assert!(stream_err.is_retryable());

        let ui_err = SpoqError::Ui(UiError::RenderFailed {
            component: "test".to_string(),
            message: "failed".to_string(),
        });
        assert!(!ui_err.is_retryable());
    }

    #[test]
    fn test_with_context() {
        let err = SpoqError::Network(NetworkError::Timeout {
            operation: "request".to_string(),
            duration_secs: 30,
        });

        let ctx = ErrorContext::new("send_message")
            .with_thread_id("thread-123")
            .with_retry_count(2);

        let with_ctx = err.with_context(ctx);

        assert!(matches!(with_ctx, SpoqError::WithContext { .. }));
        assert!(with_ctx.context().is_some());
        assert_eq!(with_ctx.context().unwrap().operation, "send_message");
        assert_eq!(with_ctx.context().unwrap().retry_count, 2);
    }

    #[test]
    fn test_with_context_preserves_category() {
        let err = SpoqError::Network(NetworkError::Timeout {
            operation: "request".to_string(),
            duration_secs: 30,
        });

        let ctx = ErrorContext::new("test");
        let with_ctx = err.with_context(ctx);

        assert_eq!(with_ctx.category(), ErrorCategory::Network);
    }

    #[test]
    fn test_with_context_preserves_retryable() {
        let err = SpoqError::Network(NetworkError::Timeout {
            operation: "request".to_string(),
            duration_secs: 30,
        });

        let ctx = ErrorContext::new("test");
        let with_ctx = err.with_context(ctx);

        assert!(with_ctx.is_retryable());
    }

    #[test]
    fn test_inner() {
        let err = SpoqError::Network(NetworkError::Timeout {
            operation: "request".to_string(),
            duration_secs: 30,
        });

        let ctx = ErrorContext::new("test");
        let with_ctx = err.with_context(ctx);

        let inner = with_ctx.inner();
        assert!(matches!(inner, SpoqError::Network(_)));
    }

    #[test]
    fn test_requires_reauth() {
        let token_expired = SpoqError::Auth(AuthError::TokenExpired);
        assert!(token_expired.requires_reauth());

        let http_401 = SpoqError::Network(NetworkError::HttpStatus {
            status: 401,
            message: "Unauthorized".to_string(),
        });
        assert!(http_401.requires_reauth());

        let http_500 = SpoqError::Network(NetworkError::HttpStatus {
            status: 500,
            message: "Internal Server Error".to_string(),
        });
        assert!(!http_500.requires_reauth());
    }

    #[test]
    fn test_user_message() {
        let err = SpoqError::Network(NetworkError::Timeout {
            operation: "connect".to_string(),
            duration_secs: 30,
        });
        let msg = err.user_message();
        assert!(msg.contains("timed out"));
        assert!(msg.contains("30 seconds"));
    }

    #[test]
    fn test_user_message_with_context() {
        let err = SpoqError::Network(NetworkError::Timeout {
            operation: "connect".to_string(),
            duration_secs: 30,
        });

        let ctx = ErrorContext::new("send_message").with_thread_id("thread-123");

        let with_ctx = err.with_context(ctx);
        let msg = with_ctx.user_message();

        assert!(msg.contains("timed out"));
        assert!(msg.contains("send_message"));
    }

    #[test]
    fn test_error_code() {
        let err = SpoqError::Network(NetworkError::Timeout {
            operation: "test".to_string(),
            duration_secs: 30,
        });
        assert_eq!(err.error_code(), "E_NET_TIMEOUT");
    }

    #[test]
    fn test_display_format() {
        let err = SpoqError::Network(NetworkError::ConnectionFailed {
            url: "https://example.com".to_string(),
            message: "refused".to_string(),
        });
        let display = format!("{}", err);
        assert!(display.contains("example.com"));
        assert!(display.contains("refused"));
    }

    #[test]
    fn test_display_with_context() {
        let err = SpoqError::Network(NetworkError::Timeout {
            operation: "request".to_string(),
            duration_secs: 30,
        });

        let ctx = ErrorContext::new("test_op");
        let with_ctx = err.with_context(ctx);

        let display = format!("{}", with_ctx);
        assert!(display.contains("test_op"));
    }

    #[test]
    fn test_from_network_error() {
        let net_err = NetworkError::Timeout {
            operation: "test".to_string(),
            duration_secs: 30,
        };
        let spoq_err: SpoqError = net_err.into();
        assert!(matches!(spoq_err, SpoqError::Network(_)));
    }

    #[test]
    fn test_from_auth_error() {
        let auth_err = AuthError::TokenExpired;
        let spoq_err: SpoqError = auth_err.into();
        assert!(matches!(spoq_err, SpoqError::Auth(_)));
    }

    #[test]
    fn test_from_stream_error() {
        let stream_err = StreamError::SessionInvalidated;
        let spoq_err: SpoqError = stream_err.into();
        assert!(matches!(spoq_err, SpoqError::Stream(_)));
    }

    #[test]
    fn test_from_ui_error() {
        let ui_err = UiError::RenderFailed {
            component: "test".to_string(),
            message: "failed".to_string(),
        };
        let spoq_err: SpoqError = ui_err.into();
        assert!(matches!(spoq_err, SpoqError::Ui(_)));
    }

    #[test]
    fn test_from_system_error() {
        let sys_err = SystemError::NoHomeDirectory;
        let spoq_err: SpoqError = sys_err.into();
        assert!(matches!(spoq_err, SpoqError::System(_)));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let spoq_err: SpoqError = io_err.into();
        assert!(matches!(spoq_err, SpoqError::System(_)));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let spoq_err: SpoqError = json_err.into();
        assert!(matches!(spoq_err, SpoqError::Stream(_)));
    }

    #[test]
    fn test_recovery_hint() {
        let err = SpoqError::Network(NetworkError::ConnectionFailed {
            url: "https://example.com".to_string(),
            message: "refused".to_string(),
        });
        let hint = err.recovery_hint();
        assert!(hint.contains("internet"));
    }

    #[test]
    fn test_error_source() {
        let err = SpoqError::Network(NetworkError::Timeout {
            operation: "test".to_string(),
            duration_secs: 30,
        });
        let source = err.source();
        assert!(source.is_some());
    }
}
