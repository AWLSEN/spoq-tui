//! Conductor API client for backend communication.
//!
//! This module provides the HTTP client for interacting with the Conductor backend,
//! including streaming responses via Server-Sent Events (SSE).

use crate::adapters::ReqwestHttpClient;
use crate::debug::{DebugEvent, DebugEventKind, DebugEventSender, RawSseEventData};
use crate::events::SseEvent;
use crate::models::{
    CancelRequest, CancelResponse, Folder, GitHubRepo, Message, StreamRequest, Thread,
    ThreadDetailResponse, ThreadListResponse,
};
use crate::models::picker::{
    CloneResponse, SearchFoldersResponse, SearchReposResponse, SearchThreadsResponse,
};
use crate::sse::{SseParseError, SseParser};
use crate::state::Task;
use crate::traits::HttpClient;
use futures_util::stream::{self, Stream};
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

// macOS Keychain access for GitHub CLI OAuth tokens
#[cfg(target_os = "macos")]
use security_framework::passwords::get_generic_password;

/// Default URL for the Conductor API
pub const DEFAULT_CONDUCTOR_URL: &str = "http://100.85.185.33:8000";

/// Central API URL for token refresh
const CENTRAL_API_URL: &str = "https://spoq-api-production.up.railway.app";

/// Error type for Conductor client operations
#[derive(Debug)]
pub enum ConductorError {
    /// HTTP request failed
    Http(reqwest::Error),
    /// SSE parsing failed
    SseParse(SseParseError),
    /// JSON deserialization failed
    Json(serde_json::Error),
    /// Server returned an error status
    ServerError { status: u16, message: String },
    /// Endpoint not yet implemented
    NotImplemented(String),
}

impl std::fmt::Display for ConductorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConductorError::Http(e) => write!(f, "HTTP error: {}", e),
            ConductorError::SseParse(e) => write!(f, "SSE parse error: {}", e),
            ConductorError::Json(e) => write!(f, "JSON error: {}", e),
            ConductorError::ServerError { status, message } => {
                write!(f, "Server error ({}): {}", status, message)
            }
            ConductorError::NotImplemented(endpoint) => {
                write!(f, "Endpoint not implemented: {}", endpoint)
            }
        }
    }
}

impl std::error::Error for ConductorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConductorError::Http(e) => Some(e),
            ConductorError::SseParse(e) => Some(e),
            ConductorError::Json(e) => Some(e),
            ConductorError::ServerError { .. } => None,
            ConductorError::NotImplemented(_) => None,
        }
    }
}

impl From<reqwest::Error> for ConductorError {
    fn from(e: reqwest::Error) -> Self {
        ConductorError::Http(e)
    }
}

impl From<SseParseError> for ConductorError {
    fn from(e: SseParseError) -> Self {
        ConductorError::SseParse(e)
    }
}

impl From<serde_json::Error> for ConductorError {
    fn from(e: serde_json::Error) -> Self {
        ConductorError::Json(e)
    }
}

/// Token status from Conductor verification
#[derive(Debug, Clone, Deserialize)]
pub struct TokenStatus {
    pub installed: bool,
    pub authenticated: bool,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    pub checked_at: String,
}

/// Debug info from conductor for troubleshooting
#[derive(Debug, Clone, Deserialize)]
pub struct DebugInfo {
    pub home_dir: String,
    pub current_user: String,
    pub path: String,
}

/// Response from token verification endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct TokensVerifyResponse {
    pub github_cli: TokenStatus,
    /// Diagnostic info for debugging
    #[serde(default)]
    pub debug_info: Option<DebugInfo>,
}

/// Post-sync verification results
#[derive(Debug, Clone, Deserialize)]
pub struct SyncVerification {
    #[serde(default)]
    pub github_cli_works: Option<bool>,
    pub home_dir_used: String,
}

/// Response from token sync endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct SyncResponse {
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub synced: Option<Vec<String>>,
    #[serde(default)]
    pub verification: Option<SyncVerification>,
}

/// Configuration for ConductorClient.
#[derive(Debug, Clone)]
pub struct ConductorConfig {
    /// Base URL for the Conductor API
    pub base_url: String,
    /// Optional authentication token for Bearer auth
    pub auth_token: Option<String>,
    /// Optional refresh token for automatic token refresh
    pub refresh_token: Option<String>,
    /// Central API URL for token refresh
    pub central_api_url: String,
}

impl Default for ConductorConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_CONDUCTOR_URL.to_string(),
            auth_token: std::env::var("SPOQ_DEV_TOKEN").ok(),
            refresh_token: None,
            central_api_url: CENTRAL_API_URL.to_string(),
        }
    }
}

impl ConductorConfig {
    /// Create a new ConductorConfig with default values.
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

/// Client for interacting with the Conductor backend API.
///
/// Provides methods for streaming conversations, health checks, and cancellation.
///
/// # Dependency Injection
///
/// The client can be constructed with a custom HTTP client implementation via
/// [`ConductorClient::with_http`] for testing or custom HTTP behavior.
/// For production use, [`ConductorClient::new`] creates a client with the default
/// reqwest-based HTTP implementation.
pub struct ConductorClient {
    /// Base URL for the Conductor API
    pub base_url: String,
    /// Reusable HTTP client (trait object for dependency injection)
    http: Arc<dyn HttpClient>,
    /// Legacy reqwest client (kept for streaming which uses reqwest-specific features)
    client: Client,
    /// Optional authentication token for Bearer auth
    auth_token: Option<String>,
    /// Optional refresh token for automatic token refresh
    refresh_token: Option<String>,
    /// Central API URL for token refresh
    central_api_url: String,
}

/// Read GitHub CLI OAuth token using `gh auth token` command.
///
/// This is the official and most reliable way to get the GitHub token.
/// It works regardless of where gh stores the token (Keychain, file, env var).
fn read_github_cli_token() -> Option<String> {
    use std::process::Command;

    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;

    if output.status.success() {
        let token = String::from_utf8(output.stdout).ok()?;
        let token = token.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }

    None
}

/// Fallback: Read GitHub CLI OAuth token from macOS Keychain directly.
///
/// Used only if `gh auth token` is not available.
#[cfg(target_os = "macos")]
fn read_github_keychain_token() -> Option<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    // GitHub CLI stores token with service name "gh:github.com"
    // Account name varies by gh version - try multiple options
    let username = std::env::var("USER").unwrap_or_default();
    let accounts_to_try = ["", &username, ":"];

