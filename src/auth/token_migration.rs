//! Token migration detection module.
//!
//! This module wraps the migration script to detect which tokens are present
//! on the user's system (GitHub CLI, Claude Code, Codex).

use std::process::Command;
use tracing::{debug, info, warn};

/// Result of token detection showing which credentials are present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenDetectionResult {
    pub claude_code: bool,
    pub github_cli: bool,
    pub codex: bool,
}

/// Detects which tokens are present by calling the migration script.
///
/// Calls `./scripts/migration/creds-migrate.sh list` and parses the output
/// to determine which credentials are available.
///
/// # Returns
///
/// Returns `Ok(TokenDetectionResult)` on success, or an error message on failure.
///
/// # Errors
///
/// Returns an error if:
/// - The migration script cannot be executed
/// - The script output cannot be parsed
pub fn detect_tokens() -> Result<TokenDetectionResult, String> {
    debug!("Starting token detection");

    // Execute the migration script
    let output = Command::new("./scripts/migration/creds-migrate.sh")
        .arg("list")
        .output()
        .map_err(|e| format!("Failed to execute migration script: {}", e))?;

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Combine stdout and stderr since the script might output to either
    let combined_output = format!("{}\n{}", stdout, stderr);

    debug!("Migration script output:\n{}", combined_output);

    // Parse the output to detect tokens
    let github_cli = combined_output.contains("[OK] GitHub CLI:");
    let claude_code = combined_output.contains("[OK] Claude Code:");
    let codex = combined_output.contains("[OK] Codex:");

    let result = TokenDetectionResult {
        claude_code,
        github_cli,
        codex,
    };

    // Log detected tokens
    if result.claude_code {
        info!("Detected Claude Code credentials");
    } else {
        warn!("Claude Code credentials not found");
    }

    if result.github_cli {
        info!("Detected GitHub CLI credentials");
    } else {
        warn!("GitHub CLI credentials not found");
    }

    if result.codex {
        info!("Detected Codex credentials");
    } else {
        warn!("Codex credentials not found");
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_tokens_returns_result() {
        // This test will actually call the migration script
        // It should succeed even if no tokens are present
        let result = detect_tokens();
        assert!(result.is_ok(), "Token detection should not fail");

        let detection = result.unwrap();
        // We can't assert specific values since they depend on the system
        // But we can verify the structure is correct
        debug!(
            "Token detection result - GitHub CLI: {}, Claude Code: {}, Codex: {}",
            detection.github_cli, detection.claude_code, detection.codex
        );
    }

    #[test]
    fn test_token_detection_result_structure() {
        let result = TokenDetectionResult {
            claude_code: true,
            github_cli: false,
            codex: true,
        };

        assert!(result.claude_code);
        assert!(!result.github_cli);
        assert!(result.codex);
    }

    #[test]
    fn test_token_detection_result_equality() {
        let result1 = TokenDetectionResult {
            claude_code: true,
            github_cli: true,
            codex: false,
        };

        let result2 = TokenDetectionResult {
            claude_code: true,
            github_cli: true,
            codex: false,
        };

        let result3 = TokenDetectionResult {
            claude_code: false,
            github_cli: true,
            codex: false,
        };

        assert_eq!(result1, result2);
        assert_ne!(result1, result3);
    }
}
