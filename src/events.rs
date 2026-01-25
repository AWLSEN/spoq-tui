//! SSE Event types for Conductor API integration.
//!
//! This module defines all the event types that can be received from the Conductor
//! backend via Server-Sent Events (SSE) during streaming conversations.

use serde::Deserialize;

/// Metadata included with every SSE event.
///
/// Contains sequencing and identification information for event ordering
/// and session/thread association. Backend sends these fields flattened
/// at root level of each event JSON.
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct EventMeta {
    /// Sequence number for ordering events within a stream
    #[serde(default)]
    pub seq: Option<u64>,
    /// Session ID for the current streaming session
    #[serde(default)]
    pub session_id: Option<String>,
    /// Thread ID this event belongs to
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Unix timestamp in milliseconds of when the event was generated
    #[serde(default)]
    pub timestamp: Option<u64>,
}

/// Content streaming event containing assistant text output.
///
/// Received incrementally as the assistant generates response text.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ContentEvent {
    /// A chunk of text content from the assistant's response
    pub text: String,
    /// Event metadata (seq, timestamp, session_id, thread_id)
    #[serde(flatten, default)]
    pub meta: EventMeta,
}

/// Reasoning/thinking event containing assistant's internal reasoning.
///
/// Shows the assistant's chain-of-thought process when enabled.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ReasoningEvent {
    /// A chunk of reasoning text from the assistant
    pub text: String,
}

/// Event indicating a tool call has started.
///
/// Sent when the assistant begins invoking a tool.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ToolCallStartEvent {
    /// Name of the tool being called
    pub tool_name: String,
    /// Unique identifier for this tool call
    pub tool_call_id: String,
}

/// Event containing incremental tool call arguments.
///
/// Sent as the tool's input arguments are being streamed.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ToolCallArgumentEvent {
    /// The tool call this argument chunk belongs to
    pub tool_call_id: String,
    /// A chunk of the argument JSON being built
    #[serde(alias = "argument_chunk")]
    pub chunk: String,
}

/// Event indicating tool execution has begun.
///
/// Sent after arguments are complete and the tool is being executed.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ToolExecutingEvent {
    /// The tool call that is now executing
    pub tool_call_id: String,
    /// Human-readable display name for the tool
    #[serde(default)]
    pub display_name: Option<String>,
    /// Optional URL associated with the tool execution
    #[serde(default)]
    pub url: Option<String>,
}

/// Event containing the result of a tool execution.
///
/// Sent after a tool has completed execution.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ToolResultEvent {
    /// The tool call this result belongs to
    pub tool_call_id: String,
    /// The result returned by the tool (typically JSON)
    pub result: String,
}

/// Event indicating the streaming response is complete.
///
/// Sent when the assistant has finished generating the response.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DoneEvent {
    /// The ID of the completed message
    pub message_id: String,
}

/// Event indicating an error occurred during streaming.
///
/// May be sent at any point if an error occurs.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ErrorEvent {
    /// Human-readable error message
    pub message: String,
    /// Optional error code for programmatic handling
    pub code: Option<String>,
}

/// Event confirming a user message has been saved.
///
/// Sent after the user's input message has been persisted.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct UserMessageSavedEvent {
    /// The ID assigned to the saved user message
    pub message_id: String,
    /// The thread ID the message was saved to
    pub thread_id: String,
}

/// A single todo item in the assistant's task list.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TodoItem {
    /// The todo item content/description
    pub content: String,
    /// Active form shown when in_progress (e.g., "Running tests")
    #[serde(default)]
    pub active_form: Option<String>,
    /// Status: "pending", "in_progress", or "completed"
    pub status: String,
}

/// Event indicating the todo list has been updated.
///
/// Sent when the assistant modifies its internal task tracking.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TodosUpdatedEvent {
    /// The current list of todos
    pub todos: Vec<TodoItem>,
}