    for account in accounts_to_try {
        if let Ok(password_bytes) = get_generic_password("gh:github.com", account) {
            let raw = match String::from_utf8(password_bytes.to_vec()) {
                Ok(s) => s,
                Err(_) => continue,
            };

            // GitHub CLI wraps the token with "go-keyring-base64:" prefix
            let token = if let Some(b64) = raw.strip_prefix("go-keyring-base64:") {
                // Decode the base64 to get the actual token
                STANDARD.decode(b64).ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
            } else {
                // Token stored directly without encoding
                Some(raw)
            };

            if token.is_some() {
                return token;
            }
        }
    }

    None
}

/// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
fn read_github_keychain_token() -> Option<String> {
    None
}

/// Inject oauth_token into hosts.yml YAML content
///
/// Finds the github.com section and adds oauth_token after the user line.
fn inject_oauth_token_into_hosts_yml(content: &str, token: &str) -> String {
    let mut result = String::new();
    let mut in_github_section = false;
    let mut token_injected = false;

    for line in content.lines() {
        result.push_str(line);
        result.push('\n');

        // Check if we're entering the github.com section
        if line.trim_start().starts_with("github.com:") {
            in_github_section = true;
            continue;
        }

        // If we're in github.com section and see user: line, inject token after it
        if in_github_section && !token_injected {
            let trimmed = line.trim_start();
            if trimmed.starts_with("user:") && line.starts_with(char::is_whitespace) {
                // Use same indentation as the user line
                let indent: String = line.chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();
                result.push_str(&indent);
                result.push_str("oauth_token: ");
                result.push_str(token);
                result.push('\n');
                token_injected = true;
            } else if !line.starts_with(char::is_whitespace) && !trimmed.is_empty() {
                // Exited github.com section
                in_github_section = false;
            }
        }
    }

    // If no user: line found, inject after github.com:
    if !token_injected && content.contains("github.com:") {
        result.clear();
        for line in content.lines() {
            result.push_str(line);
            result.push('\n');
            if line.trim_start().starts_with("github.com:") {
                result.push_str("    oauth_token: ");
                result.push_str(token);
                result.push('\n');
            }
        }
    }

    result
}

/// Read local token files for syncing to VPS
///
/// # Arguments
/// * `sync_type` - What to sync: "github_cli" or "all"
///
/// # Returns
/// JSON object containing token data to send to Conductor
fn read_local_tokens(sync_type: &str) -> Result<serde_json::Value, ConductorError> {
    let home = std::env::var("HOME").map_err(|_| ConductorError::ServerError {
        status: 500,
        message: "HOME environment variable not set".to_string(),
    })?;

    let mut data = serde_json::Map::new();

    // Read GitHub CLI tokens
    if sync_type == "github_cli" || sync_type == "all" {
        let gh_dir = PathBuf::from(&home).join(".config").join("gh");
        let hosts_yml_path = gh_dir.join("hosts.yml");

        if hosts_yml_path.exists() {
            let mut contents =
                fs::read_to_string(&hosts_yml_path).map_err(|e| ConductorError::ServerError {
                    status: 500,
                    message: format!("Failed to read ~/.config/gh/hosts.yml: {}", e),
                })?;

            // If hosts.yml doesn't contain oauth_token, get token from gh CLI or Keychain
            if !contents.contains("oauth_token:") {
                // Primary: Use `gh auth token` (official, always works if gh is authenticated)
                // Fallback: Read directly from Keychain (for edge cases)
                let token = read_github_cli_token().or_else(|| {
                    #[cfg(target_os = "macos")]
                    {
                        read_github_keychain_token()
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        None
                    }
                });

                if let Some(token) = token {
                    // Inject token into hosts.yml content before sending
                    contents = inject_oauth_token_into_hosts_yml(&contents, &token);
                }
            }

            let mut gh_data = serde_json::Map::new();
            gh_data.insert("hosts_yml".to_string(), serde_json::Value::String(contents));

            // Also read config.yml if it exists
            let config_yml_path = gh_dir.join("config.yml");
            if config_yml_path.exists() {
                if let Ok(config_contents) = fs::read_to_string(&config_yml_path) {
                    gh_data.insert(
                        "config_yml".to_string(),
                        serde_json::Value::String(config_contents),
                    );
                }
            }

            data.insert("github_cli".to_string(), serde_json::Value::Object(gh_data));
        }
    }

    // Read Codex tokens
    if sync_type == "codex" || sync_type == "all" {
        let codex_auth_path = PathBuf::from(&home).join(".codex").join("auth.json");

        if codex_auth_path.exists() {
            let contents =
                fs::read_to_string(&codex_auth_path).map_err(|e| ConductorError::ServerError {
                    status: 500,
                    message: format!("Failed to read ~/.codex/auth.json: {}", e),
                })?;

            let mut codex_data = serde_json::Map::new();
            codex_data.insert("auth_json".to_string(), serde_json::Value::String(contents));

            data.insert("codex".to_string(), serde_json::Value::Object(codex_data));
        }
    }

    Ok(serde_json::Value::Object(data))
}

impl ConductorClient {
    /// Create a new ConductorClient with the default base URL and HTTP client.
    ///
    /// If `SPOQ_DEV_TOKEN` environment variable is set, it will be used
    /// as the Bearer token for all requests (useful for local development).
    ///
    /// This is the primary constructor for production use.
    pub fn new() -> Self {
        Self::with_default_http(ConductorConfig::default())
    }

    /// Create a new ConductorClient with a custom HTTP client implementation.
    ///
    /// This constructor enables dependency injection for testing or custom HTTP behavior.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use spoq::conductor::{ConductorClient, ConductorConfig};
    /// use spoq::adapters::MockHttpClient;
    /// use std::sync::Arc;
    ///
    /// let mock_http = Arc::new(MockHttpClient::new());
    /// let config = ConductorConfig::with_base_url("http://localhost:8000".to_string());
    /// let client = ConductorClient::with_http(mock_http, config);
    /// ```
    pub fn with_http(http: Arc<dyn HttpClient>, config: ConductorConfig) -> Self {
        Self {
            base_url: config.base_url,
            http,
            client: Client::new(),
            auth_token: config.auth_token,
            refresh_token: config.refresh_token,
            central_api_url: config.central_api_url,
        }
    }

