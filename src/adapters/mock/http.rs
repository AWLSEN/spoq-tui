//! Mock HTTP client for testing.
//!
//! Provides a configurable mock HTTP client that can return predefined
//! responses or errors for testing purposes.

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::traits::{Headers, HttpClient, HttpError, Response};

/// A recorded HTTP request for verification in tests.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    /// HTTP method (GET or POST)
    pub method: String,
    /// Request URL
    pub url: String,
    /// Request headers
    pub headers: Headers,
    /// Request body (for POST requests)
    pub body: Option<String>,
}

/// Configuration for a mock response.
#[derive(Debug, Clone)]
pub enum MockResponse {
    /// Return a successful response
    Success(Response),
    /// Return an error
    Error(HttpError),
    /// Return a stream of bytes
    Stream(Vec<Bytes>),
    /// Return a stream error
    StreamError(HttpError),
}

/// Mock HTTP client for testing.
///
/// This client can be configured to return specific responses for URLs,
/// allowing tests to verify HTTP interactions without network access.
///
/// # Example
///
/// ```ignore
/// use spoq::adapters::mock::MockHttpClient;
/// use spoq::traits::{HttpClient, Response, Headers};
/// use bytes::Bytes;
///
/// let mut client = MockHttpClient::new();
///
/// // Configure a response
/// client.set_response(
///     "https://api.example.com/data",
///     MockResponse::Success(Response::new(200, Bytes::from("Hello")))
/// );
///
/// // Make a request
/// let response = client.get("https://api.example.com/data", &Headers::new()).await?;
/// assert_eq!(response.status, 200);
///
/// // Verify the request was made
/// let requests = client.get_requests();
/// assert_eq!(requests.len(), 1);
/// assert_eq!(requests[0].url, "https://api.example.com/data");
/// ```
#[derive(Debug, Clone)]
pub struct MockHttpClient {
    /// Configured responses by URL pattern
    responses: Arc<Mutex<HashMap<String, MockResponse>>>,
    /// Default response when no specific match
    default_response: Arc<Mutex<Option<MockResponse>>>,
    /// Recorded requests for verification
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

impl MockHttpClient {
    /// Create a new mock HTTP client.
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
            default_response: Arc::new(Mutex::new(None)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Set a response for a specific URL.
    ///
    /// The URL is matched exactly.
    pub fn set_response(&self, url: &str, response: MockResponse) {
        let mut responses = self.responses.lock().unwrap();
        responses.insert(url.to_string(), response);
    }

    /// Set a default response for URLs without specific matches.
    pub fn set_default_response(&self, response: MockResponse) {
        let mut default = self.default_response.lock().unwrap();
        *default = Some(response);
    }

    /// Get all recorded requests.
    pub fn get_requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }

    /// Clear all recorded requests.
    pub fn clear_requests(&self) {
        self.requests.lock().unwrap().clear();
    }

    /// Clear all configured responses.
    pub fn clear_responses(&self) {
        self.responses.lock().unwrap().clear();
    }

    /// Record a request.
    fn record_request(&self, method: &str, url: &str, headers: &Headers, body: Option<String>) {
        let mut requests = self.requests.lock().unwrap();
        requests.push(RecordedRequest {
            method: method.to_string(),
            url: url.to_string(),
            headers: headers.clone(),
            body,
        });
    }

    /// Get the response for a URL.
    fn get_response(&self, url: &str) -> Option<MockResponse> {
        let responses = self.responses.lock().unwrap();

        // First try exact match
        if let Some(response) = responses.get(url) {
            return Some(response.clone());
        }

        // Then try prefix match (for URL patterns)
        for (pattern, response) in responses.iter() {
            if url.starts_with(pattern) {
                return Some(response.clone());
            }
        }

        // Finally use default
        let default = self.default_response.lock().unwrap();
        default.clone()
    }
}

impl Default for MockHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for MockHttpClient {
    async fn get(&self, url: &str, headers: &Headers) -> Result<Response, HttpError> {
        self.record_request("GET", url, headers, None);

        match self.get_response(url) {
            Some(MockResponse::Success(response)) => Ok(response),
            Some(MockResponse::Error(err)) => Err(err),
            Some(MockResponse::Stream(_)) => {
                Err(HttpError::Other("Stream response on non-stream request".to_string()))
            }
            Some(MockResponse::StreamError(err)) => Err(err),
            None => Err(HttpError::Other(format!("No mock response for URL: {}", url))),
        }
    }

    async fn post(&self, url: &str, body: &str, headers: &Headers) -> Result<Response, HttpError> {
        self.record_request("POST", url, headers, Some(body.to_string()));

        match self.get_response(url) {
            Some(MockResponse::Success(response)) => Ok(response),
            Some(MockResponse::Error(err)) => Err(err),
            Some(MockResponse::Stream(_)) => {
                Err(HttpError::Other("Stream response on non-stream request".to_string()))
            }
            Some(MockResponse::StreamError(err)) => Err(err),
            None => Err(HttpError::Other(format!("No mock response for URL: {}", url))),
        }
    }

