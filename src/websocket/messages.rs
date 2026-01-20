use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::{PlanSummary, ThreadStatus, WaitingFor};

/// Incoming WebSocket messages from the client
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum WsIncomingMessage {
    #[serde(rename = "permission_request")]
    PermissionRequest(WsPermissionRequest),
    /// Agent status update (thinking, idle, tool_use, etc.)
    #[serde(rename = "agent_status")]
    AgentStatus(WsAgentStatus),
    /// Connection confirmation from server
    #[serde(rename = "connected")]
    Connected(WsConnected),
    /// Thread status update for dashboard view
    #[serde(rename = "thread_status_update")]
    ThreadStatusUpdate(WsThreadStatusUpdate),
    /// Plan approval request from agent
    #[serde(rename = "plan_approval_request")]
    PlanApprovalRequest(WsPlanApprovalRequest),
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

/// Agent status update
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsAgentStatus {
    pub thread_id: String,
    pub state: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub timestamp: u64,
}

/// Permission request from client
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsPermissionRequest {
    pub request_id: String,
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
    /// When this update occurred
    pub timestamp: DateTime<Utc>,
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
    /// When this request was created
    pub timestamp: DateTime<Utc>,
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
            tool_name: "Test".to_string(),
            tool_input: serde_json::json!({"key": "value"}),
            description: "Test description".to_string(),
            timestamp: 1234567890,
        };

        let cloned = req.clone();
        assert_eq!(req.request_id, cloned.request_id);
        assert_eq!(req.tool_name, cloned.tool_name);
        assert_eq!(req.description, cloned.description);
        assert_eq!(req.timestamp, cloned.timestamp);
    }

    #[test]
    fn test_ws_permission_request_debug() {
        let req = WsPermissionRequest {
            request_id: "req-debug".to_string(),
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
            "timestamp": "2024-01-15T10:30:00Z"
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadStatusUpdate(update) => {
                assert_eq!(update.thread_id, "thread-123");
                assert_eq!(update.status, ThreadStatus::Running);
                assert!(update.waiting_for.is_none());
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
            "timestamp": "2024-01-15T10:30:00Z"
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::ThreadStatusUpdate(update) => {
                assert_eq!(update.thread_id, "thread-456");
                assert_eq!(update.status, ThreadStatus::Waiting);
                assert!(update.waiting_for.is_some());
                match update.waiting_for.unwrap() {
                    WaitingFor::Permission { request_id, tool_name } => {
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
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&update).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-serialize");
        assert_eq!(parsed["status"], "done");
        assert!(parsed.get("waiting_for").is_none() || parsed["waiting_for"].is_null());
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
            "timestamp": "2024-01-15T10:30:00Z"
        }"#;

        let msg: WsIncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            WsIncomingMessage::PlanApprovalRequest(req) => {
                assert_eq!(req.thread_id, "thread-plan-1");
                assert_eq!(req.request_id, "plan-req-123");
                assert_eq!(req.plan_summary.title, "Add dark mode");
                assert_eq!(req.plan_summary.phases.len(), 3);
                assert_eq!(req.plan_summary.file_count, 15);
                assert_eq!(req.plan_summary.estimated_tokens, 50000);
            }
            _ => panic!("Expected PlanApprovalRequest"),
        }
    }

    #[test]
    fn test_serialize_plan_approval_request() {
        use crate::models::PlanSummary;

        let req = WsPlanApprovalRequest {
            thread_id: "thread-plan-serialize".to_string(),
            request_id: "plan-req-456".to_string(),
            plan_summary: PlanSummary::new(
                "Refactor module".to_string(),
                vec!["Phase 1".to_string(), "Phase 2".to_string()],
                5,
                10000,
            ),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&req).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["thread_id"], "thread-plan-serialize");
        assert_eq!(parsed["request_id"], "plan-req-456");
        assert_eq!(parsed["plan_summary"]["title"], "Refactor module");
        assert_eq!(parsed["plan_summary"]["phases"].as_array().unwrap().len(), 2);
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
}
