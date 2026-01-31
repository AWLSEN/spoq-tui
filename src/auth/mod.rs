//! Authentication module for Spoq TUI.
//!
//! This module handles authentication before the TUI starts. Authentication flows
//! run synchronously during application startup to ensure users are authenticated
//! before entering the TUI interface.
//!
//! ## Step 0: AUTH (per SETUP_FLOW.md)
//!
//! The `ensure_authenticated()` function implements Step 0:
//! 1. Check for `~/.spoq/.credentials.json`
//! 2. If not found → Start device flow login
//! 3. If found → Check `expires_at` timestamp
//! 4. If expired → Use `refresh_token` to get new credentials
//! 5. Save updated credentials to `~/.spoq/.credentials.json`
//!
//! This module provides:
//! - Credentials storage and management
//! - Central API client for authentication endpoints
//! - Device authorization flow (RFC 8628)
//! - Pre-TUI authentication and provisioning flows

pub mod central_api;
pub mod credentials;
pub mod device_flow;
pub mod flow;
pub mod provisioning_flow;
pub mod token_migration;
pub mod token_verification;

use chrono::Utc;

pub use central_api::CentralApiClient;
pub use credentials::{Credentials, CredentialsManager};
pub use device_flow::{run_device_flow, DeviceFlowError};
pub use flow::run_auth_flow;
pub use provisioning_flow::{run_provisioning_flow, start_stopped_vps, validate_ip_address};
pub use token_migration::{detect_tokens, get_local_credentials_info, TokenDetectionResult};
pub use token_verification::{
    display_missing_tokens_error, verify_local_tokens, LocalTokenVerification,
    TokenVerificationError,
};

/// Error type for authentication operations.
#[derive(Debug)]
pub enum AuthError {
    /// Device flow authentication failed
    DeviceFlow(DeviceFlowError),
    /// Token refresh failed
    RefreshFailed(String),
    /// Failed to initialize credentials manager
    CredentialsManager(String),
    /// Failed to save credentials
    SaveCredentials(String),
    /// API error
    Api(central_api::CentralApiError),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::DeviceFlow(e) => write!(f, "Device flow failed: {}", e),
            AuthError::RefreshFailed(msg) => write!(f, "Token refresh failed: {}", msg),
            AuthError::CredentialsManager(msg) => write!(f, "Credentials manager error: {}", msg),
            AuthError::SaveCredentials(msg) => write!(f, "Failed to save credentials: {}", msg),
            AuthError::Api(e) => write!(f, "API error: {}", e),
        }
    }
}

impl std::error::Error for AuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AuthError::DeviceFlow(e) => Some(e),
            AuthError::Api(e) => Some(e),
            _ => None,
        }
    }
}

impl From<DeviceFlowError> for AuthError {
    fn from(e: DeviceFlowError) -> Self {
        AuthError::DeviceFlow(e)
    }
}

impl From<central_api::CentralApiError> for AuthError {
    fn from(e: central_api::CentralApiError) -> Self {
        AuthError::Api(e)
    }
}

/// Proactive refresh threshold in seconds (5 minutes).
/// If a token expires within this time, refresh it proactively.
const PROACTIVE_REFRESH_THRESHOLD: i64 = 300;

