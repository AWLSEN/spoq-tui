//! Unified error handling architecture for Spoq.
//!
//! This module provides a comprehensive error handling system with:
//!
//! - **Error Categories**: High-level classification for handling decisions
//! - **Domain-specific Errors**: Network, Auth, Stream, UI, and System errors
//! - **Unified Error Type**: `SpoqError` consolidates all error types
//! - **Error Context**: Rich debugging information attached to errors
//! - **Result Type Alias**: `SpoqResult<T>` for consistent return types
//!
//! # Example
//!
//! ```ignore
//! use spoq::error::{SpoqResult, SpoqError, ErrorContext, ResultExt};
//!
//! fn send_message(content: &str, thread_id: &str) -> SpoqResult<Message> {
//!     make_request(content)
//!         .context(ErrorContext::new("send_message")
//!             .with_thread_id(thread_id))
//! }
//!
//! // Handle the error
//! match send_message("Hello", "thread-123") {
//!     Ok(msg) => println!("Sent: {}", msg.id),
//!     Err(err) => {
//!         eprintln!("Error: {}", err.user_message());
//!         if err.is_retryable() {
//!             eprintln!("Hint: {}", err.recovery_hint());
//!         }
//!     }
//! }
//! ```
//!
//! # Error Categories
//!
//! Errors are categorized to enable consistent handling:
//!
//! | Category | Description | Retryable |
//! |----------|-------------|-----------|
//! | Network | Connection, DNS, timeout | Yes |
//! | Auth | Authentication issues | Sometimes |
//! | Server | Backend errors (5xx) | Yes |
//! | Client | Programming errors | No |
//! | User | User action required | No |
//! | System | OS/filesystem errors | Sometimes |
//! | Configuration | Config issues | No |

mod auth;
mod category;
mod context;
mod network;
mod result;
mod spoq_error;
mod stream;
mod system;
mod ui;

