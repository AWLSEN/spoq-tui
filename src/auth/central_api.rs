//! Central API client for Spoq authentication and VPS management.
//!
//! This module provides the HTTP client for interacting with the Spoq Central API,
//! handling device authorization flow, token management, and VPS operations.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
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
    /// User code for display - may be None if embedded in verification_uri
    #[serde(default)]
    pub user_code: Option<String>,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

/// Response from token endpoints (POST /auth/device/token and POST /auth/refresh).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    #[serde(default)]
    pub expires_in: Option<u32>, // API may not return this; decode from JWT
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

/// JWT claims for extracting expiration time.
#[derive(Deserialize)]
struct JwtClaims {
    exp: i64,
}

/// Extract the expiration time from a JWT access token.
///
/// Returns the number of seconds until the token expires, or None if the token
/// cannot be parsed or the expiration has already passed.
pub fn get_jwt_expires_in(access_token: &str) -> Option<u32> {
    let parts: Vec<&str> = access_token.split('.').collect();
    let payload = URL_SAFE_NO_PAD.decode(parts.get(1)?).ok()?;
    let claims: JwtClaims = serde_json::from_slice(&payload).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some((claims.exp - now).max(0) as u32)
}

/// Wrapper for VPS plans API response (GET /api/vps/plans).
/// The API returns {"plans": [...]} not a bare array.
#[derive(Debug, Clone, Deserialize)]
pub struct VpsPlansResponse {
    pub plans: Vec<VpsPlan>,
}

/// Deserialize ram_gb (from API) to ram_mb (internal representation).
/// API sends GB, we store MB. Values > 100 are assumed to already be in MB (backwards compat).
fn deserialize_ram_to_mb<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: u32 = serde::Deserialize::deserialize(deserializer)?;
    // API sends GB, we store MB. Values > 100 are already MB (backwards compat)
    if value <= 100 {
        Ok(value * 1024)
    } else {
        Ok(value)
    }
}

/// VPS plan information (GET /api/vps/plans).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VpsPlan {
    pub id: String,
    pub name: String,
    #[serde(alias = "vcpu")]
    pub vcpus: u32,
    #[serde(alias = "ram_gb", deserialize_with = "deserialize_ram_to_mb")]
    pub ram_mb: u32,
    pub disk_gb: u32,
    #[serde(alias = "monthly_price_cents")]
    pub price_cents: u32,
    #[serde(default)]
    pub bandwidth_tb: Option<u32>,
    #[serde(default)]
    pub first_month_price_cents: Option<u32>,
}

/// Response from VPS provision endpoint (POST /api/vps/provision).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionResponse {
    #[serde(alias = "id")]
    pub vps_id: String,
    pub status: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

/// Response from VPS status endpoint (GET /api/vps/status).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpsStatusResponse {
    #[serde(alias = "id")]
    pub vps_id: String,
    pub status: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default, alias = "ip_address")]
    pub ip: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub ssh_username: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub plan_id: Option<String>,
    #[serde(default)]
    pub data_center_id: Option<u32>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub ready_at: Option<String>,
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

        // Get system hostname for device identification
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "hostname": hostname }))
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
            "grant_type": "device_code",
        });

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status().as_u16();

        // Get response text for parsing
        let text = response.text().await.unwrap_or_default();

        // Check for OAuth2 error response format: {"error": "..."}
        if let Ok(error_response) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(error) = error_response.get("error").and_then(|e| e.as_str()) {
                return match error {
                    "authorization_pending" => Err(CentralApiError::AuthorizationPending),
                    "expired_token" | "expired" => Err(CentralApiError::AuthorizationExpired),
                    "access_denied" => Err(CentralApiError::AccessDenied),
                    "invalid_grant" => Err(CentralApiError::AuthorizationPending), // Treat as pending
                    _ => Err(CentralApiError::ServerError { status, message: text }),
                };
            }
        }

        // Try to parse as successful token response
        match serde_json::from_str::<TokenResponse>(&text) {
            Ok(data) => Ok(data),
            Err(e) => Err(CentralApiError::ServerError {
                status,
                message: format!("Failed to parse response: {}. Raw: {}", e, &text[..text.len().min(200)]),
            }),
        }
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

        let wrapper: VpsPlansResponse = response.json().await?;
        Ok(wrapper.plans)
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
        assert_eq!(response.user_code, Some("ABCD-1234".to_string()));
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
        assert_eq!(response.expires_in, Some(3600));
        assert_eq!(response.token_type, "Bearer");
        assert!(response.user_id.is_none());
        assert!(response.username.is_none());
    }

    #[test]
    fn test_token_response_deserialize_without_expires_in() {
        let json = r#"{
            "access_token": "access-123",
            "refresh_token": "refresh-456",
            "token_type": "Bearer"
        }"#;

        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, "access-123");
        assert_eq!(response.refresh_token, "refresh-456");
        assert!(response.expires_in.is_none());
        assert_eq!(response.token_type, "Bearer");
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

    #[test]
    fn test_get_jwt_expires_in_valid_token() {
        // Create a valid JWT with exp claim set to 1 hour in the future
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let exp = now + 3600; // 1 hour from now

        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{}}}"#, exp));
        let signature = URL_SAFE_NO_PAD.encode("fake-signature");
        let token = format!("{}.{}.{}", header, payload, signature);

        let result = get_jwt_expires_in(&token);
        assert!(result.is_some());
        // Should be close to 3600, allow some tolerance for test execution time
        let expires_in = result.unwrap();
        assert!(expires_in >= 3590 && expires_in <= 3600);
    }

    #[test]
    fn test_get_jwt_expires_in_expired_token() {
        // Create a JWT with exp claim in the past
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let exp = now - 3600; // 1 hour ago

        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{}}}"#, exp));
        let signature = URL_SAFE_NO_PAD.encode("fake-signature");
        let token = format!("{}.{}.{}", header, payload, signature);

        let result = get_jwt_expires_in(&token);
        assert!(result.is_some());
        // Should return 0 for expired token
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_get_jwt_expires_in_invalid_token() {
        // Test with invalid token format
        assert!(get_jwt_expires_in("not-a-jwt").is_none());
        assert!(get_jwt_expires_in("only.two").is_none());
        assert!(get_jwt_expires_in("").is_none());
    }

    #[test]
    fn test_get_jwt_expires_in_invalid_payload() {
        // Token with invalid base64 payload
        assert!(get_jwt_expires_in("header.!!!invalid-base64!!!.signature").is_none());
    }

    #[test]
    fn test_get_jwt_expires_in_missing_exp_claim() {
        // Create a JWT without exp claim
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(r#"{"sub":"user123"}"#);
        let signature = URL_SAFE_NO_PAD.encode("fake-signature");
        let token = format!("{}.{}.{}", header, payload, signature);

        let result = get_jwt_expires_in(&token);
        assert!(result.is_none());
    }
}
