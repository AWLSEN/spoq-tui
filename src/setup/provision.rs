//! VPS provisioning module for the setup flow.
//!
//! This module handles Step 2 of the setup flow: calling the VPS provisioning API
//! to create a new VPS for the user. Provisioning is asynchronous on the server side,
//! so this module returns immediately after initiating the provisioning request.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Error type for provisioning operations.
#[derive(Debug)]
pub enum ProvisionError {
    /// HTTP request failed
    Http(reqwest::Error),
    /// JSON deserialization failed
    Json(serde_json::Error),
    /// Server returned an error status
    ServerError { status: u16, message: String },
    /// User already has a VPS
    AlreadyHasVps,
    /// User has exceeded VPS quota
    QuotaExceeded,
    /// Unauthorized - invalid or expired token
    Unauthorized,
    /// Payment required - user doesn't have active subscription
    PaymentRequired,
}

impl fmt::Display for ProvisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProvisionError::Http(e) => write!(f, "HTTP error: {}", e),
            ProvisionError::Json(e) => write!(f, "JSON error: {}", e),
            ProvisionError::ServerError { status, message } => {
                write!(f, "Server error ({}): {}", status, message)
            }
            ProvisionError::AlreadyHasVps => {
                write!(f, "You already have a VPS provisioned")
            }
            ProvisionError::QuotaExceeded => {
                write!(f, "VPS quota exceeded - please contact support")
            }
            ProvisionError::Unauthorized => {
                write!(f, "Unauthorized - please sign in again")
            }
            ProvisionError::PaymentRequired => {
                write!(f, "Payment required - please subscribe to provision a VPS")
            }
        }
    }
}

impl std::error::Error for ProvisionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProvisionError::Http(e) => Some(e),
            ProvisionError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for ProvisionError {
    fn from(e: reqwest::Error) -> Self {
        ProvisionError::Http(e)
    }
}

impl From<serde_json::Error> for ProvisionError {
    fn from(e: serde_json::Error) -> Self {
        ProvisionError::Json(e)
    }
}

/// Response from the provision endpoint.
///
/// The provisioning request returns immediately with this response,
/// even though the actual VPS creation happens asynchronously.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionResponse {
    /// Unique identifier for the VPS being provisioned
    #[serde(alias = "id")]
    pub vps_id: String,
    /// Current status of the provisioning (typically "provisioning")
    pub status: String,
    /// Domain assigned to the VPS (e.g., "user123.spoq.dev")
    #[serde(default)]
    pub domain: Option<String>,
    /// Hostname of the VPS (alternative to domain)
    #[serde(default)]
    pub hostname: Option<String>,
    /// Optional message from the server
    #[serde(default)]
    pub message: Option<String>,
}

impl ProvisionResponse {
    /// Get the domain or hostname for the VPS.
    pub fn get_domain(&self) -> Option<&str> {
        self.domain.as_deref().or(self.hostname.as_deref())
    }
}

/// Request body for provisioning endpoint.
#[derive(Debug, Serialize)]
struct ProvisionRequest {
    /// SSH password for the new VPS (optional - server may generate)
    #[serde(skip_serializing_if = "Option::is_none")]
    ssh_password: Option<String>,
    /// Plan ID for the VPS (optional - uses default if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_id: Option<String>,
    /// Data center ID for VPS location (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    data_center_id: Option<u32>,
}

/// Parse error response from API.
/// Tries to extract {"error": "message"} format, falls back to raw body.
fn parse_error_response(status: u16, body: &str) -> ProvisionError {
    // Try to parse as JSON with "error" field
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = json.get("error").and_then(|e| e.as_str()) {
            // Check for specific error types
            let msg_lower = msg.to_lowercase();
            if msg_lower.contains("already") && msg_lower.contains("vps") {
                return ProvisionError::AlreadyHasVps;
            }
            if msg_lower.contains("quota") || msg_lower.contains("limit") {
                return ProvisionError::QuotaExceeded;
            }
            if msg_lower.contains("payment") || msg_lower.contains("subscription") {
                return ProvisionError::PaymentRequired;
            }
            return ProvisionError::ServerError {
                status,
                message: msg.to_string(),
            };
        }
    }

    // Fall back to raw body
    ProvisionError::ServerError {
        status,
        message: body.to_string(),
    }
}

