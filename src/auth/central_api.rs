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

/// Response from the device code endpoint (POST /auth/device).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

/// Response from token endpoints (POST /auth/device/token and POST /auth/refresh).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u32,
    pub token_type: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

/// VPS plan information (GET /api/vps/plans).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VpsPlan {
    pub id: String,
    pub name: String,
    pub vcpus: u32,
    pub ram_mb: u32,
    pub disk_gb: u32,
    pub price_cents: u32,
}

/// Response from VPS provision endpoint (POST /api/vps/provision).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionResponse {
    pub vps_id: String,
    pub status: String,
}

/// Response from VPS status endpoint (GET /api/vps/status).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpsStatusResponse {
    pub vps_id: String,
    pub status: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
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

    /// Initiate the device code authentication flow.
    ///
    /// POST /auth/device
    ///
    /// Returns the device code response containing the user code and verification URL.
    pub async fn request_device_code(&self) -> Result<DeviceCodeResponse, CentralApiError> {
        let url = format!("{}/auth/device", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({}))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        // Get the response text first for better error messages
        let text = response.text().await.map_err(|e| {
            CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to read response: {}", e),
            }
        })?;

        // Try to parse the JSON
        serde_json::from_str::<DeviceCodeResponse>(&text).map_err(|e| {
            CentralApiError::ServerError {
                status: 0,
                message: format!("Invalid response format: {}. Response: {}", e, &text[..text.len().min(200)]),
            }
        })
    }

    /// Poll for the device token after user authorization.
    ///
    /// POST /auth/device/token
    ///
    /// Returns the token response on success, or specific errors for pending/denied states.
    pub async fn poll_device_token(&self, device_code: &str) -> Result<TokenResponse, CentralApiError> {
        let url = format!("{}/auth/device/token", self.base_url);

        let body = serde_json::json!({
            "device_code": device_code,
        });

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status().as_u16();

        if response.status().is_success() {
            let data: TokenResponse = response.json().await?;
            return Ok(data);
        }

        // Handle OAuth2 device flow specific errors
        let text = response.text().await.unwrap_or_default();
        if text.contains("authorization_pending") {
            return Err(CentralApiError::AuthorizationPending);
        } else if text.contains("expired") {
            return Err(CentralApiError::AuthorizationExpired);
        } else if text.contains("access_denied") {
            return Err(CentralApiError::AccessDenied);
        }

        Err(CentralApiError::ServerError { status, message: text })
    }

    /// Refresh an access token using a refresh token.
    ///
    /// POST /auth/refresh
    ///
    /// Returns a new token response with fresh access and refresh tokens.
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, CentralApiError> {
        let url = format!("{}/auth/refresh", self.base_url);

        let body = serde_json::json!({
            "refresh_token": refresh_token,
        });

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        let data: TokenResponse = response.json().await?;
        Ok(data)
    }

    /// Fetch available VPS plans.
    ///
    /// GET /api/vps/plans
    ///
    /// Returns a list of available VPS plans.
    pub async fn fetch_vps_plans(&self) -> Result<Vec<VpsPlan>, CentralApiError> {
        let url = format!("{}/api/vps/plans", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        let plans: Vec<VpsPlan> = response.json().await?;
        Ok(plans)
    }

    /// Provision a new VPS.
    ///
    /// POST /api/vps/provision
    ///
    /// Requires authentication. Returns the provision response with VPS ID and initial status.
    pub async fn provision_vps(&self, plan_id: &str) -> Result<ProvisionResponse, CentralApiError> {
        let url = format!("{}/api/vps/provision", self.base_url);

        let body = serde_json::json!({
            "plan_id": plan_id,
        });

        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CentralApiError::ServerError { status, message });
        }

        let data: ProvisionResponse = response.json().await?;
        Ok(data)
    }

    /// Get the status of the user's VPS.
    ///
    /// GET /api/vps/status
    ///
    /// Requires authentication. Returns the current VPS status including hostname, IP, and URL.
    pub async fn fetch_vps_status(&self) -> Result<VpsStatusResponse, CentralApiError> {
        let url = format!("{}/api/vps/status", self.base_url);

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
    fn test_central_api_client_default() {
        let client = CentralApiClient::default();
        assert_eq!(client.base_url, CENTRAL_API_URL);
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
    fn test_central_api_error_authorization_expired() {
        let err = CentralApiError::AuthorizationExpired;
        let display = format!("{}", err);
        assert!(display.contains("expired"));
    }

    #[test]
    fn test_central_api_error_access_denied() {
        let err = CentralApiError::AccessDenied;
        let display = format!("{}", err);
        assert!(display.contains("denied"));
    }

    #[test]
    fn test_device_code_response_deserialize() {
        let json = r#"{
            "device_code": "dev-code-123",
            "user_code": "ABCD-1234",
            "verification_uri": "https://example.com/verify",
            "expires_in": 900,
            "interval": 5
        }"#;

        let response: DeviceCodeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.device_code, "dev-code-123");
        assert_eq!(response.user_code, "ABCD-1234");
        assert_eq!(response.verification_uri, "https://example.com/verify");
        assert_eq!(response.expires_in, 900);
        assert_eq!(response.interval, 5);
    }

    #[test]
    fn test_token_response_deserialize() {
        let json = r#"{
            "access_token": "access-123",
            "refresh_token": "refresh-456",
            "expires_in": 3600,
            "token_type": "Bearer"
        }"#;

        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, "access-123");
        assert_eq!(response.refresh_token, "refresh-456");
        assert_eq!(response.expires_in, 3600);
        assert_eq!(response.token_type, "Bearer");
        assert!(response.user_id.is_none());
        assert!(response.username.is_none());
    }

    #[test]
    fn test_token_response_deserialize_with_user_info() {
        let json = r#"{
            "access_token": "access-123",
            "refresh_token": "refresh-456",
            "expires_in": 3600,
            "token_type": "Bearer",
            "user_id": "user-789",
            "username": "testuser"
        }"#;

        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.user_id, Some("user-789".to_string()));
        assert_eq!(response.username, Some("testuser".to_string()));
    }

    #[test]
    fn test_vps_plan_deserialize() {
        let json = r#"{
            "id": "plan-small",
            "name": "Small",
            "vcpus": 1,
            "ram_mb": 1024,
            "disk_gb": 25,
            "price_cents": 500
        }"#;

        let plan: VpsPlan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.id, "plan-small");
        assert_eq!(plan.name, "Small");
        assert_eq!(plan.vcpus, 1);
        assert_eq!(plan.ram_mb, 1024);
        assert_eq!(plan.disk_gb, 25);
        assert_eq!(plan.price_cents, 500);
    }

    #[test]
    fn test_vps_plan_serialize() {
        let plan = VpsPlan {
            id: "plan-medium".to_string(),
            name: "Medium".to_string(),
            vcpus: 2,
            ram_mb: 2048,
            disk_gb: 50,
            price_cents: 1000,
        };

        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("\"id\":\"plan-medium\""));
        assert!(json.contains("\"vcpus\":2"));
    }

    #[test]
    fn test_provision_response_deserialize() {
        let json = r#"{
            "vps_id": "vps-abc123",
            "status": "provisioning"
        }"#;

        let response: ProvisionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.vps_id, "vps-abc123");
        assert_eq!(response.status, "provisioning");
    }

    #[test]
    fn test_vps_status_response_deserialize_minimal() {
        let json = r#"{
            "vps_id": "vps-abc123",
            "status": "provisioning"
        }"#;

        let response: VpsStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.vps_id, "vps-abc123");
        assert_eq!(response.status, "provisioning");
        assert!(response.hostname.is_none());
        assert!(response.ip.is_none());
        assert!(response.url.is_none());
    }

    #[test]
    fn test_vps_status_response_deserialize_full() {
        let json = r#"{
            "vps_id": "vps-abc123",
            "status": "running",
            "hostname": "vps-abc123.spoq.io",
            "ip": "192.168.1.100",
            "url": "https://vps-abc123.spoq.io:8000"
        }"#;

        let response: VpsStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.vps_id, "vps-abc123");
        assert_eq!(response.status, "running");
        assert_eq!(response.hostname, Some("vps-abc123.spoq.io".to_string()));
        assert_eq!(response.ip, Some("192.168.1.100".to_string()));
        assert_eq!(response.url, Some("https://vps-abc123.spoq.io:8000".to_string()));
    }

    // Async tests for HTTP methods (with invalid server to test error handling)
    #[tokio::test]
    async fn test_request_device_code_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.request_device_code().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_poll_device_token_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.poll_device_token("test-code").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_refresh_token_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.refresh_token("test-refresh").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_vps_plans_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.fetch_vps_plans().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_provision_vps_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client.provision_vps("plan-small").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_vps_status_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client.fetch_vps_status().await;
        assert!(result.is_err());
    }
}