    async fn post_stream(
        &self,
        url: &str,
        body: &str,
        headers: &Headers,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, HttpError>> + Send>>, HttpError> {
        self.record_request("POST", url, headers, Some(body.to_string()));

        match self.get_response(url) {
            Some(MockResponse::Stream(chunks)) => {
                let stream = futures::stream::iter(chunks.into_iter().map(Ok));
                Ok(Box::pin(stream))
            }
            Some(MockResponse::StreamError(err)) => Err(err),
            Some(MockResponse::Success(_)) => {
                Err(HttpError::Other("Non-stream response on stream request".to_string()))
            }
            Some(MockResponse::Error(err)) => Err(err),
            None => Err(HttpError::Other(format!("No mock response for URL: {}", url))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_http_client_new() {
        let client = MockHttpClient::new();
        assert!(client.get_requests().is_empty());
    }

    #[test]
    fn test_mock_http_client_default() {
        let client = MockHttpClient::default();
        assert!(client.get_requests().is_empty());
    }

    #[tokio::test]
    async fn test_get_with_response() {
        let client = MockHttpClient::new();
        client.set_response(
            "https://example.com/test",
            MockResponse::Success(Response::new(200, Bytes::from("Hello"))),
        );

        let response = client
            .get("https://example.com/test", &Headers::new())
            .await
            .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, Bytes::from("Hello"));

        let requests = client.get_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].url, "https://example.com/test");
    }

    #[tokio::test]
    async fn test_get_with_error() {
        let client = MockHttpClient::new();
        client.set_response(
            "https://example.com/error",
            MockResponse::Error(HttpError::ServerError {
                status: 500,
                message: "Internal Server Error".to_string(),
            }),
        );

        let result = client
            .get("https://example.com/error", &Headers::new())
            .await;

        assert!(result.is_err());
        match result {
            Err(HttpError::ServerError { status, message }) => {
                assert_eq!(status, 500);
                assert_eq!(message, "Internal Server Error");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[tokio::test]
    async fn test_post_with_response() {
        let client = MockHttpClient::new();
        client.set_response(
            "https://example.com/api",
            MockResponse::Success(Response::new(201, Bytes::from(r#"{"id": 1}"#))),
        );

        let response = client
            .post("https://example.com/api", r#"{"name": "test"}"#, &Headers::new())
            .await
            .unwrap();

        assert_eq!(response.status, 201);

        let requests = client.get_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].body, Some(r#"{"name": "test"}"#.to_string()));
    }

    #[tokio::test]
    async fn test_post_stream_with_chunks() {
        use futures_util::StreamExt;

        let client = MockHttpClient::new();
        client.set_response(
            "https://example.com/stream",
            MockResponse::Stream(vec![
                Bytes::from("chunk1"),
                Bytes::from("chunk2"),
                Bytes::from("chunk3"),
            ]),
        );

        let mut stream = client
            .post_stream("https://example.com/stream", "{}", &Headers::new())
            .await
            .unwrap();

        let mut chunks = Vec::new();
        while let Some(result) = stream.next().await {
            chunks.push(result.unwrap());
        }

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], Bytes::from("chunk1"));
        assert_eq!(chunks[1], Bytes::from("chunk2"));
        assert_eq!(chunks[2], Bytes::from("chunk3"));
    }

    #[tokio::test]
    async fn test_no_response_configured() {
        let client = MockHttpClient::new();

        let result = client
            .get("https://example.com/missing", &Headers::new())
            .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(HttpError::Other(_))));
    }

    #[tokio::test]
    async fn test_default_response() {
        let client = MockHttpClient::new();
        client.set_default_response(MockResponse::Success(Response::new(
            404,
            Bytes::from("Not Found"),
        )));

        let response = client
            .get("https://example.com/anything", &Headers::new())
            .await
            .unwrap();

        assert_eq!(response.status, 404);
    }

    #[tokio::test]
    async fn test_headers_recorded() {
        let client = MockHttpClient::new();
        client.set_response(
            "https://example.com/auth",
            MockResponse::Success(Response::new(200, Bytes::new())),
        );

        let mut headers = Headers::new();
        headers.insert("Authorization".to_string(), "Bearer token123".to_string());

        client
            .get("https://example.com/auth", &headers)
            .await
            .unwrap();

        let requests = client.get_requests();
        assert_eq!(
            requests[0].headers.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
    }

    #[test]
    fn test_clear_requests() {
        let client = MockHttpClient::new();
        client.record_request("GET", "https://example.com", &Headers::new(), None);
        assert_eq!(client.get_requests().len(), 1);

        client.clear_requests();
        assert!(client.get_requests().is_empty());
    }

    #[test]
    fn test_clear_responses() {
        let client = MockHttpClient::new();
        client.set_response(
            "https://example.com",
            MockResponse::Success(Response::new(200, Bytes::new())),
        );

        client.clear_responses();

        // After clearing, the response should not be found
        assert!(client.get_response("https://example.com").is_none());
    }

    #[tokio::test]
    async fn test_prefix_match() {
        let client = MockHttpClient::new();
        client.set_response(
            "https://example.com/api",
            MockResponse::Success(Response::new(200, Bytes::from("API response"))),
        );

        let response = client
            .get("https://example.com/api/v1/users", &Headers::new())
            .await
            .unwrap();

        assert_eq!(response.status, 200);
    }

    #[tokio::test]
    async fn test_clone() {
        let client = MockHttpClient::new();
        client.set_response(
            "https://example.com",
            MockResponse::Success(Response::new(200, Bytes::from("Hello"))),
        );

        let cloned = client.clone();

        let response = cloned
            .get("https://example.com", &Headers::new())
            .await
            .unwrap();

        assert_eq!(response.status, 200);

        // Both should share the same recorded requests
        assert_eq!(client.get_requests().len(), 1);
        assert_eq!(cloned.get_requests().len(), 1);
    }
}
