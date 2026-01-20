//! Synchronous authentication flow module.
//!
//! This module provides blocking authentication flows for the TUI application.
//! It uses the Tokio runtime to call existing async methods on CentralApiClient.

use super::central_api::CentralApiError;
use super::credentials::Credentials;

/// Run the authentication flow to obtain user credentials.
///
/// This function blocks until the user completes authentication.
///
/// # Arguments
/// * `runtime` - The Tokio runtime to use for async operations
///
/// # Returns
/// * `Ok(Credentials)` - Successfully authenticated credentials
/// * `Err(CentralApiError)` - Authentication failed
pub fn run_auth_flow(_runtime: &tokio::runtime::Runtime) -> Result<Credentials, CentralApiError> {
    // Stub implementation - will be filled in by Phase 1
    todo!("Phase 1 will implement the auth flow")
}
