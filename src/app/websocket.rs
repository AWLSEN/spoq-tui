//! WebSocket integration for the App.
//!
//! This module handles connecting the WebSocket client to the application,
//! routing incoming messages to AppMessage, and managing connection state.

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::websocket::{WsClient, WsClientConfig, WsConnectionState, WsIncomingMessage};

use super::AppMessage;

/// Start the WebSocket client and spawn a task to handle incoming messages.
///
/// Returns the response sender if connection succeeds, or None if it fails.
/// On failure, the app continues in SSE-only mode.
pub async fn start_websocket(
    message_tx: mpsc::UnboundedSender<AppMessage>,
) -> Option<tokio::sync::mpsc::Sender<crate::websocket::WsCommandResponse>> {
    start_websocket_with_config(message_tx, WsClientConfig::default()).await
}

/// Start the WebSocket client with custom configuration.
///
/// This is useful for testing with different server addresses.
pub async fn start_websocket_with_config(
    message_tx: mpsc::UnboundedSender<AppMessage>,
    config: WsClientConfig,
) -> Option<tokio::sync::mpsc::Sender<crate::websocket::WsCommandResponse>> {
    info!("Attempting to connect WebSocket to {}", config.host);

    match WsClient::connect(config).await {
        Ok(mut client) => {
            info!("WebSocket connected successfully");

            // Get the response sender before moving client into the task
            // We need to create a channel that bridges to the client's send method
            let (response_tx, mut response_rx) =
                mpsc::channel::<crate::websocket::WsCommandResponse>(100);

            // Get the state receiver for monitoring connection state
            let mut state_rx = client.state_receiver();

            // Send initial connected message
            let _ = message_tx.send(AppMessage::WsConnected);

            // Spawn task to handle incoming messages
            let message_tx_clone = message_tx.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        // Handle incoming WebSocket messages
                        msg = client.recv() => {
                            match msg {
                                Some(ws_msg) => {
                                    if let Err(e) = route_ws_message(ws_msg, &message_tx_clone) {
                                        warn!("Failed to route WebSocket message: {}", e);
                                    }
                                }
                                None => {
                                    // Channel closed, WebSocket task ended
                                    info!("WebSocket incoming channel closed");
                                    let _ = message_tx_clone.send(AppMessage::WsDisconnected);
                                    break;
                                }
                            }
                        }

                        // Handle outgoing responses
                        response = response_rx.recv() => {
                            match response {
                                Some(resp) => {
                                    if let Err(e) = client.send_response(resp).await {
                                        error!("Failed to send WebSocket response: {}", e);
                                    }
                                }
                                None => {
                                    // Response channel closed, shutdown
                                    info!("WebSocket response channel closed");
                                    break;
                                }
                            }
                        }

                        // Monitor connection state changes
                        _ = state_rx.changed() => {
                            let state = state_rx.borrow().clone();
                            match state {
                                WsConnectionState::Connected => {
                                    let _ = message_tx_clone.send(AppMessage::WsConnected);
                                }
                                WsConnectionState::Disconnected => {
                                    let _ = message_tx_clone.send(AppMessage::WsDisconnected);
                                }
                                WsConnectionState::Reconnecting { attempt } => {
                                    let _ = message_tx_clone.send(AppMessage::WsReconnecting { attempt });
                                }
                            }
                        }
                    }
                }
            });

            Some(response_tx)
        }
        Err(e) => {
            warn!(
                "Failed to connect WebSocket: {}. Continuing in SSE-only mode.",
                e
            );
            None
        }
    }
}

/// Route a WebSocket incoming message to the appropriate AppMessage.
fn route_ws_message(
    msg: WsIncomingMessage,
    message_tx: &mpsc::UnboundedSender<AppMessage>,
) -> Result<(), String> {
    match msg {
        WsIncomingMessage::PermissionRequest(req) => {
            info!(
                "Received permission request: {} for tool {}",
                req.request_id, req.tool_name
            );
            message_tx
                .send(AppMessage::PermissionRequested {
                    permission_id: req.request_id,
                    tool_name: req.tool_name,
                    description: req.description,
                    tool_input: Some(req.tool_input),
                })
                .map_err(|e| format!("Failed to send PermissionRequested: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_permission_request() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let ws_msg = WsIncomingMessage::PermissionRequest(crate::websocket::WsPermissionRequest {
            request_id: "req-123".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls -la"}),
            description: "List directory contents".to_string(),
            timestamp: 1234567890,
        });

        let result = route_ws_message(ws_msg, &tx);
        assert!(result.is_ok());

        // Verify the message was sent
        let received = rx.try_recv();
        assert!(received.is_ok());

        match received.unwrap() {
            AppMessage::PermissionRequested {
                permission_id,
                tool_name,
                description,
                tool_input,
            } => {
                assert_eq!(permission_id, "req-123");
                assert_eq!(tool_name, "Bash");
                assert_eq!(description, "List directory contents");
                assert!(tool_input.is_some());
            }
            _ => panic!("Expected PermissionRequested message"),
        }
    }

    #[tokio::test]
    async fn test_start_websocket_connection_failure() {
        // Try to connect to a non-existent server
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = WsClientConfig {
            host: "127.0.0.1:59999".to_string(),
            max_retries: 1,
            max_backoff_secs: 1,
        };

        let result = start_websocket_with_config(tx, config).await;
        // Should return None on connection failure (graceful degradation)
        assert!(result.is_none());
    }
}
