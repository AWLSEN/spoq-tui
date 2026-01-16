use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a tool event for inline display
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolEventStatus {
    /// Tool is currently running
    Running,
    /// Tool completed successfully
    Complete,
    /// Tool failed
    Failed,
}

/// A tool event that can be displayed inline with message content
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolEvent {
    /// The tool call ID from the backend
    pub tool_call_id: String,
    /// Name of the tool (e.g., "Bash", "Read", "Glob")
    pub function_name: String,
    /// Optional display name (e.g., "Read src/main.rs" instead of just "Read")
    pub display_name: Option<String>,
    /// Current status of the tool
    pub status: ToolEventStatus,
    /// When the tool started
    pub started_at: DateTime<Utc>,
    /// When the tool completed (if complete)
    pub completed_at: Option<DateTime<Utc>>,
    /// Duration in seconds (calculated when complete)
    pub duration_secs: Option<f64>,
    /// Accumulated JSON arguments from streaming chunks
    #[serde(default)]
    pub args_json: String,
    /// Formatted display string for arguments (e.g., "Reading /src/main.rs")
    #[serde(default)]
    pub args_display: Option<String>,
    /// Truncated result preview (max ~500 chars)
    #[serde(default)]
    pub result_preview: Option<String>,
    /// Whether the result was an error
    #[serde(default)]
    pub result_is_error: bool,
}

impl ToolEvent {
    /// Create a new running tool event
    pub fn new(tool_call_id: String, function_name: String) -> Self {
        Self {
            tool_call_id,
            function_name,
            display_name: None,
            status: ToolEventStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            duration_secs: None,
            args_json: String::new(),
            args_display: None,
            result_preview: None,
            result_is_error: false,
        }
    }

    /// Mark the tool as complete
    pub fn complete(&mut self) {
        self.status = ToolEventStatus::Complete;
        self.completed_at = Some(Utc::now());
        self.duration_secs = Some((Utc::now() - self.started_at).num_milliseconds() as f64 / 1000.0);
    }

    /// Mark the tool as failed
    pub fn fail(&mut self) {
        self.status = ToolEventStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.duration_secs = Some((Utc::now() - self.started_at).num_milliseconds() as f64 / 1000.0);
    }

    /// Append a chunk of JSON arguments from streaming
    pub fn append_arg_chunk(&mut self, chunk: &str) {
        self.args_json.push_str(chunk);
    }

    /// Set the result preview, truncating if necessary
    ///
    /// # Arguments
    /// * `content` - The full result content
    /// * `is_error` - Whether the result represents an error
    pub fn set_result(&mut self, content: &str, is_error: bool) {
        const MAX_PREVIEW_LEN: usize = 500;

        self.result_is_error = is_error;

        if content.len() <= MAX_PREVIEW_LEN {
            self.result_preview = Some(content.to_string());
        } else {
            // Find a valid UTF-8 char boundary at or before MAX_PREVIEW_LEN
            let mut end = MAX_PREVIEW_LEN;
            while end > 0 && !content.is_char_boundary(end) {
                end -= 1;
            }
            let truncated = &content[..end];

            // Try to truncate at a word boundary
            let preview = if let Some(last_space) = truncated.rfind(char::is_whitespace) {
                if last_space > end.saturating_sub(50) {
                    // Only use word boundary if we're not cutting off too much
                    &truncated[..last_space]
                } else {
                    truncated
                }
            } else {
                truncated
            };
            self.result_preview = Some(format!("{}...", preview));
        }
    }
}

/// Function details within a tool call
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallFunction {
    /// Name of the function being called
    pub name: String,
    /// Arguments passed to the function (JSON string)
    #[serde(default)]
    pub arguments: String,
}

/// Represents a tool call made by the assistant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub id: String,
    /// Type of tool call (usually "function")
    #[serde(rename = "type")]
    pub call_type: String,
    /// Function details
    pub function: ToolCallFunction,
}

/// Status of a subagent event for inline display
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SubagentEventStatus {
    /// Subagent is currently running
    Running,
    /// Subagent completed successfully
    Complete,
}

/// A subagent event that can be displayed inline with message content
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubagentEvent {
    /// The task ID from the Task tool
    pub task_id: String,
    /// Description of the subagent task
    pub description: String,
    /// Type of subagent (e.g., "Explore", "general-purpose")
    pub subagent_type: String,
    /// Current status of the subagent
    pub status: SubagentEventStatus,
    /// When the subagent started
    pub started_at: DateTime<Utc>,
    /// When the subagent completed (if complete)
    pub completed_at: Option<DateTime<Utc>>,
    /// Duration in seconds (calculated when complete)
    pub duration_secs: Option<f64>,
    /// Current progress message from the subagent
    #[serde(default)]
    pub progress_message: Option<String>,
    /// Summary of subagent results (when complete)
    #[serde(default)]
    pub summary: Option<String>,
    /// Number of tool calls made by the subagent
    #[serde(default)]
    pub tool_call_count: usize,
}

impl SubagentEvent {
    /// Create a new running subagent event
    pub fn new(task_id: String, description: String, subagent_type: String) -> Self {
        Self {
            task_id,
            description,
            subagent_type,
            status: SubagentEventStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            duration_secs: None,
            progress_message: None,
            summary: None,
            tool_call_count: 0,
        }
    }

