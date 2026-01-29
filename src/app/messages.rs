//! AppMessage enum for async communication within the application.

use crate::models::dashboard::{PlanSummary, ThreadStatus, WaitingFor};
use crate::models::picker::PickerItem;
use crate::models::{Folder, GitHubRepo, Thread, ThreadMode};
use crate::state::session::AskUserQuestionData;
use crate::state::Todo;
use crate::ui::dashboard::SystemStats;
use crate::websocket::messages::PhaseStatus;

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
    /// Stream was cancelled by user request (Ctrl+C)
    StreamCancelled { thread_id: String, reason: String },
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
    MessagesLoadError { thread_id: String, error: String },
    /// Todos updated from the assistant
    TodosUpdated { todos: Vec<Todo> },
    /// Permission request from the assistant - needs user approval
    PermissionRequested {
        permission_id: String,
        thread_id: Option<String>,
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
    SkillsInjected { skills: Vec<String> },
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
    SubagentProgress { task_id: String, message: String },
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
    /// GitHub repos loaded from API
    ReposLoaded(Vec<GitHubRepo>),
    /// Failed to load repos from API
    ReposLoadFailed(String),
    /// Files loaded from API for file picker
    FilesLoaded(Vec<crate::models::FileEntry>),
    /// Failed to load files from API
    FilesLoadFailed(String),
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
    /// System stats update (CPU, RAM)
    SystemStatsUpdate(SystemStats),
    /// Thread status update from WebSocket (for dashboard)
    ThreadStatusUpdate {
        thread_id: String,
        status: ThreadStatus,
        waiting_for: Option<WaitingFor>,
    },
    /// Agent status update from WebSocket (for dashboard)
    AgentStatusUpdate {
        thread_id: String,
        state: String,
        current_operation: Option<String>,
    },
    /// Plan approval request from WebSocket (for dashboard)
    PlanApprovalRequest {
        thread_id: String,
        request_id: String,
        plan_summary: PlanSummary,
    },
    /// New thread created notification from WebSocket (for dashboard)
    ///
    /// Allows dashboard to immediately add new threads without polling.
    WsThreadCreated { thread: Thread },
    /// Thread mode update from WebSocket (normal, plan, exec)
    ThreadModeUpdate { thread_id: String, mode: ThreadMode },
    /// Phase progress update during plan execution
    PhaseProgressUpdate {
        thread_id: Option<String>,
        plan_id: String,
        phase_index: u32,
        total_phases: u32,
        phase_name: String,
        status: PhaseStatus,
        tool_count: u32,
        last_tool: String,
        last_file: Option<String>,
    },
    /// Thread verification notification
    ThreadVerified {
        thread_id: String,
        verified_at: String,
    },
    /// Pending question from AskUserQuestion tool
    ///
    /// Sent when a permission request with tool_name="AskUserQuestion" is received.
    /// The question data is extracted from the tool_input and stored in dashboard state.
    PendingQuestion {
        thread_id: String,
        request_id: String,
        question_data: AskUserQuestionData,
    },
    // =========================================================================
    // Unified Picker Messages
    // =========================================================================
    /// Folders search results received for unified picker
    UnifiedPickerFoldersLoaded(Vec<PickerItem>),
    /// Folders search failed for unified picker
    UnifiedPickerFoldersFailed(String),
    /// Repos search results received for unified picker
    UnifiedPickerReposLoaded(Vec<PickerItem>),
    /// Repos search failed for unified picker
    UnifiedPickerReposFailed(String),
    /// Threads search results received for unified picker
    UnifiedPickerThreadsLoaded(Vec<PickerItem>),
    /// Threads search failed for unified picker
    UnifiedPickerThreadsFailed(String),
    /// Clone operation completed successfully
    UnifiedPickerCloneComplete {
        local_path: String,
        name: String,
        message: String,
    },
    /// Clone operation failed
    UnifiedPickerCloneFailed { error: String },
    // =========================================================================
    // Credential Auto-Sync Messages
    // =========================================================================
    /// File watcher detected a credential file change
    CredentialFileChanged {
        /// Path of the changed file
        path: String,
    },
    /// Debounce timer expired - time to sync
    CredentialDebounceExpired,
    // =========================================================================
    // Claude CLI Login Messages
    // =========================================================================
    /// Claude CLI login required - user needs to authenticate via browser
    ClaudeLoginRequired {
        request_id: String,
        auth_url: String,
        auto_open: bool,
    },
    /// Claude CLI login verification result from backend
    ClaudeLoginVerificationResult {
        request_id: String,
        success: bool,
        account_email: Option<String>,
        error: Option<String>,
    },
    // =========================================================================
    // Sync Messages
    // =========================================================================
    /// Trigger token sync operation (from /sync command)
    TriggerSync,
    /// Sync operation started
    SyncStarted,
    /// Sync progress update
    SyncProgress { message: String },
    /// Sync completed successfully
    SyncComplete {
        github_cli: bool,
    },
    /// Sync operation failed
    SyncFailed { error: String },
    // =========================================================================
    // Browse List Messages (for /threads and /repos full-screen views)
    // =========================================================================
    /// Threads loaded for browse list
    BrowseListThreadsLoaded {
        threads: Vec<crate::models::picker::ThreadEntry>,
        offset: usize,
        has_more: bool,
    },
    /// Repos loaded for browse list
    BrowseListReposLoaded {
        repos: Vec<crate::models::picker::RepoEntry>,
        offset: usize,
        has_more: bool,
    },
    /// Error loading browse list data
    BrowseListError(String),
    /// Trigger debounced search (fired after 300ms delay)
    BrowseListSearchDebounced {
        query: String,
    },
    /// Clone completed (for /repos remote repo selection)
    BrowseListCloneComplete {
        local_path: String,
        name: String,
    },
    /// Clone failed (for /repos remote repo selection)
    BrowseListCloneFailed {
        error: String,
    },
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
    fn test_stream_cancelled_construction() {
        let msg = AppMessage::StreamCancelled {
            thread_id: "thread-123".to_string(),
            reason: "user_requested".to_string(),
        };
        let cloned = msg.clone();
        match cloned {
            AppMessage::StreamCancelled { thread_id, reason } => {
                assert_eq!(thread_id, "thread-123");
                assert_eq!(reason, "user_requested");
            }
            _ => panic!("Expected StreamCancelled variant"),
        }
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