// Re-export all public types
pub use auth::AuthError;
pub use category::ErrorCategory;
pub use context::ErrorContext;
pub use network::{classify_reqwest_error, NetworkError};
pub use result::{ResultExt, SpoqResult};
pub use spoq_error::SpoqError;
pub use stream::StreamError;
pub use system::{classify_io_error, SystemError};
pub use ui::UiError;

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Test that errors can be converted and handled through the unified system.
    #[test]
    fn test_error_unification() {
        // Create various error types
        let net_err: SpoqError = NetworkError::Timeout {
            operation: "test".to_string(),
            duration_secs: 30,
        }
        .into();

        let auth_err: SpoqError = AuthError::TokenExpired.into();

        let stream_err: SpoqError = StreamError::ConnectionLost {
            message: "lost".to_string(),
        }
        .into();

        let ui_err: SpoqError = UiError::RenderFailed {
            component: "test".to_string(),
            message: "failed".to_string(),
        }
        .into();

        let sys_err: SpoqError = SystemError::NoHomeDirectory.into();

        // All can be categorized
        assert_eq!(net_err.category(), ErrorCategory::Network);
        assert_eq!(auth_err.category(), ErrorCategory::Auth);
        assert_eq!(stream_err.category(), ErrorCategory::Network);
        assert_eq!(ui_err.category(), ErrorCategory::User);
        assert_eq!(sys_err.category(), ErrorCategory::System);

        // All have error codes
        assert!(!net_err.error_code().is_empty());
        assert!(!auth_err.error_code().is_empty());
        assert!(!stream_err.error_code().is_empty());
        assert!(!ui_err.error_code().is_empty());
        assert!(!sys_err.error_code().is_empty());

        // All have user messages
        assert!(!net_err.user_message().is_empty());
        assert!(!auth_err.user_message().is_empty());
        assert!(!stream_err.user_message().is_empty());
        assert!(!ui_err.user_message().is_empty());
        assert!(!sys_err.user_message().is_empty());
    }

    /// Test context propagation through the error chain.
    #[test]
    fn test_context_propagation() {
        let err: SpoqError = NetworkError::Timeout {
            operation: "connect".to_string(),
            duration_secs: 30,
        }
        .into();

        let ctx = ErrorContext::new("send_message")
            .with_thread_id("thread-123")
            .with_retry_count(2)
            .with_component("api_client");

        let with_ctx = err.with_context(ctx);

        // Context should be accessible
        assert!(with_ctx.context().is_some());
        let ctx = with_ctx.context().unwrap();
        assert_eq!(ctx.operation, "send_message");
        assert_eq!(ctx.thread_id, Some("thread-123".to_string()));
        assert_eq!(ctx.retry_count, 2);
        assert_eq!(ctx.component, Some("api_client".to_string()));

        // Original error properties should still work
        assert_eq!(with_ctx.category(), ErrorCategory::Network);
        assert!(with_ctx.is_retryable());
        assert!(!with_ctx.error_code().is_empty());
    }

    /// Test the ResultExt trait for adding context to Results.
    #[test]
    fn test_result_ext() {
        fn might_fail() -> SpoqResult<i32> {
            Err(NetworkError::Cancelled.into())
        }

        let result = might_fail().context(ErrorContext::new("test_operation"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.context().is_some());
        assert_eq!(err.context().unwrap().operation, "test_operation");
    }

    /// Test that std::io::Error converts correctly.
    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let spoq_err: SpoqError = io_err.into();

        assert_eq!(spoq_err.category(), ErrorCategory::System);
        assert!(matches!(spoq_err, SpoqError::System(_)));
    }

    /// Test that serde_json::Error converts correctly.
    #[test]
    fn test_json_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let spoq_err: SpoqError = json_err.into();

        // JSON parse errors are stream errors (parsing SSE data)
        assert!(matches!(spoq_err, SpoqError::Stream(_)));
    }

    /// Test retry logic based on error type.
    #[test]
    fn test_retry_logic() {
        let retryable_errors: Vec<SpoqError> = vec![
            NetworkError::Timeout {
                operation: "test".to_string(),
                duration_secs: 30,
            }
            .into(),
            NetworkError::ConnectionFailed {
                url: "test".to_string(),
                message: "test".to_string(),
            }
            .into(),
            StreamError::ConnectionLost {
                message: "test".to_string(),
            }
            .into(),
        ];

        for err in retryable_errors {
            assert!(err.is_retryable(), "Expected {:?} to be retryable", err);
        }

        let non_retryable_errors: Vec<SpoqError> = vec![
            AuthError::AccessDenied { resource: None }.into(),
            UiError::RenderFailed {
                component: "test".to_string(),
                message: "test".to_string(),
            }
            .into(),
            SystemError::FileNotFound {
                path: std::path::PathBuf::from("/test"),
            }
            .into(),
        ];

        for err in non_retryable_errors {
            assert!(
                !err.is_retryable(),
                "Expected {:?} to not be retryable",
                err
            );
        }
    }

    /// Test reauth detection.
    #[test]
    fn test_reauth_detection() {
        let reauth_errors: Vec<SpoqError> = vec![
            AuthError::TokenExpired.into(),
            AuthError::RefreshTokenInvalid {
                message: "expired".to_string(),
            }
            .into(),
            AuthError::NotAuthenticated.into(),
            NetworkError::HttpStatus {
                status: 401,
                message: "Unauthorized".to_string(),
            }
            .into(),
        ];

        for err in reauth_errors {
            assert!(err.requires_reauth(), "Expected {:?} to require reauth", err);
        }

        let no_reauth_errors: Vec<SpoqError> = vec![
            AuthError::AccessDenied { resource: None }.into(),
            NetworkError::HttpStatus {
                status: 403,
                message: "Forbidden".to_string(),
            }
            .into(),
            NetworkError::HttpStatus {
                status: 500,
                message: "Server Error".to_string(),
            }
            .into(),
        ];

        for err in no_reauth_errors {
            assert!(
                !err.requires_reauth(),
                "Expected {:?} to not require reauth",
                err
            );
        }
    }
}
