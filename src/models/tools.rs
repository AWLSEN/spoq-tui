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
            // Truncate and add ellipsis
            let truncated = &content[..MAX_PREVIEW_LEN];
            // Try to truncate at a word boundary
            let preview = if let Some(last_space) = truncated.rfind(char::is_whitespace) {
                if last_space > MAX_PREVIEW_LEN - 50 {
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
