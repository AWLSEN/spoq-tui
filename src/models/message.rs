use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::tools::{SubagentEvent, SubagentEventStatus, ToolCall, ToolEvent, ToolEventStatus};

/// Content block in the new API format - either text or tool use
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        #[serde(default)]
        result: Option<String>,
        #[serde(default)]
        is_error: bool,
    },
}

/// Message content - can be either an array of content blocks (new format) or a legacy string
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    Blocks(Vec<ContentBlock>),
    Legacy(String),
}

/// Role of a message in a conversation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// A segment of message content - either text or a tool event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageSegment {
    /// Plain text content
    Text(String),
    /// An inline tool event
    ToolEvent(ToolEvent),
    /// An inline subagent event
    SubagentEvent(SubagentEvent),
}

/// Message format from the server (different from client Message)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerMessage {
    /// Role of the message sender
    pub role: MessageRole,
    /// Content of the message (may be empty for tool calls)
    #[serde(default)]
    pub content: Option<String>,
    /// Tool calls made by the assistant
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// ID of the tool call this message responds to
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// Name for tool responses
    #[serde(default)]
    pub name: Option<String>,
}

impl ServerMessage {
    /// Convert a ServerMessage to a client Message.
    ///
    /// # Arguments
    /// * `thread_id` - The thread ID to associate with the message
    /// * `id` - The message ID to assign
    pub fn to_client_message(self, thread_id: &str, id: i64) -> Message {
        let role = self.role;

        Message {
            id,
            thread_id: thread_id.to_string(),
            role,
            content: self.content.unwrap_or_default(),
            created_at: Utc::now(),  // Server doesn't provide per-message timestamps
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),  // Server may not provide reasoning history
            reasoning_collapsed: true,
            segments: Vec::new(),
        }
    }
}

/// Represents a message within a thread from the backend API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// Message ID from backend (message_id)
    pub id: i64,
    /// ID of the thread this message belongs to
    pub thread_id: String,
    /// Role of the message sender
    pub role: MessageRole,
    /// Content of the message
    pub content: String,
    /// When the message was created
    pub created_at: DateTime<Utc>,
    /// Whether the message is currently being streamed
    #[serde(default)]
    pub is_streaming: bool,
    /// Partial content accumulated during streaming
    #[serde(default)]
    pub partial_content: String,
    /// Reasoning/thinking content from the assistant
    #[serde(default)]
    pub reasoning_content: String,
    /// Whether the reasoning block is collapsed in the UI
    #[serde(default)]
    pub reasoning_collapsed: bool,
    /// Segments of content including inline tool events
    #[serde(default)]
    pub segments: Vec<MessageSegment>,
}

impl Message {
    /// Append a token to the partial content during streaming
    pub fn append_token(&mut self, token: &str) {
        self.partial_content.push_str(token);
        self.add_text_segment(token.to_string());
    }

    /// Append a token to the reasoning content during streaming
    pub fn append_reasoning_token(&mut self, token: &str) {
        self.reasoning_content.push_str(token);
    }

    /// Finalize the message by moving partial_content to content and marking as not streaming
    pub fn finalize(&mut self) {
        if self.is_streaming {
            self.content = std::mem::take(&mut self.partial_content);
            self.is_streaming = false;
            // Collapse reasoning by default when message is finalized
            if !self.reasoning_content.is_empty() {
                self.reasoning_collapsed = true;
            }
        }
    }

    /// Toggle the reasoning collapsed state
    pub fn toggle_reasoning_collapsed(&mut self) {
        self.reasoning_collapsed = !self.reasoning_collapsed;
    }

    /// Count tokens in the reasoning content (approximation using whitespace)
    pub fn reasoning_token_count(&self) -> usize {
        // Simple approximation: split on whitespace and count
        self.reasoning_content.split_whitespace().count()
    }

    /// Add a text segment to the message
    pub fn add_text_segment(&mut self, text: String) {
        // If the last segment is text, append to it instead of creating a new one
        if let Some(MessageSegment::Text(last_text)) = self.segments.last_mut() {
            last_text.push_str(&text);
        } else if !text.is_empty() {
            self.segments.push(MessageSegment::Text(text));
        }
    }

    /// Start a new tool event
    pub fn start_tool_event(&mut self, tool_call_id: String, function_name: String) {
        let event = ToolEvent::new(tool_call_id, function_name);
        self.segments.push(MessageSegment::ToolEvent(event));
    }

    /// Complete a tool event by its tool_call_id
    pub fn complete_tool_event(&mut self, tool_call_id: &str) {
        for segment in &mut self.segments {
            if let MessageSegment::ToolEvent(event) = segment {
                if event.tool_call_id == tool_call_id {
                    event.complete();
                    break;
                }
            }
        }
    }

