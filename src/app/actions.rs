//! Thread action handlers for the App.
//!
//! This module provides methods for sending responses to backend requests:
//! - Permission responses (using existing command_response wire format)
//! - Plan approval responses (using plan_approval_response wire format)
//! - Thread verification (via REST endpoint with async task)

use std::sync::Arc;

use tracing::{debug, error, info, warn};

use crate::models::WaitingFor;
use crate::websocket::{
    WsClaudeLoginResponse, WsCommandResponse, WsCommandResult, WsConnectionState,
    WsOutgoingMessage, WsPermissionData, WsPlanApprovalResponse,
};

use super::App;

impl App {
    // ========================================================================
    // Permission Response (uses existing command_response wire format)
    // ========================================================================

    /// Send a permission response for a dashboard thread.
    ///
    /// This method handles permission requests from threads in the dashboard view,
    /// using the existing `command_response` wire format that the backend expects.
    ///
    /// Wire format:
    /// ```json
    /// {
    ///   "type": "command_response",
    ///   "request_id": "perm_123",
    ///   "result": { "status": "success", "data": { "allowed": true } }
    /// }
    /// ```
    pub fn send_permission_response_for_thread(&self, request_id: &str, allowed: bool) -> bool {
        self.send_permission_response_with_message(request_id, allowed, None)
    }

