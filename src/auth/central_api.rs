//! Central API client for Spoq authentication and VPS management.
//!
//! This module provides the HTTP client for interacting with the Spoq Central API,
//! handling device authorization flow, token management, and VPS operations.

use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Default URL for the Central API
pub const CENTRAL_API_URL: &str = "https://spoq-api-production.up.railway.app";

/// Error type for Central API client operations
#[derive(Debug)]
pub enum CentralApiError {
    /// HTTP request failed
    Http(reqwest::Error),
    /// JSON deserialization failed
    Json(serde_json::Error),
    /// Server returned an error status
    ServerError { status: u16, message: String },
    /// Authorization pending (user hasn't completed auth yet)
    AuthorizationPending,
    /// Authorization expired
    AuthorizationExpired,
    /// Access denied
    AccessDenied,
}

impl std::fmt::Display for CentralApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CentralApiError::Http(e) => write!(f, "HTTP error: {}", e),
            CentralApiError::Json(e) => write!(f, "JSON error: {}", e),
            CentralApiError::ServerError { status, message } => {
                write!(f, "Server error ({}): {}", status, message)
            }
            CentralApiError::AuthorizationPending => write!(f, "Authorization pending"),
            CentralApiError::AuthorizationExpired => write!(f, "Authorization expired"),
            CentralApiError::AccessDenied => write!(f, "Access denied"),
        }
    }
}

impl std::error::Error for CentralApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CentralApiError::Http(e) => Some(e),
            CentralApiError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for CentralApiError {
    fn from(e: reqwest::Error) -> Self {
        CentralApiError::Http(e)
    }
}

impl From<serde_json::Error> for CentralApiError {
    fn from(e: serde_json::Error) -> Self {
        CentralApiError::Json(e)
    }
}

/// Response from device authorization endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuthResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i64,
    pub interval: i64,
}

/// Response from device token endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
}

/// Response from token refresh endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: String,
}

/// VPS plan information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpsPlan {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub price_monthly: f64,
    pub cpu_cores: i32,
    pub memory_mb: i64,
    pub storage_gb: i64,
}

/// VPS provision request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionVpsRequest {
    pub plan_id: String,
    pub hostname: String,
    pub region: Option<String>,
}

/// VPS provision response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionVpsResponse {
    pub vps_id: String,
    pub hostname: String,
    pub ip_address: Option<String>,
    pub status: String,
}

/// VPS status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpsStatusResponse {
    pub vps_id: String,
    pub hostname: String,
    pub ip_address: Option<String>,
    pub status: String,
    pub url: Option<String>,
}

/// Client for interacting with the Spoq Central API.
pub struct CentralApiClient {
    /// Base URL for the Central API
    pub base_url: String,
    /// Reusable HTTP client
    client: Client,
    /// Optional authentication token for Bearer auth
    auth_token: Option<String>,
}

impl CentralApiClient {
    /// Create a new CentralApiClient with the default base URL.
    pub fn new() -> Self {
        Self {
            base_url: CENTRAL_API_URL.to_string(),
            client: Client::new(),
            auth_token: None,
        }
    }

