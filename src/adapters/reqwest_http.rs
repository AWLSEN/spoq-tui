//! Reqwest-based HTTP client adapter.
//!
//! This module provides a production HTTP client implementation using reqwest,
//! implementing the [`HttpClient`] trait from `crate::traits`.

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use futures_util::StreamExt;
use std::pin::Pin;

use crate::traits::{Headers, HttpClient, HttpError, Response};

/// HTTP client implementation using reqwest.
///
/// This adapter wraps a `reqwest::Client` and implements the [`HttpClient`] trait,
/// providing GET, POST, and streaming POST operations.
///
/// # Example
///
/// ```ignore
/// use spoq::adapters::ReqwestHttpClient;
/// use spoq::traits::HttpClient;
///
/// let client = ReqwestHttpClient::new();
/// let response = client.get("https://api.example.com/data", &Headers::new()).await?;
/// println!("Status: {}", response.status);
/// ```
#[derive(Debug, Clone)]
pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

impl ReqwestHttpClient {
    /// Create a new ReqwestHttpClient with default settings.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Create a new ReqwestHttpClient with a custom reqwest::Client.
    ///
    /// This allows for advanced configuration like custom timeouts,
    /// connection pools, or TLS settings.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Get a reference to the underlying reqwest::Client.
    pub fn inner(&self) -> &reqwest::Client {
        &self.client
    }

    /// Convert reqwest error to HttpError.
    fn convert_error(err: reqwest::Error) -> HttpError {
        if err.is_timeout() {
            HttpError::Timeout(err.to_string())
        } else if err.is_connect() {
            HttpError::ConnectionFailed(err.to_string())
        } else {
            HttpError::Other(err.to_string())
        }
    }

    /// Convert reqwest headers to our Headers type.
    fn convert_headers(headers: &reqwest::header::HeaderMap) -> Headers {
        headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|v| (name.to_string(), v.to_string()))
            })
            .collect()
    }

    /// Apply headers to a request builder.
    fn apply_headers(
        builder: reqwest::RequestBuilder,
        headers: &Headers,
    ) -> reqwest::RequestBuilder {
        let mut builder = builder;
        for (key, value) in headers {
            builder = builder.header(key, value);
        }
        builder
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get(&self, url: &str, headers: &Headers) -> Result<Response, HttpError> {
        let builder = self.client.get(url);
        let builder = Self::apply_headers(builder, headers);

        let response = builder.send().await.map_err(Self::convert_error)?;

        let status = response.status().as_u16();
        let response_headers = Self::convert_headers(response.headers());
        let body = response.bytes().await.map_err(Self::convert_error)?;

        Ok(Response::with_headers(status, response_headers, body))
    }

    async fn post(&self, url: &str, body: &str, headers: &Headers) -> Result<Response, HttpError> {
        let builder = self.client.post(url).body(body.to_string());
        let builder = Self::apply_headers(builder, headers);

        let response = builder.send().await.map_err(Self::convert_error)?;

        let status = response.status().as_u16();
        let response_headers = Self::convert_headers(response.headers());
        let body = response.bytes().await.map_err(Self::convert_error)?;

        Ok(Response::with_headers(status, response_headers, body))
    }

    async fn post_stream(
        &self,
        url: &str,
        body: &str,
        headers: &Headers,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, HttpError>> + Send>>, HttpError> {
        let builder = self.client.post(url).body(body.to_string());
        let builder = Self::apply_headers(builder, headers);

        let response = builder.send().await.map_err(Self::convert_error)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(HttpError::ServerError { status, message });
        }

        let stream = response.bytes_stream().map(|result| {
            result.map_err(|e| {
                if e.is_timeout() {
                    HttpError::Timeout(e.to_string())
                } else {
                    HttpError::Io(e.to_string())
                }
            })
        });

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reqwest_http_client_new() {
        let client = ReqwestHttpClient::new();
        // Just verify it can be created and has a valid inner client
        let _inner = client.inner();
    }

    #[test]
    fn test_reqwest_http_client_default() {
        let client = ReqwestHttpClient::default();
        let _ = client.inner();
    }

    #[test]
    fn test_reqwest_http_client_clone() {
        let client = ReqwestHttpClient::new();
        let cloned = client.clone();
        let _ = cloned.inner();
    }

    #[test]
    fn test_reqwest_http_client_with_custom_client() {
        let custom = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
        let client = ReqwestHttpClient::with_client(custom);
        let _ = client.inner();
    }

    #[test]
    fn test_apply_headers() {
        let mut headers = Headers::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Authorization".to_string(), "Bearer token".to_string());

        let client = reqwest::Client::new();
        let builder = client.get("https://example.com");
        let _builder = ReqwestHttpClient::apply_headers(builder, &headers);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_convert_headers() {
        let mut header_map = reqwest::header::HeaderMap::new();
        header_map.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        header_map.insert(reqwest::header::CONTENT_LENGTH, "100".parse().unwrap());

        let headers = ReqwestHttpClient::convert_headers(&header_map);
        assert_eq!(
            headers.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(headers.get("content-length"), Some(&"100".to_string()));
    }

    #[tokio::test]
    async fn test_get_invalid_url() {
        let client = ReqwestHttpClient::new();
        let result = client.get("not-a-valid-url", &Headers::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_connection_refused() {
        let client = ReqwestHttpClient::new();
        // Use a port that's unlikely to be in use
        let result = client
            .get("http://127.0.0.1:59999/test", &Headers::new())
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            // Should be a connection error
            assert!(matches!(
                e,
                HttpError::ConnectionFailed(_) | HttpError::Other(_)
            ));
        }
    }

    #[tokio::test]
    async fn test_post_connection_refused() {
        let client = ReqwestHttpClient::new();
        let result = client
            .post("http://127.0.0.1:59999/test", "{}", &Headers::new())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_post_stream_connection_refused() {
        let client = ReqwestHttpClient::new();
        let result = client
            .post_stream("http://127.0.0.1:59999/test", "{}", &Headers::new())
            .await;
        assert!(result.is_err());
    }
}
