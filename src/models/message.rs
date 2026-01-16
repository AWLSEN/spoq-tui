use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::tools::{SubagentEvent, ToolCall, ToolEvent, ToolEventStatus};

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
        let role = match self.role {
            MessageRole::User => MessageRole::User,
            MessageRole::Assistant => MessageRole::Assistant,
            MessageRole::System => MessageRole::System,
            MessageRole::Tool => MessageRole::Tool,
        };

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
}
