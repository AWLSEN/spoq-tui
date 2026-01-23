//! VPS pre-check module for the setup flow.
//!
//! This module handles Step 1 of the setup flow: checking if a VPS already exists
//! for the authenticated user.

use crate::auth::central_api::{CentralApiClient, CentralApiError, VpsStatusResponse};

/// Status of the user's VPS.
#[derive(Debug, Clone)]
pub enum VpsStatus {
    /// User does not have a VPS
    None,
    /// User has a VPS that is currently provisioning
    Provisioning { vps_id: String },
    /// User has a VPS that is ready
    Ready {
        vps_id: String,
        hostname: Option<String>,
        ip: Option<String>,
        url: Option<String>,
        ssh_username: Option<String>,
    },
    /// User has a VPS in an unknown/other state
    Other { vps_id: String, status: String },
}

impl From<VpsStatusResponse> for VpsStatus {
    fn from(response: VpsStatusResponse) -> Self {
        match response.status.to_lowercase().as_str() {
            "provisioning" | "pending" | "creating" => VpsStatus::Provisioning {
                vps_id: response.vps_id,
            },
            "ready" | "running" | "active" => VpsStatus::Ready {
                vps_id: response.vps_id,
                hostname: response.hostname,
                ip: response.ip,
                url: response.url,
                ssh_username: response.ssh_username,
            },
            _ => VpsStatus::Other {
                vps_id: response.vps_id,
                status: response.status,
            },
        }
    }
}

/// Check if the user already has a VPS.
///
/// This function calls GET /api/vps/status to determine if the authenticated user
/// already has a VPS provisioned.
///
/// # Arguments
///
/// * `client` - CentralApiClient configured with authentication tokens
///
/// # Returns
///
/// * `Ok(VpsStatus)` - The user's current VPS status
/// * `Err(CentralApiError)` - Failed to check VPS status
///
/// # Example
///
/// ```no_run
/// use spoq::setup::precheck::{precheck, VpsStatus};
/// use spoq::auth::central_api::CentralApiClient;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client = CentralApiClient::new()
///     .with_auth("my_token");
///
/// match precheck(&mut client).await? {
///     VpsStatus::None => println!("No VPS - proceed to provisioning"),
///     VpsStatus::Ready { hostname, .. } => println!("VPS ready: {:?}", hostname),
///     VpsStatus::Provisioning { vps_id } => println!("VPS {} still provisioning", vps_id),
///     VpsStatus::Other { status, .. } => println!("VPS status: {}", status),
/// }
/// # Ok(())
/// # }
/// ```
pub async fn precheck(client: &mut CentralApiClient) -> Result<VpsStatus, CentralApiError> {
    match client.fetch_user_vps().await? {
        Some(vps_response) => Ok(VpsStatus::from(vps_response)),
        None => Ok(VpsStatus::None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vps_status_from_response_ready() {
        let response = VpsStatusResponse {
            vps_id: "vps-123".to_string(),
            status: "ready".to_string(),
            hostname: Some("user.spoq.dev".to_string()),
            ip: Some("1.2.3.4".to_string()),
            url: Some("https://user.spoq.dev:8000".to_string()),
            ssh_username: Some("root".to_string()),
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        let status = VpsStatus::from(response);
        match status {
            VpsStatus::Ready {
                vps_id,
                hostname,
                ip,
                url,
                ssh_username,
            } => {
                assert_eq!(vps_id, "vps-123");
                assert_eq!(hostname, Some("user.spoq.dev".to_string()));
                assert_eq!(ip, Some("1.2.3.4".to_string()));
                assert_eq!(url, Some("https://user.spoq.dev:8000".to_string()));
                assert_eq!(ssh_username, Some("root".to_string()));
            }
            _ => panic!("Expected VpsStatus::Ready"),
        }
    }

    #[test]
    fn test_vps_status_from_response_provisioning() {
        let response = VpsStatusResponse {
            vps_id: "vps-456".to_string(),
            status: "provisioning".to_string(),
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

        let status = VpsStatus::from(response);
        match status {
            VpsStatus::Provisioning { vps_id } => {
                assert_eq!(vps_id, "vps-456");
            }
            _ => panic!("Expected VpsStatus::Provisioning"),
        }
    }

    #[test]
    fn test_vps_status_from_response_running() {
        // "running" should map to Ready
        let response = VpsStatusResponse {
            vps_id: "vps-789".to_string(),
            status: "running".to_string(),
            hostname: Some("host.spoq.dev".to_string()),
            ip: None,
            url: None,
            ssh_username: None,
            provider: None,
            plan_id: None,
            data_center_id: None,
            created_at: None,
            ready_at: None,
        };

        let status = VpsStatus::from(response);
        assert!(matches!(status, VpsStatus::Ready { .. }));
    }

    #[test]
    fn test_vps_status_from_response_other() {
        let response = VpsStatusResponse {
            vps_id: "vps-other".to_string(),
            status: "maintenance".to_string(),
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

        let status = VpsStatus::from(response);
        match status {
            VpsStatus::Other { vps_id, status } => {
                assert_eq!(vps_id, "vps-other");
                assert_eq!(status, "maintenance");
            }
            _ => panic!("Expected VpsStatus::Other"),
        }
    }
}