/// Provision a new VPS for the authenticated user.
///
/// This function initiates VPS provisioning by calling POST /api/vps/provision.
/// The request returns immediately - actual VPS creation happens asynchronously
/// on the server. Use the returned `vps_id` to poll for status updates.
///
/// # Arguments
///
/// * `token` - JWT access token for authentication
/// * `api_base` - Base URL for the API (e.g., "https://spoq.dev")
///
/// # Returns
///
/// * `Ok(ProvisionResponse)` - Provisioning initiated successfully
/// * `Err(ProvisionError)` - Provisioning failed
///
/// # Errors
///
/// * `Unauthorized` - Token is invalid or expired
/// * `AlreadyHasVps` - User already has a VPS provisioned
/// * `QuotaExceeded` - User has exceeded their VPS quota
/// * `PaymentRequired` - User needs an active subscription
/// * `ServerError` - Other server-side errors
/// * `Http` - Network or connection errors
///
/// # Example
///
/// ```no_run
/// use spoq::setup::provision;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let response = provision::provision("my_jwt_token", "https://spoq.dev").await?;
/// println!("VPS {} is {}", response.vps_id, response.status);
/// if let Some(domain) = response.get_domain() {
///     println!("Domain: {}", domain);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn provision(token: &str, api_base: &str) -> Result<ProvisionResponse, ProvisionError> {
    provision_with_options(token, api_base, None, None, None).await
}

