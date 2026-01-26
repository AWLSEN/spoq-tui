use serde::{Deserialize, Serialize};

use crate::models::{PlanSummary, Thread, ThreadMode, ThreadStatus, WaitingFor};

/// Incoming WebSocket messages from the client
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum WsIncomingMessage {
    #[serde(rename = "permission_request")]
    PermissionRequest(WsPermissionRequest),
    /// Agent status update (thinking, idle, streaming, tool_use)
    #[serde(rename = "agent_status")]
    AgentStatus(WsAgentStatus),
    /// Connection confirmation from server
    #[serde(rename = "connected")]
    Connected(WsConnected),
    /// Thread status update for dashboard view
    #[serde(rename = "thread_status_update")]
    ThreadStatusUpdate(WsThreadStatusUpdate),
    /// New thread created (first message to a new thread_id)
    #[serde(rename = "thread_created")]
    ThreadCreated(WsThreadCreated),
    /// Plan approval request from agent
    #[serde(rename = "plan_approval_request")]
    PlanApprovalRequest(WsPlanApprovalRequest),
    /// Thread mode update (normal, plan, exec)
    #[serde(rename = "thread_mode_update")]
    ThreadModeUpdate(WsThreadModeUpdate),
    /// Phase progress update during plan execution
    #[serde(rename = "phase_progress_update")]
    PhaseProgressUpdate(WsPhaseProgressUpdate),
    /// Thread verification notification
    #[serde(rename = "thread_verified")]
    ThreadVerified(WsThreadVerified),
    /// Thread metadata updated (title, description)
    #[serde(rename = "thread_updated")]
    ThreadUpdated(WsThreadUpdated),
    /// System metrics update (CPU, RAM usage)
    #[serde(rename = "system_metrics_update")]
    SystemMetricsUpdate(WsSystemMetricsUpdate),
    /// Stream started - notifies frontend of thread_id immediately when stream begins
    #[serde(rename = "stream_started")]
    StreamStarted(WsStreamStarted),
    /// Raw message received (for debugging - not deserialized from JSON)
    #[serde(skip)]
    RawMessage(String),
    /// Parse error occurred (for debugging - not deserialized from JSON)
    #[serde(skip)]
    ParseError { error: String, raw: String },
}

/// Connection confirmation from WebSocket server
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsConnected {
    pub session_id: String,
    pub timestamp: u64,
}

/// Stream started - notifies frontend immediately when stream begins
/// Sent when a new SSE stream starts for a thread
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsStreamStarted {
    pub thread_id: String,
    pub session_id: String,
    pub timestamp: u64,
}

/// Agent status update
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsAgentStatus {
    pub thread_id: String,
    pub state: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_operation: Option<String>,
    pub timestamp: u64,
}

/// Permission request from client
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsPermissionRequest {
    pub request_id: String,
    /// Thread ID this request belongs to (optional for backwards compatibility)
    #[serde(default)]
    pub thread_id: Option<String>,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub description: String,
    pub timestamp: u64,
}

/// Thread status update for dashboard view
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsThreadStatusUpdate {
    /// Thread ID being updated
    pub thread_id: String,
    /// New status
    pub status: ThreadStatus,
    /// What the thread is waiting for (if status is Waiting)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_for: Option<WaitingFor>,
    /// When this update occurred (Unix milliseconds)
    pub timestamp: u64,
}

/// New thread created notification
///
/// Sent when a new thread is created (first message to a new thread_id).
/// Allows clients to immediately add the thread without polling.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsThreadCreated {
    /// The newly created thread
    pub thread: Thread,
    /// When this event occurred (unix ms)
    pub timestamp: u64,
}

/// Plan approval request from agent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsPlanApprovalRequest {
    /// Thread ID requesting approval
    pub thread_id: String,
    /// Request ID for tracking the response
    pub request_id: String,
    /// Summary of the plan
    pub plan_summary: PlanSummary,
    /// When this request was created (Unix milliseconds)
    pub timestamp: u64,
}

/// Thread mode update notification
///
/// Sent when a thread's mode changes (normal -> plan -> exec)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsThreadModeUpdate {
    /// Thread ID being updated
    pub thread_id: String,
    /// New mode for the thread
    pub mode: ThreadMode,
    /// When this update occurred (ISO8601 timestamp string)
    pub timestamp: String,
}

/// Status of a phase during plan execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PhaseStatus {
    /// Phase is pending (not yet started)
    Pending,
    /// Phase is starting
    Starting,
    /// Phase is currently running
    Running,
    /// Phase completed successfully
    Completed,
    /// Phase failed
    Failed,
}

/// Phase progress update during plan execution
///
/// Sent periodically during Pulsar execution to report phase progress
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsPhaseProgressUpdate {
    /// Thread ID (optional - may be null for plan-level updates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Plan ID being executed
    pub plan_id: String,
    /// Current phase index (0-based)
    pub phase_index: u32,
    /// Total number of phases
    pub total_phases: u32,
    /// Name of the current phase
    pub phase_name: String,
    /// Status of the phase
    pub status: PhaseStatus,
    /// Number of tools used in this phase
    pub tool_count: u32,
    /// Name of the last tool used (optional - may be null if no tools used yet)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_tool: Option<String>,
    /// Last file modified (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_file: Option<String>,
    /// When the phase started (Unix milliseconds)
    pub started_at: i64,
    /// When this update was generated (Unix milliseconds)
    pub updated_at: i64,
    /// Message timestamp (Unix milliseconds)
    pub timestamp: i64,
}

/// Thread verification notification
///
/// Sent when a thread's work has been verified/tested
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsThreadVerified {
    /// Thread ID that was verified
    pub thread_id: String,
    /// When the verification occurred (ISO8601 timestamp string)
    pub verified_at: String,
    /// When this notification was sent (ISO8601 timestamp string)
    pub timestamp: String,
}

/// Thread metadata updated notification
///
/// Sent when a thread's title or description has been updated
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsThreadUpdated {
    /// Thread ID being updated
    pub thread_id: String,
    /// Updated title
    pub title: String,
    /// Updated description
    pub description: String,
    /// When this update occurred (Unix milliseconds)
    pub timestamp: u64,
}

/// System metrics update from backend
///
/// Sent periodically by the backend with current system resource usage
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsSystemMetricsUpdate {
    /// CPU usage percentage (0-100)
    pub cpu_percent: f32,
    /// Memory used in megabytes
    pub memory_used_mb: u64,
    /// Total memory in megabytes
    pub memory_total_mb: u64,
    /// Memory usage percentage (0-100)
    pub memory_percent: f32,
    /// When this update was generated (Unix milliseconds)
    pub timestamp: u64,
}

/// Command response sent to client
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsCommandResponse {
    #[serde(rename = "type")]
    pub type_: String,
    pub request_id: String,
    pub result: WsCommandResult,
}

/// Result of a command
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsCommandResult {
    pub status: String,
    pub data: WsPermissionData,
}

/// Permission decision data
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsPermissionData {
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Cancel permission request (user pressed Shift+Escape)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsCancelPermission {
    #[serde(rename = "type")]
    pub type_: String,
    pub request_id: String,
}

impl WsCancelPermission {
    pub fn new(request_id: String) -> Self {
        Self {
            type_: "cancel_permission".to_string(),
            request_id,
        }
    }
}

/// Plan approval response (user approved or rejected plan)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsPlanApprovalResponse {
    #[serde(rename = "type")]
    pub type_: String,
    /// Request ID from the original plan approval request
    pub request_id: String,
    /// Whether the plan was approved
    pub approved: bool,
}

impl WsPlanApprovalResponse {
    pub fn new(request_id: String, approved: bool) -> Self {
        Self {
            type_: "plan_approval_response".to_string(),
            request_id,
            approved,
        }
    }
}

