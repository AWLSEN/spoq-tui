//! macOS Keychain extraction module for Claude Code credentials.
//!
//! This module provides functions to extract Claude Code OAuth tokens
//! from the macOS Keychain using the `security` command-line utility.

use std::process::Command;

/// Result of a Keychain extraction attempt.
#[derive(Debug, Clone)]
pub enum KeychainResult {
    /// Successfully extracted credentials
    Found(String),
    /// Credentials not found in Keychain
    NotFound,
    /// User cancelled the Keychain access prompt
    UserCancelled,
    /// Error running the security command
    Error(String),
}

/// Extract Claude Code credentials from macOS Keychain.
///
/// Uses the `security find-generic-password` command to extract credentials
/// stored by Claude Code under the service name "Claude Code-credentials".
///
/// # Returns
/// * `KeychainResult::Found(creds)` - Credentials extracted successfully
/// * `KeychainResult::NotFound` - No credentials found in Keychain
/// * `KeychainResult::UserCancelled` - User cancelled the access prompt
/// * `KeychainResult::Error(msg)` - Error running the command
///
/// # Note
/// This may trigger a macOS password prompt if Keychain access
/// requires user authorization.
///
/// # Example
/// ```no_run
/// use spoq::setup::keychain::extract_claude_credentials;
///
/// let result = extract_claude_credentials();
/// match result {
///     spoq::setup::keychain::KeychainResult::Found(creds) => {
///         println!("Got credentials: {} bytes", creds.len());
///     }
///     spoq::setup::keychain::KeychainResult::NotFound => {
///         println!("No Claude credentials in Keychain");
///     }
///     spoq::setup::keychain::KeychainResult::UserCancelled => {
///         println!("User cancelled Keychain access");
///     }
///     spoq::setup::keychain::KeychainResult::Error(e) => {
///         eprintln!("Keychain error: {}", e);
///     }
/// }
/// ```
pub fn extract_claude_credentials() -> KeychainResult {
    // Use the security command to extract from Keychain
    // Service name: "Claude Code-credentials"
    // -w flag outputs only the password (credential data)
    let output = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let creds = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if creds.is_empty() {
                KeychainResult::NotFound
            } else {
                KeychainResult::Found(creds)
            }
        }
        Ok(out) => {
            // Command ran but didn't find credentials or user cancelled
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stderr_lower = stderr.to_lowercase();

            // Check for common error patterns
            if stderr_lower.contains("could not be found")
                || stderr_lower.contains("the specified item could not be found")
            {
                KeychainResult::NotFound
            } else if stderr_lower.contains("user interaction is not allowed")
                || stderr_lower.contains("authorization cancelled")
                || stderr_lower.contains("user canceled")
            {
                KeychainResult::UserCancelled
            } else if stderr.is_empty() {
                // No stderr but non-zero exit - likely not found
                KeychainResult::NotFound
            } else {
                KeychainResult::Error(stderr.trim().to_string())
            }
        }
        Err(e) => KeychainResult::Error(format!("Failed to run security command: {}", e)),
    }
}

/// Extract Claude Code credentials, returning Option for simpler handling.
///
/// Convenience wrapper that returns `Some(creds)` on success,
/// `None` for not found/cancelled, and logs errors.
///
/// # Returns
/// * `Some(String)` - Credentials extracted successfully
/// * `None` - Not found, cancelled, or error occurred
pub fn extract_claude_credentials_simple() -> Option<String> {
    match extract_claude_credentials() {
        KeychainResult::Found(creds) => Some(creds),
        KeychainResult::NotFound => None,
        KeychainResult::UserCancelled => None,
        KeychainResult::Error(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keychain_result_found() {
        let result = KeychainResult::Found("test-creds".to_string());
        match result {
            KeychainResult::Found(creds) => assert_eq!(creds, "test-creds"),
            _ => panic!("Expected Found variant"),
        }
    }

    #[test]
    fn test_keychain_result_not_found() {
        let result = KeychainResult::NotFound;
        assert!(matches!(result, KeychainResult::NotFound));
    }

    #[test]
    fn test_keychain_result_user_cancelled() {
        let result = KeychainResult::UserCancelled;
        assert!(matches!(result, KeychainResult::UserCancelled));
    }

    #[test]
    fn test_keychain_result_error() {
        let result = KeychainResult::Error("test error".to_string());
        match result {
            KeychainResult::Error(msg) => assert_eq!(msg, "test error"),
            _ => panic!("Expected Error variant"),
        }
    }
}