    /// Send a permission response with an optional feedback message via WebSocket.
    ///
    /// This is the general form that supports passing a message (e.g., plan feedback).
    /// `send_permission_response_for_thread` delegates to this with `message: None`.
    pub fn send_permission_response_with_message(
        &self,
        request_id: &str,
        allowed: bool,
        message: Option<String>,
    ) -> bool {
        let sender = match &self.ws_sender {
            Some(s) => s,
            None => {
                warn!("No WebSocket sender available for permission response");
                return false;
            }
        };

        if self.ws_connection_state != WsConnectionState::Connected {
            warn!("WebSocket not connected for permission response");
            return false;
        }

        let has_message = message.is_some();
        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: request_id.to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed,
                    message,
                },
            },
        };

        match sender.try_send(WsOutgoingMessage::CommandResponse(response)) {
            Ok(()) => {
                info!(
                    "Sent permission response via WebSocket: {} -> {} (has_message={})",
                    request_id, allowed, has_message
                );
                true
            }
            Err(e) => {
                error!("Failed to send permission response: {}", e);
                false
            }
        }
    }

    // ========================================================================
    // Plan Approval Response (uses plan_approval_response wire format)
    // ========================================================================

    /// Send a plan approval response via WebSocket.
    ///
    /// This method handles plan approval requests from threads in the dashboard view.
    ///
    /// Wire format:
    /// ```json
    /// {
    ///   "type": "plan_approval_response",
    ///   "request_id": "plan_789",
    ///   "approved": true
    /// }
    /// ```
    pub fn send_plan_approval_response(&self, request_id: &str, approved: bool) -> bool {
        let sender = match &self.ws_sender {
            Some(s) => s,
            None => {
                warn!("No WebSocket sender available for plan approval response");
                return false;
            }
        };

        if self.ws_connection_state != WsConnectionState::Connected {
            warn!("WebSocket not connected for plan approval response");
            return false;
        }

        let response = WsPlanApprovalResponse::new(request_id.to_string(), approved);

        match sender.try_send(WsOutgoingMessage::PlanApprovalResponse(response)) {
            Ok(()) => {
                info!(
                    "Sent plan approval response via WebSocket: {} -> {}",
                    request_id, approved
                );
                true
            }
            Err(e) => {
                error!("Failed to send plan approval response: {}", e);
                false
            }
        }
    }

    // ========================================================================
    // Claude Login Response (uses claude_login_response wire format)
    // ========================================================================

    /// Send a Claude login response via WebSocket.
    ///
    /// This method handles Claude CLI login responses from the login dialog.
    ///
    /// Wire format:
    /// ```json
    /// {
    ///   "type": "claude_login_response",
    ///   "request_id": "login-123",
    ///   "status": "completed" | "cancelled"
    /// }
    /// ```
    pub fn send_claude_login_response(&self, request_id: String, completed: bool) -> bool {
        let sender = match &self.ws_sender {
            Some(s) => s,
            None => {
                warn!("No WebSocket sender available for Claude login response");
                return false;
            }
        };

        if self.ws_connection_state != WsConnectionState::Connected {
            warn!("WebSocket not connected for Claude login response");
            return false;
        }

        let response = if completed {
            WsClaudeLoginResponse::completed(request_id.clone())
        } else {
            WsClaudeLoginResponse::cancelled(request_id.clone())
        };

        match sender.try_send(WsOutgoingMessage::ClaudeLoginResponse(response)) {
            Ok(()) => {
                info!(
                    "Sent Claude login response via WebSocket: {} -> {}",
                    request_id,
                    if completed { "completed" } else { "cancelled" }
                );
                true
            }
            Err(e) => {
                error!("Failed to send Claude login response: {}", e);
                false
            }
        }
    }

    // ========================================================================
    // Thread Verification (REST endpoint with async task)
    // ========================================================================

    /// Spawn an async task to verify a thread via REST endpoint.
    ///
    /// This method attempts to call `POST /v1/threads/{id}/verify`.
    /// If the endpoint returns 404 (not implemented), it falls back to local state.
    ///
    /// Response format:
    /// ```json
    /// { "verified": true, "verified_at": "2024-01-15T10:30:00Z" }
    /// ```
    pub fn spawn_verify_task(&self, thread_id: String) {
        let client = Arc::clone(&self.client);

        tokio::spawn(async move {
            debug!(
                "Attempting to verify thread {} via REST endpoint",
                thread_id
            );

            match client.verify_thread(&thread_id).await {
                Ok(verified) => {
                    if verified {
                        info!("Thread {} verified successfully via REST", thread_id);
                        // Could send message to update UI if needed
                        // For now, the dashboard already marks it locally
                    } else {
                        warn!("Thread {} verification returned false", thread_id);
                    }
                }
                Err(e) => {
                    // Check if this is a 404 (endpoint not implemented)
                    let error_str = e.to_string();
                    if error_str.contains("404") || error_str.contains("Not Found") {
                        debug!(
                            "Verify endpoint not implemented (404), using local state for thread {}",
                            thread_id
                        );
                        // Endpoint not implemented - local state is already set by click handler
                    } else {
                        error!("Failed to verify thread {}: {}", thread_id, e);
                    }
                }
            }
        });
    }

    // ========================================================================
    // Composite Action Handlers (for click_handler integration)
    // ========================================================================

    /// Handle thread approval action from dashboard.
    ///
    /// Determines what the thread is waiting for (permission or plan)
    /// and sends the appropriate response.
    ///
    /// Returns true if a response was sent, false otherwise.
    pub fn handle_thread_approval(&mut self, thread_id: &str) -> bool {
        let waiting_for = self.dashboard.get_waiting_for(thread_id).cloned();

        match waiting_for {
            Some(WaitingFor::Permission { request_id, .. }) => {
                let sent = self.send_permission_response_for_thread(&request_id, true);
                if sent {
                    self.dashboard.clear_waiting_for(thread_id);
                }
                sent
            }
            Some(WaitingFor::PlanApproval { .. }) => {
                if let Some(request_id) = self.dashboard.get_plan_request_id(thread_id) {
                    let request_id = request_id.to_string();
                    let sent = self.send_plan_approval_response(&request_id, true);
                    if sent {
                        self.dashboard.remove_plan_request(thread_id);
                        self.dashboard.clear_waiting_for(thread_id);
                    }
                    sent
                } else {
                    warn!(
                        "No plan request ID found for thread {} despite waiting for plan approval",
                        thread_id
                    );
                    false
                }
            }
            Some(WaitingFor::UserInput) => {
                debug!(
                    "Thread {} is waiting for user input, not permission/plan",
                    thread_id
                );
                false
            }
            None => {
                debug!("Thread {} is not waiting for anything", thread_id);
                false
            }
        }
    }

    /// Handle thread rejection action from dashboard.
    ///
    /// Determines what the thread is waiting for (permission or plan)
    /// and sends the appropriate rejection response.
    ///
    /// Returns true if a response was sent, false otherwise.
    pub fn handle_thread_rejection(&mut self, thread_id: &str) -> bool {
        let waiting_for = self.dashboard.get_waiting_for(thread_id).cloned();

        match waiting_for {
            Some(WaitingFor::Permission { request_id, .. }) => {
                let sent = self.send_permission_response_for_thread(&request_id, false);
                if sent {
                    self.dashboard.clear_waiting_for(thread_id);
                }
                sent
            }
            Some(WaitingFor::PlanApproval { .. }) => {
                if let Some(request_id) = self.dashboard.get_plan_request_id(thread_id) {
                    let request_id = request_id.to_string();
                    let sent = self.send_plan_approval_response(&request_id, false);
                    if sent {
                        self.dashboard.remove_plan_request(thread_id);
                        self.dashboard.clear_waiting_for(thread_id);
                    }
                    sent
                } else {
                    warn!(
                        "No plan request ID found for thread {} despite waiting for plan approval",
                        thread_id
                    );
                    false
                }
            }
            Some(WaitingFor::UserInput) => {
                debug!(
                    "Thread {} is waiting for user input, not permission/plan",
                    thread_id
                );
                false
            }
            None => {
                debug!("Thread {} is not waiting for anything", thread_id);
                false
            }
        }
    }

    /// Handle thread verification action from dashboard.
    ///
    /// Marks the thread as verified locally and spawns an async task
    /// to call the backend verification endpoint.
    pub fn handle_thread_verification(&mut self, thread_id: &str) {
        // Mark verified locally first (optimistic update)
        self.dashboard.mark_verified_local(thread_id);

        // Spawn async task to verify via REST
        self.spawn_verify_task(thread_id.to_string());

        info!("Thread {} marked as verified", thread_id);
    }

    // ========================================================================
    // VPS Config Actions
    // ========================================================================

    /// Start the VPS replacement process.
    ///
    /// This method transitions the VPS config overlay to "Provisioning" state
    /// and spawns an async task to:
    /// 1. Call the backend replace_byovps endpoint
    /// 2. Poll operation status until completion
    /// 3. Backend auto-confirms VPS when healthy
    /// 4. Send success/failure messages back to the UI
    pub fn start_vps_replace(&mut self, ip: String, password: String) {
        use crate::view_state::{ProvisioningPhase, VpsConfigState};

        // Store credentials for retry after re-auth (username is always "root")
        self.dashboard.set_vps_pending_credentials(ip.clone(), password.clone());

        // Update overlay to Provisioning state
        self.dashboard.update_vps_config_state(VpsConfigState::Provisioning {
            phase: ProvisioningPhase::Connecting,
            spinner_frame: 0,
        });

        // Clone necessary data for the async task
        let tx = self.message_tx.clone();

        // Extract tokens from the existing central_api client
        let (auth_token, refresh_token) = match &self.central_api {
            Some(api) => (
                api.auth_token().map(|s| s.to_string()),
                api.get_refresh_token().map(|s| s.to_string()),
            ),
            None => {
                let _ = tx.send(crate::app::AppMessage::VpsConfigFailed {
                    error: "Central API client not available".to_string(),
                    is_auth_error: true,
                });
                return;
            }
        };

        tokio::spawn(async move {
            use crate::app::AppMessage;
            use crate::auth::central_api::CentralApiClient;

            // Send progress update
            let _ = tx.send(AppMessage::VpsConfigProgress {
                phase: "Replacing VPS...".to_string(),
            });

            // Create a new CentralApiClient with the same tokens
            let mut api = CentralApiClient::new();
            if let Some(token) = auth_token {
                api = api.with_auth(&token);
            }
            if let Some(token) = refresh_token {
                api = api.with_refresh_token(&token);
            }

            // Call the replace_byovps endpoint (username is always "root")
            // This now returns immediately with an operation_id
            tracing::info!(
                "Calling replace_byovps: ip={}, ssh_username=root, password_len={}",
                ip, password.len()
            );

            let async_response = match api.replace_byovps(&ip, "root", &password).await {
                Ok(response) => response,
                Err(e) => {
                    tracing::error!("replace_byovps failed: {:?}", e);
                    let (error_msg, is_auth_error) = match &e {
                        crate::auth::central_api::CentralApiError::ServerError { status, message } => {
                            tracing::error!("Server error {}: {}", status, message);
                            if *status == 401 {
                                ("Session expired. Press [L] to log in again.".to_string(), true)
                            } else {
                                (format!("{}", e), false)
                            }
                        }
                        crate::auth::central_api::CentralApiError::Http(_) => {
                            ("Network error. Check your connection and retry.".to_string(), false)
                        }
                        _ => (format!("{}", e), false),
                    };
                    let _ = tx.send(AppMessage::VpsConfigFailed {
                        error: error_msg,
                        is_auth_error,
                    });
                    return;
                }
            };

            let operation_id = async_response.operation_id.clone();
            let hostname = async_response.hostname.clone();
            tracing::info!(
                "VPS replace queued: operation_id={}, hostname={}",
                operation_id, hostname
            );

            // Poll operation status until completed or failed
            let poll_interval = std::time::Duration::from_secs(3);
            let timeout = std::time::Duration::from_secs(600); // 10 minutes max
            let start = std::time::Instant::now();

            loop {
                if start.elapsed() > timeout {
                    let _ = tx.send(AppMessage::VpsConfigFailed {
                        error: "Timeout waiting for VPS provisioning to complete".to_string(),
                        is_auth_error: false,
                    });
                    return;
                }

                match api.poll_operation(&operation_id).await {
                    Ok(status) => {
                        // Send progress update with current phase
                        let phase = format!("{}% - {}", status.progress, status.message);
                        let _ = tx.send(AppMessage::VpsConfigProgress { phase });

                        match status.status.as_str() {
                            "completed" => {
                                // Success! Backend has auto-confirmed the VPS
                                if let Some(result) = status.result {
                                    let vps_url = format!("https://{}", result.hostname);
                                    tracing::info!(
                                        "VPS provisioning completed: hostname={}, vps_id={:?}",
                                        result.hostname, result.vps_id
                                    );
                                    let _ = tx.send(AppMessage::VpsConfigSuccess {
                                        vps_url,
                                        hostname: result.hostname,
                                    });
                                } else {
                                    // Shouldn't happen, but handle gracefully
                                    let vps_url = format!("https://{}", hostname);
                                    let _ = tx.send(AppMessage::VpsConfigSuccess {
                                        vps_url,
                                        hostname,
                                    });
                                }
                                return;
                            }
                            "failed" => {
                                let error = status.error.unwrap_or_else(|| "Unknown error".to_string());
                                tracing::error!("VPS provisioning failed: {}", error);
                                let _ = tx.send(AppMessage::VpsConfigFailed {
                                    error,
                                    is_auth_error: false,
                                });
                                return;
                            }
                            _ => {
                                // Still running, continue polling
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to poll operation status: {:?}", e);
                        // Don't fail immediately on poll error, might be transient
                        // But do fail if it's an auth error
                        if let crate::auth::central_api::CentralApiError::ServerError { status: 401, .. } = &e {
                            let _ = tx.send(AppMessage::VpsConfigFailed {
                                error: "Session expired. Press [L] to log in again.".to_string(),
                                is_auth_error: true,
                            });
                            return;
                        }
                    }
                }

                tokio::time::sleep(poll_interval).await;
            }
        });
    }

    /// Start device flow re-authentication from the VPS config dialog.
    /// Triggers the OAuth device flow, updates the UI with verification URL/code,
    /// and on success sends new credentials back to the app.
    pub fn start_vps_reauth(&mut self) {
        use crate::view_state::{ProvisioningPhase, VpsConfigState};

        self.dashboard.update_vps_config_state(VpsConfigState::Provisioning {
            phase: ProvisioningPhase::Connecting,
            spinner_frame: 0,
        });

        let tx = self.message_tx.clone();

        tokio::spawn(async move {
            use crate::app::AppMessage;
            use crate::auth::central_api::CentralApiClient;

            let api = CentralApiClient::new();

            // Step 1: Request device code
            match api.request_device_code().await {
                Ok(device_response) => {
                    let verification_url = device_response.verification_uri.clone();
                    let user_code = device_response.user_code.clone().unwrap_or_default();

                    // Show the verification URL in the UI
                    let _ = tx.send(AppMessage::VpsAuthStarted {
                        verification_url: verification_url.clone(),
                        user_code: user_code.clone(),
                    });

                    // Try to open browser
                    let _ = open::that(&verification_url);

                    // Step 2: Poll for authorization
                    let interval = std::time::Duration::from_secs(
                        device_response.interval.max(5) as u64,
                    );
                    let timeout = std::time::Duration::from_secs(300); // 5 min timeout
                    let start = std::time::Instant::now();

                    loop {
                        if start.elapsed() > timeout {
                            let _ = tx.send(AppMessage::VpsConfigFailed {
                                error: "Authentication timed out. Try again.".to_string(),
                                is_auth_error: true,
                            });
                            return;
                        }

                        tokio::time::sleep(interval).await;

                        match api.poll_device_token(&device_response.device_code).await {
                            Ok(token_response) => {
                                // Success - send tokens back
                                let _ = tx.send(AppMessage::VpsAuthComplete {
                                    access_token: token_response.access_token,
                                    refresh_token: token_response.refresh_token,
                                    expires_in: token_response.expires_in,
                                    user_id: token_response.user_id,
                                });
                                return;
                            }
                            Err(crate::auth::central_api::CentralApiError::AuthorizationPending) => {
                                // Keep polling
                                continue;
                            }
                            Err(crate::auth::central_api::CentralApiError::AuthorizationExpired) => {
                                let _ = tx.send(AppMessage::VpsConfigFailed {
                                    error: "Authorization expired. Try again.".to_string(),
                                    is_auth_error: true,
                                });
                                return;
                            }
                            Err(crate::auth::central_api::CentralApiError::AccessDenied) => {
                                let _ = tx.send(AppMessage::VpsConfigFailed {
                                    error: "Access denied.".to_string(),
                                    is_auth_error: true,
                                });
                                return;
                            }
                            Err(e) => {
                                let _ = tx.send(AppMessage::VpsConfigFailed {
                                    error: format!("Auth error: {}", e),
                                    is_auth_error: true,
                                });
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::VpsConfigFailed {
                        error: format!("Could not start login: {}", e),
                        is_auth_error: true,
                    });
                }
            }
        });
    }

    /// Start local conductor: download binary if needed, start process, wait for health.
    /// Full implementation in Phase 6 (conductor/local module).
    pub fn start_local_conductor(&mut self) {
        use crate::view_state::{ProvisioningPhase, VpsConfigState};

        self.dashboard.update_vps_config_state(VpsConfigState::Provisioning {
            phase: ProvisioningPhase::Connecting,
            spinner_frame: 0,
        });

        let tx = self.message_tx.clone();
        let owner_id = self.credentials.user_id.clone().unwrap_or_else(|| "local-user".to_string());

        tokio::spawn(async move {
            use crate::app::AppMessage;
            use crate::conductor::local;

            let port = local::default_port();

            // Step 1: Check if already running
            if local::is_running(port).await {
                let vps_url = format!("http://127.0.0.1:{}", port);
                let _ = tx.send(AppMessage::VpsConfigSuccess {
                    vps_url,
                    hostname: "localhost".to_string(),
                });
                return;
            }

            // Step 2: Download if binary not present
            if !local::conductor_exists() {
                let _ = tx.send(AppMessage::VpsConfigProgress {
                    phase: "Downloading conductor...".to_string(),
                });
                if let Err(e) = local::download_conductor().await {
                    let _ = tx.send(AppMessage::VpsConfigFailed {
                        error: format!("Download failed: {}", e),
                        is_auth_error: false,
                    });
                    return;
                }
            }

            // Step 3: Start conductor process
            let _ = tx.send(AppMessage::VpsConfigProgress {
                phase: "Starting local conductor...".to_string(),
            });
            match local::start_conductor(port, &owner_id).await {
                Ok(child) => {
                    let _ = tx.send(AppMessage::LocalConductorStarted {
                        child: std::sync::Arc::new(tokio::sync::Mutex::new(Some(child))),
                    });
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::VpsConfigFailed {
                        error: format!("Failed to start: {}", e),
                        is_auth_error: false,
                    });
                    return;
                }
            }

            // Step 4: Wait for health
            let _ = tx.send(AppMessage::VpsConfigProgress {
                phase: "Waiting for conductor...".to_string(),
            });
            if let Err(e) = local::wait_for_health(port, 30).await {
                let _ = tx.send(AppMessage::VpsConfigFailed {
                    error: e,
                    is_auth_error: false,
                });
                return;
            }

            // Step 5: Success
            let vps_url = format!("http://127.0.0.1:{}", port);
            let _ = tx.send(AppMessage::VpsConfigSuccess {
                vps_url,
                hostname: "localhost".to_string(),
            });
        });
    }

    /// Reconnect the WebSocket to the current VPS URL.
    ///
    /// This should be called after a successful VPS swap to establish
    /// a connection to the new conductor.
    pub fn reconnect_websocket(&mut self) {
        use crate::websocket::{WsClientConfig};

        // Drop the old sender to disconnect
        self.ws_sender = None;
        self.ws_connection_state = WsConnectionState::Disconnected;
        info!("WebSocket disconnected, ready for reconnection to new VPS");

        // Spawn a new WebSocket connection to the current vps_url
        let vps_url = match &self.vps_url {
            Some(url) => url.clone(),
            None => {
                info!("No VPS URL set, skipping WebSocket reconnection");
                return;
            }
        };

        let auth_token = self.credentials.access_token.clone();
        let tx = self.message_tx.clone();

        tokio::spawn(async move {
            // Build config from the current VPS URL
            let mut ws_config = WsClientConfig::default();

            let (host, use_tls) = if vps_url.starts_with("https://") {
                (vps_url.strip_prefix("https://").unwrap(), true)
            } else if vps_url.starts_with("http://") {
                (vps_url.strip_prefix("http://").unwrap(), false)
            } else {
                let is_ip = vps_url.split(':').next().map_or(false, |h| {
                    h.parse::<std::net::Ipv4Addr>().is_ok()
                        || h.parse::<std::net::Ipv6Addr>().is_ok()
                });
                (vps_url.as_str(), !is_ip)
            };
            ws_config = ws_config.with_host(host).with_tls(use_tls);

            if let Some(ref token) = auth_token {
                ws_config = ws_config.with_auth(token);
            }

            info!("Reconnecting WebSocket to {} (tls={})", host, use_tls);

            match crate::app::start_websocket_with_config(tx.clone(), ws_config).await {
                Ok(sender) => {
                    let _ = tx.send(crate::app::AppMessage::WsReconnected { sender });
                }
                Err(e) => {
                    tracing::error!("Failed to reconnect WebSocket: {}", e);
                }
            }
        });
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::dashboard::{PlanRequest, PlanSummary};
    use crate::models::ThreadStatus;
    use tokio::sync::mpsc;

    /// Helper to create a test App with WebSocket sender
    fn create_test_app_with_ws() -> (App, mpsc::Receiver<WsOutgoingMessage>) {
        let mut app = App::default();
        let (tx, rx) = mpsc::channel(10);
        app.ws_sender = Some(tx);
        app.ws_connection_state = WsConnectionState::Connected;
        (app, rx)
    }

    // -------------------- Permission Response Tests --------------------

    #[tokio::test]
    async fn test_send_permission_response_for_thread_success() {
        let (app, mut rx) = create_test_app_with_ws();

        let result = app.send_permission_response_for_thread("perm-123", true);
        assert!(result);

        let msg = rx.recv().await.unwrap();
        match msg {
            WsOutgoingMessage::CommandResponse(resp) => {
                assert_eq!(resp.type_, "command_response");
                assert_eq!(resp.request_id, "perm-123");
                assert_eq!(resp.result.status, "success");
                assert!(resp.result.data.allowed);
            }
            _ => panic!("Expected CommandResponse"),
        }
    }

    #[tokio::test]
    async fn test_send_permission_response_for_thread_denied() {
        let (app, mut rx) = create_test_app_with_ws();

        let result = app.send_permission_response_for_thread("perm-456", false);
        assert!(result);

        let msg = rx.recv().await.unwrap();
        match msg {
            WsOutgoingMessage::CommandResponse(resp) => {
                assert_eq!(resp.request_id, "perm-456");
                assert!(!resp.result.data.allowed);
            }
            _ => panic!("Expected CommandResponse"),
        }
    }

    #[test]
    fn test_send_permission_response_no_sender() {
        let app = App::default();
        let result = app.send_permission_response_for_thread("perm-789", true);
        assert!(!result);
    }

    #[test]
    fn test_send_permission_response_disconnected() {
        let mut app = App::default();
        let (tx, _rx) = mpsc::channel(10);
        app.ws_sender = Some(tx);
        app.ws_connection_state = WsConnectionState::Disconnected;

        let result = app.send_permission_response_for_thread("perm-abc", true);
        assert!(!result);
    }

    // -------------------- Plan Approval Response Tests --------------------

    #[tokio::test]
    async fn test_send_plan_approval_response_approved() {
        let (app, mut rx) = create_test_app_with_ws();

        let result = app.send_plan_approval_response("plan-123", true);
        assert!(result);

        let msg = rx.recv().await.unwrap();
        match msg {
            WsOutgoingMessage::PlanApprovalResponse(resp) => {
                assert_eq!(resp.type_, "plan_approval_response");
                assert_eq!(resp.request_id, "plan-123");
                assert!(resp.approved);
            }
            _ => panic!("Expected PlanApprovalResponse"),
        }
    }

    #[tokio::test]
    async fn test_send_plan_approval_response_rejected() {
        let (app, mut rx) = create_test_app_with_ws();

        let result = app.send_plan_approval_response("plan-456", false);
        assert!(result);

        let msg = rx.recv().await.unwrap();
        match msg {
            WsOutgoingMessage::PlanApprovalResponse(resp) => {
                assert_eq!(resp.request_id, "plan-456");
                assert!(!resp.approved);
            }
            _ => panic!("Expected PlanApprovalResponse"),
        }
    }

    #[test]
    fn test_send_plan_approval_no_sender() {
        let app = App::default();
        let result = app.send_plan_approval_response("plan-789", true);
        assert!(!result);
    }

    // -------------------- Wire Format Tests --------------------

    #[tokio::test]
    async fn test_permission_response_wire_format() {
        let (app, mut rx) = create_test_app_with_ws();

        app.send_permission_response_for_thread("req-format", true);

        let msg = rx.recv().await.unwrap();
        let json = serde_json::to_value(&msg).unwrap();

        // Verify exact wire format expected by backend
        assert_eq!(json["type"], "command_response");
        assert_eq!(json["request_id"], "req-format");
        assert_eq!(json["result"]["status"], "success");
        assert_eq!(json["result"]["data"]["allowed"], true);
        // message should not be present when None
        assert!(json["result"]["data"]["message"].is_null());
    }

    #[tokio::test]
    async fn test_plan_approval_response_wire_format() {
        let (app, mut rx) = create_test_app_with_ws();

        app.send_plan_approval_response("plan-format", true);

        let msg = rx.recv().await.unwrap();
        let json = serde_json::to_value(&msg).unwrap();

        // Verify exact wire format expected by backend
        assert_eq!(json["type"], "plan_approval_response");
        assert_eq!(json["request_id"], "plan-format");
        assert_eq!(json["approved"], true);
    }

    // -------------------- Composite Handler Tests --------------------

    #[tokio::test]
    async fn test_handle_thread_approval_permission() {
        let (mut app, mut rx) = create_test_app_with_ws();

        // Set up a thread waiting for permission
        use crate::models::Thread;
        use chrono::Utc;

        let thread = Thread {
            id: "t1".to_string(),
            title: "Test Thread".to_string(),
            description: None,
            preview: String::new(),
            updated_at: Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: Some("/test".to_string()),
            status: Some(ThreadStatus::Waiting),
            verified: None,
            verified_at: None,
        };

        app.dashboard
            .set_threads(vec![thread], &std::collections::HashMap::new());
        app.dashboard.update_thread_status(
            "t1",
            ThreadStatus::Waiting,
            Some(WaitingFor::Permission {
                request_id: "perm-t1".to_string(),
                tool_name: "Bash".to_string(),
            }),
        );

        let result = app.handle_thread_approval("t1");
        assert!(result);

        // Verify permission response was sent
        let msg = rx.recv().await.unwrap();
        match msg {
            WsOutgoingMessage::CommandResponse(resp) => {
                assert_eq!(resp.request_id, "perm-t1");
                assert!(resp.result.data.allowed);
            }
            _ => panic!("Expected CommandResponse"),
        }

        // Verify waiting_for was cleared
        assert!(app.dashboard.get_waiting_for("t1").is_none());
    }

    #[tokio::test]
    async fn test_handle_thread_approval_plan() {
        let (mut app, mut rx) = create_test_app_with_ws();

        // Set up a thread waiting for plan approval
        use crate::models::Thread;
        use chrono::Utc;

        let thread = Thread {
            id: "t2".to_string(),
            title: "Test Thread 2".to_string(),
            description: None,
            preview: String::new(),
            updated_at: Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: Some("/test".to_string()),
            status: Some(ThreadStatus::Waiting),
            verified: None,
            verified_at: None,
        };

        app.dashboard
            .set_threads(vec![thread], &std::collections::HashMap::new());
        app.dashboard.update_thread_status(
            "t2",
            ThreadStatus::Waiting,
            Some(WaitingFor::PlanApproval {
                request_id: "plan-t2".to_string(),
            }),
        );
        app.dashboard.set_plan_request(
            "t2",
            PlanRequest::new(
                "plan-t2".to_string(),
                PlanSummary::new(
                    "Test Plan".to_string(),
                    vec!["Phase 1".to_string()],
                    3,
                    Some(1000),
                ),
            ),
        );

        let result = app.handle_thread_approval("t2");
        assert!(result);

        // Verify plan approval response was sent
        let msg = rx.recv().await.unwrap();
        match msg {
            WsOutgoingMessage::PlanApprovalResponse(resp) => {
                assert_eq!(resp.request_id, "plan-t2");
                assert!(resp.approved);
            }
            _ => panic!("Expected PlanApprovalResponse"),
        }

        // Verify plan request and waiting_for were cleared
        assert!(app.dashboard.get_plan_request_id("t2").is_none());
        assert!(app.dashboard.get_waiting_for("t2").is_none());
    }

    #[tokio::test]
    async fn test_handle_thread_rejection_permission() {
        let (mut app, mut rx) = create_test_app_with_ws();

        use crate::models::Thread;
        use chrono::Utc;

        let thread = Thread {
            id: "t3".to_string(),
            title: "Test Thread 3".to_string(),
            description: None,
            preview: String::new(),
            updated_at: Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: Some("/test".to_string()),
            status: Some(ThreadStatus::Waiting),
            verified: None,
            verified_at: None,
        };

        app.dashboard
            .set_threads(vec![thread], &std::collections::HashMap::new());
        app.dashboard.update_thread_status(
            "t3",
            ThreadStatus::Waiting,
            Some(WaitingFor::Permission {
                request_id: "perm-t3".to_string(),
                tool_name: "Write".to_string(),
            }),
        );

        let result = app.handle_thread_rejection("t3");
        assert!(result);

        let msg = rx.recv().await.unwrap();
        match msg {
            WsOutgoingMessage::CommandResponse(resp) => {
                assert_eq!(resp.request_id, "perm-t3");
                assert!(!resp.result.data.allowed);
            }
            _ => panic!("Expected CommandResponse"),
        }
    }

    #[test]
    fn test_handle_thread_approval_not_waiting() {
        let (mut app, _rx) = create_test_app_with_ws();

        let result = app.handle_thread_approval("nonexistent");
        assert!(!result);
    }

    #[tokio::test]
    async fn test_handle_thread_verification() {
        let (mut app, _rx) = create_test_app_with_ws();

        use crate::models::Thread;
        use chrono::Utc;

        let thread = Thread {
            id: "t4".to_string(),
            title: "Test Thread 4".to_string(),
            description: None,
            preview: String::new(),
            updated_at: Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            mode: crate::models::ThreadMode::default(),
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: Some("/test".to_string()),
            status: Some(ThreadStatus::Done),
            verified: None,
            verified_at: None,
        };

        app.dashboard
            .set_threads(vec![thread], &std::collections::HashMap::new());

        // Verify the thread
        app.handle_thread_verification("t4");

        // Check that it's marked as locally verified
        assert!(app.dashboard.is_locally_verified("t4"));
    }
}