    /// Create a new ConductorClient with the default reqwest-based HTTP client.
    ///
    /// This is a convenience constructor that uses the production HTTP implementation.
    pub fn with_default_http(config: ConductorConfig) -> Self {
        Self::with_http(Arc::new(ReqwestHttpClient::new()), config)
    }

    /// Create a new ConductorClient with a custom base URL.
    ///
    /// If `SPOQ_DEV_TOKEN` environment variable is set, it will be used
    /// as the Bearer token for all requests (useful for local development).
    pub fn with_base_url(base_url: String) -> Self {
        Self::with_default_http(ConductorConfig::with_base_url(base_url))
    }

    /// Create a new ConductorClient with a custom URL (alias for with_base_url).
    pub fn with_url(base_url: &str) -> Self {
        Self::with_base_url(base_url.to_string())
    }

    /// Set the authentication token for Bearer auth.
    ///
    /// Returns self for method chaining.
    pub fn with_auth(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    /// Set the refresh token for automatic token refresh.
    ///
    /// Returns self for method chaining.
    pub fn with_refresh_token(mut self, token: &str) -> Self {
        self.refresh_token = Some(token.to_string());
        self
    }

    /// Get a reference to the underlying HTTP client.
    ///
    /// This is useful for testing to verify the injected client.
    pub fn http_client(&self) -> &Arc<dyn HttpClient> {
        &self.http
    }

    /// Set the authentication token on an existing client.
    pub fn set_auth_token(&mut self, token: Option<String>) {
        self.auth_token = token;
    }

    /// Get the current authentication token, if set.
    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    /// Get both current tokens (access_token, refresh_token).
    pub fn get_tokens(&self) -> (Option<String>, Option<String>) {
        (self.auth_token.clone(), self.refresh_token.clone())
    }

    /// Refresh the access token using the refresh token.
    ///
    /// Calls the central API's refresh endpoint and updates the stored tokens.
    async fn refresh_access_token(&mut self) -> Result<(), ConductorError> {
        let refresh_token =
            self.refresh_token
                .as_ref()
                .ok_or_else(|| ConductorError::ServerError {
                    status: 401,
                    message: "No refresh token available".to_string(),
                })?;

        let url = format!("{}/auth/refresh", self.central_api_url);
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
            return Err(ConductorError::ServerError {
                status: response.status().as_u16(),
                message: "Token refresh failed".to_string(),
            });
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            refresh_token: Option<String>,
        }

        let token_response: TokenResponse = response.json().await?;
        self.auth_token = Some(token_response.access_token);

        // Update refresh token if a new one was provided
        if let Some(new_refresh) = token_response.refresh_token {
            self.refresh_token = Some(new_refresh);
        }

        Ok(())
    }

