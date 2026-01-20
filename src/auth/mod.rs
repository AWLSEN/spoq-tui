//! Authentication module for Spoq TUI.
//!
//! This module handles authentication before the TUI starts. Authentication flows
//! run synchronously during application startup to ensure users are authenticated
//! before entering the TUI interface.
//!
//! This module provides:
//! - Credentials storage and management
//! - Central API client for authentication endpoints
//! - Device authorization flow (RFC 8628)
//! - Pre-TUI authentication and provisioning flows

pub mod central_api;
pub mod credentials;
pub mod flow;
pub mod provisioning_flow;

pub use central_api::CentralApiClient;
pub use credentials::{Credentials, CredentialsManager};
pub use flow::run_auth_flow;
pub use provisioning_flow::run_provisioning_flow;

use central_api::CentralApiError;

/// Error type for authentication flows with user-friendly messages.
#[derive(Debug)]
pub enum AuthFlowError {
    /// Network-related error (connection failed, timeout, etc.)
    NetworkError(String),
    /// User denied authorization
    AuthorizationDenied,
    /// Authorization code expired before user completed auth
    AuthorizationExpired,
    /// Invalid credentials (token expired, refresh failed, etc.)
    InvalidCredentials,
    /// I/O error (file operations, stdin/stdout, etc.)
    IoError(std::io::Error),
}

impl AuthFlowError {
    /// Returns a user-friendly error message suitable for display.
    pub fn user_message(&self) -> &str {
        match self {
            AuthFlowError::NetworkError(_) => {
                "Unable to connect to the server. Please check your internet connection and try again."
            }
            AuthFlowError::AuthorizationDenied => {
                "Authorization was denied. Please try again and approve the sign-in request."
            }
            AuthFlowError::AuthorizationExpired => {
                "The authorization request expired. Please try signing in again."
            }
            AuthFlowError::InvalidCredentials => {
                "Your session has expired. Please sign in again."
            }
            AuthFlowError::IoError(_) => {
                "An error occurred while reading or writing data. Please try again."
            }
        }
    }
}

impl std::fmt::Display for AuthFlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthFlowError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            AuthFlowError::AuthorizationDenied => write!(f, "Authorization denied"),
            AuthFlowError::AuthorizationExpired => write!(f, "Authorization expired"),
            AuthFlowError::InvalidCredentials => write!(f, "Invalid credentials"),
            AuthFlowError::IoError(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for AuthFlowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AuthFlowError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AuthFlowError {
    fn from(e: std::io::Error) -> Self {
        AuthFlowError::IoError(e)
    }
}

impl From<CentralApiError> for AuthFlowError {
    fn from(e: CentralApiError) -> Self {
        match e {
            CentralApiError::Http(err) => AuthFlowError::NetworkError(err.to_string()),
            CentralApiError::Json(err) => {
                AuthFlowError::NetworkError(format!("Invalid response: {}", err))
            }
            CentralApiError::ServerError { status, message } => {
                if status == 401 || status == 403 {
                    AuthFlowError::InvalidCredentials
                } else {
                    AuthFlowError::NetworkError(format!("Server error ({}): {}", status, message))
                }
            }
            CentralApiError::AuthorizationPending => {
                // This shouldn't typically be converted to AuthFlowError
                // as it's a transient state, but handle it gracefully
                AuthFlowError::NetworkError("Authorization still pending".to_string())
            }
            CentralApiError::AuthorizationExpired => AuthFlowError::AuthorizationExpired,
            CentralApiError::AccessDenied => AuthFlowError::AuthorizationDenied,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_auth_flow_error_user_messages() {
        let errors = [
            (
                AuthFlowError::NetworkError("connection refused".to_string()),
                "Unable to connect to the server",
            ),
            (
                AuthFlowError::AuthorizationDenied,
                "Authorization was denied",
            ),
            (
                AuthFlowError::AuthorizationExpired,
                "authorization request expired",
            ),
            (AuthFlowError::InvalidCredentials, "session has expired"),
            (
                AuthFlowError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "test",
                )),
                "error occurred while reading or writing",
            ),
        ];

        for (error, expected_substring) in errors {
            let message = error.user_message();
            assert!(
                message.to_lowercase().contains(&expected_substring.to_lowercase()),
                "Expected '{}' to contain '{}', got: {}",
                message,
                expected_substring,
                message
            );
        }
    }

    #[test]
    fn test_auth_flow_error_display() {
        let err = AuthFlowError::NetworkError("timeout".to_string());
        assert!(format!("{}", err).contains("timeout"));

        let err = AuthFlowError::AuthorizationDenied;
        assert!(format!("{}", err).contains("denied"));

        let err = AuthFlowError::AuthorizationExpired;
        assert!(format!("{}", err).contains("expired"));

        let err = AuthFlowError::InvalidCredentials;
        assert!(format!("{}", err).contains("Invalid"));

        let err = AuthFlowError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(format!("{}", err).contains("file not found"));
    }

    #[test]
    fn test_auth_flow_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let auth_err: AuthFlowError = io_err.into();
        assert!(matches!(auth_err, AuthFlowError::IoError(_)));
    }

    #[test]
    fn test_auth_flow_error_from_central_api_error() {
        // Test AccessDenied conversion
        let api_err = CentralApiError::AccessDenied;
        let auth_err: AuthFlowError = api_err.into();
        assert!(matches!(auth_err, AuthFlowError::AuthorizationDenied));

        // Test AuthorizationExpired conversion
        let api_err = CentralApiError::AuthorizationExpired;
        let auth_err: AuthFlowError = api_err.into();
        assert!(matches!(auth_err, AuthFlowError::AuthorizationExpired));

        // Test ServerError 401 conversion
        let api_err = CentralApiError::ServerError {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        let auth_err: AuthFlowError = api_err.into();
        assert!(matches!(auth_err, AuthFlowError::InvalidCredentials));

        // Test ServerError 500 conversion
        let api_err = CentralApiError::ServerError {
            status: 500,
            message: "Internal error".to_string(),
        };
        let auth_err: AuthFlowError = api_err.into();
        assert!(matches!(auth_err, AuthFlowError::NetworkError(_)));
    }

    #[test]
    fn test_auth_flow_error_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "test error");
        let auth_err = AuthFlowError::IoError(io_err);
        assert!(auth_err.source().is_some());

        let auth_err = AuthFlowError::AuthorizationDenied;
        assert!(auth_err.source().is_none());
    }
}
