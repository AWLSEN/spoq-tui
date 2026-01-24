//! Error category classification for unified error handling.
//!
//! This module provides a high-level categorization of errors to enable
//! consistent handling, recovery strategies, and user messaging.

use std::fmt;

/// High-level categorization of errors for handling decisions.
///
/// Categories enable consistent:
/// - Retry policies (transient vs. permanent errors)
/// - User messaging (technical vs. user-actionable)
/// - Recovery strategies (automatic vs. manual intervention)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Network-related errors (connection, DNS, timeout).
    /// Generally transient and retryable.
    Network,

    /// Authentication/authorization errors.
    /// May require re-authentication or token refresh.
    Auth,

    /// Backend/server-side errors (HTTP 5xx, service unavailable).
    /// Generally transient and retryable after delay.
    Server,

    /// Client-side errors (bugs, invalid state, assertion failures).
    /// Not retryable - indicates a programming error.
    Client,

    /// User action required (invalid input, missing configuration).
    /// Not retryable until user takes corrective action.
    User,

    /// System/OS errors (filesystem, permissions, resources).
    /// May or may not be retryable depending on specific error.
    System,

    /// Configuration errors (missing settings, invalid config files).
    /// Not retryable until configuration is corrected.
    Configuration,
}

impl ErrorCategory {
    /// Returns true if errors in this category are generally transient
    /// and the operation can be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(self, ErrorCategory::Network | ErrorCategory::Server)
    }

    /// Returns a short label for the category suitable for logging.
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCategory::Network => "network",
            ErrorCategory::Auth => "auth",
            ErrorCategory::Server => "server",
            ErrorCategory::Client => "client",
            ErrorCategory::User => "user",
            ErrorCategory::System => "system",
            ErrorCategory::Configuration => "configuration",
        }
    }

    /// Returns a user-friendly description of the category.
    pub fn description(&self) -> &'static str {
        match self {
            ErrorCategory::Network => "Network connectivity issue",
            ErrorCategory::Auth => "Authentication problem",
            ErrorCategory::Server => "Server-side issue",
            ErrorCategory::Client => "Application error",
            ErrorCategory::User => "User action required",
            ErrorCategory::System => "System error",
            ErrorCategory::Configuration => "Configuration problem",
        }
    }

    /// Returns suggested recovery actions for this category.
    pub fn recovery_hint(&self) -> &'static str {
        match self {
            ErrorCategory::Network => {
                "Check your internet connection and try again"
            }
            ErrorCategory::Auth => {
                "Try signing out and signing back in"
            }
            ErrorCategory::Server => {
                "The server may be experiencing issues. Please try again later"
            }
            ErrorCategory::Client => {
                "This may be a bug. Please report this issue if it persists"
            }
            ErrorCategory::User => {
                "Please check your input and try again"
            }
            ErrorCategory::System => {
                "Check file permissions and available disk space"
            }
            ErrorCategory::Configuration => {
                "Check your configuration settings"
            }
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_retryable() {
        assert!(ErrorCategory::Network.is_retryable());
        assert!(ErrorCategory::Server.is_retryable());
        assert!(!ErrorCategory::Auth.is_retryable());
        assert!(!ErrorCategory::Client.is_retryable());
        assert!(!ErrorCategory::User.is_retryable());
        assert!(!ErrorCategory::System.is_retryable());
        assert!(!ErrorCategory::Configuration.is_retryable());
    }

    #[test]
    fn test_category_as_str() {
        assert_eq!(ErrorCategory::Network.as_str(), "network");
        assert_eq!(ErrorCategory::Auth.as_str(), "auth");
        assert_eq!(ErrorCategory::Server.as_str(), "server");
        assert_eq!(ErrorCategory::Client.as_str(), "client");
        assert_eq!(ErrorCategory::User.as_str(), "user");
        assert_eq!(ErrorCategory::System.as_str(), "system");
        assert_eq!(ErrorCategory::Configuration.as_str(), "configuration");
    }

    #[test]
    fn test_category_display() {
        assert_eq!(format!("{}", ErrorCategory::Network), "network");
        assert_eq!(format!("{}", ErrorCategory::Auth), "auth");
    }

    #[test]
    fn test_category_description() {
        assert!(ErrorCategory::Network.description().contains("Network"));
        assert!(ErrorCategory::Auth.description().contains("Authentication"));
        assert!(ErrorCategory::Server.description().contains("Server"));
    }

    #[test]
    fn test_category_recovery_hint() {
        assert!(ErrorCategory::Network.recovery_hint().contains("internet"));
        assert!(ErrorCategory::Auth.recovery_hint().contains("signing"));
        assert!(ErrorCategory::Server.recovery_hint().contains("try again"));
    }

    #[test]
    fn test_category_clone_and_eq() {
        let cat1 = ErrorCategory::Network;
        let cat2 = cat1;
        assert_eq!(cat1, cat2);

        let cat3 = ErrorCategory::Auth;
        assert_ne!(cat1, cat3);
    }

    #[test]
    fn test_category_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ErrorCategory::Network);
        set.insert(ErrorCategory::Auth);
        set.insert(ErrorCategory::Network); // Duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&ErrorCategory::Network));
        assert!(set.contains(&ErrorCategory::Auth));
    }
}