    /// Helper to add auth header to a request builder if token is set.
    fn add_auth_header(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref token) = self.auth_token {
            builder.header("Authorization", format!("Bearer {}", token))
        } else {
            builder
        }
    }

    /// Stream a conversation response from the Conductor API.
    ///
    /// Sends a POST request to `/v1/stream` and returns a stream of SSE events.
    ///
    /// # Arguments
    /// * `request` - The stream request containing the prompt and optional thread info
    /// * `debug_tx` - Optional debug event sender for emitting raw SSE events
    ///
    /// # Returns
    /// A stream of `Result<SseEvent, ConductorError>` items
    pub async fn stream(
        &self,
        request: &StreamRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<SseEvent, ConductorError>> + Send>>, ConductorError>
    {
        self.stream_with_debug(request, None).await
    }

    /// Stream a conversation response from the Conductor API with optional debug events.
    ///
    /// This is the internal implementation that supports debug event emission.
    ///
    /// # Arguments
    /// * `request` - The stream request containing the prompt and optional thread info
    /// * `debug_tx` - Optional debug event sender for emitting raw SSE events
    ///
    /// # Returns
    /// A stream of `Result<SseEvent, ConductorError>` items
    pub async fn stream_with_debug(
        &self,
        request: &StreamRequest,
        debug_tx: Option<DebugEventSender>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<SseEvent, ConductorError>> + Send>>, ConductorError>
    {
        let url = format!("{}/v1/stream", self.base_url);

        // Debug: Log stream request details
        tracing::info!(
            "STREAM_REQUEST: url={}, has_auth={}, thread_id={:?}",
            url,
            self.auth_token.is_some(),
            request.thread_id
        );

        let builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(request);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        // Get the byte stream from the response
        let bytes_stream = response.bytes_stream();

        // Create an SSE parser and process the byte stream
        // Include debug_tx in the state tuple for emitting debug events
        // Use Vec<u8> buffer to avoid data loss when UTF-8 chars are split across TCP chunks
        let event_stream = stream::unfold(
            (bytes_stream, SseParser::new(), Vec::<u8>::new(), debug_tx),
            |(mut bytes_stream, mut parser, mut byte_buffer, debug_tx)| async move {
                loop {
                    // First, try to process any complete lines in the buffer
                    // Look for newline in the byte buffer
                    if let Some(newline_pos) = byte_buffer.iter().position(|&b| b == b'\n') {
                        // Extract the line bytes (including newline)
                        let line_bytes: Vec<u8> = byte_buffer.drain(..=newline_pos).collect();

                        // Decode to string using lossy conversion to handle edge cases
                        // where a multi-byte UTF-8 char might still be incomplete
                        let line = String::from_utf8_lossy(&line_bytes[..line_bytes.len() - 1])
                            .trim_end_matches('\r')
                            .to_string();

                        match parser.feed_line(&line) {
                            Ok(Some(sse_event)) => {
                                // Emit raw SSE debug event if debug channel is available
                                if let Some(ref tx) = debug_tx {
                                    let raw_data = RawSseEventData::new(
                                        sse_event.event_type_name(),
                                        format!("{:?}", sse_event),
                                    );
                                    let debug_event =
                                        DebugEvent::new(DebugEventKind::RawSseEvent(raw_data));
                                    let _ = tx.send(debug_event);
                                }

                                // Convert the sse::SseEvent to events::SseEvent
                                let event = convert_sse_event(sse_event);
                                return Some((
                                    Ok(event),
                                    (bytes_stream, parser, byte_buffer, debug_tx),
                                ));
                            }
                            Ok(None) => {
                                // Continue processing buffer
                                continue;
                            }
                            Err(e) => {
                                return Some((
                                    Err(ConductorError::SseParse(e)),
                                    (bytes_stream, parser, byte_buffer, debug_tx),
                                ));
                            }
                        }
                    }

                    // Need more data from the stream
                    match bytes_stream.next().await {
                        Some(Ok(chunk)) => {
                            // Append raw bytes to buffer - no UTF-8 conversion that could fail
                            byte_buffer.extend_from_slice(&chunk);
                            // Loop back to process the buffer
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(ConductorError::Http(e)),
                                (bytes_stream, parser, byte_buffer, debug_tx),
                            ));
                        }
                        None => {
                            // Stream ended - process any remaining data in buffer
                            if !byte_buffer.is_empty() {
                                let line = String::from_utf8_lossy(&byte_buffer)
                                    .trim_end_matches('\r')
                                    .to_string();
                                byte_buffer.clear();
                                match parser.feed_line(&line) {
                                    Ok(Some(sse_event)) => {
                                        // Emit raw SSE debug event if debug channel is available
                                        if let Some(ref tx) = debug_tx {
                                            let raw_data = RawSseEventData::new(
                                                sse_event.event_type_name(),
                                                format!("{:?}", sse_event),
                                            );
                                            let debug_event = DebugEvent::new(
                                                DebugEventKind::RawSseEvent(raw_data),
                                            );
                                            let _ = tx.send(debug_event);
                                        }

                                        let event = convert_sse_event(sse_event);
                                        return Some((
                                            Ok(event),
                                            (bytes_stream, parser, byte_buffer, debug_tx),
                                        ));
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        return Some((
                                            Err(ConductorError::SseParse(e)),
                                            (bytes_stream, parser, byte_buffer, debug_tx),
                                        ));
                                    }
                                }
                            }
                            return None;
                        }
                    }
                }
            },
        );

        Ok(Box::pin(event_stream))
    }

    /// Check if the Conductor API is healthy and reachable.
    ///
    /// # Returns
    /// `true` if the health endpoint returns 200 OK, `false` otherwise
    pub async fn health_check(&self) -> Result<bool, ConductorError> {
        let url = format!("{}/v1/health", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        Ok(response.status().is_success())
    }

    /// Verify tokens on the VPS via Conductor.
    ///
    /// Checks if Claude Code and GitHub CLI are installed and authenticated
    /// on the VPS by asking Conductor to run local verification commands.
    /// Automatically refreshes the access token if it expires (401).
    ///
    /// # Returns
    /// Token status for both Claude Code and GitHub CLI
    pub async fn verify_tokens(&mut self) -> Result<TokensVerifyResponse, ConductorError> {
        let url = format!("{}/v1/tokens/verify", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        // Check for 401 and try to refresh
        if response.status().as_u16() == 401 && self.refresh_token.is_some() {
            // Try to refresh the token
            self.refresh_access_token().await?;

            // Retry the request with new token
            let builder = self.client.get(&url);
            let response = self.add_auth_header(builder).send().await?;

            if response.status().is_success() {
                return Ok(response.json::<TokensVerifyResponse>().await?);
            } else {
                let status = response.status().as_u16();
                let text = response.text().await.unwrap_or_default();
                return Err(ConductorError::ServerError {
                    status,
                    message: text,
                });
            }
        }

        if response.status().is_success() {
            let result = response.json::<TokensVerifyResponse>().await?;
            Ok(result)
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(ConductorError::ServerError {
                status,
                message: text,
            })
        }
    }

    /// Sync local tokens to VPS via Conductor.
    ///
    /// Reads local token files and transfers them to the VPS via Conductor.
    /// Automatically refreshes the access token if it expires (401).
    ///
    /// Note: Claude CLI uses server-side OAuth and is not synced from the client.
    ///
    /// # Arguments
    /// * `sync_type` - What to sync: "github_cli" or "all"
    ///
    /// # Returns
    /// Full sync response including post-sync verification results
    pub async fn sync_tokens(&mut self, sync_type: &str) -> Result<SyncResponse, ConductorError> {
        let url = format!("{}/v1/tokens/sync", self.base_url);

        // Read local token files based on sync_type
        let data = read_local_tokens(sync_type)?;

        let body = serde_json::json!({
            "sync_type": sync_type,
            "data": data
        });

        let builder = self.client.post(&url).json(&body);
        let response = self.add_auth_header(builder).send().await?;

        // Check for 401 and try to refresh
        if response.status().as_u16() == 401 && self.refresh_token.is_some() {
            // Try to refresh the token
            self.refresh_access_token().await?;

            // Retry the request with new token
            let builder = self.client.post(&url).json(&body);
            let response = self.add_auth_header(builder).send().await?;

            if response.status().is_success() {
                return Ok(response.json::<SyncResponse>().await?);
            } else {
                let status = response.status().as_u16();
                let text = response.text().await.unwrap_or_default();
                return Err(ConductorError::ServerError {
                    status,
                    message: text,
                });
            }
        }

        if response.status().is_success() {
            Ok(response.json::<SyncResponse>().await?)
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(ConductorError::ServerError {
                status,
                message: text,
            })
        }
    }

    /// Cancel an active stream for a thread.
    ///
    /// Sends a POST request to `/v1/cancel` to stop the streaming response
    /// for the specified thread. The backend will send SIGTERM to the
    /// Claude CLI process group.
    ///
    /// # Arguments
    /// * `thread_id` - The thread ID whose stream should be cancelled
    ///
    /// # Returns
    /// * `Ok(CancelResponse)` - Cancellation was processed (check `is_cancelled()`)
    /// * `Err(ConductorError)` - Request failed
    pub async fn cancel_stream(&self, thread_id: &str) -> Result<CancelResponse, ConductorError> {
        let url = format!("{}/v1/cancel", self.base_url);

        let request = CancelRequest::new(thread_id.to_string());

        let builder = self.client.post(&url).json(&request);
        let response = self.add_auth_header(builder).send().await?;

        // Both 200 (cancelled) and 404 (not_found) are valid responses
        // that return JSON CancelResponse
        let status = response.status();
        if status.is_success() || status.as_u16() == 404 {
            Ok(response.json().await?)
        } else {
            let status_code = status.as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(ConductorError::ServerError {
                status: status_code,
                message,
            })
        }
    }

    /// Fetch all threads from the backend.
    ///
    /// # Returns
    /// A vector of threads, or an error if the request fails
    pub async fn fetch_threads(&self) -> Result<Vec<Thread>, ConductorError> {
        let url = format!("{}/v1/threads", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        let data: ThreadListResponse = response.json().await?;
        Ok(data.threads)
    }

    /// Fetch all folders from the backend.
    ///
    /// # Returns
    /// A vector of folders, or an error if the request fails
    pub async fn fetch_folders(&self) -> Result<Vec<Folder>, ConductorError> {
        let url = format!("{}/v1/folders", self.base_url);
        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }
        let folders: Vec<Folder> = response.json().await?;
        Ok(folders)
    }

    /// Fetch GitHub repositories from conductor.
    ///
    /// Returns top 10 most recent repos (personal + organization).
    ///
    /// # Returns
    /// A vector of GitHub repositories, or an error if the request fails
    pub async fn fetch_repos(&self) -> Result<Vec<GitHubRepo>, ConductorError> {
        let url = format!("{}/v1/repos?limit=10", self.base_url);

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        let repos: Vec<GitHubRepo> = response.json().await?;
        Ok(repos)
    }

    /// Fetch files in a directory from the conductor.
    ///
    /// # Arguments
    /// * `path` - The directory path to list files from
    /// * `search` - Optional search/filter string
    ///
    /// # Returns
    /// A vector of file entries, or an error if the request fails
    pub async fn fetch_files(
        &self,
        path: &str,
        search: Option<&str>,
    ) -> Result<Vec<crate::models::FileEntry>, ConductorError> {
        let mut url = format!("{}/v1/files?path={}", self.base_url, urlencoding::encode(path));
        if let Some(s) = search {
            url.push_str(&format!("&search={}", urlencoding::encode(s)));
        }

        // Log request details for debugging 404 errors
        tracing::info!(
            "fetch_files - URL: {}, base_url: {}, path: {}",
            url,
            self.base_url,
            path
        );

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        // Log response status
        let status = response.status();
        tracing::info!("fetch_files - Response status: {}", status);

        if !status.is_success() {
            let status_code = status.as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!(
                "fetch_files - Error: status={}, message={}",
                status_code,
                message
            );
            return Err(ConductorError::ServerError {
                status: status_code,
                message,
            });
        }

        let files: Vec<crate::models::FileEntry> = response.json().await?;
        tracing::info!("fetch_files - Success, got {} files", files.len());
        Ok(files)
    }

    /// Fetch all tasks from the backend.
    ///
    /// TODO: Expected endpoint: GET /v1/tasks
    ///
    /// # Returns
    /// A vector of tasks, or an error if the request fails
    pub async fn fetch_tasks(&self) -> Result<Vec<Task>, ConductorError> {
        // Stub: return empty vec for now
        Ok(Vec::new())
    }

    /// Fetch messages for a specific thread from the backend.
    ///
    /// TODO: Expected endpoint: GET /v1/threads/{id}/messages
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the thread to fetch messages for
    ///
    /// # Returns
    /// A vector of messages for the specified thread, or an error if the request fails
    pub async fn fetch_thread_messages(
        &self,
        _thread_id: &str,
    ) -> Result<Vec<Message>, ConductorError> {
        // Stub: return empty vec for now
        Ok(Vec::new())
    }

    /// Fetch a thread with its messages from the backend.
    ///
    /// GET /v1/threads/{id}?include_messages=true
    pub async fn fetch_thread_with_messages(
        &self,
        thread_id: &str,
    ) -> Result<ThreadDetailResponse, ConductorError> {
        let url = format!(
            "{}/v1/threads/{}?include_messages=true",
            self.base_url, thread_id
        );

        let builder = self.client.get(&url);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        let data: ThreadDetailResponse = response.json().await?;
        Ok(data)
    }

    /// Get a thread by ID (stub - will implement with REST API)
    #[allow(dead_code)]
    pub fn get_thread(&self, _thread_id: &str) -> Option<Thread> {
        // Stub: return None for now
        None
    }

    /// Get recent messages (stub - will implement with REST API)
    #[allow(dead_code)]
    pub fn get_recent_messages(&self) -> Vec<Message> {
        // Stub: return empty vec for now
        Vec::new()
    }

    /// Respond to a permission request from the assistant.
    ///
    /// POST /v1/permissions/{permission_id}
    ///
    /// # Arguments
    /// * `permission_id` - The ID of the permission request
    /// * `approved` - Whether to approve (true) or deny (false) the permission
    ///
    /// # Returns
    /// Ok(()) on success, or an error if the request fails
    pub async fn respond_to_permission(
        &self,
        permission_id: &str,
        approved: bool,
    ) -> Result<(), ConductorError> {
        let url = format!("{}/v1/permissions/{}", self.base_url, permission_id);

        let body = serde_json::json!({
            "approved": approved
        });

        let builder = self.client.post(&url).json(&body);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        Ok(())
    }

    /// Verify a thread via the REST endpoint.
    ///
    /// Calls `POST /v1/threads/{thread_id}/verify` to mark a thread as verified.
    /// The endpoint may return 404 if not implemented by the backend.
    ///
    /// # Returns
    /// - `Ok(true)` if the thread was successfully verified
    /// - `Ok(false)` if the response indicates verification failed
    /// - `Err(ConductorError::NotImplemented)` if the endpoint returns 404
    /// - `Err(ConductorError::ServerError)` for other errors
    pub async fn verify_thread(&self, thread_id: &str) -> Result<bool, ConductorError> {
        let url = format!("{}/v1/threads/{}/verify", self.base_url, thread_id);

        let builder = self.client.post(&url);
        let response = self.add_auth_header(builder).send().await?;

        let status = response.status();

        if status.as_u16() == 404 {
            return Err(ConductorError::NotImplemented(format!(
                "/v1/threads/{}/verify",
                thread_id
            )));
        }

        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError {
                status: status.as_u16(),
                message,
            });
        }

        // Parse the response to check if verified
        let body: serde_json::Value = response.json().await?;
        let verified = body
            .get("verified")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(verified)
    }

    /// Update the mode of a thread.
    ///
    /// Calls `PUT /v1/threads/{thread_id}/mode` to update the thread's mode.
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the thread to update
    /// * `mode` - The new mode for the thread
    ///
    /// # Returns
    /// - `Ok(())` on success
    /// - `Err(ConductorError::ServerError)` if the server returns an error (404, 400, etc.)
    pub async fn update_thread_mode(
        &self,
        thread_id: &str,
        mode: &str,
    ) -> Result<(), ConductorError> {
        let url = format!("{}/v1/threads/{}/mode", self.base_url, thread_id);

        let body = serde_json::json!({
            "mode": mode
        });

        let builder = self.client.put(&url).json(&body);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        Ok(())
    }

    /// Update the permission mode of a thread.
    ///
    /// Calls `PUT /v1/threads/{thread_id}/permission` to update the thread's permission mode.
    ///
    /// # Arguments
    /// * `thread_id` - The ID of the thread to update
    /// * `permission_mode` - The new permission mode for the thread ("default" | "plan" | "execution")
    ///
    /// # Returns
    /// - `Ok(())` on success
    /// - `Err(ConductorError::ServerError)` if the server returns an error (404, 400, etc.)
    pub async fn update_thread_permission(
        &self,
        thread_id: &str,
        permission_mode: &str,
    ) -> Result<(), ConductorError> {
        let url = format!("{}/v1/threads/{}/permission", self.base_url, thread_id);

        let body = serde_json::json!({
            "mode": permission_mode
        });

        let builder = self.client.put(&url).json(&body);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        Ok(())
    }

    // ==================== Unified Picker Search API ====================

    /// Search folders by name.
    ///
    /// GET /v1/search/folders?q={query}&limit={limit}
    ///
    /// # Arguments
    /// * `query` - Search query to filter folders by name
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// Search response containing matching folders
    pub async fn search_folders(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchFoldersResponse, ConductorError> {
        let url = format!("{}/v1/search/folders", self.base_url);

        let builder = self
            .client
            .get(&url)
            .query(&[("q", query), ("limit", &limit.to_string())]);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        // Conductor returns array directly, wrap in response struct
        let folders: Vec<crate::models::picker::FolderEntry> = response.json().await?;
        Ok(SearchFoldersResponse { folders })
    }

    /// Search threads by title.
    ///
    /// GET /v1/search/threads?q={query}&limit={limit}
    ///
    /// # Arguments
    /// * `query` - Search query to filter threads by title
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// Search response containing matching threads
    pub async fn search_threads(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchThreadsResponse, ConductorError> {
        let url = format!("{}/v1/search/threads", self.base_url);

        let builder = self
            .client
            .get(&url)
            .query(&[("q", query), ("limit", &limit.to_string())]);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        // Conductor returns array directly, wrap in response struct
        let threads: Vec<crate::models::picker::ThreadEntry> = response.json().await?;
        Ok(SearchThreadsResponse { threads })
    }

    /// Search GitHub repositories by name.
    ///
    /// GET /v1/search/repos?q={query}&limit={limit}
    ///
    /// # Arguments
    /// * `query` - Search query to filter repos by name
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// Search response containing matching repos with local_path if cloned
    pub async fn search_repos(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchReposResponse, ConductorError> {
        let url = format!("{}/v1/search/repos", self.base_url);

        let builder = self
            .client
            .get(&url)
            .query(&[("q", query), ("limit", &limit.to_string())]);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        // Conductor returns array directly, wrap in response struct
        let repos: Vec<crate::models::picker::RepoEntry> = response.json().await?;
        Ok(SearchReposResponse { repos })
    }

    /// Clone a GitHub repository to the workspace.
    ///
    /// POST /v1/clone with body {"repo": repo_name}
    ///
    /// This triggers the conductor to clone the repo using `gh repo clone`
    /// to the configured workspace root (default: ~/workspaces/programming).
    ///
    /// # Arguments
    /// * `repo_name` - Repository name in "owner/repo" format (e.g., "anthropics/claude-code")
    ///
    /// # Returns
    /// Clone response containing the local path where the repo was cloned
    pub async fn clone_repo(&self, repo_name: &str) -> Result<CloneResponse, ConductorError> {
        let url = format!("{}/v1/clone", self.base_url);

        let body = serde_json::json!({
            "repo": repo_name
        });

        let builder = self.client.post(&url).json(&body);
        let response = self.add_auth_header(builder).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ConductorError::ServerError { status, message });
        }

        let data: CloneResponse = response.json().await?;
        Ok(data)
    }
}