/// Event indicating a subagent has been started.
///
/// Sent when the assistant spawns a new subagent to handle a task.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SubagentStartedEvent {
    /// Unique identifier for this subagent task
    pub task_id: String,
    /// Human-readable description of the task
    pub description: String,
    /// Type of subagent (e.g., "Explore", "Plan", "Bash")
    pub subagent_type: String,
}

/// Event containing progress updates from a subagent.
///
/// Sent when a running subagent reports progress on its task.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SubagentProgressEvent {
    /// The task ID this progress update belongs to
    pub task_id: String,
    /// Progress message from the subagent
    pub message: String,
}

/// Event indicating a subagent has completed its task.
///
/// Sent when a subagent finishes execution and returns results.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SubagentCompletedEvent {
    /// The task ID that has completed
    pub task_id: String,
    /// Summary of the subagent's work
    pub summary: String,
    /// Number of tool calls made by the subagent
    #[serde(default)]
    pub tool_call_count: Option<u32>,
}

/// Event requesting user permission for an action.
///
/// Sent when the assistant needs explicit user approval to proceed.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PermissionRequestEvent {
    /// Unique identifier for this permission request
    pub permission_id: String,
    /// Human-readable description of what permission is being requested
    pub description: String,
    /// The tool that requires permission to execute
    #[serde(alias = "tool")]
    pub tool_name: String,
    /// The input parameters for the tool call
    #[serde(default)]
    pub tool_input: Option<serde_json::Value>,
    /// The specific tool call ID that needs permission
    #[serde(default)]
    pub tool_call_id: Option<String>,
}

/// Event indicating context has been compacted.
///
/// Sent when the conversation context is compacted to free up space.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ContextCompactedEvent {
    /// Number of messages removed during compaction
    pub messages_removed: u32,
    /// Number of tokens freed by the compaction
    pub tokens_freed: u32,
    /// Current context token usage (after compaction)
    #[serde(default)]
    pub tokens_used: Option<u32>,
    /// Context token limit
    #[serde(default)]
    pub token_limit: Option<u32>,
}

/// Skills injected event - sent when skills are loaded in the session
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SkillsInjectedEvent {
    /// List of skill names that were injected
    pub skills: Vec<String>,
}

/// OAuth consent required event
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OAuthConsentRequiredEvent {
    /// OAuth provider name
    pub provider: String,
    /// Consent URL to open in browser
    #[serde(default)]
    pub url: Option<String>,
    /// Skill name that requires OAuth
    #[serde(default)]
    pub skill_name: Option<String>,
}

/// Thread updated event - sent when thread metadata is changed
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ThreadUpdatedEvent {
    /// The ID of the thread that was updated
    pub thread_id: String,
    /// Updated title (if changed)
    #[serde(default)]
    pub title: Option<String>,
    /// Updated description (if changed)
    #[serde(default)]
    pub description: Option<String>,
}

/// Usage event - sent after done to provide context window usage info
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct UsageEvent {
    /// Current context window tokens used
    pub context_window_used: u32,
    /// Maximum context window limit
    pub context_window_limit: u32,
}

/// SystemInit event - sent when Claude CLI starts with session info
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SystemInitEvent {
    /// Session ID for the Claude CLI session
    pub session_id: String,
    /// Permission mode: "auto" or "prompt"
    pub permission_mode: String,
    /// Model name: "opus" or "sonnet"
    pub model: String,
    /// List of available tools
    pub tools: Vec<String>,
}

