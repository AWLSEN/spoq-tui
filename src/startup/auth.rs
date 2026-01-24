//! Authentication startup module.
//!
//! This module handles credential validation and token refresh during startup.
//! It uses dependency injection via the CredentialsProvider trait for testability.

use crate::auth::central_api::{get_jwt_expires_in, CentralApiClient};
use crate::auth::credentials::{Credentials, CredentialsManager};
use crate::auth::{run_auth_flow, AuthError};

/// Proactive refresh threshold in seconds (5 minutes).
/// If a token expires within this time, refresh it proactively.
const PROACTIVE_REFRESH_THRESHOLD: i64 = 300;

/// Validate and refresh credentials if needed.
///
/// This function implements the credential validation logic:
/// 1. If no token exists, run full auth flow
/// 2. If token is expired, try refresh, fall back to full auth
/// 3. If token expires soon, proactively refresh
/// 4. Otherwise, use existing valid token
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `provider` - Credentials provider (for dependency injection)
/// * `manager` - Credentials manager (for save operations)
///
/// # Returns
/// * `Ok(Credentials)` - Valid authenticated credentials
/// * `Err(AuthError)` - Authentication failed
pub fn validate_credentials(
    runtime: &tokio::runtime::Runtime,
    manager: &CredentialsManager,
) -> Result<Credentials, AuthError> {
    let credentials = manager.load();

    // Case 1: No access token - run full auth flow
    if credentials.access_token.is_none() {
        println!("No credentials found. Starting authentication...");
        return run_auth_flow_and_save(runtime, manager);
    }

    // Case 2: Token is expired - try refresh, fall back to auth flow
    if credentials.is_expired() {
        println!("Token expired. Attempting refresh...");
        return refresh_or_auth_flow(runtime, &credentials, manager);
    }

    // Case 3: Token expires soon - proactively refresh
    let now = chrono::Utc::now().timestamp();
    let expires_at = credentials.expires_at.unwrap_or(0);
    let time_remaining = expires_at - now;

    if time_remaining < PROACTIVE_REFRESH_THRESHOLD && time_remaining > 0 {
        // Silently attempt proactive refresh
        if let Ok(refreshed) = attempt_token_refresh(runtime, &credentials, manager) {
            return Ok(refreshed);
        }
        // If proactive refresh fails, continue with existing valid token
    }

    // Case 4: Token is valid
    Ok(credentials)
}

/// Run full auth flow and save credentials.
fn run_auth_flow_and_save(
    runtime: &tokio::runtime::Runtime,
    manager: &CredentialsManager,
) -> Result<Credentials, AuthError> {
    let credentials = run_auth_flow(runtime)?;

    if !manager.save(&credentials) {
        return Err(AuthError::SaveCredentials(
            "Failed to save credentials after authentication".to_string(),
        ));
    }

    Ok(credentials)
}

/// Attempt refresh, fall back to full auth flow if refresh fails.
fn refresh_or_auth_flow(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
    manager: &CredentialsManager,
) -> Result<Credentials, AuthError> {
    match attempt_token_refresh(runtime, credentials, manager) {
        Ok(_) => {
            // Reload from disk to ensure consistency
            Ok(manager.load())
        }
        Err(_) => {
            // Refresh failed - run full auth flow
            println!("Token refresh failed. Starting full authentication...");
            run_auth_flow_and_save(runtime, manager)
        }
    }
}

/// Attempt to refresh an expired access token using the refresh token.
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `credentials` - Current credentials with refresh_token
/// * `manager` - Credentials manager for saving
///
/// # Returns
/// * `Ok(Credentials)` - New credentials with refreshed tokens
/// * `Err(AuthError)` - Refresh failed
pub fn attempt_token_refresh(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
    manager: &CredentialsManager,
) -> Result<Credentials, AuthError> {
    // Check for refresh token availability
    let refresh_token = credentials
        .refresh_token
        .as_ref()
        .ok_or_else(|| AuthError::RefreshFailed("No refresh token available".to_string()))?;

    let client = CentralApiClient::new();

    // Attempt the refresh
    let refresh_response = runtime.block_on(client.refresh_token(refresh_token))?;

    // Build new credentials with refreshed tokens
    let mut new_credentials = credentials.clone();
    new_credentials.access_token = Some(refresh_response.access_token.clone());

    // Update refresh token if server provided a new one
    if let Some(new_refresh) = refresh_response.refresh_token {
        new_credentials.refresh_token = Some(new_refresh);
    }

    // Calculate expiration from response or JWT
    let expires_in = refresh_response
        .expires_in
        .or_else(|| get_jwt_expires_in(&refresh_response.access_token))
        .unwrap_or(900);

    let new_expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
    new_credentials.expires_at = Some(new_expires_at);

    // Save credentials immediately after successful refresh
    if !manager.save(&new_credentials) {
        eprintln!("Warning: Failed to save refreshed credentials to disk");
    }

    Ok(new_credentials)
}

/// Verify local tokens (Claude Code, GitHub CLI).
///
/// This is a non-blocking verification that warns the user if tokens are missing
/// but does not prevent startup.
pub fn verify_local_tokens() {
    println!("Verifying local tokens...");
    match crate::auth::verify_local_tokens() {
        Ok(verification) => {
            if verification.all_required_present {
                println!("  Required tokens verified (Claude Code, GitHub CLI)");
            } else {
                eprintln!("\n  Warning: Required tokens missing on local machine:");
                if !verification.claude_code_present {
                    eprintln!("    - Claude Code - not found. Run: claude, then type /login");
                }
                if !verification.github_cli_present {
                    eprintln!("    - GitHub CLI - not found. Run: gh auth login");
                }
                eprintln!("\nThese tokens are required for VPS provisioning.");
                eprintln!("You can continue, but provisioning will fail without them.\n");
            }
        }
        Err(e) => {
            eprintln!("  Warning: Could not verify local tokens: {}", e);
            eprintln!("Continuing anyway, but VPS provisioning may fail.\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proactive_refresh_threshold() {
        // Threshold should be 5 minutes (300 seconds)
        assert_eq!(PROACTIVE_REFRESH_THRESHOLD, 300);
    }

    // Integration tests would use mock credentials provider
    // See adapters/mock/credentials.rs for InMemoryCredentials
}
