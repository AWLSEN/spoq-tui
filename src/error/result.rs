//! Result type alias for Spoq operations.
//!
//! This module provides a convenient type alias for Result types that use
//! SpoqError as the error type.

use super::spoq_error::SpoqError;

/// Type alias for Results using SpoqError.
///
/// Use this type for functions that can fail with any Spoq-related error.
///
/// # Example
///
/// ```ignore
/// use spoq::error::SpoqResult;
///
/// fn send_message(content: &str) -> SpoqResult<Message> {
///     // Implementation that may return various error types
///     Ok(message)
/// }
/// ```
pub type SpoqResult<T> = Result<T, SpoqError>;

/// Extension trait for Result types to add context to errors.
pub trait ResultExt<T> {
    /// Add context to an error if the result is Err.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use spoq::error::{ErrorContext, ResultExt};
    ///
    /// let result = send_message()
    ///     .context(ErrorContext::new("send_message")
    ///         .with_thread_id("thread-123"));
    /// ```
    fn context(self, ctx: super::context::ErrorContext) -> SpoqResult<T>;

    /// Add context using a closure (only called on error).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use spoq::error::{ErrorContext, ResultExt};
    ///
    /// let result = send_message()
    ///     .with_context(|| ErrorContext::new("send_message")
    ///         .with_thread_id(&thread_id));
    /// ```
    fn with_context<F>(self, f: F) -> SpoqResult<T>
    where
        F: FnOnce() -> super::context::ErrorContext;
}

impl<T> ResultExt<T> for SpoqResult<T> {
    fn context(self, ctx: super::context::ErrorContext) -> SpoqResult<T> {
        self.map_err(|e| e.with_context(ctx))
    }

    fn with_context<F>(self, f: F) -> SpoqResult<T>
    where
        F: FnOnce() -> super::context::ErrorContext,
    {
        self.map_err(|e| e.with_context(f()))
    }
}

impl<T> ResultExt<T> for Result<T, std::io::Error> {
    fn context(self, ctx: super::context::ErrorContext) -> SpoqResult<T> {
        self.map_err(|e| {
            let spoq_err: SpoqError = e.into();
            spoq_err.with_context(ctx)
        })
    }

    fn with_context<F>(self, f: F) -> SpoqResult<T>
    where
        F: FnOnce() -> super::context::ErrorContext,
    {
        self.map_err(|e| {
            let spoq_err: SpoqError = e.into();
            spoq_err.with_context(f())
        })
    }
}

impl<T> ResultExt<T> for Result<T, serde_json::Error> {
    fn context(self, ctx: super::context::ErrorContext) -> SpoqResult<T> {
        self.map_err(|e| {
            let spoq_err: SpoqError = e.into();
            spoq_err.with_context(ctx)
        })
    }

    fn with_context<F>(self, f: F) -> SpoqResult<T>
    where
        F: FnOnce() -> super::context::ErrorContext,
    {
        self.map_err(|e| {
            let spoq_err: SpoqError = e.into();
            spoq_err.with_context(f())
        })
    }
}

impl<T> ResultExt<T> for Result<T, reqwest::Error> {
    fn context(self, ctx: super::context::ErrorContext) -> SpoqResult<T> {
        self.map_err(|e| {
            let spoq_err: SpoqError = e.into();
            spoq_err.with_context(ctx)
        })
    }

    fn with_context<F>(self, f: F) -> SpoqResult<T>
    where
        F: FnOnce() -> super::context::ErrorContext,
    {
        self.map_err(|e| {
            let spoq_err: SpoqError = e.into();
            spoq_err.with_context(f())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorContext, NetworkError, SpoqError};

    #[test]
    fn test_spoq_result_ok() {
        let result: SpoqResult<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_spoq_result_err() {
        let result: SpoqResult<i32> = Err(SpoqError::Network(NetworkError::Cancelled));
        assert!(result.is_err());
    }

    #[test]
    fn test_context_extension() {
        let result: SpoqResult<i32> = Err(SpoqError::Network(NetworkError::Cancelled));

        let with_ctx = result.context(ErrorContext::new("test_operation"));

        assert!(with_ctx.is_err());
        let err = with_ctx.unwrap_err();
        assert!(err.context().is_some());
        assert_eq!(err.context().unwrap().operation, "test_operation");
    }

    #[test]
    fn test_context_extension_preserves_ok() {
        let result: SpoqResult<i32> = Ok(42);

        let with_ctx = result.context(ErrorContext::new("test_operation"));

        assert!(with_ctx.is_ok());
        assert_eq!(with_ctx.unwrap(), 42);
    }

    #[test]
    fn test_with_context_lazy_evaluation() {
        let result: SpoqResult<i32> = Ok(42);
        let mut called = false;

        let with_ctx = result.with_context(|| {
            called = true;
            ErrorContext::new("test")
        });

        assert!(with_ctx.is_ok());
        assert!(!called); // Closure should not be called for Ok
    }

    #[test]
    fn test_with_context_on_error() {
        let result: SpoqResult<i32> = Err(SpoqError::Network(NetworkError::Cancelled));
        let mut called = false;

        let with_ctx = result.with_context(|| {
            called = true;
            ErrorContext::new("lazy_context")
        });

        assert!(with_ctx.is_err());
        assert!(called); // Closure should be called for Err
        let err = with_ctx.unwrap_err();
        assert!(err.context().is_some());
        assert_eq!(err.context().unwrap().operation, "lazy_context");
    }

    #[test]
    fn test_context_from_io_error() {
        let io_result: Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"));

        let with_ctx = io_result.context(ErrorContext::new("read_file"));

        assert!(with_ctx.is_err());
        let err = with_ctx.unwrap_err();
        assert!(err.context().is_some());
        assert_eq!(err.context().unwrap().operation, "read_file");
    }

    #[test]
    fn test_result_ext_chaining() {
        let result: SpoqResult<i32> = Err(SpoqError::Network(NetworkError::Timeout {
            operation: "connect".to_string(),
            duration_secs: 30,
        }));

        let with_ctx = result
            .context(ErrorContext::new("send_message").with_thread_id("thread-123"));

        let err = with_ctx.unwrap_err();
        assert_eq!(err.context().unwrap().operation, "send_message");
        assert_eq!(
            err.context().unwrap().thread_id,
            Some("thread-123".to_string())
        );
    }
}
