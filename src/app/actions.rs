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
    WsCommandResponse, WsCommandResult, WsConnectionState, WsOutgoingMessage, WsPermissionData,
    WsPlanApprovalResponse,
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

        match sender.try_send(WsOutgoingMessage::CommandResponse(response)) {
            Ok(()) => {
                info!(
                    "Sent permission response via WebSocket: {} -> {}",
                    request_id, allowed
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
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::dashboard::PlanSummary;
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
                plan_summary: "Test Plan".to_string(),
            }),
        );
        app.dashboard.set_plan_request(
            "t2",
            "plan-t2".to_string(),
            PlanSummary::new(
                "Test Plan".to_string(),
                vec!["Phase 1".to_string()],
                3,
                1000,
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
