//! Network-related error types.
//!
//! This module defines errors that occur during network operations,
//! including HTTP requests, connections, and DNS resolution.

use std::fmt;

/// Network-specific error variants.
///
/// These errors represent issues with network connectivity, HTTP requests,
/// and related network operations.
#[derive(Debug, Clone)]
pub enum NetworkError {
    /// Connection to the server failed.
    ConnectionFailed {
        url: String,
        message: String,
    },

    /// DNS resolution failed.
    DnsResolutionFailed {
        host: String,
    },

    /// Request timed out.
    Timeout {
        operation: String,
        duration_secs: u64,
    },

    /// TLS/SSL error.
    TlsError {
        message: String,
    },

    /// HTTP status error (non-2xx response).
    HttpStatus {
        status: u16,
        message: String,
    },

    /// Rate limited by server.
    RateLimited {
        retry_after_secs: Option<u64>,
    },

    /// Invalid response format.
    InvalidResponse {
        message: String,
    },

    /// Request was cancelled.
    Cancelled,

    /// Generic network error.
    Other {
        message: String,
    },
}

impl NetworkError {
    /// Check if this error is likely transient and can be retried.
    pub fn is_retryable(&self) -> bool {
        match self {
            NetworkError::ConnectionFailed { .. } => true,
            NetworkError::DnsResolutionFailed { .. } => true,
            NetworkError::Timeout { .. } => true,
            NetworkError::TlsError { .. } => false, // Usually config issue
            NetworkError::HttpStatus { status, .. } => {
                // Retry server errors and some specific client errors
                *status >= 500 || *status == 429 || *status == 408
            }
            NetworkError::RateLimited { .. } => true,
            NetworkError::InvalidResponse { .. } => false,
            NetworkError::Cancelled => false,
            NetworkError::Other { .. } => false,
        }
    }

    /// Get a user-friendly error message.
    pub fn user_message(&self) -> String {
        match self {
            NetworkError::ConnectionFailed { .. } => {
                "Unable to connect to the server. Please check your internet connection.".to_string()
            }
            NetworkError::DnsResolutionFailed { host } => {
                format!(
                    "Could not resolve server address '{}'. Please check your internet connection or DNS settings.",
                    host
                )
            }
            NetworkError::Timeout { operation, duration_secs } => {
                format!(
                    "The {} operation timed out after {} seconds. The server may be slow or unreachable.",
                    operation, duration_secs
                )
            }
            NetworkError::TlsError { .. } => {
                "A secure connection could not be established. Please check your system's SSL/TLS configuration.".to_string()
            }
            NetworkError::HttpStatus { status, .. } => {
                match *status {
                    400 => "The request was invalid. Please try again.".to_string(),
                    401 => "Authentication required. Please sign in again.".to_string(),
                    403 => "Access denied. You don't have permission for this action.".to_string(),
                    404 => "The requested resource was not found.".to_string(),
                    429 => "Too many requests. Please wait a moment and try again.".to_string(),
                    500..=599 => "The server is experiencing issues. Please try again later.".to_string(),
                    _ => format!("The server returned an error (HTTP {}). Please try again.", status),
                }
            }
            NetworkError::RateLimited { retry_after_secs } => {
                match retry_after_secs {
                    Some(secs) => format!(
                        "Too many requests. Please wait {} seconds before trying again.",
                        secs
                    ),
                    None => "Too many requests. Please wait a moment and try again.".to_string(),
                }
            }
            NetworkError::InvalidResponse { .. } => {
                "Received an invalid response from the server. Please try again.".to_string()
            }
            NetworkError::Cancelled => {
                "The request was cancelled.".to_string()
            }
            NetworkError::Other { message } => {
                format!("Network error: {}", message)
            }
        }
    }

