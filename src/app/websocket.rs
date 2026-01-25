//! WebSocket integration for the App.
//!
//! This module handles connecting the WebSocket client to the application,
//! routing incoming messages to AppMessage, and managing connection state.

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::state::session::AskUserQuestionData;
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

            // Check if this is an AskUserQuestion request and extract question data
            if req.tool_name == "AskUserQuestion" {
                if let Some(thread_id) = &req.thread_id {
                    // Try to parse the question data from tool_input
                    match serde_json::from_value::<AskUserQuestionData>(req.tool_input.clone()) {
                        Ok(question_data) => {
                            info!(
                                "Extracted AskUserQuestion data for thread {}: {} questions",
                                thread_id,
                                question_data.questions.len()
                            );
                            // Send the pending question message
                            let _ = message_tx.send(AppMessage::PendingQuestion {
                                thread_id: thread_id.clone(),
                                request_id: req.request_id.clone(),
                                question_data,
                            });
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse AskUserQuestion data for thread {}: {}",
                                thread_id, e
                            );
                        }
                    }
                } else {
                    warn!("AskUserQuestion request missing thread_id: {}", req.request_id);
                }
            }

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
            info!(
                "Received thread_created: thread_id={}, title={:?}, type={:?}, mode={:?}, status={:?}",
                created.thread.id,
                created.thread.title,
                created.thread.thread_type,
                created.thread.mode,
                created.thread.status
            );
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
        WsIncomingMessage::ThreadUpdated(update) => {
            // Thread metadata update notification
            info!(
                "Received thread updated: thread={}, title={}, description={}",
                update.thread_id, update.title, update.description
            );
            message_tx
                .send(AppMessage::ThreadMetadataUpdated {
                    thread_id: update.thread_id,
                    title: Some(update.title),
                    description: Some(update.description),
                })
                .map_err(|e| format!("Failed to send ThreadMetadataUpdated: {}", e))
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
            thread_id: Some("thread-456".to_string()),
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

    #[test]
    fn test_route_ask_user_question_permission_request() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Create an AskUserQuestion permission request
        let ws_msg = WsIncomingMessage::PermissionRequest(crate::websocket::WsPermissionRequest {
            request_id: "perm-uuid-123".to_string(),
            thread_id: Some("thread-123".to_string()),
            tool_name: "AskUserQuestion".to_string(),
            tool_input: serde_json::json!({
                "questions": [
                    {
                        "question": "Which authentication method?",
                        "header": "Auth",
                        "options": [
                            {"label": "JWT", "description": "Stateless tokens"},
                            {"label": "Sessions", "description": "Server-side"}
                        ],
                        "multiSelect": false
                    }
                ],
                "answers": {}
            }),
            description: "Ask user about authentication".to_string(),
            timestamp: 1234567890,
        });

        let result = route_ws_message(ws_msg, &tx);
        assert!(result.is_ok());

        // First message should be PendingQuestion
        let first_msg = rx.try_recv();
        assert!(first_msg.is_ok());

        match first_msg.unwrap() {
            AppMessage::PendingQuestion {
                thread_id,
                request_id,
                question_data,
            } => {
                assert_eq!(thread_id, "thread-123");
                assert_eq!(request_id, "perm-uuid-123");
                assert_eq!(question_data.questions.len(), 1);
                assert_eq!(question_data.questions[0].question, "Which authentication method?");
                assert_eq!(question_data.questions[0].header, "Auth");
                assert_eq!(question_data.questions[0].options.len(), 2);
                assert_eq!(question_data.questions[0].options[0].label, "JWT");
                assert!(!question_data.questions[0].multi_select);
            }
            other => panic!("Expected PendingQuestion message, got {:?}", other),
        }

        // Second message should be PermissionRequested
        let second_msg = rx.try_recv();
        assert!(second_msg.is_ok());

        match second_msg.unwrap() {
            AppMessage::PermissionRequested {
                permission_id,
                tool_name,
                description,
                tool_input,
            } => {
                assert_eq!(permission_id, "perm-uuid-123");
                assert_eq!(tool_name, "AskUserQuestion");
                assert_eq!(description, "Ask user about authentication");
                assert!(tool_input.is_some());
            }
            other => panic!("Expected PermissionRequested message, got {:?}", other),
        }
    }

    #[test]
    fn test_route_ask_user_question_without_thread_id() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        // AskUserQuestion without thread_id should still route the permission request
        // but not send a PendingQuestion message
        let ws_msg = WsIncomingMessage::PermissionRequest(crate::websocket::WsPermissionRequest {
            request_id: "perm-no-thread".to_string(),
            thread_id: None,
            tool_name: "AskUserQuestion".to_string(),
            tool_input: serde_json::json!({
                "questions": [{"question": "Test?", "header": "T", "options": []}],
                "answers": {}
            }),
            description: "Test question".to_string(),
            timestamp: 1234567890,
        });

        let result = route_ws_message(ws_msg, &tx);
        assert!(result.is_ok());

        // Should only receive PermissionRequested (no PendingQuestion without thread_id)
        let msg = rx.try_recv();
        assert!(msg.is_ok());

        match msg.unwrap() {
            AppMessage::PermissionRequested { permission_id, .. } => {
                assert_eq!(permission_id, "perm-no-thread");
            }
            other => panic!("Expected PermissionRequested message, got {:?}", other),
        }

        // No more messages
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_route_ask_user_question_with_multi_select() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let ws_msg = WsIncomingMessage::PermissionRequest(crate::websocket::WsPermissionRequest {
            request_id: "perm-multi".to_string(),
            thread_id: Some("thread-multi".to_string()),
            tool_name: "AskUserQuestion".to_string(),
            tool_input: serde_json::json!({
                "questions": [
                    {
                        "question": "Select features to enable",
                        "header": "Features",
                        "options": [
                            {"label": "Dark mode", "description": "Enable dark theme"},
                            {"label": "Notifications", "description": "Push notifications"},
                            {"label": "Analytics", "description": "Usage analytics"}
                        ],
                        "multiSelect": true
                    }
                ],
                "answers": {}
            }),
            description: "Select features".to_string(),
            timestamp: 1234567890,
        });

        let result = route_ws_message(ws_msg, &tx);
        assert!(result.is_ok());

        // First message should be PendingQuestion
        let msg = rx.try_recv().unwrap();
        match msg {
            AppMessage::PendingQuestion { question_data, .. } => {
                assert!(question_data.questions[0].multi_select);
                assert_eq!(question_data.questions[0].options.len(), 3);
            }
            other => panic!("Expected PendingQuestion, got {:?}", other),
        }
    }

    #[test]
    fn test_route_thread_updated() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let ws_msg = WsIncomingMessage::ThreadUpdated(crate::websocket::messages::WsThreadUpdated {
            thread_id: "thread-456".to_string(),
            title: "Updated Title".to_string(),
            description: "Updated description".to_string(),
            timestamp: 1705315800000,
        });

        let result = route_ws_message(ws_msg, &tx);
        assert!(result.is_ok());

        // Verify the message was sent
        let received = rx.try_recv();
        assert!(received.is_ok());

        match received.unwrap() {
            AppMessage::ThreadMetadataUpdated {
                thread_id,
                title,
                description,
            } => {
                assert_eq!(thread_id, "thread-456");
                assert_eq!(title, Some("Updated Title".to_string()));
                assert_eq!(description, Some("Updated description".to_string()));
            }
            _ => panic!("Expected ThreadMetadataUpdated message"),
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
