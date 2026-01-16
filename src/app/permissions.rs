//! Permission handling methods for the App.

use std::sync::Arc;
use std::time::Duration;

use crate::websocket::{WsCommandResponse, WsCommandResult, WsConnectionState, WsPermissionData};
use tracing::{debug, error, info, warn};

use super::App;

/// Maximum elapsed time before considering a permission expired (server times out at 55s)
const PERMISSION_TIMEOUT_SECS: u64 = 50;

/// Retry delay for WebSocket send failures
const WS_RETRY_DELAY_MS: u64 = 500;

/// Result of sending a permission response
#[derive(Debug)]
pub enum PermissionResponseResult {
    /// Successfully sent via WebSocket
    SentViaWebSocket,
    /// Sent via HTTP fallback
    SentViaHttpFallback,
    /// Permission expired before sending
    Expired,
    /// Failed to send (connection lost and no HTTP fallback)
    Failed(String),
}

impl App {
    /// Check if the current pending permission has expired
    ///
    /// Returns true if the permission was received more than PERMISSION_TIMEOUT_SECS ago
    fn is_permission_expired(&self, permission_id: &str) -> bool {
        if let Some(ref perm) = self.session_state.pending_permission {
            if perm.permission_id == permission_id {
                return perm.received_at.elapsed().as_secs() >= PERMISSION_TIMEOUT_SECS;
            }
        }
        false
    }