/// Ensure the user is authenticated.
///
/// This is the main orchestrating function for Step 0 (AUTH) of the setup flow.
/// It handles all authentication scenarios:
///
/// 1. **No credentials** - Runs device flow login
/// 2. **Expired token** - Attempts refresh, falls back to device flow
/// 3. **Token expiring soon** - Proactively refreshes
/// 4. **Valid token** - Returns existing credentials
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
///
/// # Returns
/// * `Ok(Credentials)` - Valid authenticated credentials
/// * `Err(AuthError)` - Authentication failed
///
/// # Example
/// ```no_run
/// use spoq::auth::ensure_authenticated;
///
/// let runtime = tokio::runtime::Runtime::new().unwrap();
/// let credentials = ensure_authenticated(&runtime).expect("Authentication failed");
/// ```
pub fn ensure_authenticated(runtime: &tokio::runtime::Runtime) -> Result<Credentials, AuthError> {
    // Initialize credentials manager
    let manager = CredentialsManager::new().ok_or_else(|| {
        AuthError::CredentialsManager("Failed to initialize credentials manager".to_string())
    })?;

    // Load existing credentials
    let credentials = manager.load();

    // Case 1: No access token - run full device flow
    if credentials.access_token.is_none() {
        return run_device_flow_and_save(runtime, &manager);
    }

    // Case 2: Token is expired - try refresh, fall back to device flow
    if credentials.is_expired() {
        return refresh_or_device_flow(runtime, &credentials, &manager);
    }

    // Case 3: Token expires soon - proactively refresh
    let now = Utc::now().timestamp();
    let expires_at = credentials.expires_at.unwrap_or(0);
    let time_remaining = expires_at - now;

    if time_remaining < PROACTIVE_REFRESH_THRESHOLD && time_remaining > 0 {
        let minutes_remaining = time_remaining / 60;
        println!(
            "Token expires soon (in {} minutes), proactively refreshing...",
            minutes_remaining
        );

        // Attempt proactive refresh - if it fails, continue with existing valid token
        match attempt_token_refresh(runtime, &credentials, &manager) {
            Ok(refreshed) => return Ok(refreshed),
            Err(_) => {
                // Proactive refresh failed, but token is still valid
                // Continue with existing credentials
            }
        }
    }

    // Case 4: Token is valid - return as-is
    Ok(credentials)
}

/// Run device flow and save credentials.
fn run_device_flow_and_save(
    runtime: &tokio::runtime::Runtime,
    manager: &CredentialsManager,
) -> Result<Credentials, AuthError> {
    let credentials = run_device_flow(runtime)?;

    if !manager.save(&credentials) {
        return Err(AuthError::SaveCredentials(
            "Failed to save credentials after device flow".to_string(),
        ));
    }

    Ok(credentials)
}

/// Attempt refresh, fall back to device flow if refresh fails.
fn refresh_or_device_flow(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
    manager: &CredentialsManager,
) -> Result<Credentials, AuthError> {
    // Try to refresh
    match attempt_token_refresh(runtime, credentials, manager) {
        Ok(_refreshed) => {
            // Reload from disk to ensure consistency
            Ok(manager.load())
        }
        Err(_) => {
            // Refresh failed - run full device flow
            run_device_flow_and_save(runtime, manager)
        }
    }
}

/// Attempt to refresh an expired access token.
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
        .or_else(|| central_api::get_jwt_expires_in(&refresh_response.access_token))
        .unwrap_or(900); // Default to 15 minutes

    let new_expires_at = Utc::now().timestamp() + expires_in as i64;
    new_credentials.expires_at = Some(new_expires_at);

    // Update user_id if provided in response
    if let Some(user_id) = refresh_response.user_id {
        new_credentials.user_id = Some(user_id);
    }

    // Save credentials immediately after successful refresh
    if !manager.save(&new_credentials) {
        eprintln!("Warning: Failed to save refreshed credentials to disk");
    }

    Ok(new_credentials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_error_display() {
        let err = AuthError::RefreshFailed("test error".to_string());
        assert!(format!("{}", err).contains("test error"));

        let err = AuthError::CredentialsManager("manager error".to_string());
        assert!(format!("{}", err).contains("manager error"));

        let err = AuthError::SaveCredentials("save error".to_string());
        assert!(format!("{}", err).contains("save error"));
    }

    #[test]
    fn test_auth_error_from_device_flow_error() {
        let device_err = DeviceFlowError::Cancelled;
        let auth_err: AuthError = device_err.into();

        match auth_err {
            AuthError::DeviceFlow(_) => {}
            _ => panic!("Expected AuthError::DeviceFlow"),
        }
    }

    #[test]
    fn test_auth_error_from_central_api_error() {
        let api_err = central_api::CentralApiError::AccessDenied;
        let auth_err: AuthError = api_err.into();

        match auth_err {
            AuthError::Api(_) => {}
            _ => panic!("Expected AuthError::Api"),
        }
    }

    #[test]
    fn test_proactive_refresh_threshold() {
        // Threshold should be 5 minutes (300 seconds)
        assert_eq!(PROACTIVE_REFRESH_THRESHOLD, 300);
    }
}
