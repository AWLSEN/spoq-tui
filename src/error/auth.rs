//! Authentication-related error types.
//!
//! This module defines errors related to authentication, authorization,
//! and credential management.

use std::fmt;

/// Authentication-specific error variants.
///
/// These errors represent issues with user authentication, token management,
/// and authorization.
#[derive(Debug, Clone)]
pub enum AuthError {
    /// Device flow authentication was cancelled by user.
    DeviceFlowCancelled,

    /// Device flow expired (user didn't complete in time).
    DeviceFlowExpired,

    /// Device flow was denied (user denied access).
    DeviceFlowDenied,

    /// Access token has expired.
    TokenExpired,

    /// Refresh token has expired or is invalid.
    RefreshTokenInvalid { message: String },

    /// Failed to refresh the access token.
    RefreshFailed { message: String },

    /// Credentials could not be loaded.
    CredentialsLoadFailed { message: String },

    /// Credentials could not be saved.
    CredentialsSaveFailed { message: String },

    /// No credentials available (user not logged in).
    NotAuthenticated,

    /// Authorization was denied by the server.
    AccessDenied { resource: Option<String> },

    /// Invalid credentials format.
    InvalidCredentials { message: String },

    /// API returned an authentication error.
    ApiError { status: u16, message: String },
}

impl AuthError {
    /// Check if this error might be resolved by re-authenticating.
    pub fn requires_reauth(&self) -> bool {
        matches!(
            self,
            AuthError::TokenExpired
                | AuthError::RefreshTokenInvalid { .. }
                | AuthError::RefreshFailed { .. }
                | AuthError::NotAuthenticated
                | AuthError::ApiError { status: 401, .. }
        )
    }

    /// Check if this error is recoverable (can retry or re-auth).
    pub fn is_recoverable(&self) -> bool {
        !matches!(
            self,
            AuthError::DeviceFlowCancelled
                | AuthError::DeviceFlowDenied
                | AuthError::AccessDenied { .. }
        )
    }

    /// Get a user-friendly error message.
    pub fn user_message(&self) -> String {
        match self {
            AuthError::DeviceFlowCancelled => "Authentication was cancelled.".to_string(),
            AuthError::DeviceFlowExpired => {
                "The authentication request expired. Please try again.".to_string()
            }
            AuthError::DeviceFlowDenied => {
                "Access was denied. Please try again and grant the necessary permissions."
                    .to_string()
            }
            AuthError::TokenExpired => {
                "Your session has expired. Please sign in again.".to_string()
            }
            AuthError::RefreshTokenInvalid { .. } => {
                "Your session could not be renewed. Please sign in again.".to_string()
            }
            AuthError::RefreshFailed { .. } => {
                "Failed to renew your session. Please sign in again.".to_string()
            }
            AuthError::CredentialsLoadFailed { .. } => {
                "Could not load your credentials. Please sign in again.".to_string()
            }
            AuthError::CredentialsSaveFailed { .. } => {
                "Could not save your credentials. Please check file permissions.".to_string()
            }
            AuthError::NotAuthenticated => {
                "You are not signed in. Please sign in to continue.".to_string()
            }
            AuthError::AccessDenied { resource } => match resource {
                Some(r) => format!("Access denied to {}.", r),
                None => "Access denied. You don't have permission for this action.".to_string(),
            },
            AuthError::InvalidCredentials { .. } => {
                "Your credentials are invalid. Please sign in again.".to_string()
            }
            AuthError::ApiError { status, message } => match *status {
                401 => "Your session has expired. Please sign in again.".to_string(),
                403 => "Access denied. You don't have permission for this action.".to_string(),
                _ => format!("Authentication error: {}", message),
            },
        }
    }

