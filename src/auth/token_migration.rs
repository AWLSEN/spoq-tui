//! Token detection module.
//!
//! This module provides functions to detect which CLI tokens are present
//! on the user's system (GitHub CLI).
//!
//! Note: Claude CLI uses server-side OAuth and is not synced from the client.
//! Token synchronization to VPS is now handled via HTTP API through
//! the Conductor client's `sync_tokens()` method.

use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Result of token detection showing which credentials are present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenDetectionResult {
    pub github_cli: bool,
}

/// Detects which tokens are present on the local system.
///
/// Checks for:
/// - GitHub CLI: `~/.config/gh/hosts.yml` file
///
/// # Returns
///
/// Returns `Ok(TokenDetectionResult)` on success, or an error message on failure.
pub fn detect_tokens() -> Result<TokenDetectionResult, String> {
    debug!("Starting token detection");

    let home = std::env::var("HOME")
        .map_err(|_| "Failed to get HOME environment variable".to_string())?;

    // Check for GitHub CLI credentials
    let gh_hosts_path = PathBuf::from(&home).join(".config/gh/hosts.yml");
    let github_cli = gh_hosts_path.exists();

    let result = TokenDetectionResult {
        github_cli,
    };

    // Log detected tokens
    if result.github_cli {
        info!("Detected GitHub CLI credentials");
    } else {
        warn!("GitHub CLI credentials not found");
    }

    Ok(result)
}

/// Get a summary of locally available credentials.
///
/// Returns whether GitHub CLI credentials are available.
pub fn get_local_credentials_info() -> bool {
    match detect_tokens() {
        Ok(result) => result.github_cli,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_detection_result_structure() {
        let result = TokenDetectionResult {
            github_cli: false,
        };

        assert!(!result.github_cli);
    }

    #[test]
    fn test_token_detection_result_equality() {
        let result1 = TokenDetectionResult {
            github_cli: true,
        };

        let result2 = TokenDetectionResult {
            github_cli: true,
        };

        let result3 = TokenDetectionResult {
            github_cli: false,
        };

        assert_eq!(result1, result2);
        assert_ne!(result1, result3);
    }

    #[test]
    fn test_detect_tokens_returns_result() {
        // This test verifies the function doesn't panic
        let result = detect_tokens();
        assert!(result.is_ok(), "Token detection should not fail");
    }

    #[test]
    fn test_get_local_credentials_info() {
        // This test just verifies the function runs without panicking
        let github = get_local_credentials_info();
        // Results are system-dependent, just verify types
        let _: bool = github;
    }
}
