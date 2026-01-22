//! VPS health check module for startup verification.
//!
//! This module provides health check functionality that runs on CLI startup
//! to verify the VPS and token status before entering the TUI.

use crate::auth::credentials::Credentials;
use crate::conductor::ConductorClient;

/// Result of health check operations
pub struct HealthCheckResult {
    pub conductor_healthy: bool,
    pub conductor_response_time_ms: Option<u64>,
    pub claude_code_works: bool,
    pub github_cli_works: bool,
    pub should_block: bool, // Block user from proceeding if true
    pub error_message: Option<String>, // Error message if blocking
}

/// Run comprehensive health checks on VPS via Conductor
///
/// # Arguments
/// * `credentials` - User credentials containing VPS URL
///
/// # Returns
/// Health check results including conductor status and token verification
pub async fn run_health_checks(credentials: &Credentials) -> HealthCheckResult {
    let mut result = HealthCheckResult {
        conductor_healthy: false,
        conductor_response_time_ms: None,
        claude_code_works: false,
        github_cli_works: false,
        should_block: false,
        error_message: None,
    };

    // Create conductor client with authentication
    let mut conductor = match &credentials.vps_url {
        Some(url) => {
            let mut client = ConductorClient::with_url(url);
            // Add JWT token if available
            if let Some(ref token) = credentials.access_token {
                client = client.with_auth(token);
            }
            // Add refresh token if available
            if let Some(ref refresh) = credentials.refresh_token {
                client = client.with_refresh_token(refresh);
            }
            client
        }
        None => return result, // No VPS URL
    };

    // Step 1: Check conductor health
    let start = std::time::Instant::now();
    match conductor.health_check().await {
        Ok(healthy) => {
            result.conductor_healthy = healthy;
            result.conductor_response_time_ms = Some(start.elapsed().as_millis() as u64);
        }
        Err(_) => {
            result.conductor_healthy = false;
            return result; // If conductor is down, skip token check
        }
    }

    // Step 2: Verify tokens via conductor
    match conductor.verify_tokens().await {
        Ok(tokens) => {
            result.claude_code_works = tokens.claude_code.installed && tokens.claude_code.authenticated;
            result.github_cli_works = tokens.github_cli.installed && tokens.github_cli.authenticated;

            // Block if any required tokens are missing
            if !result.claude_code_works || !result.github_cli_works {
                result.should_block = true;
                let mut missing = Vec::new();
                if !result.claude_code_works {
                    missing.push("Claude Code");
                }
                if !result.github_cli_works {
                    missing.push("GitHub CLI");
                }
                result.error_message = Some(format!(
                    "VPS credentials not set up: {} missing",
                    missing.join(", ")
                ));
            }
        }
        Err(e) => {
            result.should_block = true;
            result.error_message = Some(format!("Failed to verify VPS credentials: {}", e));
        }
    }

    result
}

/// Display health check results to user
///
/// # Arguments
/// * `result` - Health check results to display
/// * `vps_ip` - Optional VPS IP address for instructions
pub fn display_health_check_results(result: &HealthCheckResult, vps_ip: Option<&str>) {
    println!();

    // Conductor health
    if result.conductor_healthy {
        if let Some(ms) = result.conductor_response_time_ms {
            println!("✓ Conductor responding ({}ms)", ms);
        } else {
            println!("✓ Conductor healthy");
        }
    } else {
        println!("✗ Conductor not responding");
        println!("  Your VPS may be offline or starting up.");
        println!("  Please check your VPS status and try again.\n");
        return; // Don't show token status if conductor is down
    }

    // Token verification - only show status if everything works
    if result.claude_code_works && result.github_cli_works {
        println!("✓ Claude Code verified on VPS");
        println!("✓ GitHub CLI verified on VPS");
        println!("\n✓ All systems ready!\n");
    } else if result.should_block {
        // Show error and block user
        println!("\n✗ VPS Setup Required\n");
        println!("Your VPS needs to be configured with credentials before you can proceed.");
        println!("\nMissing credentials:");

        if !result.claude_code_works {
            println!("  • Claude Code");
        }
        if !result.github_cli_works {
            println!("  • GitHub CLI");
        }

        println!("\nTo set up credentials:");
        if let Some(ip) = vps_ip {
            println!("  1. SSH to your VPS:");
            println!("     ssh spoq@{}", ip);
        } else {
            println!("  1. SSH to your VPS");
        }

        if !result.claude_code_works {
            println!("  2. Authenticate Claude Code:");
            println!("     claude login");
        }

        if !result.github_cli_works {
            println!("  3. Authenticate GitHub CLI:");
            println!("     gh auth login");
        }

        println!("\nAfter setting up credentials, restart this application.\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_result_structure() {
        let result = HealthCheckResult {
            conductor_healthy: true,
            conductor_response_time_ms: Some(100),
            claude_code_works: true,
            github_cli_works: true,
            should_block: false,
            error_message: None,
        };

        assert!(result.conductor_healthy);
        assert_eq!(result.conductor_response_time_ms, Some(100));
        assert!(result.claude_code_works);
        assert!(result.github_cli_works);
        assert!(!result.should_block);
    }

    #[test]
    fn test_health_check_result_partial_failure() {
        let result = HealthCheckResult {
            conductor_healthy: true,
            conductor_response_time_ms: Some(150),
            claude_code_works: true,
            github_cli_works: false, // GitHub CLI not working
            should_block: true,
            error_message: Some("GitHub CLI missing".to_string()),
        };

        assert!(result.conductor_healthy);
        assert!(result.claude_code_works);
        assert!(!result.github_cli_works);
        assert!(result.should_block);
    }

    #[test]
    fn test_health_check_result_conductor_down() {
        let result = HealthCheckResult {
            conductor_healthy: false,
            conductor_response_time_ms: None,
            claude_code_works: false,
            github_cli_works: false,
            should_block: false,
            error_message: None,
        };

        assert!(!result.conductor_healthy);
        assert!(result.conductor_response_time_ms.is_none());
    }
}
