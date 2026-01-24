//! VPS verification startup module.
//!
//! This module handles VPS status checking and management during startup.
//! It uses dependency injection via the HttpClient trait for testability.

use crate::auth::central_api::{CentralApiClient, VpsStatusResponse};
use crate::auth::credentials::{Credentials, CredentialsManager};
use crate::auth::{run_provisioning_flow, start_stopped_vps};

/// Error type for VPS operations during startup.
#[derive(Debug)]
pub enum VpsError {
    /// Failed to check VPS status
    StatusCheckFailed(String),
    /// VPS provisioning failed
    ProvisioningFailed(String),
    /// VPS in unrecoverable state
    UnrecoverableState(String),
    /// VPS start failed
    StartFailed(String),
}

impl std::fmt::Display for VpsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VpsError::StatusCheckFailed(msg) => write!(f, "VPS status check failed: {}", msg),
            VpsError::ProvisioningFailed(msg) => write!(f, "VPS provisioning failed: {}", msg),
            VpsError::UnrecoverableState(msg) => write!(f, "VPS in unrecoverable state: {}", msg),
            VpsError::StartFailed(msg) => write!(f, "VPS start failed: {}", msg),
        }
    }
}

impl std::error::Error for VpsError {}

/// Fetch VPS state from the Central API.
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `credentials` - User credentials with access token
///
/// # Returns
/// * `Ok(Some(VpsStatusResponse))` - VPS exists
/// * `Ok(None)` - No VPS configured
/// * `Err(VpsError)` - Failed to check status
pub fn fetch_vps_status(
    runtime: &tokio::runtime::Runtime,
    credentials: &Credentials,
) -> Result<Option<VpsStatusResponse>, VpsError> {
    println!("Checking VPS status...");

    let mut client = if let Some(ref token) = credentials.access_token {
        CentralApiClient::new().with_auth(token)
    } else {
        CentralApiClient::new()
    };

    if let Some(ref refresh) = credentials.refresh_token {
        client = client.with_refresh_token(refresh);
    }

    runtime
        .block_on(client.fetch_user_vps())
        .map_err(|e| VpsError::StatusCheckFailed(e.to_string()))
}

/// Verify and manage VPS state.
///
/// This function handles all VPS states:
/// - ready/running/active: Continue
/// - provisioning/pending/creating: Continue to health check
/// - stopped: Auto-start
/// - failed/terminated: Error
/// - No VPS: Run provisioning flow
///
/// # Arguments
/// * `runtime` - Tokio runtime for async operations
/// * `credentials` - User credentials (may be updated during provisioning)
/// * `manager` - Credentials manager for saving updated credentials
///
/// # Returns
/// * `Ok(VpsStatusResponse)` - VPS is ready or starting
/// * `Err(VpsError)` - VPS cannot be used
pub fn verify_vps(
    runtime: &tokio::runtime::Runtime,
    credentials: &mut Credentials,
    manager: &CredentialsManager,
) -> Result<VpsStatusResponse, VpsError> {
    // Fetch VPS state from API
    let mut vps_state = fetch_vps_status(runtime, credentials)?;

    // If no VPS exists, run provisioning flow
    if vps_state.is_none() {
        println!("No VPS found. Starting provisioning...");
        run_provisioning_flow(runtime, credentials)
            .map_err(|e| VpsError::ProvisioningFailed(e.to_string()))?;

        // Save credentials after provisioning (tokens may have been refreshed)
        if !manager.save(credentials) {
            eprintln!("Warning: Failed to save credentials after provisioning");
        }

        // Fetch VPS state again after provisioning
        vps_state = fetch_vps_status(runtime, credentials)?;
    }

    // At this point, VPS should exist
    let vps = vps_state.ok_or_else(|| {
        VpsError::ProvisioningFailed("VPS still not found after provisioning".to_string())
    })?;

    // Handle VPS state
    match vps.status.as_str() {
        "ready" | "running" | "active" => {
            println!("  VPS is ready (status: {})", vps.status);
            Ok(vps)
        }
        "provisioning" | "pending" | "creating" => {
            println!("  VPS is still provisioning, checking health...");
            Ok(vps)
        }
        "stopped" => {
            println!("  VPS is stopped, starting...");
            let started_vps = start_stopped_vps(runtime, credentials)
                .map_err(|e| VpsError::StartFailed(e.to_string()))?;
            Ok(started_vps)
        }
        "failed" | "terminated" => {
            Err(VpsError::UnrecoverableState(format!(
                "VPS is in {} state. Please contact support@spoq.dev for assistance.",
                vps.status
            )))
        }
        other => {
            Err(VpsError::UnrecoverableState(format!(
                "VPS in unexpected state: {}. Please wait or contact support.",
                other
            )))
        }
    }
}

/// Build VPS URL from VpsStatusResponse.
///
/// Priority order:
/// 1. hostname (as https://{hostname})
/// 2. url (as-is)
/// 3. ip (as http://{ip}:8000)
pub fn build_vps_url(vps: &VpsStatusResponse) -> Option<String> {
    vps.hostname
        .as_ref()
        .map(|h| format!("https://{}", h))
        .or_else(|| vps.url.clone())
        .or_else(|| vps.ip.as_ref().map(|ip| format!("http://{}:8000", ip)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vps_error_display() {
        let err = VpsError::StatusCheckFailed("network error".to_string());
        assert!(err.to_string().contains("network error"));

        let err = VpsError::ProvisioningFailed("timeout".to_string());
        assert!(err.to_string().contains("timeout"));

        let err = VpsError::UnrecoverableState("failed".to_string());
        assert!(err.to_string().contains("failed"));

        let err = VpsError::StartFailed("api error".to_string());
        assert!(err.to_string().contains("api error"));
    }

    #[test]
    fn test_build_vps_url_hostname() {
        let vps = VpsStatusResponse {
            vps_id: "test".to_string(),
            status: "ready".to_string(),
            hostname: Some("test.spoq.dev".to_string()),
            ip: Some("1.2.3.4".to_string()),
            url: Some("http://custom".to_string()),
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        assert_eq!(build_vps_url(&vps), Some("https://test.spoq.dev".to_string()));
    }

    #[test]
    fn test_build_vps_url_fallback_url() {
        let vps = VpsStatusResponse {
            vps_id: "test".to_string(),
            status: "ready".to_string(),
            hostname: None,
            ip: Some("1.2.3.4".to_string()),
            url: Some("http://custom.url".to_string()),
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        assert_eq!(build_vps_url(&vps), Some("http://custom.url".to_string()));
    }

    #[test]
    fn test_build_vps_url_fallback_ip() {
        let vps = VpsStatusResponse {
            vps_id: "test".to_string(),
            status: "ready".to_string(),
            hostname: None,
            ip: Some("1.2.3.4".to_string()),
            url: None,
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        assert_eq!(build_vps_url(&vps), Some("http://1.2.3.4:8000".to_string()));
    }

    #[test]
    fn test_build_vps_url_none() {
        let vps = VpsStatusResponse {
            vps_id: "test".to_string(),
            status: "ready".to_string(),
            hostname: None,
            ip: None,
            url: None,
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        assert_eq!(build_vps_url(&vps), None);
    }
}
