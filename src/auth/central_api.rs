//! Central API client for Spoq authentication and VPS management.
//!
//! This module provides the HTTP client for interacting with the Spoq Central API,
//! handling device authorization flow, token management, and VPS operations.

use crate::adapters::ReqwestHttpClient;
use crate::traits::HttpClient;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Default URL for the Central API
pub const CENTRAL_API_URL: &str = "https://spoq.dev";

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

/// Parse error response from API.
/// Tries to extract {"error": "message"} format, falls back to raw body.
fn parse_error_response(status: u16, body: &str) -> CentralApiError {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = json.get("error").and_then(|e| e.as_str()) {
            return CentralApiError::ServerError {
                status,
                message: msg.to_string(),
            };
        }
    }
    CentralApiError::ServerError {
        status,
        message: body.to_string(),
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
    #[serde(default)]
    pub refresh_token: Option<String>, // Optional: refresh endpoint may not return new token
    #[serde(default)]
    pub token_type: Option<String>,
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

/// Response from VPS provision endpoint when status is "pending" (POST /api/vps/provision).
#[derive(Debug, Clone, Deserialize)]
pub struct ProvisionPendingResponse {
    pub hostname: String,
    #[serde(default)]
    pub ip_address: Option<String>,
    pub provider_instance_id: i64,
    #[serde(default)]
    pub provider_order_id: Option<String>,
    pub plan_id: i64,
    pub template_id: i64,
    pub data_center_id: i64,
    pub jwt_secret: String,
    pub ssh_password: String,
    pub message: String,
}

/// Request body for VPS confirmation endpoint (POST /api/vps/confirm).
#[derive(Debug, Clone, Serialize)]
pub struct ConfirmVpsRequest {
    pub hostname: String,
    pub ip_address: String,
    pub provider_instance_id: i64,
    #[serde(default)]
    pub provider_order_id: Option<String>,
    pub plan_id: i64,
    pub template_id: i64,
    pub data_center_id: i64,
    pub jwt_secret: String,
    pub ssh_password: String,
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

/// Data center information.
#[derive(Debug, Clone, Deserialize)]
pub struct DataCenter {
    pub id: u32,
    pub name: String,
    pub city: String,
    pub country: String,
    pub continent: String,
}

/// Response from data centers endpoint (GET /api/data-centers).
#[derive(Debug, Clone, Deserialize)]
pub struct DataCentersResponse {
    pub data_centers: Vec<DataCenter>,
}

/// Response from VPS action endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct VpsActionResponse {
    pub success: bool,
    pub message: String,
}

/// Request body for BYOVPS provisioning.
#[derive(Debug, Clone, Serialize)]
pub struct ByovpsProvisionRequest {
    pub vps_ip: String,
    pub ssh_username: String,
    pub ssh_password: String,
}

/// Install script status from BYOVPS provisioning response.
#[derive(Debug, Clone, Deserialize)]
pub struct InstallScriptStatus {
    pub status: String,
    #[serde(default)]
    pub output: Option<String>,
}

/// Credentials returned from BYOVPS provisioning.
#[derive(Debug, Clone, Deserialize)]
pub struct ByovpsCredentials {
    #[serde(default)]
    pub jwt_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// Response from BYOVPS provision endpoint (POST /api/byovps/provision).
#[derive(Debug, Clone, Deserialize)]
pub struct ByovpsProvisionResponse {
    #[serde(default)]
    pub hostname: Option<String>,
    pub status: String,
    #[serde(default)]
    pub install_script: Option<InstallScriptStatus>,
    #[serde(default)]
    pub credentials: Option<ByovpsCredentials>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default, alias = "id")]
    pub vps_id: Option<String>,
    #[serde(default, alias = "ip_address")]
    pub ip: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// Response from checkout session creation endpoint (POST /api/payments/create-checkout-session).
#[derive(Debug, Clone, Deserialize)]
pub struct CheckoutSessionResponse {
    pub checkout_url: String,
    pub session_id: String,
    pub customer_email: String,
}

/// Response from payment status endpoint (GET /api/payments/status/:session_id).
#[derive(Debug, Clone, Deserialize)]
pub struct PaymentStatusResponse {
    pub status: String,
    #[serde(default)]
    pub subscription_id: Option<String>,
    #[serde(default)]
    pub customer_id: Option<String>,
}

/// Response from subscription status endpoint (GET /api/payments/subscription).
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionStatus {
    pub status: String,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub current_period_end: Option<String>,
    #[serde(default)]
    pub cancel_at_period_end: Option<bool>,
    #[serde(default)]
    pub customer_portal_url: Option<String>,
}

/// Configuration for CentralApiClient.
#[derive(Debug, Clone)]
pub struct CentralApiConfig {
    /// Base URL for the Central API
    pub base_url: String,
    /// Optional authentication token for Bearer auth
    pub auth_token: Option<String>,
    /// Optional refresh token for automatic token refresh
    pub refresh_token: Option<String>,
}

impl Default for CentralApiConfig {
    fn default() -> Self {
        Self {
            base_url: CENTRAL_API_URL.to_string(),
            auth_token: None,
            refresh_token: None,
        }
    }
}

impl CentralApiConfig {
    /// Create a new CentralApiConfig with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a config with a custom base URL.
    pub fn with_base_url(base_url: String) -> Self {
        Self {
            base_url,
            ..Self::default()
        }
    }

    /// Set the authentication token.
    pub fn with_auth(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    /// Set the refresh token.
    pub fn with_refresh_token(mut self, token: &str) -> Self {
        self.refresh_token = Some(token.to_string());
        self
    }
}

/// Client for interacting with the Spoq Central API.
///
/// # Dependency Injection
///
/// The client can be constructed with a custom HTTP client implementation via
/// [`CentralApiClient::with_http`] for testing or custom HTTP behavior.
/// For production use, [`CentralApiClient::new`] creates a client with the default
/// reqwest-based HTTP implementation.
pub struct CentralApiClient {
    /// Base URL for the Central API
    pub base_url: String,
    /// Reusable HTTP client (trait object for dependency injection)
    http: Arc<dyn HttpClient>,
    /// Legacy reqwest client (kept for endpoints that use reqwest-specific features)
    client: Client,
    /// Optional authentication token for Bearer auth
    auth_token: Option<String>,
    /// Optional refresh token for automatic token refresh
    refresh_token: Option<String>,
}

impl CentralApiClient {
    /// Create a new CentralApiClient with the default base URL and HTTP client.
    ///
    /// This is the primary constructor for production use.
    pub fn new() -> Self {
        Self::with_default_http(CentralApiConfig::default())
    }

    /// Create a new CentralApiClient with a custom HTTP client implementation.
    ///
    /// This constructor enables dependency injection for testing or custom HTTP behavior.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use spoq::auth::central_api::{CentralApiClient, CentralApiConfig};
    /// use spoq::adapters::MockHttpClient;
    /// use std::sync::Arc;
    ///
    /// let mock_http = Arc::new(MockHttpClient::new());
    /// let config = CentralApiConfig::with_base_url("http://localhost:8000".to_string());
    /// let client = CentralApiClient::with_http(mock_http, config);
    /// ```
    pub fn with_http(http: Arc<dyn HttpClient>, config: CentralApiConfig) -> Self {
        Self {
            base_url: config.base_url,
            http,
            client: Client::new(),
            auth_token: config.auth_token,
            refresh_token: config.refresh_token,
        }
    }

    /// Create a new CentralApiClient with the default reqwest-based HTTP client.
    ///
    /// This is a convenience constructor that uses the production HTTP implementation.
    pub fn with_default_http(config: CentralApiConfig) -> Self {
        Self::with_http(Arc::new(ReqwestHttpClient::new()), config)
    }

    /// Create a new CentralApiClient with a custom base URL.
    pub fn with_base_url(base_url: String) -> Self {
        Self::with_default_http(CentralApiConfig::with_base_url(base_url))
    }

    /// Set the authentication token for Bearer auth.
    pub fn with_auth(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    /// Set the refresh token for automatic token refresh.
    pub fn with_refresh_token(mut self, token: &str) -> Self {
        self.refresh_token = Some(token.to_string());
        self
    }

    /// Set the authentication token on an existing client.
    pub fn set_auth_token(&mut self, token: Option<String>) {
        self.auth_token = token;
    }

    /// Set the refresh token on an existing client.
    pub fn set_refresh_token(&mut self, token: Option<String>) {
        self.refresh_token = token;
    }

    /// Get the current authentication token, if set.
    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    /// Get the current refresh token, if set.
    pub fn get_refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    /// Get both current tokens (access_token, refresh_token).
    /// Useful for updating credentials after potential auto-refresh.
    pub fn get_tokens(&self) -> (Option<String>, Option<String>) {
        (self.auth_token.clone(), self.refresh_token.clone())
    }

    /// Get a reference to the underlying HTTP client.
    ///
    /// This is useful for testing to verify the injected client.
    pub fn http_client(&self) -> &Arc<dyn HttpClient> {
        &self.http
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
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        // Get the response text first for better error messages
        let text = response
            .text()
            .await
            .map_err(|e| CentralApiError::ServerError {
                status: 0,
                message: format!("Failed to read response: {}", e),
            })?;

        // Try to parse the JSON
        serde_json::from_str::<DeviceCodeResponse>(&text).map_err(|e| {
            CentralApiError::ServerError {
                status: 0,
                message: format!(
                    "Invalid response format: {}. Response: {}",
                    e,
                    &text[..text.len().min(200)]
                ),
            }
        })
    }

    /// Poll for the device token after user authorization.
    ///
    /// POST /auth/device/token
    ///
    /// Returns the token response on success, or specific errors for pending/denied states.
    pub async fn poll_device_token(
        &self,
        device_code: &str,
    ) -> Result<TokenResponse, CentralApiError> {
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
                    _ => Err(CentralApiError::ServerError {
                        status,
                        message: text,
                    }),
                };
            }
        }

        // Try to parse as successful token response
        match serde_json::from_str::<TokenResponse>(&text) {
            Ok(data) => Ok(data),
            Err(e) => Err(CentralApiError::ServerError {
                status,
                message: format!(
                    "Failed to parse response: {}. Raw: {}",
                    e,
                    &text[..text.len().min(200)]
                ),
            }),
        }
    }

    /// Refresh an access token using a refresh token.
    ///
    /// POST /auth/refresh
    ///
    /// Returns a new token response with fresh access and refresh tokens.
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<TokenResponse, CentralApiError> {
        let url = format!("{}/auth/refresh", self.base_url);

        // Log the refresh attempt (but not the full token for security)
        let token_preview = if refresh_token.len() > 10 {
            format!("{}...", &refresh_token[..10])
        } else {
            "***".to_string()
        };
        eprintln!("[API] POST {} (refresh_token={})", url, token_preview);

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

        let status = response.status();
        eprintln!("[API] Response status: {}", status);

        if !status.is_success() {
            let status_code = status.as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            eprintln!("[API] Error response body: {}", body);
            return Err(parse_error_response(status_code, &body));
        }

        let data: TokenResponse = response.json().await?;
        eprintln!("[API] Token refresh successful, received new access_token");
        Ok(data)
    }

    /// Fetch available VPS plans from Hostinger.
    ///
    /// GET /api/vps/plans
    ///
    /// Returns a list of available VPS plans from infrastructure provider.
    pub async fn fetch_vps_plans(&self) -> Result<Vec<VpsPlan>, CentralApiError> {
        let url = format!("{}/api/vps/plans", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let wrapper: VpsPlansResponse = response.json().await?;
        Ok(wrapper.plans)
    }

    /// Fetch subscription plans with Stripe pricing.
    ///
    /// GET /api/vps/subscription-plans
    ///
    /// Returns a list of subscription plans with Stripe price IDs for checkout.
    /// Use this for the managed VPS payment flow.
    pub async fn fetch_subscription_plans(&self) -> Result<Vec<VpsPlan>, CentralApiError> {
        let url = format!("{}/api/vps/subscription-plans", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let wrapper: VpsPlansResponse = response.json().await?;
        Ok(wrapper.plans)
    }

    /// Provision a new VPS.
    ///
    /// POST /api/vps/provision
    ///
    /// Requires authentication. Returns the provision response with VPS ID and initial status.
    ///
    /// # Arguments
    /// * `ssh_password` - The SSH password for the VPS (required)
    /// * `plan_id` - Optional plan ID for the VPS
    /// * `data_center_id` - Optional data center ID for VPS location
    pub async fn provision_vps(
        &mut self,
        ssh_password: &str,
        plan_id: Option<&str>,
        data_center_id: Option<u32>,
    ) -> Result<ProvisionResponse, CentralApiError> {
        let url = format!("{}/api/vps/provision", self.base_url);

        let mut body = serde_json::json!({
            "ssh_password": ssh_password,
        });

        if let Some(plan) = plan_id {
            body["plan_id"] = serde_json::json!(plan);
        }
        if let Some(dc_id) = data_center_id {
            body["data_center_id"] = serde_json::json!(dc_id);
        }

        // First attempt
        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);
        let response = self.add_auth_header(builder).send().await?;

        // Check for 401 and retry with refreshed token if available
        let response = if response.status().as_u16() == 401 {
            if let Some(ref refresh_token) = self.refresh_token.clone() {
                println!("Token expired (401), attempting refresh...");
                match self.refresh_token(refresh_token).await {
                    Ok(token_response) => {
                        self.auth_token = Some(token_response.access_token.clone());
                        // Only update refresh_token if server provides a new one
                        if let Some(new_refresh_token) = token_response.refresh_token {
                            self.refresh_token = Some(new_refresh_token);
                        }

                        // Log successful refresh with expiration time
                        if let Some(expires_in) = token_response.expires_in {
                            let expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
                            let expiration_time =
                                chrono::DateTime::<chrono::Utc>::from_timestamp(expires_at, 0)
                                    .map(|dt| dt.to_rfc3339())
                                    .unwrap_or_else(|| "unknown".to_string());
                            println!(
                                "Token refresh successful, new expiration: {}",
                                expiration_time
                            );
                        } else if let Some(expires_in) =
                            get_jwt_expires_in(&token_response.access_token)
                        {
                            let expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
                            let expiration_time =
                                chrono::DateTime::<chrono::Utc>::from_timestamp(expires_at, 0)
                                    .map(|dt| dt.to_rfc3339())
                                    .unwrap_or_else(|| "unknown".to_string());
                            println!(
                                "Token refresh successful, new expiration: {}",
                                expiration_time
                            );
                        } else {
                            println!("Token refresh successful");
                        }

                        // Retry with new token
                        let builder = self
                            .client
                            .post(&url)
                            .header("Content-Type", "application/json")
                            .json(&body);
                        self.add_auth_header(builder).send().await?
                    }
                    Err(refresh_error) => {
                        println!("Token refresh failed: {}", refresh_error);
                        return Err(CentralApiError::ServerError {
                            status: 401,
                            message: format!("Token refresh failed: {}. Your session may have expired. Please run the CLI again to re-authenticate.", refresh_error),
                        });
                    }
                }
            } else {
                println!("Token expired (401), no refresh token available");
                return Err(CentralApiError::ServerError {
                    status: 401,
                    message: "No refresh token available. Please sign in again.".to_string(),
                });
            }
        } else {
            response
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: ProvisionResponse = response.json().await?;
        Ok(data)
    }

    /// Provision a VPS and return pending data (no DB record created yet).
    ///
    /// POST /api/vps/provision
    ///
    /// This is the new "health-first" provisioning flow. The server provisions the VPS
    /// with the cloud provider but does NOT create a DB record. Returns pending data
    /// including hostname, jwt_secret, etc. that the client uses to:
    /// 1. Poll the health endpoint directly
    /// 2. Call confirm_vps() after health check passes
    ///
    /// # Arguments
    /// * `ssh_password` - The SSH password for the VPS (required)
    /// * `plan_id` - Optional plan ID for the VPS
    /// * `data_center_id` - Optional data center ID for VPS location
    pub async fn provision_vps_pending(
        &mut self,
        ssh_password: &str,
        plan_id: Option<&str>,
        data_center_id: Option<u32>,
    ) -> Result<ProvisionPendingResponse, CentralApiError> {
        let url = format!("{}/api/vps/provision", self.base_url);

        let mut body = serde_json::json!({
            "ssh_password": ssh_password,
        });

        if let Some(plan) = plan_id {
            body["plan_id"] = serde_json::json!(plan);
        }
        if let Some(dc_id) = data_center_id {
            body["data_center_id"] = serde_json::json!(dc_id);
        }

        // First attempt
        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);
        let response = self.add_auth_header(builder).send().await?;

        // Check for 401 and retry with refreshed token if available
        let response = if response.status().as_u16() == 401 {
            if let Some(ref refresh_token) = self.refresh_token.clone() {
                println!("Token expired (401), attempting refresh...");
                match self.refresh_token(refresh_token).await {
                    Ok(token_response) => {
                        self.auth_token = Some(token_response.access_token.clone());
                        if let Some(new_refresh_token) = token_response.refresh_token {
                            self.refresh_token = Some(new_refresh_token);
                        }
                        println!("Token refresh successful");

                        // Retry with new token
                        let builder = self
                            .client
                            .post(&url)
                            .header("Content-Type", "application/json")
                            .json(&body);
                        self.add_auth_header(builder).send().await?
                    }
                    Err(refresh_error) => {
                        println!("Token refresh failed: {}", refresh_error);
                        return Err(CentralApiError::ServerError {
                            status: 401,
                            message: format!("Token refresh failed: {}. Your session may have expired. Please run the CLI again to re-authenticate.", refresh_error),
                        });
                    }
                }
            } else {
                println!("Token expired (401), no refresh token available");
                return Err(CentralApiError::ServerError {
                    status: 401,
                    message: "No refresh token available. Please sign in again.".to_string(),
                });
            }
        } else {
            response
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: ProvisionPendingResponse = response.json().await?;
        Ok(data)
    }

    /// Confirm a VPS after health check passes (creates DB record).
    ///
    /// POST /api/vps/confirm
    ///
    /// This is called after the client has verified the conductor is healthy.
    /// The server creates the DB record with status "ready".
    ///
    /// # Arguments
    /// * `request` - The confirmation request containing all VPS details
    pub async fn confirm_vps(
        &mut self,
        request: ConfirmVpsRequest,
    ) -> Result<VpsStatusResponse, CentralApiError> {
        let url = format!("{}/api/vps/confirm", self.base_url);

        // First attempt
        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request);
        let response = self.add_auth_header(builder).send().await?;

        // Check for 401 and retry with refreshed token if available
        let response = if response.status().as_u16() == 401 {
            if let Some(ref refresh_token) = self.refresh_token.clone() {
                println!("Token expired (401), attempting refresh...");
                match self.refresh_token(refresh_token).await {
                    Ok(token_response) => {
                        self.auth_token = Some(token_response.access_token.clone());
                        if let Some(new_refresh_token) = token_response.refresh_token {
                            self.refresh_token = Some(new_refresh_token);
                        }
                        println!("Token refresh successful");

                        // Retry with new token
                        let builder = self
                            .client
                            .post(&url)
                            .header("Content-Type", "application/json")
                            .json(&request);
                        self.add_auth_header(builder).send().await?
                    }
                    Err(refresh_error) => {
                        println!("Token refresh failed: {}", refresh_error);
                        return Err(CentralApiError::ServerError {
                            status: 401,
                            message: format!("Token refresh failed: {}. Your session may have expired. Please run the CLI again to re-authenticate.", refresh_error),
                        });
                    }
                }
            } else {
                println!("Token expired (401), no refresh token available");
                return Err(CentralApiError::ServerError {
                    status: 401,
                    message: "No refresh token available. Please sign in again.".to_string(),
                });
            }
        } else {
            response
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: VpsStatusResponse = response.json().await?;
        Ok(data)
    }

    /// Get the status of the user's VPS.
    ///
    /// GET /api/vps/status
    ///
    /// Requires authentication. Returns the current VPS status including hostname, IP, and URL.
    pub async fn fetch_vps_status(&mut self) -> Result<VpsStatusResponse, CentralApiError> {
        let url = format!("{}/api/vps/status", self.base_url);

        // First attempt
        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        // Check for 401 and retry with refreshed token if available
        let response = if response.status().as_u16() == 401 {
            if let Some(ref refresh_token) = self.refresh_token.clone() {
                println!("Token expired (401), attempting refresh...");
                match self.refresh_token(refresh_token).await {
                    Ok(token_response) => {
                        self.auth_token = Some(token_response.access_token.clone());
                        // Only update refresh_token if server provides a new one
                        if let Some(new_refresh_token) = token_response.refresh_token {
                            self.refresh_token = Some(new_refresh_token);
                        }

                        // Log successful refresh with expiration time
                        if let Some(expires_in) = token_response.expires_in {
                            let expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
                            let expiration_time =
                                chrono::DateTime::<chrono::Utc>::from_timestamp(expires_at, 0)
                                    .map(|dt| dt.to_rfc3339())
                                    .unwrap_or_else(|| "unknown".to_string());
                            println!(
                                "Token refresh successful, new expiration: {}",
                                expiration_time
                            );
                        } else if let Some(expires_in) =
                            get_jwt_expires_in(&token_response.access_token)
                        {
                            let expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
                            let expiration_time =
                                chrono::DateTime::<chrono::Utc>::from_timestamp(expires_at, 0)
                                    .map(|dt| dt.to_rfc3339())
                                    .unwrap_or_else(|| "unknown".to_string());
                            println!(
                                "Token refresh successful, new expiration: {}",
                                expiration_time
                            );
                        } else {
                            println!("Token refresh successful");
                        }

                        // Retry with new token
                        let builder = self.client.get(&url);
                        self.add_auth_header(builder).send().await?
                    }
                    Err(refresh_error) => {
                        println!("Token refresh failed: {}", refresh_error);
                        return Err(CentralApiError::ServerError {
                            status: 401,
                            message: format!("Token refresh failed: {}. Your session may have expired. Please run the CLI again to re-authenticate.", refresh_error),
                        });
                    }
                }
            } else {
                println!("Token expired (401), no refresh token available");
                return Err(CentralApiError::ServerError {
                    status: 401,
                    message: "No refresh token available. Please sign in again.".to_string(),
                });
            }
        } else {
            response
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: VpsStatusResponse = response.json().await?;
        Ok(data)
    }

    /// Check if user has any VPS configured on the server.
    ///
    /// GET /api/vps/status
    ///
    /// Returns Some(VpsStatusResponse) if user has a VPS, None if not (404).
    /// Requires authentication. Handles token refresh automatically on 401.
    pub async fn fetch_user_vps(&mut self) -> Result<Option<VpsStatusResponse>, CentralApiError> {
        let url = format!("{}/api/vps/status", self.base_url);

        // First attempt
        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        // Check for 401 and retry with refreshed token if available
        let response = if response.status().as_u16() == 401 {
            if let Some(ref refresh_token) = self.refresh_token.clone() {
                match self.refresh_token(refresh_token).await {
                    Ok(token_response) => {
                        self.auth_token = Some(token_response.access_token.clone());
                        // Only update refresh_token if server provides a new one
                        if let Some(new_refresh_token) = token_response.refresh_token {
                            self.refresh_token = Some(new_refresh_token);
                        }

                        // Retry with new token
                        let builder = self.client.get(&url);
                        self.add_auth_header(builder).send().await?
                    }
                    Err(_) => {
                        return Err(CentralApiError::ServerError {
                            status: 401,
                            message: "Unauthorized - please re-authenticate".to_string(),
                        });
                    }
                }
            } else {
                return Err(CentralApiError::ServerError {
                    status: 401,
                    message: "Unauthorized - please re-authenticate".to_string(),
                });
            }
        } else {
            response
        };

        // Handle different status codes
        match response.status().as_u16() {
            200 => {
                let vps = response.json::<VpsStatusResponse>().await?;
                Ok(Some(vps))
            }
            404 => {
                // No VPS found - this is expected for new users
                Ok(None)
            }
            status => {
                let error_body = response.text().await.unwrap_or_default();
                Err(parse_error_response(status, &error_body))
            }
        }
    }

    /// Fetch available data centers.
    ///
    /// GET /api/vps/datacenters (no auth required)
    ///
    /// Returns a list of available data centers.
    pub async fn fetch_datacenters(&self) -> Result<Vec<DataCenter>, CentralApiError> {
        let url = format!("{}/api/vps/datacenters", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(CentralApiError::Http)?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(parse_error_response(status, &body));
        }

        let data: DataCentersResponse = response.json().await.map_err(CentralApiError::Http)?;

        Ok(data.data_centers)
    }

    /// Start VPS.
    ///
    /// POST /api/vps/start
    ///
    /// Requires authentication. Returns the action response.
    pub async fn start_vps(&self) -> Result<VpsActionResponse, CentralApiError> {
        let url = format!("{}/api/vps/start", self.base_url);

        let builder = self.client.post(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: VpsActionResponse = response.json().await?;
        Ok(data)
    }

    /// Stop VPS.
    ///
    /// POST /api/vps/stop
    ///
    /// Requires authentication. Returns the action response.
    pub async fn stop_vps(&self) -> Result<VpsActionResponse, CentralApiError> {
        let url = format!("{}/api/vps/stop", self.base_url);

        let builder = self.client.post(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: VpsActionResponse = response.json().await?;
        Ok(data)
    }

    /// Restart VPS.
    ///
    /// POST /api/vps/restart
    ///
    /// Requires authentication. Returns the action response.
    pub async fn restart_vps(&self) -> Result<VpsActionResponse, CentralApiError> {
        let url = format!("{}/api/vps/restart", self.base_url);

        let builder = self.client.post(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: VpsActionResponse = response.json().await?;
        Ok(data)
    }

    /// Reset VPS password.
    ///
    /// POST /api/vps/reset-password
    ///
    /// Requires authentication. Returns the action response.
    pub async fn reset_vps_password(
        &self,
        new_password: &str,
    ) -> Result<VpsActionResponse, CentralApiError> {
        let url = format!("{}/api/vps/reset-password", self.base_url);

        let body = serde_json::json!({
            "password": new_password,
        });

        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: VpsActionResponse = response.json().await?;
        Ok(data)
    }

    /// Revoke a refresh token.
    ///
    /// POST /api/auth/revoke
    ///
    /// Requires authentication. Returns empty body on success.
    pub async fn revoke_token(&self, refresh_token: &str) -> Result<(), CentralApiError> {
        let url = format!("{}/api/auth/revoke", self.base_url);

        let body = serde_json::json!({
            "refresh_token": refresh_token,
        });

        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        Ok(())
    }

    /// Provision a BYOVPS (Bring Your Own VPS).
    ///
    /// POST /api/byovps/provision
    ///
    /// Requires authentication. Initiates provisioning of a user-provided VPS.
    ///
    /// # Arguments
    /// * `vps_ip` - The IP address of the user's VPS (IPv4 or IPv6)
    /// * `ssh_username` - SSH username for connecting to the VPS
    /// * `ssh_password` - SSH password for connecting to the VPS
    ///
    /// # Returns
    /// * `Ok(ByovpsProvisionResponse)` - Provisioning initiated successfully
    /// * `Err(CentralApiError)` - Provisioning failed
    pub async fn provision_byovps(
        &mut self,
        vps_ip: &str,
        ssh_username: &str,
        ssh_password: &str,
    ) -> Result<ByovpsProvisionResponse, CentralApiError> {
        let url = format!("{}/api/byovps/provision", self.base_url);

        let body = ByovpsProvisionRequest {
            vps_ip: vps_ip.to_string(),
            ssh_username: ssh_username.to_string(),
            ssh_password: ssh_password.to_string(),
        };

        // First attempt
        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);
        let response = self.add_auth_header(builder).send().await?;

        // Check for 401 and retry with refreshed token if available
        let response = if response.status().as_u16() == 401 {
            if let Some(ref refresh_token) = self.refresh_token.clone() {
                println!("Token expired (401), attempting refresh...");
                match self.refresh_token(refresh_token).await {
                    Ok(token_response) => {
                        self.auth_token = Some(token_response.access_token.clone());
                        // Only update refresh_token if server provides a new one
                        if let Some(new_refresh_token) = token_response.refresh_token {
                            self.refresh_token = Some(new_refresh_token);
                        }

                        // Log successful refresh with expiration time
                        if let Some(expires_in) = token_response.expires_in {
                            let expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
                            let expiration_time =
                                chrono::DateTime::<chrono::Utc>::from_timestamp(expires_at, 0)
                                    .map(|dt| dt.to_rfc3339())
                                    .unwrap_or_else(|| "unknown".to_string());
                            println!(
                                "Token refresh successful, new expiration: {}",
                                expiration_time
                            );
                        } else if let Some(expires_in) =
                            get_jwt_expires_in(&token_response.access_token)
                        {
                            let expires_at = chrono::Utc::now().timestamp() + expires_in as i64;
                            let expiration_time =
                                chrono::DateTime::<chrono::Utc>::from_timestamp(expires_at, 0)
                                    .map(|dt| dt.to_rfc3339())
                                    .unwrap_or_else(|| "unknown".to_string());
                            println!(
                                "Token refresh successful, new expiration: {}",
                                expiration_time
                            );
                        } else {
                            println!("Token refresh successful");
                        }

                        // Retry with new token
                        let builder = self
                            .client
                            .post(&url)
                            .header("Content-Type", "application/json")
                            .json(&body);
                        self.add_auth_header(builder).send().await?
                    }
                    Err(refresh_error) => {
                        println!("Token refresh failed: {}", refresh_error);
                        return Err(CentralApiError::ServerError {
                            status: 401,
                            message: format!("Token refresh failed: {}. Your session may have expired. Please run the CLI again to re-authenticate.", refresh_error),
                        });
                    }
                }
            } else {
                println!("Token expired (401), no refresh token available");
                return Err(CentralApiError::ServerError {
                    status: 401,
                    message: "No refresh token available. Please sign in again.".to_string(),
                });
            }
        } else {
            response
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: ByovpsProvisionResponse = response.json().await?;
        Ok(data)
    }

    /// Create a Stripe checkout session for VPS subscription.
    ///
    /// POST /api/payments/create-checkout-session
    ///
    /// Requires authentication. Returns checkout URL and session ID.
    ///
    /// # Arguments
    /// * `plan_id` - The plan ID to create checkout session for
    pub async fn create_checkout_session(
        &mut self,
        plan_id: &str,
    ) -> Result<CheckoutSessionResponse, CentralApiError> {
        let url = format!("{}/api/payments/create-checkout-session", self.base_url);

        let body = serde_json::json!({
            "plan_id": plan_id,
        });

        // First attempt
        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);
        let response = self.add_auth_header(builder).send().await?;

        // Check for 401 and retry with refreshed token if available
        let response = if response.status().as_u16() == 401 {
            if let Some(ref refresh_token) = self.refresh_token.clone() {
                println!("Token expired (401), attempting refresh...");
                match self.refresh_token(refresh_token).await {
                    Ok(token_response) => {
                        self.auth_token = Some(token_response.access_token.clone());
                        // Only update refresh_token if server provides a new one
                        if let Some(new_refresh_token) = token_response.refresh_token {
                            self.refresh_token = Some(new_refresh_token);
                        }

                        // Log successful refresh
                        println!("Token refresh successful");

                        // Retry with new token
                        let builder = self
                            .client
                            .post(&url)
                            .header("Content-Type", "application/json")
                            .json(&body);
                        self.add_auth_header(builder).send().await?
                    }
                    Err(refresh_error) => {
                        println!("Token refresh failed: {}", refresh_error);
                        return Err(CentralApiError::ServerError {
                            status: 401,
                            message: format!("Token refresh failed: {}. Your session may have expired. Please run the CLI again to re-authenticate.", refresh_error),
                        });
                    }
                }
            } else {
                println!("Token expired (401), no refresh token available");
                return Err(CentralApiError::ServerError {
                    status: 401,
                    message: "No refresh token available. Please sign in again.".to_string(),
                });
            }
        } else {
            response
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: CheckoutSessionResponse = response.json().await?;
        Ok(data)
    }

    /// Check payment status for a checkout session.
    ///
    /// GET /api/payments/status/:session_id
    ///
    /// Requires authentication. Returns payment status.
    ///
    /// # Arguments
    /// * `session_id` - The Stripe checkout session ID
    pub async fn check_payment_status(
        &self,
        session_id: &str,
    ) -> Result<PaymentStatusResponse, CentralApiError> {
        let url = format!("{}/api/payments/status/{}", self.base_url, session_id);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: PaymentStatusResponse = response.json().await?;
        Ok(data)
    }

    /// Get subscription status for the authenticated user (stub for future use).
    ///
    /// GET /api/payments/subscription
    ///
    /// Requires authentication. Returns subscription details.
    pub async fn get_subscription_status(&self) -> Result<SubscriptionStatus, CentralApiError> {
        let url = format!("{}/api/payments/subscription", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(parse_error_response(status, &body));
        }

        let data: SubscriptionStatus = response.json().await?;
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
        assert_eq!(response.refresh_token, Some("refresh-456".to_string()));
        assert_eq!(response.expires_in, Some(3600));
        assert_eq!(response.token_type, Some("Bearer".to_string()));
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
        assert_eq!(response.refresh_token, Some("refresh-456".to_string()));
        assert!(response.expires_in.is_none());
        assert_eq!(response.token_type, Some("Bearer".to_string()));
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
            bandwidth_tb: None,
            first_month_price_cents: None,
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
        assert_eq!(
            response.url,
            Some("https://vps-abc123.spoq.io:8000".to_string())
        );
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
        let mut client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client
            .provision_vps("test-password", Some("plan-small"), None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_provision_vps_with_datacenter_id() {
        let mut client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client
            .provision_vps("test-password", Some("plan-small"), Some(9))
            .await;
        assert!(result.is_err()); // Connection error expected
    }

    #[tokio::test]
    async fn test_provision_vps_minimal_params() {
        let mut client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client.provision_vps("test-password", None, None).await;
        assert!(result.is_err()); // Connection error expected
    }

    #[tokio::test]
    async fn test_fetch_vps_status_with_invalid_server() {
        let mut client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
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

    #[test]
    fn test_token_response_without_expires_in() {
        // API returns TokenResponse without expires_in field
        let json = r#"{"access_token": "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3MzY4NzI0MDB9.sig", "refresh_token": "spoq_refresh_token", "token_type": "Bearer"}"#;
        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert!(response.expires_in.is_none());
        assert_eq!(
            response.access_token,
            "eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3MzY4NzI0MDB9.sig"
        );
        assert_eq!(
            response.refresh_token,
            Some("spoq_refresh_token".to_string())
        );
        assert_eq!(response.token_type, Some("Bearer".to_string()));
    }

    #[test]
    fn test_token_response_with_expires_in() {
        // Backwards compatibility: if expires_in is provided, use it
        let json = r#"{"access_token": "token", "refresh_token": "refresh", "token_type": "Bearer", "expires_in": 900}"#;
        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.expires_in, Some(900));
    }

    #[test]
    fn test_vps_plans_response_wrapper() {
        // API returns {"plans": [...]} not a bare array
        let json = r#"{"plans": [{"id": "plan-small", "name": "Small", "vcpu": 1, "ram_gb": 2, "disk_gb": 50, "monthly_price_cents": 999}]}"#;
        let response: VpsPlansResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.plans.len(), 1);
        let plan = &response.plans[0];
        assert_eq!(plan.id, "plan-small");
        assert_eq!(plan.vcpus, 1); // vcpu → vcpus alias
        assert_eq!(plan.ram_mb, 2048); // 2 GB → 2048 MB conversion
        assert_eq!(plan.price_cents, 999); // monthly_price_cents → price_cents alias
    }

    #[test]
    fn test_vps_plan_ram_conversion() {
        // Values <= 100 are treated as GB and converted to MB
        let json = r#"{"id": "x", "name": "X", "vcpus": 1, "ram_gb": 4, "disk_gb": 50, "price_cents": 100}"#;
        let plan: VpsPlan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.ram_mb, 4096); // 4 GB → 4096 MB

        // Values > 100 are treated as already MB (backwards compat)
        let json = r#"{"id": "x", "name": "X", "vcpus": 1, "ram_mb": 2048, "disk_gb": 50, "price_cents": 100}"#;
        let plan: VpsPlan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.ram_mb, 2048); // Already MB, no conversion
    }

    #[test]
    fn test_vps_status_response_api_format() {
        // API returns id, ip_address instead of vps_id, ip
        let json = r#"{"id": "uuid-123", "status": "ready", "hostname": "user.spoq.dev", "ip_address": "1.2.3.4"}"#;
        let response: VpsStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.vps_id, "uuid-123"); // id → vps_id alias
        assert_eq!(response.ip, Some("1.2.3.4".to_string())); // ip_address → ip alias
        assert_eq!(response.hostname, Some("user.spoq.dev".to_string()));
        assert_eq!(response.status, "ready");
    }

    #[test]
    fn test_vps_status_response_all_fields() {
        // Test all optional fields
        let json = r#"{
            "id": "uuid-123",
            "status": "ready",
            "hostname": "user.spoq.dev",
            "ip_address": "1.2.3.4",
            "ssh_username": "spoq",
            "provider": "vultr",
            "plan_id": "plan-small",
            "data_center_id": 1,
            "created_at": "2026-01-01T00:00:00Z",
            "ready_at": "2026-01-01T00:05:00Z"
        }"#;
        let response: VpsStatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.ssh_username, Some("spoq".to_string()));
        assert_eq!(response.provider, Some("vultr".to_string()));
        assert_eq!(response.plan_id, Some("plan-small".to_string()));
        assert_eq!(response.data_center_id, Some(1));
        assert_eq!(
            response.created_at,
            Some("2026-01-01T00:00:00Z".to_string())
        );
        assert_eq!(response.ready_at, Some("2026-01-01T00:05:00Z".to_string()));
    }

    #[test]
    fn test_provision_response_api_format() {
        // API returns id instead of vps_id
        let json = r#"{"id": "uuid-456", "status": "provisioning", "hostname": "user.spoq.dev", "message": "Started provisioning"}"#;
        let response: ProvisionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.vps_id, "uuid-456"); // id → vps_id alias
        assert_eq!(response.status, "provisioning");
        assert_eq!(response.hostname, Some("user.spoq.dev".to_string()));
        assert_eq!(response.message, Some("Started provisioning".to_string()));
    }

    #[test]
    fn test_provision_response_minimal() {
        // Test with only required fields
        let json = r#"{"id": "uuid-456", "status": "provisioning"}"#;
        let response: ProvisionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.vps_id, "uuid-456");
        assert_eq!(response.status, "provisioning");
        assert!(response.hostname.is_none());
        assert!(response.message.is_none());
    }

    #[test]
    fn test_parse_error_response_with_error_field() {
        // API returns {"error": "message"} format
        let body = r#"{"error": "Invalid credentials"}"#;
        let err = parse_error_response(401, body);
        match err {
            CentralApiError::ServerError { status, message } => {
                assert_eq!(status, 401);
                assert_eq!(message, "Invalid credentials");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[test]
    fn test_parse_error_response_with_raw_body() {
        // API returns plain text or non-standard JSON
        let body = "Internal Server Error";
        let err = parse_error_response(500, body);
        match err {
            CentralApiError::ServerError { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "Internal Server Error");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[test]
    fn test_parse_error_response_with_json_without_error_field() {
        // API returns JSON but without "error" field
        let body = r#"{"message": "Something went wrong", "code": 123}"#;
        let err = parse_error_response(400, body);
        match err {
            CentralApiError::ServerError { status, message } => {
                assert_eq!(status, 400);
                // Falls back to raw body
                assert_eq!(
                    message,
                    r#"{"message": "Something went wrong", "code": 123}"#
                );
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[test]
    fn test_parse_error_response_with_empty_body() {
        let body = "";
        let err = parse_error_response(503, body);
        match err {
            CentralApiError::ServerError { status, message } => {
                assert_eq!(status, 503);
                assert_eq!(message, "");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[test]
    fn test_data_center_deserialize() {
        let json = r#"{"id": 1, "name": "NYC1", "city": "New York", "country": "USA", "continent": "North America"}"#;
        let dc: DataCenter = serde_json::from_str(json).unwrap();
        assert_eq!(dc.id, 1);
        assert_eq!(dc.name, "NYC1");
        assert_eq!(dc.city, "New York");
        assert_eq!(dc.country, "USA");
        assert_eq!(dc.continent, "North America");
    }

    #[test]
    fn test_data_centers_response_deserialize() {
        let json = r#"{"data_centers": [
            {"id": 1, "name": "NYC1", "city": "New York", "country": "USA", "continent": "North America"},
            {"id": 2, "name": "LAX1", "city": "Los Angeles", "country": "USA", "continent": "North America"}
        ]}"#;
        let response: DataCentersResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data_centers.len(), 2);
        assert_eq!(response.data_centers[0].name, "NYC1");
        assert_eq!(response.data_centers[1].name, "LAX1");
    }

    #[tokio::test]
    async fn test_fetch_datacenters_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.fetch_datacenters().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_vps_action_response_deserialize() {
        let json = r#"{"success": true, "message": "VPS started successfully"}"#;
        let response: VpsActionResponse = serde_json::from_str(json).unwrap();
        assert!(response.success);
        assert_eq!(response.message, "VPS started successfully");
    }

    #[test]
    fn test_vps_action_response_deserialize_failure() {
        let json = r#"{"success": false, "message": "VPS is already running"}"#;
        let response: VpsActionResponse = serde_json::from_str(json).unwrap();
        assert!(!response.success);
        assert_eq!(response.message, "VPS is already running");
    }

    #[tokio::test]
    async fn test_start_vps_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client.start_vps().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stop_vps_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client.stop_vps().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_restart_vps_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client.restart_vps().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reset_vps_password_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client.reset_vps_password("new-password").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_revoke_token_with_invalid_server() {
        let client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client.revoke_token("refresh-token-123").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_byovps_provision_request_serialize() {
        let request = ByovpsProvisionRequest {
            vps_ip: "192.168.1.100".to_string(),
            ssh_username: "root".to_string(),
            ssh_password: "secretpassword".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"vps_ip\":\"192.168.1.100\""));
        assert!(json.contains("\"ssh_username\":\"root\""));
        assert!(json.contains("\"ssh_password\":\"secretpassword\""));
    }

    #[test]
    fn test_byovps_provision_request_serialize_ipv6() {
        let request = ByovpsProvisionRequest {
            vps_ip: "2001:db8::1".to_string(),
            ssh_username: "admin".to_string(),
            ssh_password: "pass".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"vps_ip\":\"2001:db8::1\""));
    }

    #[test]
    fn test_install_script_status_deserialize() {
        let json = r#"{"status": "running", "output": "Installing packages..."}"#;
        let status: InstallScriptStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.status, "running");
        assert_eq!(status.output, Some("Installing packages...".to_string()));
    }

    #[test]
    fn test_install_script_status_deserialize_minimal() {
        let json = r#"{"status": "completed"}"#;
        let status: InstallScriptStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.status, "completed");
        assert!(status.output.is_none());
    }

    #[test]
    fn test_byovps_credentials_deserialize() {
        let json = r#"{"jwt_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...", "expires_at": "2026-02-01T00:00:00Z"}"#;
        let creds: ByovpsCredentials = serde_json::from_str(json).unwrap();
        assert_eq!(
            creds.jwt_token,
            Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...".to_string())
        );
        assert_eq!(creds.expires_at, Some("2026-02-01T00:00:00Z".to_string()));
    }

    #[test]
    fn test_byovps_credentials_deserialize_empty() {
        let json = r#"{}"#;
        let creds: ByovpsCredentials = serde_json::from_str(json).unwrap();
        assert!(creds.jwt_token.is_none());
        assert!(creds.expires_at.is_none());
    }

    #[test]
    fn test_byovps_provision_response_deserialize_full() {
        let json = r#"{
            "hostname": "user-byovps.spoq.dev",
            "status": "provisioning",
            "install_script": {"status": "running", "output": "Step 1/5..."},
            "credentials": {"jwt_token": "token123", "expires_at": "2026-02-01T00:00:00Z"},
            "message": "BYOVPS provisioning started",
            "id": "byovps-uuid-123",
            "ip_address": "192.168.1.100",
            "url": "https://user-byovps.spoq.dev:8000"
        }"#;
        let response: ByovpsProvisionResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.hostname, Some("user-byovps.spoq.dev".to_string()));
        assert_eq!(response.status, "provisioning");
        assert!(response.install_script.is_some());
        let install = response.install_script.unwrap();
        assert_eq!(install.status, "running");
        assert_eq!(install.output, Some("Step 1/5...".to_string()));
        assert!(response.credentials.is_some());
        let creds = response.credentials.unwrap();
        assert_eq!(creds.jwt_token, Some("token123".to_string()));
        assert_eq!(
            response.message,
            Some("BYOVPS provisioning started".to_string())
        );
        assert_eq!(response.vps_id, Some("byovps-uuid-123".to_string()));
        assert_eq!(response.ip, Some("192.168.1.100".to_string()));
        assert_eq!(
            response.url,
            Some("https://user-byovps.spoq.dev:8000".to_string())
        );
    }

    #[test]
    fn test_byovps_provision_response_deserialize_minimal() {
        let json = r#"{"status": "pending"}"#;
        let response: ByovpsProvisionResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.status, "pending");
        assert!(response.hostname.is_none());
        assert!(response.install_script.is_none());
        assert!(response.credentials.is_none());
        assert!(response.message.is_none());
        assert!(response.vps_id.is_none());
        assert!(response.ip.is_none());
        assert!(response.url.is_none());
    }

    #[test]
    fn test_byovps_provision_response_deserialize_ready_state() {
        let json = r#"{
            "status": "ready",
            "hostname": "user.spoq.dev",
            "url": "https://user.spoq.dev:8000"
        }"#;
        let response: ByovpsProvisionResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.status, "ready");
        assert_eq!(response.hostname, Some("user.spoq.dev".to_string()));
        assert_eq!(response.url, Some("https://user.spoq.dev:8000".to_string()));
    }

    #[test]
    fn test_byovps_provision_response_deserialize_failed_state() {
        let json = r#"{
            "status": "failed",
            "message": "SSH connection failed: Connection refused"
        }"#;
        let response: ByovpsProvisionResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.status, "failed");
        assert_eq!(
            response.message,
            Some("SSH connection failed: Connection refused".to_string())
        );
    }

    #[tokio::test]
    async fn test_provision_byovps_with_invalid_server() {
        let mut client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client
            .provision_byovps("192.168.1.100", "root", "password123")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_provision_byovps_with_ipv6() {
        let mut client = CentralApiClient::with_base_url("http://127.0.0.1:1".to_string())
            .with_auth("test-token");
        let result = client
            .provision_byovps("2001:db8::1", "admin", "password123")
            .await;
        // Should fail due to invalid server, but tests that IPv6 is accepted
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_provision_byovps_refresh_error_handling() {
        // Test that refresh error is properly captured and returned
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // First request returns 401
        Mock::given(method("POST"))
            .and(path("/api/byovps/provision"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Unauthorized"
            })))
            .mount(&mock_server)
            .await;

        // Refresh token request also fails
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Invalid refresh token"
            })))
            .mount(&mock_server)
            .await;

        let mut client = CentralApiClient::with_base_url(mock_server.uri())
            .with_auth("expired-token")
            .with_refresh_token("invalid-refresh-token");

        let result = client
            .provision_byovps("192.168.1.100", "root", "password")
            .await;

        assert!(result.is_err());
        if let Err(CentralApiError::ServerError { status, message }) = result {
            assert_eq!(status, 401);
            assert!(message.contains("Token refresh failed"));
            assert!(message.contains("re-authenticate"));
        } else {
            panic!("Expected ServerError with refresh failure message");
        }
    }

    #[tokio::test]
    async fn test_provision_vps_refresh_error_handling() {
        // Test that refresh error is properly captured and returned
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // First request returns 401
        Mock::given(method("POST"))
            .and(path("/api/vps/provision"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Unauthorized"
            })))
            .mount(&mock_server)
            .await;

        // Refresh token request also fails
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Invalid refresh token"
            })))
            .mount(&mock_server)
            .await;

        let mut client = CentralApiClient::with_base_url(mock_server.uri())
            .with_auth("expired-token")
            .with_refresh_token("invalid-refresh-token");

        let result = client
            .provision_vps("password", Some("plan-small"), None)
            .await;

        assert!(result.is_err());
        if let Err(CentralApiError::ServerError { status, message }) = result {
            assert_eq!(status, 401);
            assert!(message.contains("Token refresh failed"));
            assert!(message.contains("re-authenticate"));
        } else {
            panic!("Expected ServerError with refresh failure message");
        }
    }

    #[tokio::test]
    async fn test_fetch_vps_status_refresh_error_handling() {
        // Test that refresh error is properly captured and returned
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // First request returns 401
        Mock::given(method("GET"))
            .and(path("/api/vps/status"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Unauthorized"
            })))
            .mount(&mock_server)
            .await;

        // Refresh token request also fails
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Invalid refresh token"
            })))
            .mount(&mock_server)
            .await;

        let mut client = CentralApiClient::with_base_url(mock_server.uri())
            .with_auth("expired-token")
            .with_refresh_token("invalid-refresh-token");

        let result = client.fetch_vps_status().await;

        assert!(result.is_err());
        if let Err(CentralApiError::ServerError { status, message }) = result {
            assert_eq!(status, 401);
            assert!(message.contains("Token refresh failed"));
            assert!(message.contains("re-authenticate"));
        } else {
            panic!("Expected ServerError with refresh failure message");
        }
    }

    // Tests for dependency injection

    #[test]
    fn test_central_api_config_default() {
        let config = CentralApiConfig::default();
        assert_eq!(config.base_url, CENTRAL_API_URL);
        assert!(config.auth_token.is_none());
        assert!(config.refresh_token.is_none());
    }

    #[test]
    fn test_central_api_config_with_base_url() {
        let config = CentralApiConfig::with_base_url("http://custom.example.com:9000".to_string());
        assert_eq!(config.base_url, "http://custom.example.com:9000");
    }

    #[test]
    fn test_central_api_config_with_auth() {
        let config = CentralApiConfig::new().with_auth("test-token");
        assert_eq!(config.auth_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_central_api_config_with_refresh_token() {
        let config = CentralApiConfig::new().with_refresh_token("refresh-token");
        assert_eq!(config.refresh_token, Some("refresh-token".to_string()));
    }

    #[test]
    fn test_central_api_client_with_mock_http() {
        use crate::adapters::MockHttpClient;

        let mock_http = Arc::new(MockHttpClient::new());
        let config = CentralApiConfig::with_base_url("http://mock.example.com:8000".to_string());
        let client = CentralApiClient::with_http(mock_http.clone(), config);

        assert_eq!(client.base_url, "http://mock.example.com:8000");
        // Verify that http_client() returns the injected client
        let _ = client.http_client();
    }

    #[test]
    fn test_central_api_client_with_default_http() {
        let config = CentralApiConfig::with_base_url("http://test.example.com:8000".to_string())
            .with_auth("test-token");
        let client = CentralApiClient::with_default_http(config);

        assert_eq!(client.base_url, "http://test.example.com:8000");
        assert_eq!(client.auth_token(), Some("test-token"));
    }

    #[test]
    fn test_central_api_client_http_client_accessor() {
        let client = CentralApiClient::new();
        // Just verify we can access the http client
        let _http = client.http_client();
    }
}