    /// Update the progress message and optionally increment tool call count
    pub fn update_progress(&mut self, message: Option<String>, increment_tool_calls: bool) {
        self.progress_message = message;
        if increment_tool_calls {
            self.tool_call_count += 1;
        }
    }

    /// Mark the subagent as complete with an optional summary
    pub fn complete(&mut self, summary: Option<String>) {
        self.status = SubagentEventStatus::Complete;
        self.completed_at = Some(Utc::now());
        self.duration_secs = Some((Utc::now() - self.started_at).num_milliseconds() as f64 / 1000.0);
        self.summary = summary;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_event_new() {
        let event = SubagentEvent::new(
            "task-123".to_string(),
            "Test task".to_string(),
            "Explore".to_string(),
        );

        assert_eq!(event.task_id, "task-123");
        assert_eq!(event.description, "Test task");
        assert_eq!(event.subagent_type, "Explore");
        assert_eq!(event.status, SubagentEventStatus::Running);
        assert!(event.completed_at.is_none());
        assert!(event.duration_secs.is_none());
        assert!(event.progress_message.is_none());
        assert!(event.summary.is_none());
        assert_eq!(event.tool_call_count, 0);
    }

    #[test]
    fn test_subagent_event_update_progress() {
        let mut event = SubagentEvent::new(
            "task-456".to_string(),
            "Test task".to_string(),
            "general-purpose".to_string(),
        );

        event.update_progress(Some("Reading files".to_string()), true);
        assert_eq!(event.progress_message, Some("Reading files".to_string()));
        assert_eq!(event.tool_call_count, 1);

        event.update_progress(Some("Processing data".to_string()), true);
        assert_eq!(event.progress_message, Some("Processing data".to_string()));
        assert_eq!(event.tool_call_count, 2);

        event.update_progress(Some("Final stage".to_string()), false);
        assert_eq!(event.progress_message, Some("Final stage".to_string()));
        assert_eq!(event.tool_call_count, 2); // Should not increment
    }

    #[test]
    fn test_subagent_event_complete() {
        let mut event = SubagentEvent::new(
            "task-789".to_string(),
            "Test task".to_string(),
            "Explore".to_string(),
        );

        let summary = Some("Found 10 matching files".to_string());
        event.complete(summary.clone());

        assert_eq!(event.status, SubagentEventStatus::Complete);
        assert!(event.completed_at.is_some());
        assert!(event.duration_secs.is_some());
        assert_eq!(event.summary, summary);

        // Duration should be non-negative
        let duration = event.duration_secs.unwrap();
        assert!(duration >= 0.0);
    }

    #[test]
    fn test_subagent_event_complete_without_summary() {
        let mut event = SubagentEvent::new(
            "task-999".to_string(),
            "Test task".to_string(),
            "Bash".to_string(),
        );

        event.complete(None);

        assert_eq!(event.status, SubagentEventStatus::Complete);
        assert!(event.completed_at.is_some());
        assert!(event.duration_secs.is_some());
        assert!(event.summary.is_none());
    }

    #[test]
    fn test_subagent_event_status_enum() {
        let running = SubagentEventStatus::Running;
        let complete = SubagentEventStatus::Complete;

        assert_ne!(running, complete);

        // Test cloning and equality
        let running_clone = running.clone();
        assert_eq!(running, running_clone);
    }

    #[test]
    fn test_subagent_event_full_workflow() {
        let mut event = SubagentEvent::new(
            "workflow-test".to_string(),
            "Complex task".to_string(),
            "general-purpose".to_string(),
        );

        // Initial state
        assert_eq!(event.status, SubagentEventStatus::Running);
        assert_eq!(event.tool_call_count, 0);

        // Simulate progress updates
        event.update_progress(Some("Starting analysis".to_string()), true);
        assert_eq!(event.tool_call_count, 1);

        event.update_progress(Some("Processing files".to_string()), true);
        assert_eq!(event.tool_call_count, 2);

        event.update_progress(Some("Generating report".to_string()), true);
        assert_eq!(event.tool_call_count, 3);

        // Complete the task
        event.complete(Some("Analysis complete with 3 tool calls".to_string()));
        assert_eq!(event.status, SubagentEventStatus::Complete);
        assert!(event.completed_at.is_some());
        assert!(event.duration_secs.is_some());
        assert_eq!(
            event.summary,
            Some("Analysis complete with 3 tool calls".to_string())
        );
        assert_eq!(event.tool_call_count, 3);
    }

    #[test]
    fn test_subagent_event_serialization() {
        let event = SubagentEvent::new(
            "ser-test".to_string(),
            "Serialization test".to_string(),
            "Explore".to_string(),
        );

        // Test that the event can be serialized and deserialized
        let json = serde_json::to_string(&event).expect("Failed to serialize");
        let deserialized: SubagentEvent =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(event.task_id, deserialized.task_id);
        assert_eq!(event.description, deserialized.description);
        assert_eq!(event.subagent_type, deserialized.subagent_type);
        assert_eq!(event.status, deserialized.status);
    }
}