/// Wrapper enum for all possible SSE event types from Conductor.
///
/// Use pattern matching to handle different event types during stream processing.
///
/// # Example
///
/// ```ignore
/// match event {
///     SseEvent::Content(content) => {
///         // Append text to UI
///     }
///     SseEvent::Done(done) => {
///         // Finalize message
///     }
///     SseEvent::Error(err) => {
///         // Handle error
///     }
///     _ => {}
/// }
/// ```
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    /// Assistant text content chunk
    Content(ContentEvent),
    /// Assistant reasoning/thinking chunk
    Reasoning(ReasoningEvent),
    /// Tool call has started
    ToolCallStart(ToolCallStartEvent),
    /// Tool call argument chunk
    ToolCallArgument(ToolCallArgumentEvent),
    /// Tool is now executing
    ToolExecuting(ToolExecutingEvent),
    /// Tool execution result
    ToolResult(ToolResultEvent),
    /// Streaming complete
    Done(DoneEvent),
    /// Error occurred
    Error(ErrorEvent),
    /// User message saved confirmation
    UserMessageSaved(UserMessageSavedEvent),
    /// Todo list updated
    TodosUpdated(TodosUpdatedEvent),
    /// Subagent started
    SubagentStarted(SubagentStartedEvent),
    /// Subagent progress update
    SubagentProgress(SubagentProgressEvent),
    /// Subagent completed
    SubagentCompleted(SubagentCompletedEvent),
    /// Permission request
    PermissionRequest(PermissionRequestEvent),
    /// Context compacted
    ContextCompacted(ContextCompactedEvent),
    /// Skills injected
    SkillsInjected(SkillsInjectedEvent),
    /// OAuth consent required
    OAuthConsentRequired(OAuthConsentRequiredEvent),
    /// Thread updated
    ThreadUpdated(ThreadUpdatedEvent),
    /// Usage information
    Usage(UsageEvent),
    /// System initialization
    SystemInit(SystemInitEvent),
}

