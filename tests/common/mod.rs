//! Common test utilities for integration tests.
//!
//! This module provides reusable test fixtures, mock configurations,
//! and helper functions for integration testing the TUI application.
//!
//! # Example
//!
//! ```ignore
//! use spoq_tests::common::{TestAppBuilder, test_credentials};
//!
//! let app = TestAppBuilder::new()
//!     .with_credentials(test_credentials())
//!     .build();
//! ```

pub mod mocks;

pub use mocks::*;

use spoq::app::App;
use spoq::auth::credentials::Credentials;

/// Creates test credentials for use in tests.
///
/// Returns credentials with test tokens that won't expire during test execution.
pub fn test_credentials() -> Credentials {
    Credentials {
        access_token: Some("test-access-token-12345".to_string()),
        refresh_token: Some("test-refresh-token-67890".to_string()),
        expires_at: Some(i64::MAX), // Never expires in tests
        user_id: Some("test-user-id".to_string()),
    }
}

/// Creates expired test credentials.
///
/// Useful for testing token refresh flows.
pub fn expired_credentials() -> Credentials {
    Credentials {
        access_token: Some("expired-access-token".to_string()),
        refresh_token: Some("test-refresh-token".to_string()),
        expires_at: Some(0), // Already expired
        user_id: Some("test-user-id".to_string()),
    }
}

/// Creates empty credentials (not authenticated).
pub fn empty_credentials() -> Credentials {
    Credentials::default()
}

/// Builder for creating test App instances with various configurations.
///
/// Provides a fluent interface for setting up test scenarios.
#[derive(Default)]
pub struct TestAppBuilder {
    with_thread: bool,
    thread_title: Option<String>,
}

impl TestAppBuilder {
    /// Creates a new TestAppBuilder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures the builder to create an initial thread.
    pub fn with_thread(mut self, title: &str) -> Self {
        self.with_thread = true;
        self.thread_title = Some(title.to_string());
        self
    }

    /// Builds the test App instance.
    pub fn build(self) -> App {
        let mut app = App::default();

        if self.with_thread {
            let title = self.thread_title.unwrap_or_else(|| "Test Thread".to_string());
            for c in title.chars() {
                app.textarea.insert_char(c);
            }
            app.submit_input(spoq::models::ThreadType::Conversation);
        }

        app
    }
}

/// Helper function to create a default test app.
pub fn test_app() -> App {
    TestAppBuilder::new().build()
}

/// Helper function to create a test app with an existing thread.
pub fn test_app_with_thread(title: &str) -> App {
    TestAppBuilder::new().with_thread(title).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_credentials() {
        let creds = test_credentials();
        assert!(creds.access_token.is_some());
        assert!(creds.refresh_token.is_some());
        assert_eq!(creds.expires_at, Some(i64::MAX));
    }

    #[test]
    fn test_expired_credentials() {
        let creds = expired_credentials();
        assert!(creds.access_token.is_some());
        assert_eq!(creds.expires_at, Some(0));
    }

    #[test]
    fn test_empty_credentials() {
        let creds = empty_credentials();
        assert!(creds.access_token.is_none());
        assert!(creds.refresh_token.is_none());
    }

    #[test]
    fn test_app_builder_default() {
        let app = TestAppBuilder::new().build();
        assert!(app.active_thread_id.is_none());
        assert_eq!(app.cache.thread_count(), 0);
    }

    #[tokio::test]
    async fn test_app_builder_with_thread() {
        let app = TestAppBuilder::new().with_thread("Test").build();
        assert!(app.active_thread_id.is_some());
        assert_eq!(app.cache.thread_count(), 1);
    }

    #[tokio::test]
    async fn test_helper_functions() {
        let app1 = test_app();
        assert!(app1.active_thread_id.is_none());

        let app2 = test_app_with_thread("Hello");
        assert!(app2.active_thread_id.is_some());
    }
}