impl Default for ConductorClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert sse::SseEvent to events::SseEvent
///
/// The sse module has a simpler SseEvent type used during parsing,
/// while events module has the full typed event structure.
fn convert_sse_event(event: crate::sse::SseEvent) -> SseEvent {
    match event {
        crate::sse::SseEvent::Content { text, meta } => {
            SseEvent::Content(crate::events::ContentEvent {
                text,
                meta: crate::events::EventMeta {
                    seq: meta.seq,
                    timestamp: meta.timestamp,
                    session_id: meta.session_id,
                    thread_id: meta.thread_id,
                },
            })
        }
        crate::sse::SseEvent::ThreadInfo {
            thread_id,
            title: _,
        } => {
            // Map to UserMessageSaved as a proxy for thread info
            SseEvent::UserMessageSaved(crate::events::UserMessageSavedEvent {
                message_id: String::new(),
                thread_id,
            })
        }
        crate::sse::SseEvent::MessageInfo { message_id } => {
            SseEvent::Done(crate::events::DoneEvent {
                message_id: message_id.to_string(),
            })
        }
        crate::sse::SseEvent::Done => SseEvent::Done(crate::events::DoneEvent {
            message_id: String::new(),
        }),
        crate::sse::SseEvent::Error { message, code } => {
            SseEvent::Error(crate::events::ErrorEvent { message, code })
        }
        crate::sse::SseEvent::Ping => {
            // Ping/keepalive - emit empty content that will be filtered
            SseEvent::Content(crate::events::ContentEvent {
                text: String::new(),
                meta: crate::events::EventMeta::default(),
            })
        }
        crate::sse::SseEvent::SkillsInjected { skills } => {
            SseEvent::SkillsInjected(crate::events::SkillsInjectedEvent { skills })
        }
        crate::sse::SseEvent::OAuthConsentRequired {
            provider,
            url,
            skill_name,
        } => SseEvent::OAuthConsentRequired(crate::events::OAuthConsentRequiredEvent {
            provider,
            url,
            skill_name,
        }),
        crate::sse::SseEvent::ContextCompacted {
            messages_removed,
            tokens_freed,
            tokens_used,
            token_limit,
        } => SseEvent::ContextCompacted(crate::events::ContextCompactedEvent {
            messages_removed,
            tokens_freed,
            tokens_used,
            token_limit,
        }),
        crate::sse::SseEvent::ToolCallStart {
            tool_name,
            tool_call_id,
        } => SseEvent::ToolCallStart(crate::events::ToolCallStartEvent {
            tool_name,
            tool_call_id,
        }),
        crate::sse::SseEvent::ToolCallArgument {
            tool_call_id,
            chunk,
        } => SseEvent::ToolCallArgument(crate::events::ToolCallArgumentEvent {
            tool_call_id,
            chunk,
        }),
        crate::sse::SseEvent::ToolExecuting {
            tool_call_id,
            display_name,
            url,
        } => SseEvent::ToolExecuting(crate::events::ToolExecutingEvent {
            tool_call_id,
            display_name,
            url,
        }),
        crate::sse::SseEvent::ToolResult {
            tool_call_id,
            result,
        } => SseEvent::ToolResult(crate::events::ToolResultEvent {
            tool_call_id,
            result,
        }),
        crate::sse::SseEvent::Reasoning { text } => {
            SseEvent::Reasoning(crate::events::ReasoningEvent { text })
        }
        crate::sse::SseEvent::PermissionRequest {
            permission_id,
            tool_name,
            description,
            tool_call_id,
            tool_input,
        } => SseEvent::PermissionRequest(crate::events::PermissionRequestEvent {
            permission_id,
            tool_name,
            description,
            tool_call_id,
            tool_input,
        }),
        crate::sse::SseEvent::TodosUpdated { todos } => {
            // Parse todos from Value to Vec<TodoItem>
            let todo_items: Vec<crate::events::TodoItem> =
                serde_json::from_value(todos).unwrap_or_default();
            SseEvent::TodosUpdated(crate::events::TodosUpdatedEvent { todos: todo_items })
        }
        crate::sse::SseEvent::SubagentStarted {
            task_id,
            description,
            subagent_type,
        } => SseEvent::SubagentStarted(crate::events::SubagentStartedEvent {
            task_id,
            description,
            subagent_type,
        }),
        crate::sse::SseEvent::SubagentProgress { task_id, message } => {
            SseEvent::SubagentProgress(crate::events::SubagentProgressEvent { task_id, message })
        }
        crate::sse::SseEvent::SubagentCompleted {
            task_id,
            summary,
            tool_call_count,
        } => SseEvent::SubagentCompleted(crate::events::SubagentCompletedEvent {
            task_id,
            summary,
            tool_call_count,
        }),
        crate::sse::SseEvent::ThreadUpdated {
            thread_id,
            title,
            description,
        } => SseEvent::ThreadUpdated(crate::events::ThreadUpdatedEvent {
            thread_id,
            title,
            description,
        }),
        crate::sse::SseEvent::Usage {
            context_window_used,
            context_window_limit,
        } => SseEvent::Usage(crate::events::UsageEvent {
            context_window_used,
            context_window_limit,
        }),
        crate::sse::SseEvent::SystemInit {
            cli_session_id,
            permission_mode,
            model,
            tool_count,
        } => SseEvent::SystemInit(crate::events::SystemInitEvent {
            cli_session_id,
            permission_mode,
            model,
            tool_count,
        }),
        crate::sse::SseEvent::Cancelled { reason } => {
            SseEvent::Cancelled(crate::events::CancelledEvent { reason })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StreamRequest;

    #[test]
    fn test_conductor_client_new() {
        let client = ConductorClient::new();
        assert_eq!(client.base_url, DEFAULT_CONDUCTOR_URL);
    }

    #[test]
    fn test_conductor_client_with_base_url() {
        let custom_url = "http://localhost:8080".to_string();
        let client = ConductorClient::with_base_url(custom_url.clone());
        assert_eq!(client.base_url, custom_url);
    }

    #[test]
    fn test_conductor_client_default() {
        let client = ConductorClient::default();
        assert_eq!(client.base_url, DEFAULT_CONDUCTOR_URL);
    }

    #[test]
    fn test_get_thread_returns_none() {
        let client = ConductorClient::new();
        let result = client.get_thread("test-id");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_recent_messages_returns_empty() {
        let client = ConductorClient::new();
        let messages = client.get_recent_messages();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_stream_request_creation() {
        let request = StreamRequest::new("test".to_string());
        assert_eq!(request.prompt, "test");
    }

    #[test]
    fn test_conductor_error_display() {
        let err = ConductorError::ServerError {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("500"));
        assert!(display.contains("Internal Server Error"));
    }

    #[test]
    fn test_conductor_error_from_sse_parse() {
        let sse_err = SseParseError::UnknownEventType("test".to_string());
        let err: ConductorError = sse_err.into();
        assert!(matches!(err, ConductorError::SseParse(_)));
    }

    #[test]
    fn test_convert_sse_event_content() {
        let sse_event = crate::sse::SseEvent::Content {
            text: "Hello".to_string(),
            meta: crate::sse::SseEventMeta {
                seq: Some(5),
                timestamp: Some(1736956800000),
                session_id: Some("sess-123".to_string()),
                thread_id: Some("thread-456".to_string()),
            },
        };
        let event = convert_sse_event(sse_event);
        match event {
            SseEvent::Content(content) => {
                assert_eq!(content.text, "Hello");
                assert_eq!(content.meta.seq, Some(5));
                assert_eq!(content.meta.timestamp, Some(1736956800000));
            }
            _ => panic!("Expected Content event"),
        }
    }

    #[test]
    fn test_convert_sse_event_done() {
        let sse_event = crate::sse::SseEvent::Done;
        let event = convert_sse_event(sse_event);
        assert!(matches!(event, SseEvent::Done(_)));
    }

    #[test]
    fn test_convert_sse_event_error() {
        let sse_event = crate::sse::SseEvent::Error {
            message: "Test error".to_string(),
            code: Some("ERR001".to_string()),
        };
        let event = convert_sse_event(sse_event);
        match event {
            SseEvent::Error(err) => {
                assert_eq!(err.message, "Test error");
                assert_eq!(err.code, Some("ERR001".to_string()));
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[test]
    fn test_convert_sse_event_thread_updated() {
        let sse_event = crate::sse::SseEvent::ThreadUpdated {
            thread_id: "thread-123".to_string(),
            title: Some("New Title".to_string()),
            description: Some("New Description".to_string()),
        };
        let event = convert_sse_event(sse_event);
        match event {
            SseEvent::ThreadUpdated(thread_updated) => {
                assert_eq!(thread_updated.thread_id, "thread-123");
                assert_eq!(thread_updated.title, Some("New Title".to_string()));
                assert_eq!(
                    thread_updated.description,
                    Some("New Description".to_string())
                );
            }
            _ => panic!("Expected ThreadUpdated event"),
        }
    }

    // Async tests for HTTP methods
    #[tokio::test]
    async fn test_health_check_with_invalid_server() {
        // Use an invalid URL that will fail to connect
        let client = ConductorClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.health_check().await;
        // Should fail with HTTP error since server doesn't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancel_stream_with_invalid_server() {
        let client = ConductorClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.cancel_stream("test-thread-123").await;
        // Should fail with HTTP error since server doesn't exist
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stream_with_invalid_server() {
        let client = ConductorClient::with_base_url("http://127.0.0.1:1".to_string());
        let request = StreamRequest::new("test prompt".to_string());
        let result = client.stream(&request).await;
        // Should fail with HTTP error since server doesn't exist
        assert!(result.is_err());
    }

    #[test]
    fn test_conductor_error_not_implemented_display() {
        let err = ConductorError::NotImplemented("/v1/test/endpoint".to_string());
        let display = format!("{}", err);
        assert!(display.contains("not implemented"));
        assert!(display.contains("/v1/test/endpoint"));
    }

    #[test]
    fn test_conductor_client_with_url() {
        let client = ConductorClient::with_url("http://custom.example.com:9000");
        assert_eq!(client.base_url, "http://custom.example.com:9000");
        assert!(client.auth_token().is_none());
    }

    #[test]
    fn test_conductor_client_with_auth() {
        let client = ConductorClient::new().with_auth("my-secret-token");
        assert_eq!(client.base_url, DEFAULT_CONDUCTOR_URL);
        assert_eq!(client.auth_token(), Some("my-secret-token"));
    }

    #[test]
    fn test_conductor_client_with_url_and_auth() {
        let client = ConductorClient::with_url("http://localhost:3000").with_auth("test-token");
        assert_eq!(client.base_url, "http://localhost:3000");
        assert_eq!(client.auth_token(), Some("test-token"));
    }

    #[test]
    fn test_conductor_client_set_auth_token() {
        let mut client = ConductorClient::new();
        assert!(client.auth_token().is_none());

        client.set_auth_token(Some("new-token".to_string()));
        assert_eq!(client.auth_token(), Some("new-token"));

        client.set_auth_token(None);
        assert!(client.auth_token().is_none());
    }

    #[test]
    fn test_conductor_client_no_auth_by_default() {
        let client = ConductorClient::new();
        assert!(client.auth_token().is_none());

        let client2 = ConductorClient::with_base_url("http://example.com".to_string());
        assert!(client2.auth_token().is_none());

        let client3 = ConductorClient::default();
        assert!(client3.auth_token().is_none());
    }

    #[tokio::test]
    async fn test_update_thread_mode_with_invalid_server() {
        let client = ConductorClient::with_base_url("http://127.0.0.1:1".to_string());
        let result = client.update_thread_mode("thread-123", "fast").await;
        // Should fail with HTTP error since server doesn't exist
        assert!(result.is_err());
    }

    // Tests for dependency injection

    #[test]
    fn test_conductor_config_default() {
        let config = ConductorConfig::default();
        assert_eq!(config.base_url, DEFAULT_CONDUCTOR_URL);
        assert_eq!(config.central_api_url, CENTRAL_API_URL);
        assert!(config.refresh_token.is_none());
    }

    #[test]
    fn test_conductor_config_with_base_url() {
        let config = ConductorConfig::with_base_url("http://custom.example.com:9000".to_string());
        assert_eq!(config.base_url, "http://custom.example.com:9000");
    }

    #[test]
    fn test_conductor_config_with_auth() {
        let config = ConductorConfig::new().with_auth("test-token");
        assert_eq!(config.auth_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_conductor_config_with_refresh_token() {
        let config = ConductorConfig::new().with_refresh_token("refresh-token");
        assert_eq!(config.refresh_token, Some("refresh-token".to_string()));
    }

    #[test]
    fn test_conductor_client_with_mock_http() {
        use crate::adapters::MockHttpClient;

        let mock_http = Arc::new(MockHttpClient::new());
        let config = ConductorConfig::with_base_url("http://mock.example.com:8000".to_string());
        let client = ConductorClient::with_http(mock_http.clone(), config);

        assert_eq!(client.base_url, "http://mock.example.com:8000");
        // Verify that http_client() returns the injected client
        let _ = client.http_client();
    }

    #[test]
    fn test_conductor_client_with_default_http() {
        let config = ConductorConfig::with_base_url("http://test.example.com:8000".to_string())
            .with_auth("test-token");
        let client = ConductorClient::with_default_http(config);

        assert_eq!(client.base_url, "http://test.example.com:8000");
        assert_eq!(client.auth_token(), Some("test-token"));
    }

    #[test]
    fn test_conductor_client_http_client_accessor() {
        let client = ConductorClient::new();
        // Just verify we can access the http client
        let _http = client.http_client();
    }
}