/// Wraps an SSE event with its metadata.
///
/// This is the top-level structure received from the SSE stream,
/// containing both the event-specific data and common metadata.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SseEventWithMeta {
    /// The event payload
    #[serde(flatten)]
    pub event: SseEvent,
    /// Metadata for sequencing and identification
    pub meta: EventMeta,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_event_meta() {
        let json = r#"{
            "seq": 42,
            "session_id": "sess-abc123",
            "thread_id": "thread-xyz789",
            "timestamp": 1736956800000
        }"#;

        let meta: EventMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.seq, Some(42));
        assert_eq!(meta.session_id, Some("sess-abc123".to_string()));
        assert_eq!(meta.thread_id, Some("thread-xyz789".to_string()));
        assert_eq!(meta.timestamp, Some(1736956800000));
    }

    #[test]
    fn test_parse_content_event() {
        let json = r#"{"text": "Hello, how can I help you?"}"#;
        let event: ContentEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.text, "Hello, how can I help you?");
    }

    #[test]
    fn test_parse_reasoning_event() {
        let json = r#"{"text": "Let me think about this..."}"#;
        let event: ReasoningEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.text, "Let me think about this...");
    }

    #[test]
    fn test_parse_tool_call_start_event() {
        let json = r#"{
            "tool_name": "read_file",
            "tool_call_id": "tc-12345"
        }"#;

        let event: ToolCallStartEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.tool_name, "read_file");
        assert_eq!(event.tool_call_id, "tc-12345");
    }

    #[test]
    fn test_parse_tool_call_argument_event() {
        let json = r#"{
            "tool_call_id": "tc-12345",
            "chunk": "{\"path\": \"/src"
        }"#;

        let event: ToolCallArgumentEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.tool_call_id, "tc-12345");
        assert_eq!(event.chunk, "{\"path\": \"/src");
    }

    #[test]
    fn test_parse_tool_call_argument_event_with_alias() {
        // Test backward compatibility with old field name
        let json = r#"{
            "tool_call_id": "tc-12345",
            "argument_chunk": "{\"path\": \"/src"
        }"#;

        let event: ToolCallArgumentEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.tool_call_id, "tc-12345");
        assert_eq!(event.chunk, "{\"path\": \"/src");
    }

    #[test]
    fn test_parse_tool_executing_event() {
        let json = r#"{"tool_call_id": "tc-12345"}"#;
        let event: ToolExecutingEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.tool_call_id, "tc-12345");
        assert_eq!(event.display_name, None);
        assert_eq!(event.url, None);
    }

    #[test]
    fn test_parse_tool_executing_event_with_fields() {
        let json = r#"{
            "tool_call_id": "tc-12345",
            "display_name": "Read File",
            "url": "https://example.com/tool"
        }"#;
        let event: ToolExecutingEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.tool_call_id, "tc-12345");
        assert_eq!(event.display_name, Some("Read File".to_string()));
        assert_eq!(event.url, Some("https://example.com/tool".to_string()));
    }

    #[test]
    fn test_parse_tool_result_event() {
        let json = r#"{
            "tool_call_id": "tc-12345",
            "result": "{\"content\": \"file contents here\"}"
        }"#;

        let event: ToolResultEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.tool_call_id, "tc-12345");
        assert_eq!(event.result, "{\"content\": \"file contents here\"}");
    }

    #[test]
    fn test_parse_done_event() {
        let json = r#"{"message_id": "msg-98765"}"#;
        let event: DoneEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.message_id, "msg-98765");
    }

    #[test]
    fn test_parse_error_event_with_code() {
        let json = r#"{
            "message": "Rate limit exceeded",
            "code": "RATE_LIMIT"
        }"#;

        let event: ErrorEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.message, "Rate limit exceeded");
        assert_eq!(event.code, Some("RATE_LIMIT".to_string()));
    }

    #[test]
    fn test_parse_error_event_without_code() {
        let json = r#"{"message": "Unknown error"}"#;
        let event: ErrorEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.message, "Unknown error");
        assert_eq!(event.code, None);
    }

    #[test]
    fn test_parse_user_message_saved_event() {
        let json = r#"{
            "message_id": "msg-11111",
            "thread_id": "thread-22222"
        }"#;

        let event: UserMessageSavedEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.message_id, "msg-11111");
        assert_eq!(event.thread_id, "thread-22222");
    }

    #[test]
    fn test_parse_todos_updated_event() {
        let json = r#"{
            "todos": [
                {"content": "Read the file", "status": "completed"},
                {"content": "Parse the JSON", "status": "in_progress"},
                {"content": "Write tests", "status": "pending"}
            ]
        }"#;

        let event: TodosUpdatedEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.todos.len(), 3);
        assert_eq!(event.todos[0].content, "Read the file");
        assert_eq!(event.todos[0].status, "completed");
        assert_eq!(event.todos[1].content, "Parse the JSON");
        assert_eq!(event.todos[1].status, "in_progress");
        assert_eq!(event.todos[2].content, "Write tests");
        assert_eq!(event.todos[2].status, "pending");
    }

    #[test]
    fn test_parse_permission_request_event() {
        let json = r#"{
            "permission_id": "perm-55555",
            "description": "Execute shell command: ls -la",
            "tool_name": "bash"
        }"#;

        let event: PermissionRequestEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.permission_id, "perm-55555");
        assert_eq!(event.description, "Execute shell command: ls -la");
        assert_eq!(event.tool_name, "bash");
        assert_eq!(event.tool_input, None);
        assert_eq!(event.tool_call_id, None);
    }

    #[test]
    fn test_parse_permission_request_event_with_tool_alias() {
        // Test backward compatibility with old field name
        let json = r#"{
            "permission_id": "perm-55555",
            "description": "Execute shell command: ls -la",
            "tool": "bash"
        }"#;

        let event: PermissionRequestEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.permission_id, "perm-55555");
        assert_eq!(event.description, "Execute shell command: ls -la");
        assert_eq!(event.tool_name, "bash");
    }

    #[test]
    fn test_parse_permission_request_event_with_all_fields() {
        let json = r#"{
            "permission_id": "perm-55555",
            "description": "Execute shell command: ls -la",
            "tool_name": "bash",
            "tool_input": {"command": "ls -la"},
            "tool_call_id": "tc-99999"
        }"#;

        let event: PermissionRequestEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.permission_id, "perm-55555");
        assert_eq!(event.description, "Execute shell command: ls -la");
        assert_eq!(event.tool_name, "bash");
        assert!(event.tool_input.is_some());
        assert_eq!(event.tool_call_id, Some("tc-99999".to_string()));
    }

    #[test]
    fn test_parse_context_compacted_event() {
        let json = r#"{
            "messages_removed": 5,
            "tokens_freed": 1500
        }"#;

        let event: ContextCompactedEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.messages_removed, 5);
        assert_eq!(event.tokens_freed, 1500);
    }

    #[test]
    fn test_parse_sse_event_content() {
        let json = r#"{
            "type": "content",
            "text": "Hello world"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::Content(content) => {
                assert_eq!(content.text, "Hello world");
            }
            _ => panic!("Expected Content event"),
        }
    }

    #[test]
    fn test_parse_sse_event_reasoning() {
        let json = r#"{
            "type": "reasoning",
            "text": "Thinking..."
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::Reasoning(reasoning) => {
                assert_eq!(reasoning.text, "Thinking...");
            }
            _ => panic!("Expected Reasoning event"),
        }
    }

    #[test]
    fn test_parse_sse_event_tool_call_start() {
        let json = r#"{
            "type": "tool_call_start",
            "tool_name": "write_file",
            "tool_call_id": "tc-99999"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ToolCallStart(e) => {
                assert_eq!(e.tool_name, "write_file");
                assert_eq!(e.tool_call_id, "tc-99999");
            }
            _ => panic!("Expected ToolCallStart event"),
        }
    }

    #[test]
    fn test_parse_sse_event_tool_call_argument() {
        let json = r#"{
            "type": "tool_call_argument",
            "tool_call_id": "tc-99999",
            "chunk": "partial_arg"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ToolCallArgument(e) => {
                assert_eq!(e.tool_call_id, "tc-99999");
                assert_eq!(e.chunk, "partial_arg");
            }
            _ => panic!("Expected ToolCallArgument event"),
        }
    }

    #[test]
    fn test_parse_sse_event_tool_executing() {
        let json = r#"{
            "type": "tool_executing",
            "tool_call_id": "tc-99999"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ToolExecuting(e) => {
                assert_eq!(e.tool_call_id, "tc-99999");
            }
            _ => panic!("Expected ToolExecuting event"),
        }
    }

    #[test]
    fn test_parse_sse_event_tool_result() {
        let json = r#"{
            "type": "tool_result",
            "tool_call_id": "tc-99999",
            "result": "success"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ToolResult(e) => {
                assert_eq!(e.tool_call_id, "tc-99999");
                assert_eq!(e.result, "success");
            }
            _ => panic!("Expected ToolResult event"),
        }
    }

    #[test]
    fn test_parse_sse_event_done() {
        let json = r#"{
            "type": "done",
            "message_id": "msg-final"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::Done(e) => {
                assert_eq!(e.message_id, "msg-final");
            }
            _ => panic!("Expected Done event"),
        }
    }

    #[test]
    fn test_parse_sse_event_error() {
        let json = r#"{
            "type": "error",
            "message": "Something went wrong",
            "code": "ERR_001"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::Error(e) => {
                assert_eq!(e.message, "Something went wrong");
                assert_eq!(e.code, Some("ERR_001".to_string()));
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[test]
    fn test_parse_sse_event_user_message_saved() {
        let json = r#"{
            "type": "user_message_saved",
            "message_id": "msg-user",
            "thread_id": "thread-main"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::UserMessageSaved(e) => {
                assert_eq!(e.message_id, "msg-user");
                assert_eq!(e.thread_id, "thread-main");
            }
            _ => panic!("Expected UserMessageSaved event"),
        }
    }

    #[test]
    fn test_parse_sse_event_todos_updated() {
        let json = r#"{
            "type": "todos_updated",
            "todos": [{"content": "Task 1", "status": "pending"}]
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::TodosUpdated(e) => {
                assert_eq!(e.todos.len(), 1);
                assert_eq!(e.todos[0].content, "Task 1");
            }
            _ => panic!("Expected TodosUpdated event"),
        }
    }

    #[test]
    fn test_parse_subagent_started_event() {
        let json = r#"{
            "task_id": "task-001",
            "description": "Explore codebase structure",
            "subagent_type": "Explore"
        }"#;

        let event: SubagentStartedEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.task_id, "task-001");
        assert_eq!(event.description, "Explore codebase structure");
        assert_eq!(event.subagent_type, "Explore");
    }

    #[test]
    fn test_parse_subagent_progress_event() {
        let json = r#"{
            "task_id": "task-001",
            "message": "Searching through src/ directory"
        }"#;

        let event: SubagentProgressEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.task_id, "task-001");
        assert_eq!(event.message, "Searching through src/ directory");
    }

    #[test]
    fn test_parse_subagent_completed_event() {
        let json = r#"{
            "task_id": "task-001",
            "summary": "Found 15 relevant files in the authentication module",
            "tool_call_count": 42
        }"#;

        let event: SubagentCompletedEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.task_id, "task-001");
        assert_eq!(
            event.summary,
            "Found 15 relevant files in the authentication module"
        );
        assert_eq!(event.tool_call_count, Some(42));
    }

    #[test]
    fn test_parse_subagent_completed_event_without_tool_count() {
        let json = r#"{
            "task_id": "task-002",
            "summary": "Analysis complete"
        }"#;

        let event: SubagentCompletedEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.task_id, "task-002");
        assert_eq!(event.summary, "Analysis complete");
        assert_eq!(event.tool_call_count, None);
    }

    #[test]
    fn test_parse_sse_event_subagent_started() {
        let json = r#"{
            "type": "subagent_started",
            "task_id": "task-123",
            "description": "Plan the implementation",
            "subagent_type": "Plan"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::SubagentStarted(e) => {
                assert_eq!(e.task_id, "task-123");
                assert_eq!(e.description, "Plan the implementation");
                assert_eq!(e.subagent_type, "Plan");
            }
            _ => panic!("Expected SubagentStarted event"),
        }
    }

    #[test]
    fn test_parse_sse_event_subagent_progress() {
        let json = r#"{
            "type": "subagent_progress",
            "task_id": "task-123",
            "message": "Analyzing file structure..."
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::SubagentProgress(e) => {
                assert_eq!(e.task_id, "task-123");
                assert_eq!(e.message, "Analyzing file structure...");
            }
            _ => panic!("Expected SubagentProgress event"),
        }
    }

    #[test]
    fn test_parse_sse_event_subagent_completed() {
        let json = r#"{
            "type": "subagent_completed",
            "task_id": "task-123",
            "summary": "Successfully analyzed project structure",
            "tool_call_count": 25
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::SubagentCompleted(e) => {
                assert_eq!(e.task_id, "task-123");
                assert_eq!(e.summary, "Successfully analyzed project structure");
                assert_eq!(e.tool_call_count, Some(25));
            }
            _ => panic!("Expected SubagentCompleted event"),
        }
    }

    #[test]
    fn test_parse_sse_event_permission_request() {
        let json = r#"{
            "type": "permission_request",
            "permission_id": "perm-001",
            "description": "Allow file write",
            "tool_name": "write"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::PermissionRequest(e) => {
                assert_eq!(e.permission_id, "perm-001");
                assert_eq!(e.description, "Allow file write");
                assert_eq!(e.tool_name, "write");
            }
            _ => panic!("Expected PermissionRequest event"),
        }
    }

    #[test]
    fn test_parse_sse_event_context_compacted() {
        let json = r#"{
            "type": "context_compacted",
            "messages_removed": 10,
            "tokens_freed": 2500
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ContextCompacted(e) => {
                assert_eq!(e.messages_removed, 10);
                assert_eq!(e.tokens_freed, 2500);
            }
            _ => panic!("Expected ContextCompacted event"),
        }
    }

    #[test]
    fn test_parse_sse_event_thread_updated() {
        let json = r#"{
            "type": "thread_updated",
            "thread_id": "thread-abc-123",
            "title": "Updated Title",
            "description": "Updated Description"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ThreadUpdated(e) => {
                assert_eq!(e.thread_id, "thread-abc-123");
                assert_eq!(e.title, Some("Updated Title".to_string()));
                assert_eq!(e.description, Some("Updated Description".to_string()));
            }
            _ => panic!("Expected ThreadUpdated event"),
        }
    }

    #[test]
    fn test_parse_sse_event_thread_updated_partial() {
        let json = r#"{
            "type": "thread_updated",
            "thread_id": "thread-xyz-789"
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ThreadUpdated(e) => {
                assert_eq!(e.thread_id, "thread-xyz-789");
                assert_eq!(e.title, None);
                assert_eq!(e.description, None);
            }
            _ => panic!("Expected ThreadUpdated event"),
        }
    }

    #[test]
    fn test_parse_sse_event_with_meta() {
        let json = r#"{
            "type": "content",
            "text": "Response text",
            "meta": {
                "seq": 1,
                "session_id": "sess-123",
                "thread_id": "thread-456",
                "timestamp": 1736956800000
            }
        }"#;

        let event_with_meta: SseEventWithMeta = serde_json::from_str(json).unwrap();

        match &event_with_meta.event {
            SseEvent::Content(content) => {
                assert_eq!(content.text, "Response text");
            }
            _ => panic!("Expected Content event"),
        }

        assert_eq!(event_with_meta.meta.seq, Some(1));
        assert_eq!(
            event_with_meta.meta.session_id,
            Some("sess-123".to_string())
        );
        assert_eq!(
            event_with_meta.meta.thread_id,
            Some("thread-456".to_string())
        );
        assert_eq!(event_with_meta.meta.timestamp, Some(1736956800000));
    }

    #[test]
    fn test_todo_item_parsing() {
        let json = r#"{"content": "Write documentation", "status": "pending"}"#;
        let item: TodoItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.content, "Write documentation");
        assert_eq!(item.status, "pending");
        assert_eq!(item.active_form, None);
    }

    #[test]
    fn test_todo_item_parsing_with_active_form() {
        let json =
            r#"{"content": "Run tests", "active_form": "Running tests", "status": "in_progress"}"#;
        let item: TodoItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.content, "Run tests");
        assert_eq!(item.active_form, Some("Running tests".to_string()));
        assert_eq!(item.status, "in_progress");
    }

    #[test]
    fn test_content_event_clone() {
        let event = ContentEvent {
            text: "Hello".to_string(),
            meta: EventMeta::default(),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn test_error_event_debug() {
        let event = ErrorEvent {
            message: "Test error".to_string(),
            code: Some("E001".to_string()),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Test error"));
        assert!(debug_str.contains("E001"));
    }

    #[test]
    fn test_sse_event_equality() {
        let event1 = SseEvent::Content(ContentEvent {
            text: "Hello".to_string(),
            meta: EventMeta::default(),
        });
        let event2 = SseEvent::Content(ContentEvent {
            text: "Hello".to_string(),
            meta: EventMeta::default(),
        });
        let event3 = SseEvent::Content(ContentEvent {
            text: "World".to_string(),
            meta: EventMeta::default(),
        });

        assert_eq!(event1, event2);
        assert_ne!(event1, event3);
    }

    #[test]
    fn test_parse_system_init_event() {
        let json = r#"{
            "session_id": "sess-abc-123",
            "permission_mode": "auto",
            "model": "opus",
            "tools": ["read", "write", "bash"]
        }"#;

        let event: SystemInitEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.session_id, "sess-abc-123");
        assert_eq!(event.permission_mode, "auto");
        assert_eq!(event.model, "opus");
        assert_eq!(event.tools, vec!["read", "write", "bash"]);
    }

    #[test]
    fn test_parse_sse_event_system_init() {
        let json = r#"{
            "type": "system_init",
            "session_id": "sess-xyz-789",
            "permission_mode": "prompt",
            "model": "sonnet",
            "tools": ["read", "write", "edit", "bash", "glob", "grep"]
        }"#;

        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::SystemInit(e) => {
                assert_eq!(e.session_id, "sess-xyz-789");
                assert_eq!(e.permission_mode, "prompt");
                assert_eq!(e.model, "sonnet");
                assert_eq!(e.tools.len(), 6);
                assert_eq!(e.tools[0], "read");
            }
            _ => panic!("Expected SystemInit event"),
        }
    }
}
