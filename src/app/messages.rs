//! AppMessage enum for async communication within the application.

use crate::auth::central_api::{VpsPlan, VpsStatusResponse};
use crate::models::Folder;
use crate::state::Todo;

/// Messages received from async operations (streaming, connection status)
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// A token received during streaming
    StreamToken { thread_id: String, token: String },
    /// A reasoning/thinking token received during streaming
    ReasoningToken { thread_id: String, token: String },
    /// Streaming completed successfully
    StreamComplete { thread_id: String, message_id: i64 },
    /// An error occurred during streaming
    StreamError { thread_id: String, error: String },
    /// Connection status changed
    ConnectionStatus(bool),
    /// Thread created on backend - reconcile pending ID with real ID
    ThreadCreated {
        pending_id: String,
        real_id: String,
        title: Option<String>,
    },
    /// Messages loaded for a thread
    MessagesLoaded {
        thread_id: String,
        messages: Vec<crate::models::Message>,
    },
    /// Error loading messages for a thread
    MessagesLoadError {
        thread_id: String,
        error: String,
    },
    /// Todos updated from the assistant
    TodosUpdated {
        todos: Vec<Todo>,
    },
    /// Permission request from the assistant - needs user approval
    PermissionRequested {
        permission_id: String,
        tool_name: String,
        description: String,
        tool_input: Option<serde_json::Value>,
    },
    /// Tool call started
    ToolStarted {
        thread_id: String,
        tool_call_id: String,
        tool_name: String,
    },
    /// Tool is executing with display info
    ToolExecuting {
        thread_id: String,
        tool_call_id: String,
        display_name: String,
    },
    /// Tool completed with result
    ToolCompleted {
        thread_id: String,
        tool_call_id: String,
        success: bool,
        summary: String,
        /// Full result content for storage in ToolEvent
        result: String,
    },
    /// Tool argument chunk received
    ToolArgumentChunk {
        thread_id: String,
        tool_call_id: String,
        chunk: String,
    },
    /// Skills injected into the session
    SkillsInjected {
        skills: Vec<String>,
    },
    /// OAuth consent required
    OAuthConsentRequired {
        provider: String,
        url: Option<String>,
        skill_name: Option<String>,
    },
    /// Context compacted
    ContextCompacted {
        tokens_used: Option<u32>,
        token_limit: Option<u32>,
    },
    /// Thread metadata updated
    ThreadMetadataUpdated {
        thread_id: String,
        title: Option<String>,
        description: Option<String>,
    },
    /// Subagent task started
    SubagentStarted {
        task_id: String,
        description: String,
        subagent_type: String,
    },
    /// Subagent progress update
    SubagentProgress {
        task_id: String,
        message: String,
    },
    /// Subagent task completed
    SubagentCompleted {
        task_id: String,
        summary: String,
        tool_call_count: Option<u32>,
    },
    /// Usage information received (context window usage)
    UsageReceived {
        context_used: u32,
        context_limit: u32,
    },
    /// WebSocket connected successfully
    WsConnected,
    /// WebSocket disconnected
    WsDisconnected,
    /// WebSocket reconnecting
    WsReconnecting { attempt: u8 },
    /// WebSocket raw message received (for debugging)
    WsRawMessage { message: String },
    /// WebSocket message parse error (for debugging)
    WsParseError { error: String, raw: String },
    /// Folders loaded from API
    FoldersLoaded(Vec<Folder>),
    /// Failed to load folders from API
    FoldersLoadFailed(String),
    /// Open the folder picker overlay
    FolderPickerOpen,
    /// Close the folder picker overlay
    FolderPickerClose,
    /// Filter text changed in folder picker
    FolderPickerFilterChanged(String),
    /// Move cursor up in folder picker
    FolderPickerCursorUp,
    /// Move cursor down in folder picker
    FolderPickerCursorDown,
    /// A folder was selected from the picker
    FolderSelected(Folder),
    /// Clear the currently selected folder
    FolderCleared,
    /// Device flow state updated
    DeviceFlowUpdated,
    /// VPS plans loaded successfully
    VpsPlansLoaded(Vec<VpsPlan>),
    /// Error loading VPS plans
    VpsPlansLoadError(String),
    /// Provisioning status update received
    ProvisioningStatusUpdate(String),
    /// Provisioning completed successfully
    ProvisioningComplete(VpsStatusResponse),
    /// Error during provisioning
    ProvisioningError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_started_construction() {
        let msg = AppMessage::SubagentStarted {
            task_id: "task-123".to_string(),
            description: "Test task description".to_string(),
            subagent_type: "general-purpose".to_string(),
        };

        // Verify it can be constructed and cloned
        let cloned = msg.clone();
        match cloned {
            AppMessage::SubagentStarted {
                task_id,
                description,
                subagent_type,
            } => {
                assert_eq!(task_id, "task-123");
                assert_eq!(description, "Test task description");
                assert_eq!(subagent_type, "general-purpose");
            }
            _ => panic!("Expected SubagentStarted variant"),
        }
    }

    #[test]
    fn test_subagent_progress_construction() {
        let msg = AppMessage::SubagentProgress {
            task_id: "task-456".to_string(),
            message: "Progress update message".to_string(),
        };

        // Verify it can be constructed and cloned
        let cloned = msg.clone();
        match cloned {
            AppMessage::SubagentProgress { task_id, message } => {
                assert_eq!(task_id, "task-456");
                assert_eq!(message, "Progress update message");
            }
            _ => panic!("Expected SubagentProgress variant"),
        }
    }

    #[test]
    fn test_subagent_completed_construction() {
        let msg = AppMessage::SubagentCompleted {
            task_id: "task-789".to_string(),
            summary: "Task completed successfully".to_string(),
            tool_call_count: Some(42),
        };

        // Verify it can be constructed and cloned
        let cloned = msg.clone();
        match cloned {
            AppMessage::SubagentCompleted {
                task_id,
                summary,
                tool_call_count,
            } => {
                assert_eq!(task_id, "task-789");
                assert_eq!(summary, "Task completed successfully");
                assert_eq!(tool_call_count, Some(42));
            }
            _ => panic!("Expected SubagentCompleted variant"),
        }
    }

    #[test]
    fn test_subagent_completed_without_tool_count() {
        let msg = AppMessage::SubagentCompleted {
            task_id: "task-999".to_string(),
            summary: "Task completed".to_string(),
            tool_call_count: None,
        };

        match msg {
            AppMessage::SubagentCompleted {
                task_id,
                summary,
                tool_call_count,
            } => {
                assert_eq!(task_id, "task-999");
                assert_eq!(summary, "Task completed");
                assert_eq!(tool_call_count, None);
            }
            _ => panic!("Expected SubagentCompleted variant"),
        }
    }

    #[test]
    fn test_all_subagent_variants_debug() {
        // Verify Debug trait works for all new variants
        let started = AppMessage::SubagentStarted {
            task_id: "t1".to_string(),
            description: "desc".to_string(),
            subagent_type: "type".to_string(),
        };
        let progress = AppMessage::SubagentProgress {
            task_id: "t2".to_string(),
            message: "msg".to_string(),
        };
        let completed = AppMessage::SubagentCompleted {
            task_id: "t3".to_string(),
            summary: "sum".to_string(),
            tool_call_count: Some(5),
        };

        // Should not panic
        let _ = format!("{:?}", started);
        let _ = format!("{:?}", progress);
        let _ = format!("{:?}", completed);
    }

    #[test]
    fn test_ws_connected_construction() {
        let msg = AppMessage::WsConnected;
        // Verify it can be cloned and debug printed
        let cloned = msg.clone();
        let _ = format!("{:?}", cloned);
    }

    #[test]
    fn test_ws_disconnected_construction() {
        let msg = AppMessage::WsDisconnected;
        // Verify it can be cloned and debug printed
        let cloned = msg.clone();
        let _ = format!("{:?}", cloned);
    }

    #[test]
    fn test_ws_reconnecting_construction() {
        let msg = AppMessage::WsReconnecting { attempt: 3 };
        // Verify it can be cloned and debug printed
        let cloned = msg.clone();
        match cloned {
            AppMessage::WsReconnecting { attempt } => {
                assert_eq!(attempt, 3);
            }
            _ => panic!("Expected WsReconnecting variant"),
        }
    }

    #[test]
    fn test_all_ws_variants_debug() {
        // Verify Debug trait works for all WebSocket variants
        let connected = AppMessage::WsConnected;
        let disconnected = AppMessage::WsDisconnected;
        let reconnecting = AppMessage::WsReconnecting { attempt: 1 };

        // Should not panic
        let _ = format!("{:?}", connected);
        let _ = format!("{:?}", disconnected);
        let _ = format!("{:?}", reconnecting);
    }
}
