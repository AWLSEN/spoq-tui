//! Health check startup module.
//!
//! This module handles VPS health verification during startup,
//! including conductor connectivity and token verification.

use crate::auth::credentials::{Credentials, CredentialsManager};
use crate::conductor::ConductorClient;
use crate::health_check::{display_health_check_results, run_health_checks};
use crate::setup::gh_auth::{ensure_gh_authenticated, is_gh_authenticated, GhAuthError};
use std::io::{BufRead, Write};

/// Run health check loop for VPS verification.
///
/// This function runs the health check and handles user retry loop
/// if credentials are missing on the VPS.
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `vps_url` - URL of the VPS to check
/// * `credentials` - User credentials (may be reloaded from manager)
/// * `manager` - Credentials manager for reloading after sync
/// * `vps_ip` - Optional VPS IP for instructions
///
/// # Returns
/// * `Ok(())` - Health checks passed
/// * `Err(String)` - Health checks failed and user aborted
pub fn run_health_check_loop(
    runtime: &tokio::runtime::Runtime,
    vps_url: &str,
    credentials: &mut Credentials,
    manager: &CredentialsManager,
    vps_ip: Option<&str>,
) -> Result<(), String> {
    print!("\n  Connecting to VPS");
    std::io::stdout().flush().ok();

    let mut first_attempt = true;

    loop {
        // Run health checks
        let health_result = runtime.block_on(run_health_checks(vps_url, credentials));

        // If tokens are missing on first attempt, try to auto-sync
        if first_attempt && health_result.should_block {
            first_attempt = false;

            // Check if GitHub CLI is authenticated (runs `gh auth status`)
            let github_cli_authenticated = is_gh_authenticated();

            // If GitHub CLI is not authenticated locally, try auto-login first
            if !github_cli_authenticated {
                println!("\r  GitHub CLI not authenticated locally. Starting auto-login...\n");

                match ensure_gh_authenticated() {
                    Ok(()) => {
                        println!("  GitHub CLI authenticated successfully\n");
                    }
                    Err(GhAuthError::NotInstalled) => {
                        println!("  GitHub CLI not installed.");
                        println!("  Install from: https://cli.github.com/\n");
                    }
                    Err(GhAuthError::Cancelled) => {
                        println!("  GitHub CLI authentication cancelled.\n");
                    }
                    Err(e) => {
                        println!("  GitHub CLI authentication failed: {}\n", e);
                    }
                }
            }

            println!("  Syncing credentials to VPS...");

            // Attempt sync via conductor (this reads keychain ONCE)
            if let Some(sync_result) = attempt_credential_sync(runtime, vps_url, credentials) {
                if sync_result {
                    println!("  Credentials synced, verifying...");

                    // Reload credentials in case conductor auto-refreshed during sync
                    *credentials = manager.load();

                    std::thread::sleep(std::time::Duration::from_secs(1));
                    continue; // Recheck immediately
                } else {
                    println!("  Credential sync failed");
                }
            }
        }

        // Clear the loading line
        println!("\r                                      ");

        // Display results
        display_health_check_results(&health_result, vps_ip);

        // If all checks pass, break out of loop
        if !health_result.should_block {
            return Ok(());
        }

        // Wait for user input to retry
        println!("Press 'r' to retry verification, or Ctrl+C to exit.");

        let stdin = std::io::stdin();
        let mut line = String::new();

        // Read user input
        match stdin.lock().read_line(&mut line) {
            Ok(_) => {
                let input = line.trim().to_lowercase();
                if input == "r" || input == "retry" {
                    // Reload credentials before retry
                    *credentials = manager.load();
                    print!("\n  Retrying VPS verification...");
                    std::io::stdout().flush().ok();
                    continue;
                } else {
                    println!("Invalid input. Press 'r' to retry.\n");
                }
            }
            Err(_) => {
                return Err("Failed to read input. Exiting.".to_string());
            }
        }
    }
}

/// Attempt to sync credentials to VPS via conductor.
///
/// # Returns
/// * `Some(true)` - Sync succeeded
/// * `Some(false)` - Sync failed
/// * `None` - Could not attempt sync
fn attempt_credential_sync(
    runtime: &tokio::runtime::Runtime,
    vps_url: &str,
    credentials: &Credentials,
) -> Option<bool> {
    let mut conductor = ConductorClient::with_url(vps_url);

    if let Some(ref token) = credentials.access_token {
        conductor = conductor.with_auth(token);
    }
    if let Some(ref refresh) = credentials.refresh_token {
        conductor = conductor.with_refresh_token(refresh);
    }

    match runtime.block_on(conductor.sync_tokens("all")) {
        Ok(_) => Some(true),
        Err(_) => Some(false),
    }
}

#[cfg(test)]
mod tests {
    // Note: Full integration tests would require mocking the conductor client
    // which is beyond the scope of unit tests. The health check functionality
    // is tested in health_check.rs.
}