    /// Create a new CentralApiClient with a custom base URL.
    pub fn with_base_url(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
            auth_token: None,
        }
    }

    /// Set the authentication token for Bearer auth.
    pub fn with_auth(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    /// Set the authentication token on an existing client.
    pub fn set_auth_token(&mut self, token: Option<String>) {
        self.auth_token = token;
    }

    /// Get the current authentication token, if set.
    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    /// Helper to add auth header to a request builder if token is set.
    fn add_auth_header(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref token) = self.auth_token {
            builder.header("Authorization", format!("Bearer {}", token))
        } else {
            builder
        }
    }

    /// Start the device authorization flow.
    ///
    /// Returns a device code and user code that the user can use to authorize.
    pub async fn device_authorize(&self) -> Result<DeviceAuthResponse, CentralApiError> {
        let url = format!("{}/auth/device/authorize", self.base_url);

        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        let data: DeviceAuthResponse = response.json().await?;
        Ok(data)
    }

    /// Poll for device token after user has authorized.
    ///
    /// Returns tokens if authorization is complete, or an error if pending/expired.
    pub async fn device_token(&self, device_code: &str) -> Result<DeviceTokenResponse, CentralApiError> {
        let url = format!("{}/auth/device/token", self.base_url);

        let body = serde_json::json!({
            "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
        });

        let response = self.client.post(&url).json(&body).send().await?;

        let status = response.status().as_u16();

        if status == 400 {
            let text = response.text().await.unwrap_or_default();
            if text.contains("authorization_pending") {
                return Err(CentralApiError::AuthorizationPending);
            } else if text.contains("expired") {
                return Err(CentralApiError::AuthorizationExpired);
            } else if text.contains("access_denied") {
                return Err(CentralApiError::AccessDenied);
            }
            return Err(CentralApiError::ServerError { status, message: text });
        }

        if !response.status().is_success() {
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        let data: DeviceTokenResponse = response.json().await?;
        Ok(data)
    }

    /// Refresh an access token using a refresh token.
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<RefreshTokenResponse, CentralApiError> {
        let url = format!("{}/auth/token/refresh", self.base_url);

        let body = serde_json::json!({
            "refresh_token": refresh_token,
            "grant_type": "refresh_token"
        });

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        let data: RefreshTokenResponse = response.json().await?;
        Ok(data)
    }

    /// Get available VPS plans.
    pub async fn get_vps_plans(&self) -> Result<Vec<VpsPlan>, CentralApiError> {
        let url = format!("{}/vps/plans", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        let data: Vec<VpsPlan> = response.json().await?;
        Ok(data)
    }

    /// Provision a new VPS.
    pub async fn provision_vps(&self, request: &ProvisionVpsRequest) -> Result<ProvisionVpsResponse, CentralApiError> {
        let url = format!("{}/vps/provision", self.base_url);

        let builder = self.client.post(&url).json(request);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        let data: ProvisionVpsResponse = response.json().await?;
        Ok(data)
    }

    /// Get the status of a VPS.
    pub async fn get_vps_status(&self, vps_id: &str) -> Result<VpsStatusResponse, CentralApiError> {
        let url = format!("{}/vps/{}/status", self.base_url, vps_id);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        let data: VpsStatusResponse = response.json().await?;
        Ok(data)
    }
}

impl Default for CentralApiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_central_api_client_new() {
        let client = CentralApiClient::new();
        assert_eq!(client.base_url, CENTRAL_API_URL);
        assert!(client.auth_token().is_none());
    }

    #[test]
    fn test_central_api_client_with_base_url() {
        let custom_url = "http://localhost:8080".to_string();
        let client = CentralApiClient::with_base_url(custom_url.clone());
        assert_eq!(client.base_url, custom_url);
    }

    #[test]
    fn test_central_api_client_with_auth() {
        let client = CentralApiClient::new().with_auth("test-token");
        assert_eq!(client.auth_token(), Some("test-token"));
    }

    #[test]
    fn test_central_api_client_set_auth_token() {
        let mut client = CentralApiClient::new();
        assert!(client.auth_token().is_none());

        client.set_auth_token(Some("new-token".to_string()));
        assert_eq!(client.auth_token(), Some("new-token"));

        client.set_auth_token(None);
        assert!(client.auth_token().is_none());
    }

    #[test]
    fn test_central_api_error_display() {
        let err = CentralApiError::ServerError {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("500"));
        assert!(display.contains("Internal Server Error"));
    }

    #[test]
    fn test_central_api_error_authorization_pending() {
        let err = CentralApiError::AuthorizationPending;
        let display = format!("{}", err);
        assert!(display.contains("pending"));
    }

    #[test]
    fn test_device_auth_response_serialization() {
        let response = DeviceAuthResponse {
            device_code: "device-123".to_string(),
            user_code: "ABCD-1234".to_string(),
            verification_uri: "https://auth.example.com/device".to_string(),
            expires_in: 1800,
            interval: 5,
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: DeviceAuthResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.device_code, response.device_code);
        assert_eq!(parsed.user_code, response.user_code);
        assert_eq!(parsed.verification_uri, response.verification_uri);
    }

    #[test]
    fn test_vps_plan_serialization() {
        let plan = VpsPlan {
            id: "plan-1".to_string(),
            name: "Basic".to_string(),
            description: Some("Basic plan".to_string()),
            price_monthly: 9.99,
            cpu_cores: 2,
            memory_mb: 2048,
            storage_gb: 50,
        };

        let json = serde_json::to_string(&plan).unwrap();
        let parsed: VpsPlan = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, plan.id);
        assert_eq!(parsed.name, plan.name);
        assert_eq!(parsed.price_monthly, plan.price_monthly);
    }

    #[tokio::test]
    async fn test_device_authorize_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.device_authorize().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_device_token_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.device_token("test-code").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_vps_plans_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.get_vps_plans().await;
        assert!(result.is_err());
    }
}