    /// Get a short error code for logging.
    pub fn error_code(&self) -> &'static str {
        match self {
            NetworkError::ConnectionFailed { .. } => "E_NET_CONN",
            NetworkError::DnsResolutionFailed { .. } => "E_NET_DNS",
            NetworkError::Timeout { .. } => "E_NET_TIMEOUT",
            NetworkError::TlsError { .. } => "E_NET_TLS",
            NetworkError::HttpStatus { .. } => "E_NET_HTTP",
            NetworkError::RateLimited { .. } => "E_NET_RATE",
            NetworkError::InvalidResponse { .. } => "E_NET_INVALID",
            NetworkError::Cancelled => "E_NET_CANCEL",
            NetworkError::Other { .. } => "E_NET_OTHER",
        }
    }
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::ConnectionFailed { url, message } => {
                write!(f, "Connection failed to '{}': {}", url, message)
            }
            NetworkError::DnsResolutionFailed { host } => {
                write!(f, "DNS resolution failed for '{}'", host)
            }
            NetworkError::Timeout { operation, duration_secs } => {
                write!(f, "{} timed out after {} seconds", operation, duration_secs)
            }
            NetworkError::TlsError { message } => {
                write!(f, "TLS error: {}", message)
            }
            NetworkError::HttpStatus { status, message } => {
                write!(f, "HTTP {} error: {}", status, message)
            }
            NetworkError::RateLimited { retry_after_secs } => {
                match retry_after_secs {
                    Some(secs) => write!(f, "Rate limited, retry after {} seconds", secs),
                    None => write!(f, "Rate limited"),
                }
            }
            NetworkError::InvalidResponse { message } => {
                write!(f, "Invalid response: {}", message)
            }
            NetworkError::Cancelled => {
                write!(f, "Request cancelled")
            }
            NetworkError::Other { message } => {
                write!(f, "Network error: {}", message)
            }
        }
    }
}

impl std::error::Error for NetworkError {}

/// Classify a reqwest error into a NetworkError.
pub fn classify_reqwest_error(err: &reqwest::Error, url: &str) -> NetworkError {
    if err.is_connect() {
        NetworkError::ConnectionFailed {
            url: url.to_string(),
            message: err.to_string(),
        }
    } else if err.is_timeout() {
        NetworkError::Timeout {
            operation: "HTTP request".to_string(),
            duration_secs: 30, // Default assumption
        }
    } else if err.is_status() {
        if let Some(status) = err.status() {
            let status_code = status.as_u16();
            if status_code == 429 {
                NetworkError::RateLimited {
                    retry_after_secs: None,
                }
            } else {
                NetworkError::HttpStatus {
                    status: status_code,
                    message: err.to_string(),
                }
            }
        } else {
            NetworkError::HttpStatus {
                status: 0,
                message: err.to_string(),
            }
        }
    } else if err.is_decode() {
        NetworkError::InvalidResponse {
            message: format!("Failed to decode response: {}", err),
        }
    } else {
        // Check for TLS errors in the error chain
        let err_str = err.to_string().to_lowercase();
        if err_str.contains("tls") || err_str.contains("ssl") || err_str.contains("certificate") {
            NetworkError::TlsError {
                message: err.to_string(),
            }
        } else if err_str.contains("dns") || err_str.contains("resolve") {
            NetworkError::DnsResolutionFailed {
                host: extract_host_from_url(url),
            }
        } else {
            NetworkError::Other {
                message: err.to_string(),
            }
        }
    }
}