    /// Get a short error code for logging.
    pub fn error_code(&self) -> &'static str {
        match self {
            AuthError::DeviceFlowCancelled => "E_AUTH_CANCELLED",
            AuthError::DeviceFlowExpired => "E_AUTH_EXPIRED",
            AuthError::DeviceFlowDenied => "E_AUTH_DENIED",
            AuthError::TokenExpired => "E_AUTH_TOKEN_EXP",
            AuthError::RefreshTokenInvalid { .. } => "E_AUTH_REFRESH_INV",
            AuthError::RefreshFailed { .. } => "E_AUTH_REFRESH_FAIL",
            AuthError::CredentialsLoadFailed { .. } => "E_AUTH_CRED_LOAD",
            AuthError::CredentialsSaveFailed { .. } => "E_AUTH_CRED_SAVE",
            AuthError::NotAuthenticated => "E_AUTH_NOT_AUTH",
            AuthError::AccessDenied { .. } => "E_AUTH_ACCESS",
            AuthError::InvalidCredentials { .. } => "E_AUTH_INVALID",
            AuthError::ApiError { .. } => "E_AUTH_API",
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::DeviceFlowCancelled => {
                write!(f, "Device flow authentication cancelled by user")
            }
            AuthError::DeviceFlowExpired => {
                write!(f, "Device flow authentication expired")
            }
            AuthError::DeviceFlowDenied => {
                write!(f, "Device flow authentication denied")
            }
            AuthError::TokenExpired => {
                write!(f, "Access token has expired")
            }
            AuthError::RefreshTokenInvalid { message } => {
                write!(f, "Refresh token invalid: {}", message)
            }
            AuthError::RefreshFailed { message } => {
                write!(f, "Token refresh failed: {}", message)
            }
            AuthError::CredentialsLoadFailed { message } => {
                write!(f, "Failed to load credentials: {}", message)
            }
            AuthError::CredentialsSaveFailed { message } => {
                write!(f, "Failed to save credentials: {}", message)
            }
            AuthError::NotAuthenticated => {
                write!(f, "Not authenticated")
            }
            AuthError::AccessDenied { resource } => match resource {
                Some(r) => write!(f, "Access denied to '{}'", r),
                None => write!(f, "Access denied"),
            },
            AuthError::InvalidCredentials { message } => {
                write!(f, "Invalid credentials: {}", message)
            }
            AuthError::ApiError { status, message } => {
                write!(f, "Authentication API error ({}): {}", status, message)
            }
        }
    }
}

impl std::error::Error for AuthError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_flow_cancelled() {
        let err = AuthError::DeviceFlowCancelled;
        assert!(!err.requires_reauth());
        assert!(!err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_CANCELLED");
        assert!(err.user_message().contains("cancelled"));
    }

    #[test]
    fn test_device_flow_expired() {
        let err = AuthError::DeviceFlowExpired;
        assert!(!err.requires_reauth());
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_EXPIRED");
    }

    #[test]
    fn test_device_flow_denied() {
        let err = AuthError::DeviceFlowDenied;
        assert!(!err.requires_reauth());
        assert!(!err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_DENIED");
    }

    #[test]
    fn test_token_expired_requires_reauth() {
        let err = AuthError::TokenExpired;
        assert!(err.requires_reauth());
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_TOKEN_EXP");
    }

    #[test]
    fn test_refresh_token_invalid_requires_reauth() {
        let err = AuthError::RefreshTokenInvalid {
            message: "token revoked".to_string(),
        };
        assert!(err.requires_reauth());
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_REFRESH_INV");
    }

    #[test]
    fn test_refresh_failed_requires_reauth() {
        let err = AuthError::RefreshFailed {
            message: "server error".to_string(),
        };
        assert!(err.requires_reauth());
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_REFRESH_FAIL");
    }

    #[test]
    fn test_not_authenticated_requires_reauth() {
        let err = AuthError::NotAuthenticated;
        assert!(err.requires_reauth());
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_NOT_AUTH");
    }

    #[test]
    fn test_access_denied_not_recoverable() {
        let err = AuthError::AccessDenied {
            resource: Some("admin panel".to_string()),
        };
        assert!(!err.requires_reauth());
        assert!(!err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_ACCESS");
        assert!(err.user_message().contains("admin panel"));
    }

    #[test]
    fn test_api_error_401_requires_reauth() {
        let err = AuthError::ApiError {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        assert!(err.requires_reauth());
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_API");
    }

    #[test]
    fn test_api_error_403_not_requires_reauth() {
        let err = AuthError::ApiError {
            status: 403,
            message: "Forbidden".to_string(),
        };
        assert!(!err.requires_reauth());
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_credentials_load_failed() {
        let err = AuthError::CredentialsLoadFailed {
            message: "file not found".to_string(),
        };
        assert!(!err.requires_reauth());
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_CRED_LOAD");
    }

    #[test]
    fn test_credentials_save_failed() {
        let err = AuthError::CredentialsSaveFailed {
            message: "permission denied".to_string(),
        };
        assert!(!err.requires_reauth());
        assert!(err.is_recoverable());
        assert_eq!(err.error_code(), "E_AUTH_CRED_SAVE");
    }

    #[test]
    fn test_display_format() {
        let err = AuthError::RefreshFailed {
            message: "server unavailable".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("Token refresh failed"));
        assert!(display.contains("server unavailable"));
    }

    #[test]
    fn test_user_message_formats() {
        let err_401 = AuthError::ApiError {
            status: 401,
            message: "Unauthorized".to_string(),
        };
        assert!(err_401.user_message().contains("sign in"));

        let err_403 = AuthError::ApiError {
            status: 403,
            message: "Forbidden".to_string(),
        };
        assert!(err_403.user_message().contains("permission"));
    }
}
