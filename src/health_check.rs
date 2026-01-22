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
    };

    // Create conductor client
    let conductor = match &credentials.vps_url {
        Some(url) => ConductorClient::with_url(url),
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
        }
        Err(e) => {
            eprintln!("⚠️  Token verification failed: {}", e);
        }
    }

    result
}

/// Display health check results to user
///
/// # Arguments
/// * `result` - Health check results to display
pub fn display_health_check_results(result: &HealthCheckResult) {
    println!();

    // Conductor health
    if result.conductor_healthy {
        if let Some(ms) = result.conductor_response_time_ms {
            println!("✓ Conductor responding ({}ms)", ms);
        } else {
            println!("✓ Conductor healthy");
        }
    } else {
        println!("⚠️  Conductor not responding");
        println!("   Your VPS may be offline or starting up.");
        return; // Don't show token status if conductor is down
    }

    // Token verification
    if result.claude_code_works && result.github_cli_works {
        println!("✓ Claude Code verified on VPS");
        println!("✓ GitHub CLI verified on VPS");
        println!("\n✓ All systems ready!\n");
    } else {
        if result.claude_code_works {
            println!("✓ Claude Code verified on VPS");
        } else {
            println!("⚠️  Claude Code not authenticated on VPS");
            println!("   Run: ssh spoq@[VPS_IP] → claude login");
        }

        if result.github_cli_works {
            println!("✓ GitHub CLI verified on VPS");
        } else {
            println!("⚠️  GitHub CLI not authenticated on VPS");
            println!("   Run: ssh spoq@[VPS_IP] → gh auth login");
        }

        println!("\n⚠️  Some checks failed. You may experience issues.\n");
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
        };

        assert!(result.conductor_healthy);
        assert_eq!(result.conductor_response_time_ms, Some(100));
        assert!(result.claude_code_works);
        assert!(result.github_cli_works);
    }

    #[test]
    fn test_health_check_result_partial_failure() {
        let result = HealthCheckResult {
            conductor_healthy: true,
            conductor_response_time_ms: Some(150),
            claude_code_works: true,
            github_cli_works: false, // GitHub CLI not working
        };

        assert!(result.conductor_healthy);
        assert!(result.claude_code_works);
        assert!(!result.github_cli_works);
    }

    #[test]
    fn test_health_check_result_conductor_down() {
        let result = HealthCheckResult {
            conductor_healthy: false,
            conductor_response_time_ms: None,
            claude_code_works: false,
            github_cli_works: false,
        };

        assert!(!result.conductor_healthy);
        assert!(result.conductor_response_time_ms.is_none());
    }
}