/// Extract the host portion from a URL string.
fn extract_host_from_url(url: &str) -> String {
    let url_lower = url.to_lowercase();
    let without_scheme = if url_lower.starts_with("https://") {
        &url[8..]
    } else if url_lower.starts_with("http://") {
        &url[7..]
    } else {
        url
    };

    without_scheme
        .split(&['/', ':'][..])
        .next()
        .unwrap_or(url)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_failed_is_retryable() {
        let err = NetworkError::ConnectionFailed {
            url: "https://example.com".to_string(),
            message: "Connection refused".to_string(),
        };
        assert!(err.is_retryable());
        assert_eq!(err.error_code(), "E_NET_CONN");
    }

    #[test]
    fn test_dns_resolution_failed_is_retryable() {
        let err = NetworkError::DnsResolutionFailed {
            host: "example.com".to_string(),
        };
        assert!(err.is_retryable());
        assert_eq!(err.error_code(), "E_NET_DNS");
    }

    #[test]
    fn test_timeout_is_retryable() {
        let err = NetworkError::Timeout {
            operation: "connect".to_string(),
            duration_secs: 30,
        };
        assert!(err.is_retryable());
        assert_eq!(err.error_code(), "E_NET_TIMEOUT");
    }

    #[test]
    fn test_tls_error_not_retryable() {
        let err = NetworkError::TlsError {
            message: "certificate expired".to_string(),
        };
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_NET_TLS");
    }

    #[test]
    fn test_http_status_retryable_for_server_errors() {
        let err_500 = NetworkError::HttpStatus {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        assert!(err_500.is_retryable());

        let err_503 = NetworkError::HttpStatus {
            status: 503,
            message: "Service Unavailable".to_string(),
        };
        assert!(err_503.is_retryable());

        let err_429 = NetworkError::HttpStatus {
            status: 429,
            message: "Too Many Requests".to_string(),
        };
        assert!(err_429.is_retryable());
    }

    #[test]
    fn test_http_status_not_retryable_for_client_errors() {
        let err_400 = NetworkError::HttpStatus {
            status: 400,
            message: "Bad Request".to_string(),
        };
        assert!(!err_400.is_retryable());

        let err_401 = NetworkError::HttpStatus {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        assert!(!err_401.is_retryable());

        let err_404 = NetworkError::HttpStatus {
            status: 404,
            message: "Not Found".to_string(),
        };
        assert!(!err_404.is_retryable());
    }

    #[test]
    fn test_rate_limited_is_retryable() {
        let err = NetworkError::RateLimited {
            retry_after_secs: Some(60),
        };
        assert!(err.is_retryable());
        assert_eq!(err.error_code(), "E_NET_RATE");
    }

    #[test]
    fn test_invalid_response_not_retryable() {
        let err = NetworkError::InvalidResponse {
            message: "JSON parse error".to_string(),
        };
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_NET_INVALID");
    }

    #[test]
    fn test_cancelled_not_retryable() {
        let err = NetworkError::Cancelled;
        assert!(!err.is_retryable());
        assert_eq!(err.error_code(), "E_NET_CANCEL");
    }

    #[test]
    fn test_user_message_connection_failed() {
        let err = NetworkError::ConnectionFailed {
            url: "https://example.com".to_string(),
            message: "Connection refused".to_string(),
        };
        let msg = err.user_message();
        assert!(msg.contains("internet connection"));
    }

    #[test]
    fn test_user_message_http_status() {
        let err_401 = NetworkError::HttpStatus {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        assert!(err_401.user_message().contains("sign in"));

        let err_403 = NetworkError::HttpStatus {
            status: 403,
            message: "Forbidden".to_string(),
        };
        assert!(err_403.user_message().contains("permission"));

        let err_500 = NetworkError::HttpStatus {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        assert!(err_500.user_message().contains("server"));
    }

    #[test]
    fn test_user_message_rate_limited() {
        let err_with_time = NetworkError::RateLimited {
            retry_after_secs: Some(60),
        };
        assert!(err_with_time.user_message().contains("60 seconds"));

        let err_without_time = NetworkError::RateLimited {
            retry_after_secs: None,
        };
        assert!(err_without_time.user_message().contains("wait"));
    }

    #[test]
    fn test_display_format() {
        let err = NetworkError::ConnectionFailed {
            url: "https://api.example.com".to_string(),
            message: "refused".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("api.example.com"));
        assert!(display.contains("refused"));
    }

    #[test]
    fn test_extract_host_from_url() {
        assert_eq!(
            extract_host_from_url("https://example.com/path"),
            "example.com"
        );
        assert_eq!(
            extract_host_from_url("http://example.com:8080/path"),
            "example.com"
        );
        assert_eq!(
            extract_host_from_url("https://api.example.com"),
            "api.example.com"
        );
        assert_eq!(extract_host_from_url("example.com"), "example.com");
    }
}
