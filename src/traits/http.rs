//! HTTP client trait abstraction.
//!
//! Provides a trait-based abstraction for HTTP operations, enabling
//! dependency injection and mocking in tests.

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;

/// HTTP headers represented as a key-value map.
pub type Headers = HashMap<String, String>;

/// HTTP response wrapper.
#[derive(Debug, Clone)]
pub struct Response {
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: Headers,
    /// Response body
    pub body: Bytes,
}

impl Response {
    /// Create a new response.
    pub fn new(status: u16, body: Bytes) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body,
        }
    }

    /// Create a new response with headers.
    pub fn with_headers(status: u16, headers: Headers, body: Bytes) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// Check if the response indicates success (2xx status).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Get the response body as a string.
    pub fn text(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.to_vec())
    }

    /// Parse the response body as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }
}

/// HTTP client errors.
#[derive(Debug, Clone)]
pub enum HttpError {
    /// Connection failed
    ConnectionFailed(String),
    /// Request timeout
    Timeout(String),
    /// Server returned an error status
    ServerError { status: u16, message: String },
    /// Request was cancelled
    Cancelled,
    /// IO error
    Io(String),
    /// Invalid URL
    InvalidUrl(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            HttpError::Timeout(msg) => write!(f, "Request timeout: {}", msg),
            HttpError::ServerError { status, message } => {
                write!(f, "Server error ({}): {}", status, message)
            }
            HttpError::Cancelled => write!(f, "Request cancelled"),
            HttpError::Io(msg) => write!(f, "IO error: {}", msg),
            HttpError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            HttpError::Other(msg) => write!(f, "HTTP error: {}", msg),
        }
    }
}

impl std::error::Error for HttpError {}

/// Trait for HTTP client operations.
///
/// This trait abstracts HTTP operations to enable dependency injection
/// and mocking in tests. Implementations include the production reqwest-based
/// client and mock clients for testing.
///
/// # Example
///
/// ```ignore
/// use spoq::traits::{HttpClient, Headers, Response, HttpError};
///
/// async fn fetch_data<C: HttpClient>(client: &C) -> Result<String, HttpError> {
///     let response = client.get("https://api.example.com/data", &Headers::new()).await?;
///     response.text().map_err(|e| HttpError::Other(e.to_string()))
/// }
/// ```
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// Perform a GET request.
    ///
    /// # Arguments
    /// * `url` - The URL to request
    /// * `headers` - Request headers
    ///
    /// # Returns
    /// The response or an error
    async fn get(&self, url: &str, headers: &Headers) -> Result<Response, HttpError>;

    /// Perform a POST request.
    ///
    /// # Arguments
    /// * `url` - The URL to request
    /// * `body` - Request body as a string
    /// * `headers` - Request headers
    ///
    /// # Returns
    /// The response or an error
    async fn post(&self, url: &str, body: &str, headers: &Headers) -> Result<Response, HttpError>;

    /// Perform a POST request and return a streaming response.
    ///
    /// This is used for Server-Sent Events (SSE) streams where the response
    /// body is received incrementally.
    ///
    /// # Arguments
    /// * `url` - The URL to request
    /// * `body` - Request body as a string
    /// * `headers` - Request headers
    ///
    /// # Returns
    /// A stream of bytes or an error
    async fn post_stream(
        &self,
        url: &str,
        body: &str,
        headers: &Headers,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, HttpError>> + Send>>, HttpError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_new() {
        let response = Response::new(200, Bytes::from("Hello"));
        assert_eq!(response.status, 200);
        assert!(response.headers.is_empty());
        assert_eq!(response.body, Bytes::from("Hello"));
    }

    #[test]
    fn test_response_with_headers() {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        let response = Response::with_headers(200, headers, Bytes::from("{}"));
        assert_eq!(response.status, 200);
        assert_eq!(
            response.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn test_response_is_success() {
        assert!(Response::new(200, Bytes::new()).is_success());
        assert!(Response::new(201, Bytes::new()).is_success());
        assert!(Response::new(204, Bytes::new()).is_success());
        assert!(Response::new(299, Bytes::new()).is_success());
        assert!(!Response::new(300, Bytes::new()).is_success());
        assert!(!Response::new(400, Bytes::new()).is_success());
        assert!(!Response::new(500, Bytes::new()).is_success());
    }

    #[test]
    fn test_response_text() {
        let response = Response::new(200, Bytes::from("Hello, World!"));
        assert_eq!(response.text().unwrap(), "Hello, World!");
    }

    #[test]
    fn test_response_json() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct TestData {
            name: String,
            value: i32,
        }

        let response = Response::new(200, Bytes::from(r#"{"name":"test","value":42}"#));
        let data: TestData = response.json().unwrap();
        assert_eq!(
            data,
            TestData {
                name: "test".to_string(),
                value: 42
            }
        );
    }

    #[test]
    fn test_http_error_display() {
        assert_eq!(
            HttpError::ConnectionFailed("timeout".to_string()).to_string(),
            "Connection failed: timeout"
        );
        assert_eq!(
            HttpError::Timeout("30s".to_string()).to_string(),
            "Request timeout: 30s"
        );
        assert_eq!(
            HttpError::ServerError {
                status: 500,
                message: "Internal Error".to_string()
            }
            .to_string(),
            "Server error (500): Internal Error"
        );
        assert_eq!(HttpError::Cancelled.to_string(), "Request cancelled");
        assert_eq!(
            HttpError::Io("read failed".to_string()).to_string(),
            "IO error: read failed"
        );
        assert_eq!(
            HttpError::InvalidUrl("bad url".to_string()).to_string(),
            "Invalid URL: bad url"
        );
        assert_eq!(
            HttpError::Other("unknown".to_string()).to_string(),
            "HTTP error: unknown"
        );
    }

    #[test]
    fn test_http_error_clone() {
        let err = HttpError::ConnectionFailed("test".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }
}
