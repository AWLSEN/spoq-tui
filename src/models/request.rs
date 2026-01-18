use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::thread::ThreadType;

/// Permission mode for Claude tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// User approval required for each tool call
    #[default]
    Default,
    /// Claude proposes changes but doesn't execute
    Plan,
    /// Auto-approve all tool calls
    BypassPermissions,
}

/// Request structure for streaming API calls
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamRequest {
    /// The prompt/message to send
    pub prompt: String,
    /// Session ID for authentication (required by backend)
    pub session_id: String,
    /// Thread ID - None means create a new thread
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Message ID to reply to - for future stitching support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<i64>,
    /// Type of thread to create (normal or programming)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_type: Option<ThreadType>,
    /// Permission mode for tool execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<PermissionMode>,
    /// Working directory for the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

impl StreamRequest {
    /// Create a new StreamRequest for a new thread
    pub fn new(prompt: String) -> Self {
        Self {
            prompt,
            session_id: Uuid::new_v4().to_string(),
            thread_id: None,
            reply_to: None,
            thread_type: None,
            permission_mode: None,
            working_directory: None,
        }
    }

    /// Create a StreamRequest for an existing thread
    pub fn with_thread(prompt: String, thread_id: String) -> Self {
        Self {
            prompt,
            session_id: Uuid::new_v4().to_string(),
            thread_id: Some(thread_id),
            reply_to: None,
            thread_type: None,
            permission_mode: None,
            working_directory: None,
        }
    }

    /// Create a StreamRequest as a reply to a specific message
    #[allow(dead_code)]
    pub fn with_reply(prompt: String, thread_id: String, reply_to: i64) -> Self {
        Self {
            prompt,
            session_id: Uuid::new_v4().to_string(),
            thread_id: Some(thread_id),
            reply_to: Some(reply_to),
            thread_type: None,
            permission_mode: None,
            working_directory: None,
        }
    }

    /// Set the thread type for this request (builder pattern)
    pub fn with_type(mut self, thread_type: ThreadType) -> Self {
        self.thread_type = Some(thread_type);
        self
    }

    /// Set permission mode for this request (builder pattern)
    pub fn with_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = Some(mode);
        self
    }

    /// Set working directory for this request (builder pattern)
    pub fn with_working_directory(mut self, path: Option<String>) -> Self {
        self.working_directory = path;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============= PermissionMode Tests =============

    #[test]
    fn test_permission_mode_default() {
        assert_eq!(PermissionMode::default(), PermissionMode::Default);
    }

    #[test]
    fn test_permission_mode_variants() {
        let default = PermissionMode::Default;
        let plan = PermissionMode::Plan;
        let bypass = PermissionMode::BypassPermissions;

        assert_eq!(default, PermissionMode::Default);
        assert_eq!(plan, PermissionMode::Plan);
        assert_eq!(bypass, PermissionMode::BypassPermissions);
        assert_ne!(default, plan);
        assert_ne!(plan, bypass);
        assert_ne!(bypass, default);
    }

    #[test]
    fn test_permission_mode_copy() {
        let original = PermissionMode::Plan;
        let copied = original;
        assert_eq!(original, copied);
    }

    #[test]
    fn test_permission_mode_clone() {
        let original = PermissionMode::BypassPermissions;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_permission_mode_debug() {
        assert_eq!(format!("{:?}", PermissionMode::Default), "Default");
        assert_eq!(format!("{:?}", PermissionMode::Plan), "Plan");
        assert_eq!(format!("{:?}", PermissionMode::BypassPermissions), "BypassPermissions");
    }

    #[test]
    fn test_permission_mode_serialization_default() {
        let mode = PermissionMode::Default;
        let json = serde_json::to_string(&mode).expect("Failed to serialize");
        assert_eq!(json, "\"default\"");

        let deserialized: PermissionMode = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn test_permission_mode_serialization_plan() {
        let mode = PermissionMode::Plan;
        let json = serde_json::to_string(&mode).expect("Failed to serialize");
        assert_eq!(json, "\"plan\"");

        let deserialized: PermissionMode = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn test_permission_mode_serialization_bypass() {
        let mode = PermissionMode::BypassPermissions;
        let json = serde_json::to_string(&mode).expect("Failed to serialize");
        assert_eq!(json, "\"bypassPermissions\"");

        let deserialized: PermissionMode = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn test_permission_mode_deserialization_camel_case() {
        let json = "\"bypassPermissions\"";
        let mode: PermissionMode = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(mode, PermissionMode::BypassPermissions);
    }

    // ============= StreamRequest Tests =============

    #[test]
    fn test_stream_request_with_permission_mode() {
        let request = StreamRequest::new("Test prompt".to_string())
            .with_permission_mode(PermissionMode::Plan);

        assert_eq!(request.prompt, "Test prompt");
        assert_eq!(request.permission_mode, Some(PermissionMode::Plan));
    }

    #[test]
    fn test_stream_request_with_permission_mode_serialization() {
        let request = StreamRequest::new("Test".to_string())
            .with_permission_mode(PermissionMode::BypassPermissions);

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let deserialized: StreamRequest = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(request.permission_mode, deserialized.permission_mode);
        assert_eq!(deserialized.permission_mode, Some(PermissionMode::BypassPermissions));
    }

    #[test]
    fn test_stream_request_without_permission_mode() {
        let request = StreamRequest::new("Test".to_string());
        assert_eq!(request.permission_mode, None);

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(!json.contains("permissionMode"));
    }

    #[test]
    fn test_stream_request_builder_pattern_chaining() {
        let request = StreamRequest::new("Test".to_string())
            .with_type(ThreadType::Programming)
            .with_permission_mode(PermissionMode::Plan);

        assert_eq!(request.thread_type, Some(ThreadType::Programming));
        assert_eq!(request.permission_mode, Some(PermissionMode::Plan));
    }

    // ============= StreamRequest Working Directory Tests =============

    #[test]
    fn test_stream_request_with_working_directory() {
        let request = StreamRequest::new("Test prompt".to_string())
            .with_working_directory(Some("/Users/test/my-project".to_string()));

        assert_eq!(request.working_directory, Some("/Users/test/my-project".to_string()));
    }

    #[test]
    fn test_stream_request_with_working_directory_none() {
        let request = StreamRequest::new("Test prompt".to_string())
            .with_working_directory(None);

        assert!(request.working_directory.is_none());
    }

    #[test]
    fn test_stream_request_with_working_directory_serialization() {
        let request = StreamRequest::new("Test".to_string())
            .with_working_directory(Some("/home/user/project".to_string()));

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("working_directory"));
        assert!(json.contains("/home/user/project"));

        let deserialized: StreamRequest = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(request.working_directory, deserialized.working_directory);
    }

    #[test]
    fn test_stream_request_without_working_directory_omits_field() {
        let request = StreamRequest::new("Test".to_string());
        assert!(request.working_directory.is_none());

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        // working_directory should be omitted entirely due to skip_serializing_if
        assert!(!json.contains("working_directory"));
    }

    #[test]
    fn test_stream_request_full_builder_chain() {
        let request = StreamRequest::new("Code task".to_string())
            .with_type(ThreadType::Programming)
            .with_permission_mode(PermissionMode::Default)
            .with_working_directory(Some("/Users/dev/project".to_string()));

        assert_eq!(request.prompt, "Code task");
        assert_eq!(request.thread_type, Some(ThreadType::Programming));
        assert_eq!(request.permission_mode, Some(PermissionMode::Default));
        assert_eq!(request.working_directory, Some("/Users/dev/project".to_string()));
    }
}
