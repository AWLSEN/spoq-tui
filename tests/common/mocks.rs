//! Mock implementations for test fixtures.
//!
//! This module re-exports the mock implementations from `spoq::adapters::mock`
//! and provides additional test-specific mock configurations.

pub use spoq::adapters::mock::http::MockResponse;
pub use spoq::adapters::mock::{InMemoryCredentials, MockHttpClient, MockWebSocket};
pub use spoq::traits::{Headers, HttpClient, Response};

use bytes::Bytes;

/// Configuration for setting up mock HTTP responses.
pub struct MockHttpConfig {
    client: MockHttpClient,
}

impl MockHttpConfig {
    /// Creates a new mock HTTP configuration.
    pub fn new() -> Self {
        Self {
            client: MockHttpClient::new(),
        }
    }

    /// Configures a successful JSON response.
    pub fn with_json_response(self, url: &str, status: u16, json: &str) -> Self {
        self.client.set_response(
            url,
            MockResponse::Success(Response::new(status, Bytes::from(json.to_string()))),
        );
        self
    }

    /// Configures an error response.
    #[allow(dead_code)]
    pub fn with_error_response(self, url: &str, status: u16, message: &str) -> Self {
        self.client.set_response(
            url,
            MockResponse::Error(spoq::traits::HttpError::ServerError {
                status,
                message: message.to_string(),
            }),
        );
        self
    }

    /// Configures a default success response for unmatched URLs.
    pub fn with_default_success(self, status: u16, body: &str) -> Self {
        self.client
            .set_default_response(MockResponse::Success(Response::new(
                status,
                Bytes::from(body.to_string()),
            )));
        self
    }

    /// Builds the configured MockHttpClient.
    pub fn build(self) -> MockHttpClient {
        self.client
    }
}

impl Default for MockHttpConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for setting up mock WebSocket.
pub struct MockWebSocketConfig {
    ws: MockWebSocket,
}

impl MockWebSocketConfig {
    /// Creates a new mock WebSocket configuration.
    pub fn new() -> Self {
        Self {
            ws: MockWebSocket::new(),
        }
    }

    /// Creates a mock WebSocket in disconnected state.
    pub fn disconnected() -> Self {
        Self {
            ws: MockWebSocket::disconnected(),
        }
    }

    /// Builds the configured MockWebSocket.
    pub fn build(self) -> MockWebSocket {
        self.ws
    }
}

impl Default for MockWebSocketConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for setting up mock credentials provider.
pub struct MockCredentialsConfig {
    provider: InMemoryCredentials,
}

impl MockCredentialsConfig {
    /// Creates a new mock credentials configuration.
    pub fn new() -> Self {
        Self {
            provider: InMemoryCredentials::new(),
        }
    }

    /// Creates with pre-populated credentials.
    #[allow(dead_code)]
    pub fn with_credentials(creds: spoq::auth::credentials::Credentials) -> Self {
        Self {
            provider: InMemoryCredentials::with_credentials(creds),
        }
    }

    /// Configures save to fail.
    #[allow(dead_code)]
    pub fn with_save_failure(self) -> Self {
        self.provider.set_save_should_fail(true);
        self
    }

    /// Configures load to fail.
    pub fn with_load_failure(self) -> Self {
        self.provider.set_load_should_fail(true);
        self
    }

    /// Builds the configured InMemoryCredentials provider.
    pub fn build(self) -> InMemoryCredentials {
        self.provider
    }
}

impl Default for MockCredentialsConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_http_config() {
        let client = MockHttpConfig::new()
            .with_json_response("https://api.example.com/test", 200, r#"{"status": "ok"}"#)
            .build();

        // Verify the client was configured
        assert!(client.get_requests().is_empty());
    }

    #[tokio::test]
    async fn test_mock_http_with_default() {
        let client = MockHttpConfig::new()
            .with_default_success(200, "OK")
            .build();

        let response = client
            .get("https://any-url.com/anything", &Headers::new())
            .await
            .unwrap();

        assert_eq!(response.status, 200);
    }

    #[test]
    fn test_mock_websocket_config() {
        let ws = MockWebSocketConfig::new().build();
        assert_eq!(ws.subscriber_count(), 0);
    }

    #[test]
    fn test_mock_websocket_disconnected() {
        use spoq::traits::WebSocketConnection;

        let ws = MockWebSocketConfig::disconnected().build();
        let state = ws.state();
        assert_eq!(
            *state.borrow(),
            spoq::websocket::WsConnectionState::Disconnected
        );
    }

    #[tokio::test]
    async fn test_mock_credentials_config() {
        use spoq::traits::CredentialsProvider;

        let provider = MockCredentialsConfig::new().build();
        let loaded = provider.load().await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_mock_credentials_with_failure() {
        use spoq::traits::CredentialsProvider;

        let provider = MockCredentialsConfig::new().with_load_failure().build();
        let result = provider.load().await;
        assert!(result.is_err());
    }
}
