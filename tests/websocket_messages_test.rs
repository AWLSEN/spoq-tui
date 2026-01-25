use spoq::websocket::{
    WsCommandResponse, WsCommandResult, WsIncomingMessage, WsPermissionData, WsPermissionRequest,
};

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
        _ => panic!("Expected PermissionRequest variant"),
    }
}

#[test]
fn test_serialize_command_response_with_message() {
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
fn test_serialize_command_response_without_message() {
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
    // When None, the field should be omitted or null
    assert!(
        !parsed["result"]["data"]
            .as_object()
            .unwrap()
            .contains_key("message")
            || parsed["result"]["data"]["message"].is_null()
    );
}

#[test]
fn test_roundtrip_serialization() {
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
            assert_eq!(req.tool_input["file_path"], "/etc/hosts");
        }
        _ => panic!("Expected PermissionRequest variant"),
    }
}
