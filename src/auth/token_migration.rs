//! Token detection module.
//!
//! This module provides functions to detect which CLI tokens are present
//! on the user's system (GitHub CLI, Claude Code).
//!
//! Note: Token synchronization to VPS is now handled via HTTP API through
//! the Conductor client's `sync_tokens()` method, which reads from macOS
//! Keychain and filesystem directly.

use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Result of token detection showing which credentials are present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenDetectionResult {
    pub claude_code: bool,
    pub github_cli: bool,
}

/// Detects which tokens are present on the local system.
///
/// Checks for:
/// - Claude Code: macOS Keychain "Claude Code-credentials" entry
/// - GitHub CLI: `~/.config/gh/hosts.yml` file
///
/// # Returns
///
/// Returns `Ok(TokenDetectionResult)` on success, or an error message on failure.
pub fn detect_tokens() -> Result<TokenDetectionResult, String> {
    debug!("Starting token detection");

    let home = std::env::var("HOME")
        .map_err(|_| "Failed to get HOME environment variable".to_string())?;

    // Check for Claude Code credentials in macOS Keychain
    let claude_code = check_claude_code_keychain();

    // Check for GitHub CLI credentials
    let gh_hosts_path = PathBuf::from(&home).join(".config/gh/hosts.yml");
    let github_cli = gh_hosts_path.exists();

    let result = TokenDetectionResult {
        claude_code,
        github_cli,
    };

    // Log detected tokens
    if result.claude_code {
        info!("Detected Claude Code credentials (Keychain)");
    } else {
        warn!("Claude Code credentials not found in Keychain");
    }

    if result.github_cli {
        info!("Detected GitHub CLI credentials");
    } else {
        warn!("GitHub CLI credentials not found");
    }

    Ok(result)
}

/// Check if Claude Code credentials exist in macOS Keychain.
#[cfg(target_os = "macos")]
fn check_claude_code_keychain() -> bool {
    use security_framework::passwords::get_generic_password;

    let username = std::env::var("USER").unwrap_or_default();
    if username.is_empty() {
        return false;
    }

    get_generic_password("Claude Code-credentials", &username).is_ok()
}

/// Stub for non-macOS platforms.
#[cfg(not(target_os = "macos"))]
fn check_claude_code_keychain() -> bool {
    false
}

/// Get a summary of locally available credentials.
///
/// Returns a tuple of (claude_available, github_available).
pub fn get_local_credentials_info() -> (bool, bool) {
    match detect_tokens() {
        Ok(result) => (result.claude_code, result.github_cli),
        Err(_) => (false, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_detection_result_structure() {
        let result = TokenDetectionResult {
            claude_code: true,
            github_cli: false,
        };

        assert!(result.claude_code);
        assert!(!result.github_cli);
    }

    #[test]
    fn test_token_detection_result_equality() {
        let result1 = TokenDetectionResult {
            claude_code: true,
            github_cli: true,
        };

        let result2 = TokenDetectionResult {
            claude_code: true,
            github_cli: true,
        };

        let result3 = TokenDetectionResult {
            claude_code: false,
            github_cli: true,
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
        let (claude, github) = get_local_credentials_info();
        // Results are system-dependent, just verify types
        let _: bool = claude;
        let _: bool = github;
    }
}