/// Provision a new VPS with optional configuration.
///
/// Extended version of `provision` that allows specifying SSH password,
/// plan ID, and data center location.
///
/// # Arguments
///
/// * `token` - JWT access token for authentication
/// * `api_base` - Base URL for the API (e.g., "https://spoq.dev")
/// * `ssh_password` - Optional SSH password for the VPS
/// * `plan_id` - Optional plan ID (e.g., "plan-small", "plan-medium")
/// * `data_center_id` - Optional data center ID for VPS location
///
/// # Returns
///
/// * `Ok(ProvisionResponse)` - Provisioning initiated successfully
/// * `Err(ProvisionError)` - Provisioning failed
pub async fn provision_with_options(
    token: &str,
    api_base: &str,
    ssh_password: Option<&str>,
    plan_id: Option<&str>,
    data_center_id: Option<u32>,
) -> Result<ProvisionResponse, ProvisionError> {
    let client = Client::new();
    let url = format!("{}/api/vps/provision", api_base.trim_end_matches('/'));

    let request_body = ProvisionRequest {
        ssh_password: ssh_password.map(String::from),
        plan_id: plan_id.map(String::from),
        data_center_id,
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status().as_u16();

    // Handle specific HTTP status codes
    match status {
        200..=202 => {
            // Success - parse response
            let text = response.text().await?;
            let provision_response: ProvisionResponse = serde_json::from_str(&text)?;
            Ok(provision_response)
        }
        401 => Err(ProvisionError::Unauthorized),
        402 => Err(ProvisionError::PaymentRequired),
        409 => {
            // Conflict - likely already has VPS
            let body = response.text().await.unwrap_or_default();
            Err(parse_error_response(status, &body))
        }
        429 => Err(ProvisionError::QuotaExceeded),
        _ => {
            // Other error
            let body = response.text().await.unwrap_or_default();
            Err(parse_error_response(status, &body))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provision_response_deserialize() {
        let json = r#"{
            "vps_id": "vps-abc123",
            "status": "provisioning",
            "domain": "user123.spoq.dev"
        }"#;

        let response: ProvisionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.vps_id, "vps-abc123");
        assert_eq!(response.status, "provisioning");
        assert_eq!(response.domain, Some("user123.spoq.dev".to_string()));
        assert!(response.hostname.is_none());
        assert!(response.message.is_none());
    }

    #[test]
    fn test_provision_response_deserialize_with_id_alias() {
        let json = r#"{
            "id": "vps-xyz789",
            "status": "provisioning",
            "hostname": "user456.spoq.dev",
            "message": "VPS provisioning started"
        }"#;

        let response: ProvisionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.vps_id, "vps-xyz789");
        assert_eq!(response.status, "provisioning");
        assert!(response.domain.is_none());
        assert_eq!(response.hostname, Some("user456.spoq.dev".to_string()));
        assert_eq!(
            response.message,
            Some("VPS provisioning started".to_string())
        );
    }

    #[test]
    fn test_provision_response_deserialize_minimal() {
        let json = r#"{
            "vps_id": "vps-min123",
            "status": "pending"
        }"#;

        let response: ProvisionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.vps_id, "vps-min123");
        assert_eq!(response.status, "pending");
        assert!(response.domain.is_none());
        assert!(response.hostname.is_none());
        assert!(response.message.is_none());
    }

    #[test]
    fn test_provision_response_get_domain() {
        // With domain
        let response = ProvisionResponse {
            vps_id: "vps-1".to_string(),
            status: "provisioning".to_string(),
            domain: Some("user.spoq.dev".to_string()),
            hostname: None,
            message: None,
        };
        assert_eq!(response.get_domain(), Some("user.spoq.dev"));

        // With hostname only
        let response = ProvisionResponse {
            vps_id: "vps-2".to_string(),
            status: "provisioning".to_string(),
            domain: None,
            hostname: Some("host.spoq.dev".to_string()),
            message: None,
        };
        assert_eq!(response.get_domain(), Some("host.spoq.dev"));

        // With both (domain takes precedence)
        let response = ProvisionResponse {
            vps_id: "vps-3".to_string(),
            status: "provisioning".to_string(),
            domain: Some("domain.spoq.dev".to_string()),
            hostname: Some("host.spoq.dev".to_string()),
            message: None,
        };
        assert_eq!(response.get_domain(), Some("domain.spoq.dev"));

        // With neither
        let response = ProvisionResponse {
            vps_id: "vps-4".to_string(),
            status: "provisioning".to_string(),
            domain: None,
            hostname: None,
            message: None,
        };
        assert!(response.get_domain().is_none());
    }

    #[test]
    fn test_provision_error_display() {
        assert!(format!("{}", ProvisionError::AlreadyHasVps).contains("already have a VPS"));
        assert!(format!("{}", ProvisionError::QuotaExceeded).contains("quota"));
        assert!(format!("{}", ProvisionError::Unauthorized).contains("sign in"));
        assert!(format!("{}", ProvisionError::PaymentRequired).contains("subscribe"));

        let server_err = ProvisionError::ServerError {
            status: 500,
            message: "Internal error".to_string(),
        };
        let display = format!("{}", server_err);
        assert!(display.contains("500"));
        assert!(display.contains("Internal error"));
    }

    #[test]
    fn test_parse_error_response_with_error_field() {
        let body = r#"{"error": "User already has a VPS"}"#;
        let err = parse_error_response(409, body);
        assert!(matches!(err, ProvisionError::AlreadyHasVps));

        let body = r#"{"error": "Quota limit reached"}"#;
        let err = parse_error_response(429, body);
        assert!(matches!(err, ProvisionError::QuotaExceeded));

        let body = r#"{"error": "Payment required for subscription"}"#;
        let err = parse_error_response(402, body);
        assert!(matches!(err, ProvisionError::PaymentRequired));
    }

    #[test]
    fn test_parse_error_response_generic() {
        let body = r#"{"error": "Something went wrong"}"#;
        let err = parse_error_response(500, body);
        match err {
            ProvisionError::ServerError { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "Something went wrong");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[test]
    fn test_parse_error_response_raw_body() {
        let body = "Internal Server Error";
        let err = parse_error_response(500, body);
        match err {
            ProvisionError::ServerError { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "Internal Server Error");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[test]
    fn test_provision_request_serialization() {
        // Full request
        let request = ProvisionRequest {
            ssh_password: Some("secret123".to_string()),
            plan_id: Some("plan-small".to_string()),
            data_center_id: Some(1),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("ssh_password"));
        assert!(json.contains("plan_id"));
        assert!(json.contains("data_center_id"));

        // Minimal request (empty - all fields skipped)
        let request = ProvisionRequest {
            ssh_password: None,
            plan_id: None,
            data_center_id: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, "{}");
    }

    #[tokio::test]
    async fn test_provision_with_invalid_server() {
        let result = provision("test-token", "http://127.0.0.1:1").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProvisionError::Http(_)));
    }

    #[tokio::test]
    async fn test_provision_with_options_invalid_server() {
        let result = provision_with_options(
            "test-token",
            "http://127.0.0.1:1",
            Some("password"),
            Some("plan-small"),
            Some(1),
        )
        .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProvisionError::Http(_)));
    }
}
