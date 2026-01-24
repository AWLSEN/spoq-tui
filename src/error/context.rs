//! Error context for enriched error information.
//!
//! This module provides context structures that can be attached to errors
//! to provide additional debugging and recovery information.

use chrono::{DateTime, Utc};

/// Context information attached to errors for debugging and recovery.
///
/// ErrorContext provides additional metadata about when and where an error
/// occurred, enabling better debugging and intelligent retry strategies.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorContext {
    /// Human-readable description of the operation that failed.
    pub operation: String,

    /// Thread ID if the error occurred within a specific thread context.
    pub thread_id: Option<String>,

    /// Timestamp when the error occurred.
    pub timestamp: DateTime<Utc>,

    /// Number of retry attempts made before this error.
    pub retry_count: u32,

    /// Optional component/module where the error originated.
    pub component: Option<String>,

    /// Optional correlation ID for tracing across services.
    pub correlation_id: Option<String>,
}

impl ErrorContext {
    /// Create a new ErrorContext for an operation.
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            thread_id: None,
            timestamp: Utc::now(),
            retry_count: 0,
            component: None,
            correlation_id: None,
        }
    }

    /// Set the thread ID for this context.
    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Set the retry count for this context.
    pub fn with_retry_count(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    /// Set the component for this context.
    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.component = Some(component.into());
        self
    }

    /// Set the correlation ID for this context.
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Increment the retry count and return a new context.
    pub fn next_retry(&self) -> Self {
        Self {
            operation: self.operation.clone(),
            thread_id: self.thread_id.clone(),
            timestamp: Utc::now(),
            retry_count: self.retry_count + 1,
            component: self.component.clone(),
            correlation_id: self.correlation_id.clone(),
        }
    }

    /// Check if we've exceeded the maximum retry count.
    pub fn exceeded_retries(&self, max_retries: u32) -> bool {
        self.retry_count >= max_retries
    }

    /// Get a formatted context string suitable for logging.
    pub fn to_log_string(&self) -> String {
        let mut parts = vec![format!("operation={}", self.operation)];

        if let Some(ref thread_id) = self.thread_id {
            parts.push(format!("thread_id={}", thread_id));
        }

        if let Some(ref component) = self.component {
            parts.push(format!("component={}", component));
        }

        if let Some(ref correlation_id) = self.correlation_id {
            parts.push(format!("correlation_id={}", correlation_id));
        }

        if self.retry_count > 0 {
            parts.push(format!("retry_count={}", self.retry_count));
        }

        parts.push(format!("timestamp={}", self.timestamp.to_rfc3339()));

        parts.join(" ")
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self {
            operation: "unknown".to_string(),
            thread_id: None,
            timestamp: Utc::now(),
            retry_count: 0,
            component: None,
            correlation_id: None,
        }
    }
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.operation)?;

        if let Some(ref thread_id) = self.thread_id {
            write!(f, " thread={}", thread_id)?;
        }

        if self.retry_count > 0 {
            write!(f, " retry={}", self.retry_count)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_new() {
        let ctx = ErrorContext::new("send_message");

        assert_eq!(ctx.operation, "send_message");
        assert!(ctx.thread_id.is_none());
        assert_eq!(ctx.retry_count, 0);
        assert!(ctx.component.is_none());
        assert!(ctx.correlation_id.is_none());
    }

    #[test]
    fn test_context_builder_pattern() {
        let ctx = ErrorContext::new("stream_response")
            .with_thread_id("thread-123")
            .with_retry_count(2)
            .with_component("sse_parser")
            .with_correlation_id("req-456");

        assert_eq!(ctx.operation, "stream_response");
        assert_eq!(ctx.thread_id, Some("thread-123".to_string()));
        assert_eq!(ctx.retry_count, 2);
        assert_eq!(ctx.component, Some("sse_parser".to_string()));
        assert_eq!(ctx.correlation_id, Some("req-456".to_string()));
    }

    #[test]
    fn test_context_next_retry() {
        let ctx = ErrorContext::new("connect")
            .with_thread_id("thread-abc")
            .with_retry_count(0);

        let retry1 = ctx.next_retry();
        assert_eq!(retry1.retry_count, 1);
        assert_eq!(retry1.operation, "connect");
        assert_eq!(retry1.thread_id, Some("thread-abc".to_string()));

        let retry2 = retry1.next_retry();
        assert_eq!(retry2.retry_count, 2);

        let retry3 = retry2.next_retry();
        assert_eq!(retry3.retry_count, 3);
    }

    #[test]
    fn test_context_exceeded_retries() {
        let ctx = ErrorContext::new("connect").with_retry_count(3);

        assert!(!ctx.exceeded_retries(5));
        assert!(ctx.exceeded_retries(3));
        assert!(ctx.exceeded_retries(2));
    }

    #[test]
    fn test_context_display() {
        let ctx = ErrorContext::new("fetch_messages")
            .with_thread_id("thread-xyz")
            .with_retry_count(1);

        let display = format!("{}", ctx);
        assert!(display.contains("fetch_messages"));
        assert!(display.contains("thread=thread-xyz"));
        assert!(display.contains("retry=1"));
    }

    #[test]
    fn test_context_display_minimal() {
        let ctx = ErrorContext::new("simple_op");

        let display = format!("{}", ctx);
        assert!(display.contains("simple_op"));
        assert!(!display.contains("thread="));
        assert!(!display.contains("retry="));
    }

    #[test]
    fn test_context_to_log_string() {
        let ctx = ErrorContext::new("authenticate")
            .with_component("auth")
            .with_correlation_id("corr-123")
            .with_retry_count(2);

        let log_str = ctx.to_log_string();
        assert!(log_str.contains("operation=authenticate"));
        assert!(log_str.contains("component=auth"));
        assert!(log_str.contains("correlation_id=corr-123"));
        assert!(log_str.contains("retry_count=2"));
        assert!(log_str.contains("timestamp="));
    }

    #[test]
    fn test_context_default() {
        let ctx = ErrorContext::default();

        assert_eq!(ctx.operation, "unknown");
        assert!(ctx.thread_id.is_none());
        assert_eq!(ctx.retry_count, 0);
    }

    #[test]
    fn test_context_clone_and_eq() {
        let ctx1 = ErrorContext::new("test_op")
            .with_thread_id("thread-1")
            .with_retry_count(1);

        let ctx2 = ctx1.clone();
        assert_eq!(ctx1.operation, ctx2.operation);
        assert_eq!(ctx1.thread_id, ctx2.thread_id);
        assert_eq!(ctx1.retry_count, ctx2.retry_count);
    }

    #[test]
    fn test_context_timestamp_is_recent() {
        let before = Utc::now();
        let ctx = ErrorContext::new("timed_op");
        let after = Utc::now();

        assert!(ctx.timestamp >= before);
        assert!(ctx.timestamp <= after);
    }
}