/// Outgoing WebSocket messages (sent to server)
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum WsOutgoingMessage {
    CommandResponse(WsCommandResponse),
    CancelPermission(WsCancelPermission),
    PlanApprovalResponse(WsPlanApprovalResponse),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_permission_request() {
        let json = r#"{
            "type": "permission_request",
            "request_id": "req-123",
            "tool_name": "Bash",
            "tool_input": {"command": "ls -la"},
            "description": "List directory contents",
            "timestamp": 1234567890
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PermissionRequest(req) => {
                assert_eq!(req.request_id, "req-123");
                assert_eq!(req.tool_name, "Bash");
                assert_eq!(req.description, "List directory contents");
                assert_eq!(req.timestamp, 1234567890);
                // thread_id should be None when not present
                assert!(req.thread_id.is_none());
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[test]
    fn test_deserialize_permission_request_with_thread_id() {
        let json = r#"{
            "type": "permission_request",
            "request_id": "perm-uuid",
            "thread_id": "thread-123",
            "tool_name": "AskUserQuestion",
            "tool_input": {
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
            },
            "description": "Ask user about authentication",
            "timestamp": 1234567890
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PermissionRequest(req) => {
                assert_eq!(req.request_id, "perm-uuid");
                assert_eq!(req.thread_id, Some("thread-123".to_string()));
                assert_eq!(req.tool_name, "AskUserQuestion");
                assert_eq!(req.description, "Ask user about authentication");
                assert_eq!(req.timestamp, 1234567890);
                // Verify tool_input structure
                assert!(req.tool_input["questions"].is_array());
                assert_eq!(req.tool_input["questions"].as_array().unwrap().len(), 1);
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[test]
    fn test_deserialize_permission_request_with_null_thread_id() {
        // Test that null thread_id deserializes to None
        let json = r#"{
            "type": "permission_request",
            "request_id": "req-null-thread",
            "thread_id": null,
            "tool_name": "Bash",
            "tool_input": {},
            "description": "Test",
            "timestamp": 1234567890
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PermissionRequest(req) => {
                assert_eq!(req.request_id, "req-null-thread");
                assert!(req.thread_id.is_none());
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[test]
    fn test_serialize_command_response() {
        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "req-123".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some("Permission granted".to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "command_response");
        assert_eq!(parsed["request_id"], "req-123");
        assert_eq!(parsed["result"]["status"], "success");
        assert_eq!(parsed["result"]["data"]["allowed"], true);
        assert_eq!(parsed["result"]["data"]["message"], "Permission granted");
    }

    #[test]
    fn test_serialize_command_response_no_message() {
        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "req-456".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: false,
                    message: None,
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "command_response");
        assert_eq!(parsed["request_id"], "req-456");
        assert_eq!(parsed["result"]["status"], "success");
        assert_eq!(parsed["result"]["data"]["allowed"], false);
        assert!(parsed["result"]["data"]["message"].is_null());
    }

    #[test]
    fn test_roundtrip_permission_request() {
        let original = WsIncomingMessage::PermissionRequest(WsPermissionRequest {
            request_id: "req-789".to_string(),
            thread_id: Some("thread-456".to_string()),
            tool_name: "Read".to_string(),
            tool_input: serde_json::json!({"file_path": "/etc/hosts"}),
            description: "Read hosts file".to_string(),
            timestamp: 9876543210,
        });

        let json = serde_json::to_string(&original).unwrap();
        let parsed: WsIncomingMessage = serde_json::from_str(&json).unwrap();

        match parsed {
            WsIncomingMessage::PermissionRequest(req) => {
                assert_eq!(req.request_id, "req-789");
                assert_eq!(req.thread_id, Some("thread-456".to_string()));
                assert_eq!(req.tool_name, "Read");
                assert_eq!(req.description, "Read hosts file");
                assert_eq!(req.timestamp, 9876543210);
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[test]
    fn test_malformed_json_missing_type() {
        let json = r#"{
            "request_id": "req-123",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"},
            "description": "List files",
            "timestamp": 1234567890
        }"#;

        let result = serde_json::from_str::<WsIncomingMessage>(json);
        assert!(result.is_err(), "Should fail without 'type' field");
    }

    #[test]
    fn test_malformed_json_invalid_type() {
        let json = r#"{
            "type": "unknown_message_type",
            "request_id": "req-123",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"},
            "description": "List files",
            "timestamp": 1234567890
        }"#;

        let result = serde_json::from_str::<WsIncomingMessage>(json);
        assert!(result.is_err(), "Should fail with unknown message type");
    }

    #[test]
    fn test_malformed_json_missing_required_field() {
        let json = r#"{
            "type": "permission_request",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"},
            "description": "List files",
            "timestamp": 1234567890
        }"#;

        let result = serde_json::from_str::<WsIncomingMessage>(json);
        assert!(result.is_err(), "Should fail without required 'request_id'");
    }

    #[test]
    fn test_malformed_json_invalid_timestamp_type() {
        let json = r#"{
            "type": "permission_request",
            "request_id": "req-123",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"},
            "description": "List files",
            "timestamp": "not-a-number"
        }"#;

        let result = serde_json::from_str::<WsIncomingMessage>(json);
        assert!(result.is_err(), "Should fail with invalid timestamp type");
    }

    #[test]
    fn test_complex_tool_input() {
        let json = r#"{
            "type": "permission_request",
            "request_id": "req-complex",
            "tool_name": "ComplexTool",
            "tool_input": {
                "nested": {
                    "array": [1, 2, 3],
                    "object": {"key": "value"}
                },
                "string": "test",
                "number": 42,
                "boolean": true,
                "null": null
            },
            "description": "Complex tool input",
            "timestamp": 1234567890
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PermissionRequest(req) => {
                assert_eq!(req.request_id, "req-complex");
                assert!(req.tool_input.is_object());
                assert_eq!(req.tool_input["string"], "test");
                assert_eq!(req.tool_input["number"], 42);
                assert_eq!(req.tool_input["boolean"], true);
                assert!(req.tool_input["null"].is_null());
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[test]
    fn test_empty_tool_input() {
        let json = r#"{
            "type": "permission_request",
            "request_id": "req-empty",
            "tool_name": "NoInputTool",
            "tool_input": {},
            "description": "Tool with no input",
            "timestamp": 1234567890
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PermissionRequest(req) => {
                assert_eq!(req.request_id, "req-empty");
                assert!(req.tool_input.is_object());
                assert_eq!(req.tool_input.as_object().unwrap().len(), 0);
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[test]
    fn test_ws_permission_request_clone() {
        let req = WsPermissionRequest {
            request_id: "req-clone".to_string(),
            thread_id: Some("thread-123".to_string()),
            tool_name: "Test".to_string(),
            tool_input: serde_json::json!({"key": "value"}),
            description: "Test description".to_string(),
            timestamp: 1234567890,
        };

        let cloned = req.clone();
        assert_eq!(req.request_id, cloned.request_id);
        assert_eq!(req.thread_id, cloned.thread_id);
        assert_eq!(req.tool_name, cloned.tool_name);
        assert_eq!(req.description, cloned.description);
        assert_eq!(req.timestamp, cloned.timestamp);
    }

    #[test]
    fn test_ws_permission_request_debug() {
        let req = WsPermissionRequest {
            request_id: "req-debug".to_string(),
            thread_id: None,
            tool_name: "DebugTool".to_string(),
            tool_input: serde_json::json!({"test": true}),
            description: "Debug test".to_string(),
            timestamp: 1234567890,
        };

        let debug_str = format!("{:?}", req);
        assert!(debug_str.contains("req-debug"));
        assert!(debug_str.contains("DebugTool"));
        assert!(debug_str.contains("Debug test"));
    }

    #[test]
    fn test_ws_command_response_serialize_structure() {
        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "req-struct".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some("Approved".to_string()),
                },
            },
        };

        let json = serde_json::to_value(&response).unwrap();

        // Verify field names are correct
        assert_eq!(json["type"], "command_response");
        assert_eq!(json["request_id"], "req-struct");
        assert!(json["result"].is_object());
        assert_eq!(json["result"]["status"], "success");
        assert!(json["result"]["data"].is_object());
        assert_eq!(json["result"]["data"]["allowed"], true);
        assert_eq!(json["result"]["data"]["message"], "Approved");
    }

    #[test]
    fn test_ws_command_response_deserialize() {
        let json = r#"{
            "type": "command_response",
            "request_id": "req-deser",
            "result": {
                "status": "success",
                "data": {
                    "allowed": false,
                    "message": "Denied"
                }
            }
        }"#;

        let response: WsCommandResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.type_, "command_response");
        assert_eq!(response.request_id, "req-deser");
        assert_eq!(response.result.status, "success");
        assert!(!response.result.data.allowed);
        assert_eq!(response.result.data.message, Some("Denied".to_string()));
    }

    #[test]
    fn test_ws_permission_data_skip_serializing_none() {
        let data = WsPermissionData {
            allowed: true,
            message: None,
        };

        let json = serde_json::to_value(&data).unwrap();

        // Verify that message field is not present when None
        assert!(json["allowed"].is_boolean());
        assert!(json.get("message").is_none() || json["message"].is_null());

        // Verify the JSON doesn't have a "message" key when serialized to string
        let json_str = serde_json::to_string(&data).unwrap();
        assert!(!json_str.contains("\"message\""));
    }

    #[test]
    fn test_ws_command_result_clone() {
        let result = WsCommandResult {
            status: "success".to_string(),
            data: WsPermissionData {
                allowed: true,
                message: Some("Test".to_string()),
            },
        };

        let cloned = result.clone();
        assert_eq!(result.status, cloned.status);
        assert_eq!(result.data.allowed, cloned.data.allowed);
        assert_eq!(result.data.message, cloned.data.message);
    }

    #[test]
    fn test_large_timestamp() {
        let json = r#"{
            "type": "permission_request",
            "request_id": "req-large-ts",
            "tool_name": "Test",
            "tool_input": {},
            "description": "Large timestamp test",
            "timestamp": 9999999999999
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PermissionRequest(req) => {
                assert_eq!(req.timestamp, 9999999999999);
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[test]
    fn test_special_characters_in_strings() {
        let json = r#"{
            "type": "permission_request",
            "request_id": "req-special",
            "tool_name": "Special\n\t\r\"Tool",
            "tool_input": {"path": "/path/with/\"quotes\"/and\\backslashes"},
            "description": "Description with\nnewlines\tand\ttabs",
            "timestamp": 1234567890
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PermissionRequest(req) => {
                assert_eq!(req.request_id, "req-special");
                assert!(req.tool_name.contains('\n'));
                assert!(req.description.contains('\n'));
                assert!(req.description.contains('\t'));
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[test]
    fn test_unicode_in_strings() {
        let json = r#"{
            "type": "permission_request",
            "request_id": "req-unicode",
            "tool_name": "🔧 Tool",
            "tool_input": {"message": "Hello 世界 🌍"},
            "description": "Test with émojis and ünïcödé",
            "timestamp": 1234567890
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PermissionRequest(req) => {
                assert_eq!(req.request_id, "req-unicode");
                assert!(req.tool_name.contains('🔧'));
                assert!(req.description.contains('é'));
            }
            _ => panic!("Unexpected message type"),
        }
    }

    // -------------------- Thread Status Update Tests --------------------

    #[test]
    fn test_deserialize_thread_status_update() {
        let json = r#"{
            "type": "thread_status_update",
            "thread_id": "thread-123",
            "status": "running",
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadStatusUpdate(update) => {
                assert_eq!(update.thread_id, "thread-123");
                assert_eq!(update.status, ThreadStatus::Running);
                assert!(update.waiting_for.is_none());
                assert_eq!(update.timestamp, 1705315800000);
            }
            _ => panic!("Expected ThreadStatusUpdate"),
        }
    }

    #[test]
    fn test_deserialize_thread_status_update_with_waiting_for() {
        let json = r#"{
            "type": "thread_status_update",
            "thread_id": "thread-456",
            "status": "waiting",
            "waiting_for": {
                "type": "permission",
                "request_id": "req-789",
                "tool_name": "Bash"
            },
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadStatusUpdate(update) => {
                assert_eq!(update.thread_id, "thread-456");
                assert_eq!(update.status, ThreadStatus::Waiting);
                assert!(update.waiting_for.is_some());
                match update.waiting_for.unwrap() {
                    WaitingFor::Permission {
                        request_id,
                        tool_name,
                    } => {
                        assert_eq!(request_id, "req-789");
                        assert_eq!(tool_name, "Bash");
                    }
                    _ => panic!("Expected Permission variant"),
                }
            }
            _ => panic!("Expected ThreadStatusUpdate"),
        }
    }

    #[test]
    fn test_serialize_thread_status_update() {
        let update = WsThreadStatusUpdate {
            thread_id: "thread-serialize".to_string(),
            status: ThreadStatus::Done,
            waiting_for: None,
            timestamp: 1705315800000, // Unix ms
        };

        let json = serde_json::to_string(&update).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-serialize");
        assert_eq!(parsed["status"], "done");
        assert!(parsed.get("waiting_for").is_none() || parsed["waiting_for"].is_null());
        assert_eq!(parsed["timestamp"], 1705315800000_i64);
    }

    // -------------------- Plan Approval Request Tests --------------------

    #[test]
    fn test_deserialize_plan_approval_request() {
        let json = r#"{
            "type": "plan_approval_request",
            "thread_id": "thread-plan-1",
            "request_id": "plan-req-123",
            "plan_summary": {
                "title": "Add dark mode",
                "phases": ["Setup theme", "Update components", "Test"],
                "file_count": 15,
                "estimated_tokens": 50000
            },
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PlanApprovalRequest(req) => {
                assert_eq!(req.thread_id, "thread-plan-1");
                assert_eq!(req.request_id, "plan-req-123");
                assert_eq!(req.plan_summary.title, "Add dark mode");
                assert_eq!(req.plan_summary.phases.len(), 3);
                assert_eq!(req.plan_summary.file_count, 15);
                assert_eq!(req.plan_summary.estimated_tokens, Some(50000));
                assert_eq!(req.timestamp, 1705315800000);
            }
            _ => panic!("Expected PlanApprovalRequest"),
        }
    }

    #[test]
    fn test_serialize_plan_approval_request() {
        use crate::models::dashboard::PlanSummary;

        let req = WsPlanApprovalRequest {
            thread_id: "thread-plan-serialize".to_string(),
            request_id: "plan-req-456".to_string(),
            plan_summary: PlanSummary::new(
                "Refactor module".to_string(),
                vec!["Phase 1".to_string(), "Phase 2".to_string()],
                5,
                Some(10000),
            ),
            timestamp: 1705315800000, // Unix ms
        };

        let json = serde_json::to_string(&req).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-plan-serialize");
        assert_eq!(parsed["request_id"], "plan-req-456");
        assert_eq!(parsed["plan_summary"]["title"], "Refactor module");
        assert_eq!(parsed["timestamp"], 1705315800000_i64);
        assert_eq!(
            parsed["plan_summary"]["phases"].as_array().unwrap().len(),
            2
        );
    }

    #[test]
    fn test_deserialize_plan_approval_request_null_estimated_tokens() {
        // Backend may send null for estimated_tokens
        let json = r#"{
            "type": "plan_approval_request",
            "thread_id": "thread-plan-null",
            "request_id": "plan-req-null",
            "plan_summary": {
                "title": "Quick fix",
                "phases": ["Fix bug"],
                "file_count": 2,
                "estimated_tokens": null
            },
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PlanApprovalRequest(req) => {
                assert_eq!(req.thread_id, "thread-plan-null");
                assert_eq!(req.request_id, "plan-req-null");
                assert_eq!(req.plan_summary.title, "Quick fix");
                assert_eq!(req.plan_summary.phases.len(), 1);
                assert_eq!(req.plan_summary.file_count, 2);
                assert!(req.plan_summary.estimated_tokens.is_none());
                assert_eq!(req.timestamp, 1705315800000);
            }
            _ => panic!("Expected PlanApprovalRequest"),
        }
    }

    #[test]
    fn test_deserialize_plan_approval_request_missing_estimated_tokens() {
        // Backend may omit estimated_tokens field entirely
        let json = r#"{
            "type": "plan_approval_request",
            "thread_id": "thread-plan-missing",
            "request_id": "plan-req-missing",
            "plan_summary": {
                "title": "Small change",
                "phases": ["Update config"],
                "file_count": 1
            },
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PlanApprovalRequest(req) => {
                assert_eq!(req.thread_id, "thread-plan-missing");
                assert_eq!(req.request_id, "plan-req-missing");
                assert_eq!(req.plan_summary.title, "Small change");
                assert_eq!(req.plan_summary.phases.len(), 1);
                assert_eq!(req.plan_summary.file_count, 1);
                assert!(req.plan_summary.estimated_tokens.is_none());
                assert_eq!(req.timestamp, 1705315800000);
            }
            _ => panic!("Expected PlanApprovalRequest"),
        }
    }

    // -------------------- Plan Approval Response Tests --------------------

    #[test]
    fn test_plan_approval_response_new_approved() {
        let response = WsPlanApprovalResponse::new("req-approve".to_string(), true);

        assert_eq!(response.type_, "plan_approval_response");
        assert_eq!(response.request_id, "req-approve");
        assert!(response.approved);
    }

    #[test]
    fn test_plan_approval_response_new_rejected() {
        let response = WsPlanApprovalResponse::new("req-reject".to_string(), false);

        assert_eq!(response.type_, "plan_approval_response");
        assert_eq!(response.request_id, "req-reject");
        assert!(!response.approved);
    }

    #[test]
    fn test_serialize_plan_approval_response() {
        let response = WsPlanApprovalResponse::new("req-serialize".to_string(), true);

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "plan_approval_response");
        assert_eq!(parsed["request_id"], "req-serialize");
        assert_eq!(parsed["approved"], true);
    }

    #[test]
    fn test_ws_outgoing_message_plan_approval() {
        let response = WsPlanApprovalResponse::new("req-outgoing".to_string(), false);
        let outgoing = WsOutgoingMessage::PlanApprovalResponse(response);

        let json = serde_json::to_string(&outgoing).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["type"], "plan_approval_response");
        assert_eq!(parsed["request_id"], "req-outgoing");
        assert_eq!(parsed["approved"], false);
    }

    // -------------------- Thread Mode Update Tests --------------------

    #[test]
    fn test_deserialize_thread_mode_update() {
        // Backend sends ISO8601 timestamp strings
        let json = r#"{
            "type": "thread_mode_update",
            "thread_id": "thread-123",
            "mode": "plan",
            "timestamp": "2026-01-15T10:30:00Z"
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadModeUpdate(update) => {
                assert_eq!(update.thread_id, "thread-123");
                assert_eq!(update.mode, crate::models::ThreadMode::Plan);
                assert_eq!(update.timestamp, "2026-01-15T10:30:00Z");
            }
            _ => panic!("Expected ThreadModeUpdate"),
        }
    }

    #[test]
    fn test_deserialize_thread_mode_update_exec() {
        let json = r#"{
            "type": "thread_mode_update",
            "thread_id": "thread-456",
            "mode": "exec",
            "timestamp": "2026-01-15T10:30:00Z"
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadModeUpdate(update) => {
                assert_eq!(update.thread_id, "thread-456");
                assert_eq!(update.mode, crate::models::ThreadMode::Exec);
                assert_eq!(update.timestamp, "2026-01-15T10:30:00Z");
            }
            _ => panic!("Expected ThreadModeUpdate"),
        }
    }

    #[test]
    fn test_serialize_thread_mode_update() {
        let update = WsThreadModeUpdate {
            thread_id: "thread-serialize".to_string(),
            mode: crate::models::ThreadMode::Plan,
            timestamp: "2026-01-15T10:30:00Z".to_string(),
        };

        let json = serde_json::to_string(&update).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-serialize");
        assert_eq!(parsed["mode"], "plan");
        assert_eq!(parsed["timestamp"], "2026-01-15T10:30:00Z");
    }

    // -------------------- Phase Progress Update Tests --------------------

    #[test]
    fn test_deserialize_phase_progress_update() {
        let json = r#"{
            "type": "phase_progress_update",
            "thread_id": "thread-123",
            "plan_id": "plan-456",
            "phase_index": 2,
            "total_phases": 5,
            "phase_name": "Add WebSocket handlers",
            "status": "running",
            "tool_count": 15,
            "last_tool": "Edit",
            "last_file": "/src/websocket/handlers.rs",
            "started_at": 1705315700000,
            "updated_at": 1705315800000,
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PhaseProgressUpdate(progress) => {
                assert_eq!(progress.thread_id, Some("thread-123".to_string()));
                assert_eq!(progress.plan_id, "plan-456");
                assert_eq!(progress.phase_index, 2);
                assert_eq!(progress.total_phases, 5);
                assert_eq!(progress.phase_name, "Add WebSocket handlers");
                assert_eq!(progress.status, PhaseStatus::Running);
                assert_eq!(progress.tool_count, 15);
                assert_eq!(progress.last_tool, Some("Edit".to_string()));
                assert_eq!(
                    progress.last_file,
                    Some("/src/websocket/handlers.rs".to_string())
                );
                assert_eq!(progress.started_at, 1705315700000);
                assert_eq!(progress.updated_at, 1705315800000);
                assert_eq!(progress.timestamp, 1705315800000);
            }
            _ => panic!("Expected PhaseProgressUpdate"),
        }
    }

    #[test]
    fn test_deserialize_phase_progress_update_no_thread_id() {
        let json = r#"{
            "type": "phase_progress_update",
            "plan_id": "plan-789",
            "phase_index": 0,
            "total_phases": 3,
            "phase_name": "Setup",
            "status": "starting",
            "tool_count": 0,
            "last_tool": "",
            "started_at": 1705315700000,
            "updated_at": 1705315700000,
            "timestamp": 1705315700000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PhaseProgressUpdate(progress) => {
                assert!(progress.thread_id.is_none());
                assert_eq!(progress.plan_id, "plan-789");
                assert_eq!(progress.status, PhaseStatus::Starting);
                assert!(progress.last_file.is_none());
                // Empty string deserializes as Some("")
                assert_eq!(progress.last_tool, Some("".to_string()));
            }
            _ => panic!("Expected PhaseProgressUpdate"),
        }
    }

    #[test]
    fn test_deserialize_phase_progress_update_null_last_tool() {
        // This is the critical test case: backend sends null for last_tool
        let json = r#"{
            "type": "phase_progress_update",
            "plan_id": "plan-null-tool",
            "phase_index": 0,
            "total_phases": 3,
            "phase_name": "Starting",
            "status": "starting",
            "tool_count": 0,
            "last_tool": null,
            "started_at": 1705315700000,
            "updated_at": 1705315700000,
            "timestamp": 1705315700000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PhaseProgressUpdate(progress) => {
                assert_eq!(progress.plan_id, "plan-null-tool");
                assert_eq!(progress.status, PhaseStatus::Starting);
                assert!(progress.last_tool.is_none());
            }
            _ => panic!("Expected PhaseProgressUpdate"),
        }
    }

    #[test]
    fn test_deserialize_phase_progress_update_missing_last_tool() {
        // Test that missing last_tool field defaults to None
        let json = r#"{
            "type": "phase_progress_update",
            "plan_id": "plan-missing-tool",
            "phase_index": 0,
            "total_phases": 3,
            "phase_name": "Starting",
            "status": "starting",
            "tool_count": 0,
            "started_at": 1705315700000,
            "updated_at": 1705315700000,
            "timestamp": 1705315700000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PhaseProgressUpdate(progress) => {
                assert_eq!(progress.plan_id, "plan-missing-tool");
                assert!(progress.last_tool.is_none());
            }
            _ => panic!("Expected PhaseProgressUpdate"),
        }
    }

    #[test]
    fn test_phase_status_all_variants() {
        // Test all PhaseStatus variants deserialize correctly
        let variants = [
            ("pending", PhaseStatus::Pending),
            ("starting", PhaseStatus::Starting),
            ("running", PhaseStatus::Running),
            ("completed", PhaseStatus::Completed),
            ("failed", PhaseStatus::Failed),
        ];

        for (json_val, expected) in variants {
            let json = format!(r#""{json_val}""#);
            let status: PhaseStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, expected);
        }
    }

    #[test]
    fn test_serialize_phase_progress_update() {
        let progress = WsPhaseProgressUpdate {
            thread_id: Some("thread-serialize".to_string()),
            plan_id: "plan-serialize".to_string(),
            phase_index: 1,
            total_phases: 4,
            phase_name: "Test Phase".to_string(),
            status: PhaseStatus::Completed,
            tool_count: 10,
            last_tool: Some("Bash".to_string()),
            last_file: None,
            started_at: 1705315700000,
            updated_at: 1705315800000,
            timestamp: 1705315800000,
        };

        let json = serde_json::to_string(&progress).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-serialize");
        assert_eq!(parsed["plan_id"], "plan-serialize");
        assert_eq!(parsed["phase_index"], 1);
        assert_eq!(parsed["total_phases"], 4);
        assert_eq!(parsed["status"], "completed");
        assert_eq!(parsed["last_tool"], "Bash");
        // last_file should not be present when None
        assert!(parsed.get("last_file").is_none() || parsed["last_file"].is_null());
    }

    #[test]
    fn test_serialize_phase_progress_update_none_last_tool() {
        let progress = WsPhaseProgressUpdate {
            thread_id: None,
            plan_id: "plan-no-tool".to_string(),
            phase_index: 0,
            total_phases: 1,
            phase_name: "Setup".to_string(),
            status: PhaseStatus::Starting,
            tool_count: 0,
            last_tool: None,
            last_file: None,
            started_at: 1705315700000,
            updated_at: 1705315700000,
            timestamp: 1705315700000,
        };

        let json = serde_json::to_string(&progress).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // last_tool should not be present when None (skip_serializing_if)
        assert!(parsed.get("last_tool").is_none() || parsed["last_tool"].is_null());
    }

    #[test]
    fn test_deserialize_phase_progress_update_i64_timestamps() {
        // Test that i64 timestamps work (including negative values and large values)
        let json = r#"{
            "type": "phase_progress_update",
            "plan_id": "plan-i64",
            "phase_index": 0,
            "total_phases": 1,
            "phase_name": "Test",
            "status": "running",
            "tool_count": 0,
            "started_at": -1000,
            "updated_at": 9223372036854775807,
            "timestamp": 1705315700000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PhaseProgressUpdate(progress) => {
                assert_eq!(progress.started_at, -1000);
                assert_eq!(progress.updated_at, 9223372036854775807_i64);
                assert_eq!(progress.timestamp, 1705315700000);
            }
            _ => panic!("Expected PhaseProgressUpdate"),
        }
    }

    // -------------------- Thread Created Tests --------------------

    #[test]
    fn test_deserialize_thread_created_minimal() {
        // Test minimal ThreadCreated message with required fields only
        let json = r#"{
            "type": "thread_created",
            "thread": {
                "id": "thread-123",
                "updated_at": "2026-01-25T10:30:00Z",
                "created_at": "2026-01-25T10:30:00Z"
            },
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadCreated(created) => {
                assert_eq!(created.thread.id, "thread-123");
                assert_eq!(created.timestamp, 1705315800000);
                // Check defaults for optional fields
                assert_eq!(created.thread.title, "");
                assert_eq!(created.thread.preview, "");
                assert_eq!(created.thread.message_count, 0);
                assert!(created.thread.description.is_none());
                assert!(created.thread.model.is_none());
                assert!(created.thread.permission_mode.is_none());
                assert!(created.thread.working_directory.is_none());
                assert!(created.thread.status.is_none());
                assert!(created.thread.verified.is_none());
                assert!(created.thread.verified_at.is_none());
            }
            _ => panic!("Expected ThreadCreated"),
        }
    }

    #[test]
    fn test_deserialize_thread_created_full() {
        // Test full ThreadCreated message with all optional fields
        let json = r#"{
            "type": "thread_created",
            "thread": {
                "id": "thread-456",
                "name": "My New Thread",
                "description": "Thread description",
                "preview": "Last message preview",
                "last_activity": "2026-01-25T10:30:00Z",
                "type": "programming",
                "mode": "normal",
                "model": "claude-opus-4-5",
                "permission_mode": "ask",
                "message_count": 5,
                "created_at": "2026-01-25T10:00:00Z",
                "working_directory": "/home/user/project",
                "status": "running",
                "verified": false,
                "verified_at": null
            },
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadCreated(created) => {
                assert_eq!(created.thread.id, "thread-456");
                assert_eq!(created.thread.title, "My New Thread");
                assert_eq!(created.thread.description, Some("Thread description".to_string()));
                assert_eq!(created.thread.preview, "Last message preview");
                assert_eq!(created.thread.thread_type, crate::models::ThreadType::Programming);
                assert_eq!(created.thread.mode, crate::models::ThreadMode::Normal);
                assert_eq!(created.thread.model, Some("claude-opus-4-5".to_string()));
                assert_eq!(created.thread.permission_mode, Some("ask".to_string()));
                assert_eq!(created.thread.message_count, 5);
                assert_eq!(created.thread.working_directory, Some("/home/user/project".to_string()));
                assert_eq!(created.thread.status, Some(crate::models::ThreadStatus::Running));
                assert_eq!(created.thread.verified, Some(false));
                assert!(created.thread.verified_at.is_none());
                assert_eq!(created.timestamp, 1705315800000);
            }
            _ => panic!("Expected ThreadCreated"),
        }
    }

    #[test]
    fn test_deserialize_thread_created_with_integer_id() {
        // Backend may send thread ID as integer
        let json = r#"{
            "type": "thread_created",
            "thread": {
                "id": 789,
                "updated_at": "2026-01-25T10:30:00Z",
                "created_at": "2026-01-25T10:30:00Z"
            },
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadCreated(created) => {
                assert_eq!(created.thread.id, "789");
                assert_eq!(created.timestamp, 1705315800000);
            }
            _ => panic!("Expected ThreadCreated"),
        }
    }

    #[test]
    fn test_deserialize_thread_created_with_null_title() {
        // Backend may send null for title/name
        let json = r#"{
            "type": "thread_created",
            "thread": {
                "id": "thread-null-title",
                "name": null,
                "updated_at": "2026-01-25T10:30:00Z",
                "created_at": "2026-01-25T10:30:00Z"
            },
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadCreated(created) => {
                assert_eq!(created.thread.id, "thread-null-title");
                assert_eq!(created.thread.title, ""); // null should deserialize to empty string
                assert_eq!(created.timestamp, 1705315800000);
            }
            _ => panic!("Expected ThreadCreated"),
        }
    }

    #[test]
    fn test_deserialize_thread_created_backend_format() {
        // Test exact backend JSON format as specified in the phase description
        // This matches what the Python backend sends in ThreadCreatedEvent
        let json = r#"{
            "type": "thread_created",
            "thread": {
                "id": "cm5xyzabc123",
                "name": "New thread",
                "description": null,
                "preview": "",
                "last_activity": "2026-01-25T14:45:00.123456Z",
                "type": "programming",
                "mode": "normal",
                "model": "claude-sonnet-4-5",
                "permission_mode": "ask",
                "message_count": 1,
                "created_at": "2026-01-25T14:45:00.123456Z",
                "working_directory": "/Users/sam/project",
                "status": "done",
                "verified": null,
                "verified_at": null
            },
            "timestamp": 1737817500123
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadCreated(created) => {
                // Verify all fields parse correctly
                assert_eq!(created.thread.id, "cm5xyzabc123");
                assert_eq!(created.thread.title, "New thread");
                assert!(created.thread.description.is_none());
                assert_eq!(created.thread.preview, "");
                assert_eq!(created.thread.thread_type, crate::models::ThreadType::Programming);
                assert_eq!(created.thread.mode, crate::models::ThreadMode::Normal);
                assert_eq!(created.thread.model, Some("claude-sonnet-4-5".to_string()));
                assert_eq!(created.thread.permission_mode, Some("ask".to_string()));
                assert_eq!(created.thread.message_count, 1);
                assert_eq!(created.thread.working_directory, Some("/Users/sam/project".to_string()));
                assert_eq!(created.thread.status, Some(crate::models::ThreadStatus::Done));
                assert!(created.thread.verified.is_none());
                assert!(created.thread.verified_at.is_none());
                assert_eq!(created.timestamp, 1737817500123);
            }
            _ => panic!("Expected ThreadCreated"),
        }
    }

    #[test]
    fn test_serialize_thread_created() {
        use crate::models::Thread;
        use chrono::Utc;

        let thread = Thread {
            id: "thread-serialize".to_string(),
            title: "Test Thread".to_string(),
            description: Some("Description".to_string()),
            preview: "Preview text".to_string(),
            updated_at: Utc::now(),
            thread_type: crate::models::ThreadType::Programming,
            mode: crate::models::ThreadMode::Normal,
            model: Some("claude-opus-4-5".to_string()),
            permission_mode: Some("ask".to_string()),
            message_count: 3,
            created_at: Utc::now(),
            working_directory: Some("/path/to/dir".to_string()),
            status: Some(crate::models::ThreadStatus::Running),
            verified: Some(false),
            verified_at: None,
        };

        let created = WsThreadCreated {
            thread,
            timestamp: 1705315800000,
        };

        let json = serde_json::to_string(&created).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread"]["id"], "thread-serialize");
        // Thread struct serializes title as "title", not "name"
        // The "name" is only an alias for deserialization
        assert_eq!(parsed["thread"]["title"], "Test Thread");
        assert_eq!(parsed["timestamp"], 1705315800000_i64);
    }

    #[test]
    fn test_roundtrip_thread_created() {
        use crate::models::Thread;
        use chrono::Utc;

        let original_thread = Thread {
            id: "thread-roundtrip".to_string(),
            title: "Roundtrip Test".to_string(),
            description: None,
            preview: "".to_string(),
            updated_at: Utc::now(),
            thread_type: crate::models::ThreadType::Conversation,
            mode: crate::models::ThreadMode::Plan,
            model: None,
            permission_mode: None,
            message_count: 0,
            created_at: Utc::now(),
            working_directory: None,
            status: None,
            verified: None,
            verified_at: None,
        };

        let original = WsIncomingMessage::ThreadCreated(WsThreadCreated {
            thread: original_thread,
            timestamp: 1705315800000,
        });

        let json = serde_json::to_string(&original).unwrap();
        let parsed: WsIncomingMessage = serde_json::from_str(&json).unwrap();

        match parsed {
            WsIncomingMessage::ThreadCreated(created) => {
                assert_eq!(created.thread.id, "thread-roundtrip");
                assert_eq!(created.thread.title, "Roundtrip Test");
                assert_eq!(created.timestamp, 1705315800000);
            }
            _ => panic!("Expected ThreadCreated"),
        }
    }

    // -------------------- Thread Verified Tests --------------------

    #[test]
    fn test_deserialize_thread_verified() {
        // Backend sends ISO8601 timestamp strings
        let json = r#"{
            "type": "thread_verified",
            "thread_id": "thread-verified-123",
            "verified_at": "2026-01-15T10:30:00Z",
            "timestamp": "2026-01-15T10:30:01Z"
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadVerified(verified) => {
                assert_eq!(verified.thread_id, "thread-verified-123");
                assert_eq!(verified.verified_at, "2026-01-15T10:30:00Z");
                assert_eq!(verified.timestamp, "2026-01-15T10:30:01Z");
            }
            _ => panic!("Expected ThreadVerified"),
        }
    }

    #[test]
    fn test_serialize_thread_verified() {
        let verified = WsThreadVerified {
            thread_id: "thread-serialize".to_string(),
            verified_at: "2026-01-15T10:30:00Z".to_string(),
            timestamp: "2026-01-15T10:30:01Z".to_string(),
        };

        let json = serde_json::to_string(&verified).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-serialize");
        assert_eq!(parsed["verified_at"], "2026-01-15T10:30:00Z");
        assert_eq!(parsed["timestamp"], "2026-01-15T10:30:01Z");
    }

    #[test]
    fn test_deserialize_thread_verified_with_timezone_offset() {
        // Backend might send timestamps with timezone offsets
        let json = r#"{
            "type": "thread_verified",
            "thread_id": "thread-tz-123",
            "verified_at": "2026-01-25T14:45:00+00:00",
            "timestamp": "2026-01-25T14:45:01+00:00"
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadVerified(verified) => {
                assert_eq!(verified.thread_id, "thread-tz-123");
                assert_eq!(verified.verified_at, "2026-01-25T14:45:00+00:00");
                assert_eq!(verified.timestamp, "2026-01-25T14:45:01+00:00");
            }
            _ => panic!("Expected ThreadVerified"),
        }
    }

    #[test]
    fn test_deserialize_thread_mode_update_with_milliseconds() {
        // Backend might include milliseconds in the timestamp
        let json = r#"{
            "type": "thread_mode_update",
            "thread_id": "thread-ms-123",
            "mode": "plan",
            "timestamp": "2026-01-25T14:45:00.123Z"
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadModeUpdate(update) => {
                assert_eq!(update.thread_id, "thread-ms-123");
                assert_eq!(update.mode, crate::models::ThreadMode::Plan);
                assert_eq!(update.timestamp, "2026-01-25T14:45:00.123Z");
            }
            _ => panic!("Expected ThreadModeUpdate"),
        }
    }

    // -------------------- Thread Updated Tests --------------------

    #[test]
    fn test_deserialize_thread_updated() {
        let json = r#"{
            "type": "thread_updated",
            "thread_id": "thread-123",
            "title": "Updated Thread Title",
            "description": "This is the updated description",
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadUpdated(update) => {
                assert_eq!(update.thread_id, "thread-123");
                assert_eq!(update.title, "Updated Thread Title");
                assert_eq!(update.description, "This is the updated description");
                assert_eq!(update.timestamp, 1705315800000);
            }
            _ => panic!("Expected ThreadUpdated"),
        }
    }

    #[test]
    fn test_serialize_thread_updated() {
        let update = WsThreadUpdated {
            thread_id: "thread-serialize".to_string(),
            title: "New Title".to_string(),
            description: "New Description".to_string(),
            timestamp: 1705315800000,
        };

        let json = serde_json::to_string(&update).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-serialize");
        assert_eq!(parsed["title"], "New Title");
        assert_eq!(parsed["description"], "New Description");
        assert_eq!(parsed["timestamp"], 1705315800000_i64);
    }

    #[test]
    fn test_roundtrip_thread_updated() {
        let original = WsIncomingMessage::ThreadUpdated(WsThreadUpdated {
            thread_id: "thread-roundtrip".to_string(),
            title: "Test Title".to_string(),
            description: "Test Description".to_string(),
            timestamp: 1705315800000,
        });

        let json = serde_json::to_string(&original).unwrap();
        let parsed: WsIncomingMessage = serde_json::from_str(&json).unwrap();

        match parsed {
            WsIncomingMessage::ThreadUpdated(update) => {
                assert_eq!(update.thread_id, "thread-roundtrip");
                assert_eq!(update.title, "Test Title");
                assert_eq!(update.description, "Test Description");
                assert_eq!(update.timestamp, 1705315800000);
            }
            _ => panic!("Expected ThreadUpdated"),
        }
    }

    // -------------------- Agent Status Tests --------------------

    #[test]
    fn test_deserialize_agent_status_basic() {
        let json = r#"{
            "type": "agent_status",
            "thread_id": "thread-123",
            "state": "thinking",
            "model": "claude-opus-4-5",
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::AgentStatus(status) => {
                assert_eq!(status.thread_id, "thread-123");
                assert_eq!(status.state, "thinking");
                assert_eq!(status.model, "claude-opus-4-5");
                assert!(status.tool.is_none());
                assert!(status.current_operation.is_none());
                assert_eq!(status.timestamp, 1705315800000);
            }
            _ => panic!("Expected AgentStatus"),
        }
    }

    #[test]
    fn test_deserialize_agent_status_with_tool() {
        let json = r#"{
            "type": "agent_status",
            "thread_id": "thread-456",
            "state": "tool_use",
            "model": "claude-sonnet-4-5",
            "tool": "Edit",
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::AgentStatus(status) => {
                assert_eq!(status.thread_id, "thread-456");
                assert_eq!(status.state, "tool_use");
                assert_eq!(status.model, "claude-sonnet-4-5");
                assert_eq!(status.tool, Some("Edit".to_string()));
                assert!(status.current_operation.is_none());
                assert_eq!(status.timestamp, 1705315800000);
            }
            _ => panic!("Expected AgentStatus"),
        }
    }

    #[test]
    fn test_deserialize_agent_status_with_current_operation() {
        let json = r#"{
            "type": "agent_status",
            "thread_id": "thread-789",
            "state": "streaming",
            "model": "claude-haiku-4",
            "tool": "Bash",
            "current_operation": "Running tests",
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::AgentStatus(status) => {
                assert_eq!(status.thread_id, "thread-789");
                assert_eq!(status.state, "streaming");
                assert_eq!(status.model, "claude-haiku-4");
                assert_eq!(status.tool, Some("Bash".to_string()));
                assert_eq!(status.current_operation, Some("Running tests".to_string()));
                assert_eq!(status.timestamp, 1705315800000);
            }
            _ => panic!("Expected AgentStatus"),
        }
    }

    #[test]
    fn test_deserialize_agent_status_current_operation_only() {
        let json = r#"{
            "type": "agent_status",
            "thread_id": "thread-abc",
            "state": "idle",
            "model": "claude-opus-4-5",
            "current_operation": "Analyzing codebase",
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::AgentStatus(status) => {
                assert_eq!(status.thread_id, "thread-abc");
                assert_eq!(status.state, "idle");
                assert_eq!(status.model, "claude-opus-4-5");
                assert!(status.tool.is_none());
                assert_eq!(
                    status.current_operation,
                    Some("Analyzing codebase".to_string())
                );
                assert_eq!(status.timestamp, 1705315800000);
            }
            _ => panic!("Expected AgentStatus"),
        }
    }

    #[test]
    fn test_serialize_agent_status_with_current_operation() {
        let status = WsAgentStatus {
            thread_id: "thread-serialize".to_string(),
            state: "streaming".to_string(),
            model: "claude-opus-4-5".to_string(),
            tool: Some("Read".to_string()),
            current_operation: Some("Reading configuration".to_string()),
            timestamp: 1705315800000,
        };

        let json = serde_json::to_string(&status).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-serialize");
        assert_eq!(parsed["state"], "streaming");
        assert_eq!(parsed["model"], "claude-opus-4-5");
        assert_eq!(parsed["tool"], "Read");
        assert_eq!(parsed["current_operation"], "Reading configuration");
        assert_eq!(parsed["timestamp"], 1705315800000_i64);
    }

    #[test]
    fn test_serialize_agent_status_skip_none_fields() {
        let status = WsAgentStatus {
            thread_id: "thread-minimal".to_string(),
            state: "thinking".to_string(),
            model: "claude-sonnet-4-5".to_string(),
            tool: None,
            current_operation: None,
            timestamp: 1705315800000,
        };

        let json = serde_json::to_string(&status).unwrap();

        // Verify that None fields are not present in serialized JSON
        assert!(!json.contains("\"tool\""));
        assert!(!json.contains("\"current_operation\""));

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["thread_id"], "thread-minimal");
        assert_eq!(parsed["state"], "thinking");
        assert!(parsed.get("tool").is_none());
        assert!(parsed.get("current_operation").is_none());
    }

    // -------------------- System Metrics Update Tests --------------------

    #[test]
    fn test_deserialize_system_metrics_update() {
        let json = r#"{
            "type": "system_metrics_update",
            "cpu_percent": 45.5,
            "memory_used_mb": 8192,
            "memory_total_mb": 16384,
            "memory_percent": 50.0,
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::SystemMetricsUpdate(metrics) => {
                assert_eq!(metrics.cpu_percent, 45.5);
                assert_eq!(metrics.memory_used_mb, 8192);
                assert_eq!(metrics.memory_total_mb, 16384);
                assert_eq!(metrics.memory_percent, 50.0);
                assert_eq!(metrics.timestamp, 1705315800000);
            }
            _ => panic!("Expected SystemMetricsUpdate"),
        }
    }

    #[test]
    fn test_serialize_system_metrics_update() {
        let metrics = WsSystemMetricsUpdate {
            cpu_percent: 75.0,
            memory_used_mb: 12288,
            memory_total_mb: 16384,
            memory_percent: 75.0,
            timestamp: 1705315800000,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["cpu_percent"], 75.0);
        assert_eq!(parsed["memory_used_mb"], 12288);
        assert_eq!(parsed["memory_total_mb"], 16384);
        assert_eq!(parsed["memory_percent"], 75.0);
        assert_eq!(parsed["timestamp"], 1705315800000_i64);
    }

    #[test]
    fn test_roundtrip_system_metrics_update() {
        let original = WsIncomingMessage::SystemMetricsUpdate(WsSystemMetricsUpdate {
            cpu_percent: 60.0,
            memory_used_mb: 4096,
            memory_total_mb: 8192,
            memory_percent: 50.0,
            timestamp: 1705315800000,
        });

        let json = serde_json::to_string(&original).unwrap();
        let parsed: WsIncomingMessage = serde_json::from_str(&json).unwrap();

        match parsed {
            WsIncomingMessage::SystemMetricsUpdate(metrics) => {
                assert_eq!(metrics.cpu_percent, 60.0);
                assert_eq!(metrics.memory_used_mb, 4096);
                assert_eq!(metrics.memory_total_mb, 8192);
                assert_eq!(metrics.memory_percent, 50.0);
                assert_eq!(metrics.timestamp, 1705315800000);
            }
            _ => panic!("Expected SystemMetricsUpdate"),
        }
    }

    // -------------------- Question Response Payload Tests --------------------
    // These tests verify the WebSocket response format matches what spoq-conductor expects

    #[test]
    fn test_question_response_payload_format_matches_conductor() {
        // Build the response as the TUI does in send_question_response()
        let mut answers = std::collections::HashMap::new();
        answers.insert("Which auth method?".to_string(), "JWT".to_string());
        answers.insert("Which database?".to_string(), "PostgreSQL".to_string());

        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "perm_test-123".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Verify structure matches conductor expectations:
        // - type: "command_response"
        // - request_id: string
        // - result.status: "success"
        // - result.data.allowed: boolean
        // - result.data.message: string (JSON-encoded answers)
        assert_eq!(parsed["type"], "command_response");
        assert_eq!(parsed["request_id"], "perm_test-123");
        assert_eq!(parsed["result"]["status"], "success");
        assert_eq!(parsed["result"]["data"]["allowed"], true);

        // Verify message is a valid JSON string that can be parsed
        let message_str = parsed["result"]["data"]["message"].as_str().unwrap();
        let parsed_answers: std::collections::HashMap<String, String> =
            serde_json::from_str(message_str).unwrap();
        assert_eq!(parsed_answers.get("Which auth method?"), Some(&"JWT".to_string()));
        assert_eq!(parsed_answers.get("Which database?"), Some(&"PostgreSQL".to_string()));
    }

    #[test]
    fn test_question_response_single_answer() {
        let mut answers = std::collections::HashMap::new();
        answers.insert("Which framework?".to_string(), "React".to_string());

        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "perm_single".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Verify the message can be parsed back to get the answer
        let message_str = parsed["result"]["data"]["message"].as_str().unwrap();
        let parsed_answers: std::collections::HashMap<String, String> =
            serde_json::from_str(message_str).unwrap();
        assert_eq!(parsed_answers.len(), 1);
        assert_eq!(parsed_answers.get("Which framework?"), Some(&"React".to_string()));
    }

    #[test]
    fn test_question_response_multi_select_comma_separated() {
        // Multi-select answers are joined with ", " in build_question_answers()
        let mut answers = std::collections::HashMap::new();
        answers.insert("Select features".to_string(), "Linting, Unit tests, E2E tests".to_string());

        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "perm_multi".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let message_str = parsed["result"]["data"]["message"].as_str().unwrap();
        let parsed_answers: std::collections::HashMap<String, String> =
            serde_json::from_str(message_str).unwrap();
        assert_eq!(
            parsed_answers.get("Select features"),
            Some(&"Linting, Unit tests, E2E tests".to_string())
        );
    }

    #[test]
    fn test_question_response_with_other_text() {
        // "Other" option uses custom text
        let mut answers = std::collections::HashMap::new();
        answers.insert("Which framework?".to_string(), "Custom framework XYZ".to_string());

        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "perm_other".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let message_str = parsed["result"]["data"]["message"].as_str().unwrap();
        let parsed_answers: std::collections::HashMap<String, String> =
            serde_json::from_str(message_str).unwrap();
        assert_eq!(
            parsed_answers.get("Which framework?"),
            Some(&"Custom framework XYZ".to_string())
        );
    }

    #[test]
    fn test_question_response_multi_select_with_other() {
        // Multi-select can include "Other" text appended
        let mut answers = std::collections::HashMap::new();
        answers.insert("Select features".to_string(), "Linting, Unit tests, Custom feature ABC".to_string());

        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "perm_multi_other".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let message_str = parsed["result"]["data"]["message"].as_str().unwrap();
        let parsed_answers: std::collections::HashMap<String, String> =
            serde_json::from_str(message_str).unwrap();
        assert_eq!(
            parsed_answers.get("Select features"),
            Some(&"Linting, Unit tests, Custom feature ABC".to_string())
        );
    }

    #[test]
    fn test_question_response_empty_answers_still_valid() {
        // Edge case: no answers (shouldn't happen in practice but should be valid JSON)
        let answers: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "perm_empty".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let message_str = parsed["result"]["data"]["message"].as_str().unwrap();
        let parsed_answers: std::collections::HashMap<String, String> =
            serde_json::from_str(message_str).unwrap();
        assert!(parsed_answers.is_empty());
    }

    #[test]
    fn test_question_response_special_characters_in_answers() {
        // Answers may contain special characters that need proper JSON escaping
        let mut answers = std::collections::HashMap::new();
        answers.insert(
            "Describe your approach".to_string(),
            "Use \"quotes\" and 'apostrophes'\nWith newlines\tand tabs".to_string(),
        );

        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "perm_special".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // The message should be valid JSON that can be parsed
        let message_str = parsed["result"]["data"]["message"].as_str().unwrap();
        let parsed_answers: std::collections::HashMap<String, String> =
            serde_json::from_str(message_str).unwrap();

        let answer = parsed_answers.get("Describe your approach").unwrap();
        assert!(answer.contains("\"quotes\""));
        assert!(answer.contains('\n'));
        assert!(answer.contains('\t'));
    }

    #[test]
    fn test_question_response_unicode_in_answers() {
        let mut answers = std::collections::HashMap::new();
        answers.insert(
            "Preferred language".to_string(),
            "日本語 (Japanese) 🇯🇵".to_string(),
        );

        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "perm_unicode".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let message_str = parsed["result"]["data"]["message"].as_str().unwrap();
        let parsed_answers: std::collections::HashMap<String, String> =
            serde_json::from_str(message_str).unwrap();

        let answer = parsed_answers.get("Preferred language").unwrap();
        assert!(answer.contains("日本語"));
        assert!(answer.contains("🇯🇵"));
    }

    #[test]
    fn test_question_response_conductor_compatible_deserialization() {
        // Simulate what conductor does: deserialize as CommandResult
        // This test validates the TUI's output can be parsed by conductor

        let mut answers = std::collections::HashMap::new();
        answers.insert("Question 1".to_string(), "Answer 1".to_string());
        answers.insert("Question 2".to_string(), "Answer 2".to_string());

        let answers_value = serde_json::to_value(&answers).unwrap_or_default();

        let response = WsCommandResponse {
            type_: "command_response".to_string(),
            request_id: "perm_conductor".to_string(),
            result: WsCommandResult {
                status: "success".to_string(),
                data: WsPermissionData {
                    allowed: true,
                    message: Some(answers_value.to_string()),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Conductor extracts: data.allowed as bool, data.message as str
        let result = &parsed["result"];
        let data = &result["data"];

        let allowed = data.get("allowed").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let message = data
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();

        assert!(allowed);
        assert!(!message.is_empty());

        // Conductor passes this message string back to the MCP tool
        // The MCP tool can then parse it as JSON to get the answers
        let parsed_answers: std::collections::HashMap<String, String> =
            serde_json::from_str(&message).unwrap();
        assert_eq!(parsed_answers.len(), 2);
        assert_eq!(parsed_answers.get("Question 1"), Some(&"Answer 1".to_string()));
        assert_eq!(parsed_answers.get("Question 2"), Some(&"Answer 2".to_string()));
    }

    // -------------------- Stream Started Tests --------------------

    #[test]
    fn test_deserialize_stream_started() {
        let json = r#"{
            "type": "stream_started",
            "thread_id": "thread-123",
            "session_id": "session-456",
            "timestamp": 1705315800000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::StreamStarted(started) => {
                assert_eq!(started.thread_id, "thread-123");
                assert_eq!(started.session_id, "session-456");
                assert_eq!(started.timestamp, 1705315800000);
            }
            _ => panic!("Expected StreamStarted"),
        }
    }

    #[test]
    fn test_serialize_stream_started() {
        let started = WsStreamStarted {
            thread_id: "thread-serialize".to_string(),
            session_id: "session-serialize".to_string(),
            timestamp: 1705315800000,
        };

        let json = serde_json::to_string(&started).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-serialize");
        assert_eq!(parsed["session_id"], "session-serialize");
        assert_eq!(parsed["timestamp"], 1705315800000_i64);
    }

    #[test]
    fn test_phase_status_pending() {
        // Test that pending status deserializes correctly
        let json = r#"{
            "type": "phase_progress_update",
            "plan_id": "plan-pending",
            "phase_index": 0,
            "total_phases": 3,
            "phase_name": "Waiting",
            "status": "pending",
            "tool_count": 0,
            "started_at": 1705315700000,
            "updated_at": 1705315700000,
            "timestamp": 1705315700000
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PhaseProgressUpdate(progress) => {
                assert_eq!(progress.plan_id, "plan-pending");
                assert_eq!(progress.status, PhaseStatus::Pending);
            }
            _ => panic!("Expected PhaseProgressUpdate"),
        }
    }
}
