//! Synchronous provisioning flow module.
//!
//! This module provides blocking provisioning flows for the TUI application.
//! It uses the Tokio runtime to call existing async methods on CentralApiClient.

use super::central_api::CentralApiError;
use super::credentials::Credentials;

/// Run the provisioning flow to set up VPS for the user.
///
/// This function blocks until provisioning is complete and updates credentials.
///
/// # Arguments
/// * `runtime` - The Tokio runtime to use for async operations
/// * `credentials` - Mutable reference to credentials (may be updated during provisioning)
///
/// # Returns
/// * `Ok(())` - Provisioning completed successfully
/// * `Err(CentralApiError)` - Provisioning failed
pub fn run_provisioning_flow(
    _runtime: &tokio::runtime::Runtime,
    _credentials: &mut Credentials,
) -> Result<(), CentralApiError> {
    // Stub implementation - will be filled in by Phase 2
    todo!("Phase 2 will implement the provisioning flow")
}