    /// Send a permission response via WebSocket
    ///
    /// Constructs a `WsCommandResponse` and sends it through the WebSocket channel.
    /// Returns a result indicating success or failure.
    fn send_ws_permission_response(
        &self,
        request_id: &str,
        allowed: bool,
    ) -> Result<(), String> {
        let sender = match &self.ws_sender {
            Some(s) => s,
            None => return Err("WebSocket sender not available".to_string()),
        };

        // Check if WebSocket is connected
        if self.ws_connection_state != WsConnectionState::Connected {
            return Err("WebSocket not connected".to_string());
        }

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: request_id.to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed,
                    message: None,
                },
            },
        };

        // Try to send - this is a non-blocking channel send
        match sender.try_send(response) {
            Ok(()) => {
                debug!("Sent permission response via WebSocket: {} -> {}", request_id, allowed);
                Ok(())
            }
            Err(e) => Err(format!("Failed to send via WebSocket: {}", e)),
        }
    }

    /// Send permission response with retry and fallback logic
    ///
    /// This method:
    /// 1. Checks if permission has expired (>50s elapsed)
    /// 2. Tries to send via WebSocket
    /// 3. If WS fails, retries once after 500ms
    /// 4. If still fails, falls back to HTTP if available
    fn send_permission_response(&mut self, permission_id: &str, allowed: bool) -> PermissionResponseResult {
        // Check if permission has expired
        if self.is_permission_expired(permission_id) {
            warn!("Permission {} expired before response could be sent", permission_id);
            return PermissionResponseResult::Expired;
        }

        // Try WebSocket first
        match self.send_ws_permission_response(permission_id, allowed) {
            Ok(()) => return PermissionResponseResult::SentViaWebSocket,
            Err(e) => {
                debug!("First WebSocket send attempt failed: {}", e);
            }
        }

        // WebSocket send failed - we need to handle retry/fallback asynchronously
        // Since we can't do async operations directly here, we spawn a task
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let perm_id = permission_id.to_string();
            let ws_sender = self.ws_sender.clone();
            let ws_state = self.ws_connection_state.clone();
            let client = Arc::clone(&self.client);

            handle.spawn(async move {
                // Retry WebSocket after delay
                tokio::time::sleep(Duration::from_millis(WS_RETRY_DELAY_MS)).await;

                if let Some(sender) = ws_sender {
                    if ws_state == WsConnectionState::Connected {
                        let response = WsCommandResponse {
                            type_: "command_response".to_string(),
                            request_id: perm_id.clone(),
                            result: WsCommandResult {
                                status: "success".to_string(),
                                data: WsPermissionData {
                                    allowed,
                                    message: None,
                                },
                            },
                        };

                        if sender.send(response).await.is_ok() {
                            debug!("Permission response sent via WebSocket on retry");
                            return;
                        }
                    }
                }

                // Fall back to HTTP
                debug!("Falling back to HTTP for permission response");
                if let Err(e) = client.respond_to_permission(&perm_id, allowed).await {
                    error!("Failed to send permission response via HTTP fallback: {:?}", e);
                }
            });

            // We've spawned a retry/fallback task
            PermissionResponseResult::SentViaHttpFallback
        } else {
            // No runtime available (e.g., in tests without async context)
            PermissionResponseResult::Failed("No runtime available for retry/fallback".to_string())
        }
    }

    /// Approve the current pending permission (user pressed 'y')
    pub fn approve_permission(&mut self, permission_id: &str) {
        let result = self.send_permission_response(permission_id, true);

        match result {
            PermissionResponseResult::SentViaWebSocket => {
                debug!("Permission {} approved via WebSocket", permission_id);
            }
            PermissionResponseResult::SentViaHttpFallback => {
                debug!("Permission {} approval sent via HTTP fallback", permission_id);
            }
            PermissionResponseResult::Expired => {
                warn!("Permission {} expired - could not approve", permission_id);
                // Could set an error notification here if needed
            }
            PermissionResponseResult::Failed(e) => {
                error!("Failed to approve permission {}: {}", permission_id, e);
            }
        }

        // Clear the pending permission
        self.session_state.clear_pending_permission();
    }

    /// Deny the current pending permission (user pressed 'n')
    pub fn deny_permission(&mut self, permission_id: &str) {
        let result = self.send_permission_response(permission_id, false);

        match result {
            PermissionResponseResult::SentViaWebSocket => {
                debug!("Permission {} denied via WebSocket", permission_id);
            }
            PermissionResponseResult::SentViaHttpFallback => {
                debug!("Permission {} denial sent via HTTP fallback", permission_id);
            }
            PermissionResponseResult::Expired => {
                warn!("Permission {} expired - could not deny", permission_id);
            }
            PermissionResponseResult::Failed(e) => {
                error!("Failed to deny permission {}: {}", permission_id, e);
            }
        }

        // Clear the pending permission
        self.session_state.clear_pending_permission();
    }

    /// Allow the tool always for this session and approve (user pressed 'a')
    pub fn allow_tool_always(&mut self, tool_name: &str, permission_id: &str) {
        // Add tool to allowed list
        self.session_state.allow_tool(tool_name.to_string());

        // Approve the current permission
        self.approve_permission(permission_id);
    }

    /// Handle a permission response key press ('y', 'a', or 'n')
    /// Returns true if a permission was handled, false if no pending permission
    pub fn handle_permission_key(&mut self, key: char) -> bool {
        info!("handle_permission_key called with key: '{}'", key);
        if let Some(ref perm) = self.session_state.pending_permission.clone() {
            info!("Pending permission found: {} for tool {}", perm.permission_id, perm.tool_name);
            match key {
                'y' | 'Y' => {
                    info!("User pressed 'y' - approving permission");
                    self.approve_permission(&perm.permission_id);
                    true
                }
                'a' | 'A' => {
                    info!("User pressed 'a' - allowing tool always");
                    self.allow_tool_always(&perm.tool_name, &perm.permission_id);
                    true
                }
                'n' | 'N' => {
                    info!("User pressed 'n' - denying permission");
                    self.deny_permission(&perm.permission_id);
                    true
                }
                _ => {
                    info!("Key '{}' not recognized for permission handling", key);
                    false
                }
            }
        } else {
            info!("No pending permission found");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PermissionRequest;
    use std::time::Instant;
    use tokio::sync::mpsc;

    /// Helper to create a test App with WebSocket sender
    fn create_test_app_with_ws() -> (App, mpsc::Receiver<WsCommandResponse>) {
        let mut app = App::default();
        let (tx, rx) = mpsc::channel(10);
        app.ws_sender = Some(tx);
        app.ws_connection_state = WsConnectionState::Connected;
        (app, rx)
    }

    /// Helper to create a test permission request
    fn create_test_permission(permission_id: &str) -> PermissionRequest {
        PermissionRequest {
            permission_id: permission_id.to_string(),
            tool_name: "Bash".to_string(),
            description: "Run a command".to_string(),
            context: Some("ls -la".to_string()),
            tool_input: Some(serde_json::json!({"command": "ls -la"})),
            received_at: Instant::now(),
        }
    }

    #[test]
    fn test_permission_response_result_debug() {
        // Test that all variants can be debug-printed
        let results = vec![
            PermissionResponseResult::SentViaWebSocket,
            PermissionResponseResult::SentViaHttpFallback,
            PermissionResponseResult::Expired,
            PermissionResponseResult::Failed("test error".to_string()),
        ];

        for result in results {
            // Just ensure debug formatting works
            let _ = format!("{:?}", result);
        }
    }

    #[test]
    fn test_is_permission_expired_no_pending_permission() {
        let app = App::default();
        assert!(!app.is_permission_expired("test-id"));
    }

    #[test]
    fn test_is_permission_expired_wrong_id() {
        let mut app = App::default();
        app.session_state.pending_permission = Some(create_test_permission("perm-123"));
        // Different ID should return false
        assert!(!app.is_permission_expired("perm-456"));
    }

    #[test]
    fn test_is_permission_expired_not_expired() {
        let mut app = App::default();
        app.session_state.pending_permission = Some(create_test_permission("perm-123"));
        // Just created, should not be expired
        assert!(!app.is_permission_expired("perm-123"));
    }

    #[test]
    fn test_send_ws_permission_response_no_sender() {
        let app = App::default();
        let result = app.send_ws_permission_response("test-id", true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not available"));
    }

    #[test]
    fn test_send_ws_permission_response_disconnected() {
        let mut app = App::default();
        let (tx, _rx) = mpsc::channel(10);
        app.ws_sender = Some(tx);
        app.ws_connection_state = WsConnectionState::Disconnected;

        let result = app.send_ws_permission_response("test-id", true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not connected"));
    }

    #[test]
    fn test_send_ws_permission_response_reconnecting() {
        let mut app = App::default();
        let (tx, _rx) = mpsc::channel(10);
        app.ws_sender = Some(tx);
        app.ws_connection_state = WsConnectionState::Reconnecting { attempt: 2 };

        let result = app.send_ws_permission_response("test-id", true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not connected"));
    }

    #[tokio::test]
    async fn test_send_ws_permission_response_success() {
        let (app, mut rx) = create_test_app_with_ws();

        let result = app.send_ws_permission_response("perm-123", true);
        assert!(result.is_ok());

        // Verify the message was sent
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.type_, "command_response");
        assert_eq!(msg.request_id, "perm-123");
        assert_eq!(msg.result.status, "success");
        assert!(msg.result.data.allowed);
        assert!(msg.result.data.message.is_none());
    }

    #[tokio::test]
    async fn test_send_ws_permission_response_denial() {
        let (app, mut rx) = create_test_app_with_ws();

        let result = app.send_ws_permission_response("perm-456", false);
        assert!(result.is_ok());

        // Verify the message was sent
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.request_id, "perm-456");
        assert!(!msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_approve_permission_clears_pending() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-123"));

        app.approve_permission("perm-123");

        assert!(app.session_state.pending_permission.is_none());
    }

    #[tokio::test]
    async fn test_deny_permission_clears_pending() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-123"));

        app.deny_permission("perm-123");

        assert!(app.session_state.pending_permission.is_none());
    }

    #[tokio::test]
    async fn test_approve_permission_sends_ws_message() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-789"));

        app.approve_permission("perm-789");

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.request_id, "perm-789");
        assert!(msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_deny_permission_sends_ws_message() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-abc"));

        app.deny_permission("perm-abc");

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.request_id, "perm-abc");
        assert!(!msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_allow_tool_always_adds_to_allowed_and_approves() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-xyz"));

        app.allow_tool_always("Bash", "perm-xyz");

        // Tool should be in allowed list
        assert!(app.session_state.allowed_tools.contains("Bash"));

        // Permission should be approved and cleared
        assert!(app.session_state.pending_permission.is_none());

        // WebSocket message should be sent
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.request_id, "perm-xyz");
        assert!(msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_handle_permission_key_y() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-y"));

        let handled = app.handle_permission_key('y');
        assert!(handled);

        let msg = rx.recv().await.unwrap();
        assert!(msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_handle_permission_key_n() {
        let (mut app, mut rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-n"));

        let handled = app.handle_permission_key('n');
        assert!(handled);

        let msg = rx.recv().await.unwrap();
        assert!(!msg.result.data.allowed);
    }

    #[tokio::test]
    async fn test_handle_permission_key_a() {
        let (mut app, mut rx) = create_test_app_with_ws();
        let mut perm = create_test_permission("perm-a");
        perm.tool_name = "Read".to_string();
        app.session_state.pending_permission = Some(perm);

        let handled = app.handle_permission_key('a');
        assert!(handled);
        assert!(app.session_state.allowed_tools.contains("Read"));

        let msg = rx.recv().await.unwrap();
        assert!(msg.result.data.allowed);
    }

    #[test]
    fn test_handle_permission_key_no_pending() {
        let mut app = App::default();
        let handled = app.handle_permission_key('y');
        assert!(!handled);
    }

    #[test]
    fn test_handle_permission_key_invalid_key() {
        let mut app = App::default();
        app.session_state.pending_permission = Some(create_test_permission("perm-x"));

        let handled = app.handle_permission_key('x');
        assert!(!handled);

        // Permission should still be pending
        assert!(app.session_state.pending_permission.is_some());
    }

    #[test]
    fn test_handle_permission_key_uppercase_y() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-Y"));

        let handled = app.handle_permission_key('Y');
        assert!(handled);
    }

    #[test]
    fn test_handle_permission_key_uppercase_n() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-N"));

        let handled = app.handle_permission_key('N');
        assert!(handled);
    }

    #[test]
    fn test_handle_permission_key_uppercase_a() {
        let (mut app, _rx) = create_test_app_with_ws();
        app.session_state.pending_permission = Some(create_test_permission("perm-A"));

        let handled = app.handle_permission_key('A');
        assert!(handled);
    }

    #[test]
    fn test_send_permission_response_no_ws_no_runtime() {
        // Without tokio runtime, should return Failed
        let mut app = App::default();
        app.session_state.pending_permission = Some(create_test_permission("perm-test"));

        let result = app.send_permission_response("perm-test", true);
        match result {
            PermissionResponseResult::Failed(msg) => {
                assert!(msg.contains("No runtime available"));
            }
            _ => panic!("Expected Failed result"),
        }
    }

    #[tokio::test]
    async fn test_ws_response_message_format() {
        let (app, mut rx) = create_test_app_with_ws();

        app.send_ws_permission_response("req-format-test", true).unwrap();

        let msg = rx.recv().await.unwrap();

        // Verify JSON serialization matches expected format
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "command_response");
        assert_eq!(json["request_id"], "req-format-test");
        assert_eq!(json["result"]["status"], "success");
        assert_eq!(json["result"]["data"]["allowed"], true);
        // message should not be present when None due to skip_serializing_if
        assert!(json["result"]["data"]["message"].is_null());
    }

    #[test]
    fn test_permission_timeout_constant() {
        // Verify the constant is reasonable (should be < 55s server timeout)
        assert!(PERMISSION_TIMEOUT_SECS < 55);
        assert!(PERMISSION_TIMEOUT_SECS >= 45);
    }

    #[test]
    fn test_retry_delay_constant() {
        // Verify retry delay is reasonable
        assert!(WS_RETRY_DELAY_MS >= 100);
        assert!(WS_RETRY_DELAY_MS <= 1000);
    }
}
