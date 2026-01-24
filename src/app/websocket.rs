//! WebSocket integration for the App.
//!
//! This module handles connecting the WebSocket client to the application,
//! routing incoming messages to AppMessage, and managing connection state.

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::view_state::SystemStats;
use crate::websocket::{WsClient, WsClientConfig, WsConnectionState, WsIncomingMessage};

use super::AppMessage;

/// Start the WebSocket client and spawn a task to handle incoming messages.
///
/// Returns Ok(sender) if connection succeeds, or Err(error_message) if it fails.
/// On failure, the app continues in SSE-only mode.
pub async fn start_websocket(
    message_tx: mpsc::UnboundedSender<AppMessage>,
) -> Result<tokio::sync::mpsc::Sender<crate::websocket::WsOutgoingMessage>, String> {
    start_websocket_with_config(message_tx, WsClientConfig::default()).await
}

/// Start the WebSocket client with custom configuration.
///
/// This is useful for testing with different server addresses.
pub async fn start_websocket_with_config(
    message_tx: mpsc::UnboundedSender<AppMessage>,
    config: WsClientConfig,
) -> Result<tokio::sync::mpsc::Sender<crate::websocket::WsOutgoingMessage>, String> {
    let host = config.host.clone();
    info!("Attempting to connect WebSocket to {}", host);

    match WsClient::connect(config).await {
        Ok(mut client) => {
            info!("WebSocket connected successfully");

            // Get the outgoing message sender before moving client into the task
            // We need to create a channel that bridges to the client's send method
            let (outgoing_tx, mut outgoing_rx) =
                mpsc::channel::<crate::websocket::WsOutgoingMessage>(100);

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

                        // Handle outgoing messages
                        outgoing = outgoing_rx.recv() => {
                            match outgoing {
                                Some(msg) => {
                                    if let Err(e) = client.send(msg).await {
                                        error!("Failed to send WebSocket message: {}", e);
                                    }
                                }
                                None => {
                                    // Outgoing channel closed, shutdown
                                    info!("WebSocket outgoing channel closed");
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

            Ok(outgoing_tx)
        }
        Err(e) => {
            let error_msg = format!("Failed to connect to ws://{}/ws: {}", host, e);
            warn!("{}. Continuing in SSE-only mode.", error_msg);
            Err(error_msg)
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
        WsIncomingMessage::AgentStatus(status) => {
            // Agent status updates are routed to dashboard state
            info!(
                "Received agent status: thread={}, state={}",
                status.thread_id, status.state
            );
            message_tx
                .send(AppMessage::AgentStatusUpdate {
                    thread_id: status.thread_id,
                    state: status.state,
                    current_operation: status.current_operation,
                })
                .map_err(|e| format!("Failed to send AgentStatusUpdate: {}", e))
        }
        WsIncomingMessage::Connected(_connected) => {
            // Connection confirmation is informational - ignore for now
            Ok(())
        }
        WsIncomingMessage::ThreadStatusUpdate(update) => {
            // Thread status updates for dashboard view
            info!(
                "Received thread status update: thread={}, status={:?}",
                update.thread_id, update.status
            );
            message_tx
                .send(AppMessage::ThreadStatusUpdate {
                    thread_id: update.thread_id,
                    status: update.status,
                    waiting_for: update.waiting_for,
                })
                .map_err(|e| format!("Failed to send ThreadStatusUpdate: {}", e))
        }
        WsIncomingMessage::PlanApprovalRequest(request) => {
            // Plan approval requests for dashboard view
            info!(
                "Received plan approval request: thread={}, request_id={}",
                request.thread_id, request.request_id
            );
            message_tx
                .send(AppMessage::PlanApprovalRequest {
                    thread_id: request.thread_id,
                    request_id: request.request_id,
                    plan_summary: request.plan_summary,
                })
                .map_err(|e| format!("Failed to send PlanApprovalRequest: {}", e))
        }
        WsIncomingMessage::ThreadCreated(created) => {
            // New thread created - add to dashboard immediately
            info!("Received thread_created: thread_id={}", created.thread.id);
            message_tx
                .send(AppMessage::WsThreadCreated {
                    thread: created.thread,
                })
                .map_err(|e| format!("Failed to send WsThreadCreated: {}", e))
        }
        WsIncomingMessage::ThreadModeUpdate(update) => {
            // Thread mode updates (normal, plan, exec)
            info!(
                "Received thread mode update: thread={}, mode={:?}",
                update.thread_id, update.mode
            );
            message_tx
                .send(AppMessage::ThreadModeUpdate {
                    thread_id: update.thread_id,
                    mode: update.mode,
                })
                .map_err(|e| format!("Failed to send ThreadModeUpdate: {}", e))
        }
        WsIncomingMessage::PhaseProgressUpdate(progress) => {
            // Phase progress updates during plan execution
            info!(
                "Received phase progress: plan={}, phase={}/{}, status={:?}, thread_id={:?}",
                progress.plan_id,
                progress.phase_index + 1,
                progress.total_phases,
                progress.status,
                progress.thread_id
            );
            message_tx
                .send(AppMessage::PhaseProgressUpdate {
                    thread_id: progress.thread_id,
                    plan_id: progress.plan_id,
                    phase_index: progress.phase_index,
                    total_phases: progress.total_phases,
                    phase_name: progress.phase_name,
                    status: progress.status,
                    tool_count: progress.tool_count,
                    // Convert Option<String> to String, using empty string if None
                    last_tool: progress.last_tool.unwrap_or_default(),
                    last_file: progress.last_file,
                })
                .map_err(|e| format!("Failed to send PhaseProgressUpdate: {}", e))
        }
        WsIncomingMessage::ThreadVerified(verified) => {
            // Thread verification notification
            info!(
                "Received thread verified: thread={}, verified_at={}",
                verified.thread_id, verified.verified_at
            );
            message_tx
                .send(AppMessage::ThreadVerified {
                    thread_id: verified.thread_id,
                    verified_at: verified.verified_at,
                })
                .map_err(|e| format!("Failed to send ThreadVerified: {}", e))
        }
        WsIncomingMessage::SystemMetricsUpdate(metrics) => {
            // System metrics update - convert MB to GB for SystemStats
            let stats = SystemStats::new(
                true, // WebSocket is connected if we're receiving this
                metrics.cpu_percent,
                metrics.memory_used_mb as f32 / 1024.0, // Convert MB to GB
                metrics.memory_total_mb as f32 / 1024.0, // Convert MB to GB
            );
            message_tx
                .send(AppMessage::SystemStatsUpdate(stats))
                .map_err(|e| format!("Failed to send SystemStatsUpdate: {}", e))
        }
        WsIncomingMessage::RawMessage(raw) => message_tx
            .send(AppMessage::WsRawMessage { message: raw })
            .map_err(|e| format!("Failed to send WsRawMessage: {}", e)),
        WsIncomingMessage::ParseError { error, raw } => message_tx
            .send(AppMessage::WsParseError { error, raw })
            .map_err(|e| format!("Failed to send WsParseError: {}", e)),
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
            auth_token: None,
        };

        let result = start_websocket_with_config(tx, config).await;
        // Should return Err with error message on connection failure
        assert!(result.is_err());
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("127.0.0.1:59999"));
        assert!(error_msg.contains("Failed to connect"));
    }
}