    /// Fail a tool event by its tool_call_id
    pub fn fail_tool_event(&mut self, tool_call_id: &str) {
        for segment in &mut self.segments {
            if let MessageSegment::ToolEvent(event) = segment {
                if event.tool_call_id == tool_call_id {
                    event.fail();
                    break;
                }
            }
        }
    }

    /// Get a tool event by its tool_call_id
    pub fn get_tool_event(&self, tool_call_id: &str) -> Option<&ToolEvent> {
        for segment in &self.segments {
            if let MessageSegment::ToolEvent(event) = segment {
                if event.tool_call_id == tool_call_id {
                    return Some(event);
                }
            }
        }
        None
    }

    /// Check if there are any running tools
    pub fn has_running_tools(&self) -> bool {
        self.segments.iter().any(|s| {
            matches!(s, MessageSegment::ToolEvent(e) if e.status == ToolEventStatus::Running)
        })
    }

    /// Set the display_name for a tool event by its tool_call_id
    pub fn set_tool_display_name(&mut self, tool_call_id: &str, display_name: String) {
        for segment in &mut self.segments {
            if let MessageSegment::ToolEvent(event) = segment {
                if event.tool_call_id == tool_call_id {
                    event.display_name = Some(display_name);
                    break;
                }
            }
        }
    }

    /// Append a chunk of JSON arguments to a tool event by its tool_call_id
    pub fn append_tool_arg_chunk(&mut self, tool_call_id: &str, chunk: &str) {
        for segment in &mut self.segments {
            if let MessageSegment::ToolEvent(event) = segment {
                if event.tool_call_id == tool_call_id {
                    event.append_arg_chunk(chunk);
                    break;
                }
            }
        }
    }

    /// Start a new subagent event
    pub fn start_subagent_event(&mut self, task_id: String, description: String, subagent_type: String) {
        let event = SubagentEvent::new(task_id, description, subagent_type);
        self.segments.push(MessageSegment::SubagentEvent(event));
    }

    /// Update subagent progress by its task_id
    pub fn update_subagent_progress(&mut self, task_id: &str, message: String) {
        for segment in &mut self.segments {
            if let MessageSegment::SubagentEvent(event) = segment {
                if event.task_id == task_id {
                    event.update_progress(Some(message), false);
                    break;
                }
            }
        }
    }

    /// Complete a subagent event by its task_id
    pub fn complete_subagent_event(&mut self, task_id: &str, summary: Option<String>, tool_call_count: usize) {
        for segment in &mut self.segments {
            if let MessageSegment::SubagentEvent(event) = segment {
                if event.task_id == task_id {
                    event.tool_call_count = tool_call_count;
                    event.complete(summary);
                    break;
                }
            }
        }
    }

    /// Get a subagent event by its task_id
    pub fn get_subagent_event(&self, task_id: &str) -> Option<&SubagentEvent> {
        for segment in &self.segments {
            if let MessageSegment::SubagentEvent(event) = segment {
                if event.task_id == task_id {
                    return Some(event);
                }
            }
        }
        None
    }

    /// Check if there are any running subagents
    pub fn has_running_subagents(&self) -> bool {
        self.segments.iter().any(|s| {
            matches!(s, MessageSegment::SubagentEvent(e) if e.status == SubagentEventStatus::Running)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_message() -> Message {
        Message {
            id: 1,
            thread_id: "test-thread".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            created_at: Utc::now(),
            is_streaming: false,
            partial_content: String::new(),
            reasoning_content: String::new(),
            reasoning_collapsed: true,
            segments: Vec::new(),
        }
    }

    #[test]
    fn test_start_subagent_event() {
        let mut message = create_test_message();

        message.start_subagent_event(
            "task-123".to_string(),
            "Test exploration".to_string(),
            "Explore".to_string(),
        );

        assert_eq!(message.segments.len(), 1);

        if let MessageSegment::SubagentEvent(event) = &message.segments[0] {
            assert_eq!(event.task_id, "task-123");
            assert_eq!(event.description, "Test exploration");
            assert_eq!(event.subagent_type, "Explore");
            assert_eq!(event.status, SubagentEventStatus::Running);
            assert!(event.progress_message.is_none());
            assert!(event.summary.is_none());
            assert_eq!(event.tool_call_count, 0);
        } else {
            panic!("Expected SubagentEvent segment");
        }
    }

    #[test]
    fn test_update_subagent_progress() {
        let mut message = create_test_message();

        message.start_subagent_event(
            "task-456".to_string(),
            "Test task".to_string(),
            "general-purpose".to_string(),
        );

        message.update_subagent_progress("task-456", "Reading files".to_string());

        if let MessageSegment::SubagentEvent(event) = &message.segments[0] {
            assert_eq!(event.progress_message, Some("Reading files".to_string()));
            assert_eq!(event.tool_call_count, 0); // Should not increment
        } else {
            panic!("Expected SubagentEvent segment");
        }

        // Update again with different message
        message.update_subagent_progress("task-456", "Processing data".to_string());

        if let MessageSegment::SubagentEvent(event) = &message.segments[0] {
            assert_eq!(event.progress_message, Some("Processing data".to_string()));
        } else {
            panic!("Expected SubagentEvent segment");
        }
    }

    #[test]
    fn test_complete_subagent_event() {
        let mut message = create_test_message();

        message.start_subagent_event(
            "task-789".to_string(),
            "Test task".to_string(),
            "Explore".to_string(),
        );

        message.complete_subagent_event(
            "task-789",
            Some("Found 5 files".to_string()),
            3,
        );

        if let MessageSegment::SubagentEvent(event) = &message.segments[0] {
            assert_eq!(event.status, SubagentEventStatus::Complete);
            assert_eq!(event.summary, Some("Found 5 files".to_string()));
            assert_eq!(event.tool_call_count, 3);
            assert!(event.completed_at.is_some());
            assert!(event.duration_secs.is_some());
        } else {
            panic!("Expected SubagentEvent segment");
        }
    }

    #[test]
    fn test_complete_subagent_event_without_summary() {
        let mut message = create_test_message();

        message.start_subagent_event(
            "task-999".to_string(),
            "Test task".to_string(),
            "Bash".to_string(),
        );

        message.complete_subagent_event("task-999", None, 0);

        if let MessageSegment::SubagentEvent(event) = &message.segments[0] {
            assert_eq!(event.status, SubagentEventStatus::Complete);
            assert!(event.summary.is_none());
            assert_eq!(event.tool_call_count, 0);
        } else {
            panic!("Expected SubagentEvent segment");
        }
    }

    #[test]
    fn test_get_subagent_event() {
        let mut message = create_test_message();

        message.start_subagent_event(
            "task-abc".to_string(),
            "First task".to_string(),
            "Explore".to_string(),
        );
        message.start_subagent_event(
            "task-def".to_string(),
            "Second task".to_string(),
            "Bash".to_string(),
        );

        // Get first event
        let event1 = message.get_subagent_event("task-abc");
        assert!(event1.is_some());
        assert_eq!(event1.unwrap().description, "First task");

        // Get second event
        let event2 = message.get_subagent_event("task-def");
        assert!(event2.is_some());
        assert_eq!(event2.unwrap().description, "Second task");

        // Try to get non-existent event
        let event3 = message.get_subagent_event("task-xyz");
        assert!(event3.is_none());
    }

    #[test]
    fn test_has_running_subagents() {
        let mut message = create_test_message();

        // Initially no subagents
        assert!(!message.has_running_subagents());

        // Add a running subagent
        message.start_subagent_event(
            "task-running".to_string(),
            "Running task".to_string(),
            "Explore".to_string(),
        );
        assert!(message.has_running_subagents());

        // Complete the subagent
        message.complete_subagent_event("task-running", None, 0);
        assert!(!message.has_running_subagents());

        // Add multiple subagents
        message.start_subagent_event(
            "task-1".to_string(),
            "Task 1".to_string(),
            "Explore".to_string(),
        );
        message.start_subagent_event(
            "task-2".to_string(),
            "Task 2".to_string(),
            "Bash".to_string(),
        );
        assert!(message.has_running_subagents());

        // Complete one
        message.complete_subagent_event("task-1", None, 0);
        assert!(message.has_running_subagents()); // Still one running

        // Complete the other
        message.complete_subagent_event("task-2", None, 0);
        assert!(!message.has_running_subagents());
    }

    #[test]
    fn test_subagent_event_workflow() {
        let mut message = create_test_message();

        // Start a subagent
        message.start_subagent_event(
            "workflow-test".to_string(),
            "Complex analysis".to_string(),
            "general-purpose".to_string(),
        );

        // Verify it's running
        assert!(message.has_running_subagents());
        let event = message.get_subagent_event("workflow-test");
        assert!(event.is_some());
        assert_eq!(event.unwrap().status, SubagentEventStatus::Running);

        // Update progress multiple times
        message.update_subagent_progress("workflow-test", "Step 1: Reading".to_string());
        message.update_subagent_progress("workflow-test", "Step 2: Processing".to_string());
        message.update_subagent_progress("workflow-test", "Step 3: Finalizing".to_string());

        let event = message.get_subagent_event("workflow-test");
        assert_eq!(event.unwrap().progress_message, Some("Step 3: Finalizing".to_string()));

        // Complete the subagent
        message.complete_subagent_event(
            "workflow-test",
            Some("Analysis complete".to_string()),
            5,
        );

        assert!(!message.has_running_subagents());
        let event = message.get_subagent_event("workflow-test");
        assert_eq!(event.unwrap().status, SubagentEventStatus::Complete);
        assert_eq!(event.unwrap().summary, Some("Analysis complete".to_string()));
        assert_eq!(event.unwrap().tool_call_count, 5);
    }

    #[test]
    fn test_mixed_segments() {
        let mut message = create_test_message();

        // Add text
        message.add_text_segment("Starting analysis...".to_string());

        // Add subagent event
        message.start_subagent_event(
            "task-mixed".to_string(),
            "Mixed test".to_string(),
            "Explore".to_string(),
        );

        // Add more text
        message.add_text_segment("After subagent start".to_string());

        // Add tool event
        message.start_tool_event("tool-1".to_string(), "Read".to_string());

        // Verify segments
        assert_eq!(message.segments.len(), 4);
        assert!(matches!(message.segments[0], MessageSegment::Text(_)));
        assert!(matches!(message.segments[1], MessageSegment::SubagentEvent(_)));
        assert!(matches!(message.segments[2], MessageSegment::Text(_)));
        assert!(matches!(message.segments[3], MessageSegment::ToolEvent(_)));

        // Verify has_running_subagents works with mixed segments
        assert!(message.has_running_subagents());

        // Complete the subagent
        message.complete_subagent_event("task-mixed", None, 2);
        assert!(!message.has_running_subagents());
    }

    #[test]
    fn test_update_nonexistent_subagent() {
        let mut message = create_test_message();

        // Try to update a subagent that doesn't exist - should not panic
        message.update_subagent_progress("nonexistent", "Progress".to_string());

        assert_eq!(message.segments.len(), 0);
    }

    #[test]
    fn test_complete_nonexistent_subagent() {
        let mut message = create_test_message();

        // Try to complete a subagent that doesn't exist - should not panic
        message.complete_subagent_event("nonexistent", Some("Done".to_string()), 1);

        assert_eq!(message.segments.len(), 0);
    }

    #[test]
    fn test_content_block_text_serialization() {
        let block = ContentBlock::Text {
            text: "Hello, world!".to_string(),
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"Hello, world!""#));

        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn test_content_block_tool_use_serialization() {
        let block = ContentBlock::ToolUse {
            id: "tool-123".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "/test/file.txt"}),
            result: Some("File content".to_string()),
            is_error: false,
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"tool_use""#));
        assert!(json.contains(r#""id":"tool-123""#));
        assert!(json.contains(r#""name":"read_file""#));

        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(block, deserialized);
    }

    #[test]
    fn test_content_block_tool_use_defaults() {
        let json = r#"{"type":"tool_use","id":"tool-456","name":"bash","input":{"command":"ls"}}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();

        match block {
            ContentBlock::ToolUse { id, name, input, result, is_error } => {
                assert_eq!(id, "tool-456");
                assert_eq!(name, "bash");
                assert_eq!(input, serde_json::json!({"command": "ls"}));
                assert_eq!(result, None);
                assert_eq!(is_error, false);
            }
            _ => panic!("Expected ToolUse variant"),
        }
    }

    #[test]
    fn test_message_content_blocks() {
        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "Processing...".to_string(),
            },
            ContentBlock::ToolUse {
                id: "tool-1".to_string(),
                name: "read".to_string(),
                input: serde_json::json!({"file": "test.txt"}),
                result: None,
                is_error: false,
            },
        ]);

        let json = serde_json::to_string(&content).unwrap();
        let deserialized: MessageContent = serde_json::from_str(&json).unwrap();
        assert_eq!(content, deserialized);
    }

    #[test]
    fn test_message_content_legacy() {
        let content = MessageContent::Legacy("Hello, world!".to_string());

        let json = serde_json::to_string(&content).unwrap();
        assert_eq!(json, r#""Hello, world!""#);

        let deserialized: MessageContent = serde_json::from_str(&json).unwrap();
        assert_eq!(content, deserialized);
    }

    #[test]
    fn test_message_content_untagged_deserialization() {
        // Test that a plain string deserializes as Legacy
        let json = r#""Simple text""#;
        let content: MessageContent = serde_json::from_str(json).unwrap();
        assert!(matches!(content, MessageContent::Legacy(_)));

        // Test that an array deserializes as Blocks
        let json = r#"[{"type":"text","text":"Block text"}]"#;
        let content: MessageContent = serde_json::from_str(json).unwrap();
        assert!(matches!(content, MessageContent::Blocks(_)));
    }
}
