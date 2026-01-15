//! Tool and subagent execution state tracking
//!
//! ToolTracker manages ephemeral state for active tool calls within a thread.
//! SubagentTracker manages ephemeral state for active subagents within a thread.
//! Both are cleared when the thread's "done" event arrives.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Display status for tool UI rendering with timing info
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolDisplayStatus {
    /// Tool call started, showing function name
    Started {
        function: String,
        started_at: u64,
    },
    /// Tool is executing, showing display name
    Executing {
        display_name: String,
    },
    /// Tool completed (success or failure)
    Completed {
        success: bool,
        summary: String,
        completed_at: u64,
    },
}

impl ToolDisplayStatus {
    /// Check if this status should still be rendered given the current tick
    /// Success fades after 30 ticks (~3 seconds), failures persist
    pub fn should_render(&self, current_tick: u64) -> bool {
        match self {
            ToolDisplayStatus::Started { .. } => true,
            ToolDisplayStatus::Executing { .. } => true,
            ToolDisplayStatus::Completed { success, completed_at, .. } => {
                // Failures always persist
                if !success {
                    return true;
                }
                // Success fades after 30 ticks
                current_tick < completed_at.saturating_add(30)
            }
        }
    }

    /// Get the display text for this status
    pub fn display_text(&self) -> String {
        match self {
            ToolDisplayStatus::Started { function, .. } => {
                format!("{function}...")
            }
            ToolDisplayStatus::Executing { display_name } => {
                display_name.clone()
            }
            ToolDisplayStatus::Completed { summary, .. } => {
                summary.clone()
            }
        }
    }

    /// Check if this is a completed success
    pub fn is_success(&self) -> bool {
        matches!(self, ToolDisplayStatus::Completed { success: true, .. })
    }

    /// Check if this is a completed failure
    pub fn is_failure(&self) -> bool {
        matches!(self, ToolDisplayStatus::Completed { success: false, .. })
    }

    /// Check if still in progress (started or executing)
    pub fn is_in_progress(&self) -> bool {
        matches!(self, ToolDisplayStatus::Started { .. } | ToolDisplayStatus::Executing { .. })
    }
}

/// State of a single tool call
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallState {
    /// Name of the tool being executed
    pub tool_name: String,
    /// Status of the tool call
    pub status: ToolCallStatus,
    /// Input/arguments provided to the tool (for display)
    pub input: Option<String>,
    /// Output from the tool (populated on completion)
    pub output: Option<String>,
    /// Error message if the tool failed
    pub error: Option<String>,
    /// Display status for UI rendering (with timing)
    pub display_status: Option<ToolDisplayStatus>,
}

/// Status of a tool call
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ToolCallStatus {
    /// Tool call is pending execution
    #[default]
    Pending,
    /// Tool is currently running
    Running,
    /// Tool completed successfully
    Completed,
    /// Tool execution failed
    Failed,
}

impl ToolCallState {
    /// Create a new pending tool call state
    pub fn new(tool_name: String) -> Self {
        Self {
            tool_name,
            status: ToolCallStatus::Pending,
            input: None,
            output: None,
            error: None,
            display_status: None,
        }
    }

    /// Create a tool call state with input
    pub fn with_input(tool_name: String, input: String) -> Self {
        Self {
            tool_name,
            status: ToolCallStatus::Pending,
            input: Some(input),
            output: None,
            error: None,
            display_status: None,
        }
    }

    /// Create a tool call state with display status (for UI)
    pub fn with_display(tool_name: String, display_status: ToolDisplayStatus) -> Self {
        Self {
            tool_name,
            status: ToolCallStatus::Pending,
            input: None,
            output: None,
            error: None,
            display_status: Some(display_status),
        }
    }

    /// Set the display status
    pub fn set_display_status(&mut self, status: ToolDisplayStatus) {
        self.display_status = Some(status);
    }

    /// Mark the tool as running
    pub fn start(&mut self) {
        self.status = ToolCallStatus::Running;
    }

    /// Mark the tool as completed with output
    pub fn complete(&mut self, output: Option<String>) {
        self.status = ToolCallStatus::Completed;
        self.output = output;
    }

    /// Mark the tool as failed with error
    pub fn fail(&mut self, error: String) {
        self.status = ToolCallStatus::Failed;
        self.error = Some(error);
    }

    /// Check if the tool is still active (pending or running)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            ToolCallStatus::Pending | ToolCallStatus::Running
        )
    }

    /// Check if the tool has finished (completed or failed)
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            ToolCallStatus::Completed | ToolCallStatus::Failed
        )
    }
}

/// Tracks active tool calls for a thread
///
/// This is ephemeral state that gets cleared when the thread's
/// streaming response completes (done event). It allows the UI
/// to show tool execution status during streaming.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolTracker {
    /// Active tool calls indexed by tool_call_id
    active_tools: HashMap<String, ToolCallState>,
}

impl ToolTracker {
    /// Create a new empty ToolTracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new tool call
    pub fn register_tool(&mut self, tool_call_id: String, state: ToolCallState) {
        self.active_tools.insert(tool_call_id, state);
    }

    /// Get a tool call state by ID
    pub fn get_tool(&self, tool_call_id: &str) -> Option<&ToolCallState> {
        self.active_tools.get(tool_call_id)
    }

    /// Get mutable access to a tool call state
    pub fn get_tool_mut(&mut self, tool_call_id: &str) -> Option<&mut ToolCallState> {
        self.active_tools.get_mut(tool_call_id)
    }

    /// Start a tool (mark as running)
    pub fn start_tool(&mut self, tool_call_id: &str) {
        if let Some(state) = self.active_tools.get_mut(tool_call_id) {
            state.start();
        }
    }

    /// Complete a tool call
    pub fn complete_tool(&mut self, tool_call_id: &str, output: Option<String>) {
        if let Some(state) = self.active_tools.get_mut(tool_call_id) {
            state.complete(output);
        }
    }

    /// Fail a tool call
    pub fn fail_tool(&mut self, tool_call_id: &str, error: String) {
        if let Some(state) = self.active_tools.get_mut(tool_call_id) {
            state.fail(error);
        }
    }

    /// Remove a tool call from tracking
    pub fn remove_tool(&mut self, tool_call_id: &str) -> Option<ToolCallState> {
        self.active_tools.remove(tool_call_id)
    }

    /// Clear all tracked tools (called when thread done event arrives)
    pub fn clear(&mut self) {
        self.active_tools.clear();
    }

    /// Get all active (pending or running) tool calls
    pub fn active_tools(&self) -> Vec<(&String, &ToolCallState)> {
        self.active_tools
            .iter()
            .filter(|(_, state)| state.is_active())
            .collect()
    }

    /// Get all tool calls (including finished ones)
    pub fn all_tools(&self) -> &HashMap<String, ToolCallState> {
        &self.active_tools
    }

    /// Check if there are any active tool calls
    pub fn has_active_tools(&self) -> bool {
        self.active_tools.values().any(|state| state.is_active())
    }

    /// Get the count of active tools
    pub fn active_count(&self) -> usize {
        self.active_tools
            .values()
            .filter(|state| state.is_active())
            .count()
    }

    /// Get the total count of tracked tools
    pub fn total_count(&self) -> usize {
        self.active_tools.len()
    }

    /// Check if a specific tool is being tracked
    pub fn contains(&self, tool_call_id: &str) -> bool {
        self.active_tools.contains_key(tool_call_id)
    }

    /// Update display status for a tool
    pub fn set_display_status(&mut self, tool_call_id: &str, status: ToolDisplayStatus) {
        if let Some(state) = self.active_tools.get_mut(tool_call_id) {
            state.set_display_status(status);
        }
    }

    /// Get tools that should be rendered at the given tick
    /// Returns tools in order: in-progress first, then completed (newest first)
    pub fn tools_to_render(&self, current_tick: u64) -> Vec<(&String, &ToolCallState)> {
        let mut tools: Vec<_> = self
            .active_tools
            .iter()
            .filter(|(_, state)| {
                state
                    .display_status
                    .as_ref()
                    .is_some_and(|ds| ds.should_render(current_tick))
            })
            .collect();

        // Sort: in-progress first, then by recency (for completed)
        tools.sort_by(|(_, a), (_, b)| {
            let a_in_progress = a.display_status.as_ref().is_some_and(|ds| ds.is_in_progress());
            let b_in_progress = b.display_status.as_ref().is_some_and(|ds| ds.is_in_progress());

            match (a_in_progress, b_in_progress) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });

        tools
    }

    /// Register a tool with display status for started state
    pub fn register_tool_started(&mut self, tool_call_id: String, tool_name: String, current_tick: u64) {
        let display_status = ToolDisplayStatus::Started {
            function: tool_name.clone(),
            started_at: current_tick,
        };
        let mut state = ToolCallState::new(tool_name);
        state.set_display_status(display_status);
        self.active_tools.insert(tool_call_id, state);
    }

    /// Update tool to executing state with display name
    pub fn set_tool_executing(&mut self, tool_call_id: &str, display_name: String) {
        if let Some(state) = self.active_tools.get_mut(tool_call_id) {
            state.status = ToolCallStatus::Running;
            state.set_display_status(ToolDisplayStatus::Executing { display_name });
        }
    }

    /// Complete tool with summary for display
    pub fn complete_tool_with_summary(&mut self, tool_call_id: &str, success: bool, summary: String, current_tick: u64) {
        if let Some(state) = self.active_tools.get_mut(tool_call_id) {
            if success {
                state.status = ToolCallStatus::Completed;
            } else {
                state.status = ToolCallStatus::Failed;
            }
            state.set_display_status(ToolDisplayStatus::Completed {
                success,
                summary,
                completed_at: current_tick,
            });
        }
    }
}

// ============= Subagent State Types =============

/// Display status for subagent UI rendering with timing info
///
/// Subagents are displayed with a spinner (not progress bar since we don't have percentage).
/// UI design:
/// ```text
/// ┌ ◐ Exploring codebase structure
/// │   Found 5 relevant files...
/// └ ✓ Complete (8 tool calls)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SubagentDisplayStatus {
    /// Subagent started, showing description
    Started {
        description: String,
        started_at: u64,
    },
    /// Subagent progress update (optional message below main description)
    Progress {
        description: String,
        progress_message: String,
    },
    /// Subagent completed
    Completed {
        success: bool,
        summary: String,
        completed_at: u64,
    },
}

impl SubagentDisplayStatus {
    /// Check if this status should still be rendered given the current tick
    /// Success fades after 30 ticks (~3 seconds), failures persist
    pub fn should_render(&self, current_tick: u64) -> bool {
        match self {
            SubagentDisplayStatus::Started { .. } => true,
            SubagentDisplayStatus::Progress { .. } => true,
            SubagentDisplayStatus::Completed { success, completed_at, .. } => {
                // Failures always persist
                if !success {
                    return true;
                }
                // Success fades after 30 ticks
                current_tick < completed_at.saturating_add(30)
            }
        }
    }

    /// Get the main description text for this status
    pub fn description(&self) -> &str {
        match self {
            SubagentDisplayStatus::Started { description, .. } => description,
            SubagentDisplayStatus::Progress { description, .. } => description,
            SubagentDisplayStatus::Completed { summary, .. } => summary,
        }
    }

    /// Get the progress message (if any)
    pub fn progress_message(&self) -> Option<&str> {
        match self {
            SubagentDisplayStatus::Progress { progress_message, .. } => Some(progress_message),
            _ => None,
        }
    }

    /// Check if this is a completed success
    pub fn is_success(&self) -> bool {
        matches!(self, SubagentDisplayStatus::Completed { success: true, .. })
    }

    /// Check if this is a completed failure
    pub fn is_failure(&self) -> bool {
        matches!(self, SubagentDisplayStatus::Completed { success: false, .. })
    }

    /// Check if still in progress (started or progress)
    pub fn is_in_progress(&self) -> bool {
        matches!(self, SubagentDisplayStatus::Started { .. } | SubagentDisplayStatus::Progress { .. })
    }
}

/// State of a single subagent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubagentState {
    /// Unique identifier for this subagent
    pub subagent_id: String,
    /// Type of subagent (e.g., "Explore", "Plan", "Bash")
    pub subagent_type: String,
    /// Description of what the subagent is doing
    pub description: String,
    /// Display status for UI rendering (with timing)
    pub display_status: SubagentDisplayStatus,
    /// Count of tool calls made by this subagent (incremented during progress)
    pub tool_call_count: u32,
}

impl SubagentState {
    /// Create a new subagent state in started status
    pub fn new(subagent_id: String, subagent_type: String, description: String, started_at: u64) -> Self {
        Self {
            subagent_id,
            subagent_type,
            description: description.clone(),
            display_status: SubagentDisplayStatus::Started {
                description,
                started_at,
            },
            tool_call_count: 0,
        }
    }

    /// Update the subagent with a progress message
    pub fn set_progress(&mut self, message: String) {
        self.tool_call_count += 1;
        self.display_status = SubagentDisplayStatus::Progress {
            description: self.description.clone(),
            progress_message: message,
        };
    }

    /// Mark the subagent as completed
    pub fn complete(&mut self, success: bool, summary: String, completed_at: u64) {
        self.display_status = SubagentDisplayStatus::Completed {
            success,
            summary,
            completed_at,
        };
    }

    /// Check if the subagent is still active (not completed)
    pub fn is_active(&self) -> bool {
        self.display_status.is_in_progress()
    }

    /// Check if the subagent has finished (completed, success or failure)
    pub fn is_finished(&self) -> bool {
        matches!(self.display_status, SubagentDisplayStatus::Completed { .. })
    }
}

/// Tracks active subagents for a thread
///
/// This is ephemeral state that gets cleared when the thread's
/// streaming response completes (done event). It allows the UI
/// to show subagent activity during streaming.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubagentTracker {
    /// Active subagents indexed by subagent_id
    active_subagents: HashMap<String, SubagentState>,
}

impl SubagentTracker {
    /// Create a new empty SubagentTracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new subagent
    pub fn register_subagent(&mut self, subagent_id: String, subagent_type: String, description: String, current_tick: u64) {
        let state = SubagentState::new(subagent_id.clone(), subagent_type, description, current_tick);
        self.active_subagents.insert(subagent_id, state);
    }

    /// Get a subagent state by ID
    pub fn get_subagent(&self, subagent_id: &str) -> Option<&SubagentState> {
        self.active_subagents.get(subagent_id)
    }

    /// Get mutable access to a subagent state
    pub fn get_subagent_mut(&mut self, subagent_id: &str) -> Option<&mut SubagentState> {
        self.active_subagents.get_mut(subagent_id)
    }

    /// Update subagent with progress message
    pub fn update_progress(&mut self, subagent_id: &str, message: String) {
        if let Some(state) = self.active_subagents.get_mut(subagent_id) {
            state.set_progress(message);
        }
    }

    /// Complete a subagent
    pub fn complete_subagent(&mut self, subagent_id: &str, success: bool, summary: String, current_tick: u64) {
        if let Some(state) = self.active_subagents.get_mut(subagent_id) {
            state.complete(success, summary, current_tick);
        }
    }

    /// Remove a subagent from tracking
    pub fn remove_subagent(&mut self, subagent_id: &str) -> Option<SubagentState> {
        self.active_subagents.remove(subagent_id)
    }

    /// Clear all tracked subagents (called when thread done event arrives)
    pub fn clear(&mut self) {
        self.active_subagents.clear();
    }

    /// Get all active (not completed) subagents
    pub fn active_subagents(&self) -> Vec<(&String, &SubagentState)> {
        self.active_subagents
            .iter()
            .filter(|(_, state)| state.is_active())
            .collect()
    }

    /// Get all subagents (including finished ones)
    pub fn all_subagents(&self) -> &HashMap<String, SubagentState> {
        &self.active_subagents
    }

    /// Check if there are any active subagents
    pub fn has_active_subagents(&self) -> bool {
        self.active_subagents.values().any(|state| state.is_active())
    }

    /// Get the count of active subagents
    pub fn active_count(&self) -> usize {
        self.active_subagents
            .values()
            .filter(|state| state.is_active())
            .count()
    }

    /// Get the total count of tracked subagents
    pub fn total_count(&self) -> usize {
        self.active_subagents.len()
    }

    /// Check if a specific subagent is being tracked
    pub fn contains(&self, subagent_id: &str) -> bool {
        self.active_subagents.contains_key(subagent_id)
    }

    /// Get subagents that should be rendered at the given tick
    /// Returns subagents in order: in-progress first, then completed (newest first)
    pub fn subagents_to_render(&self, current_tick: u64) -> Vec<(&String, &SubagentState)> {
        let mut subagents: Vec<_> = self
            .active_subagents
            .iter()
            .filter(|(_, state)| state.display_status.should_render(current_tick))
            .collect();

        // Sort: in-progress first, then by recency (for completed)
        subagents.sort_by(|(_, a), (_, b)| {
            let a_in_progress = a.display_status.is_in_progress();
            let b_in_progress = b.display_status.is_in_progress();

            match (a_in_progress, b_in_progress) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });

        subagents
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============= ToolCallState Tests =============

    #[test]
    fn test_tool_call_state_new() {
        let state = ToolCallState::new("Bash".to_string());
        assert_eq!(state.tool_name, "Bash");
        assert_eq!(state.status, ToolCallStatus::Pending);
        assert!(state.input.is_none());
        assert!(state.output.is_none());
        assert!(state.error.is_none());
    }

    #[test]
    fn test_tool_call_state_with_input() {
        let state = ToolCallState::with_input("Bash".to_string(), "ls -la".to_string());
        assert_eq!(state.tool_name, "Bash");
        assert_eq!(state.status, ToolCallStatus::Pending);
        assert_eq!(state.input, Some("ls -la".to_string()));
    }

    #[test]
    fn test_tool_call_state_start() {
        let mut state = ToolCallState::new("Bash".to_string());
        assert_eq!(state.status, ToolCallStatus::Pending);
        state.start();
        assert_eq!(state.status, ToolCallStatus::Running);
    }

    #[test]
    fn test_tool_call_state_complete() {
        let mut state = ToolCallState::new("Bash".to_string());
        state.start();
        state.complete(Some("file1.txt\nfile2.txt".to_string()));
        assert_eq!(state.status, ToolCallStatus::Completed);
        assert_eq!(state.output, Some("file1.txt\nfile2.txt".to_string()));
    }

    #[test]
    fn test_tool_call_state_complete_without_output() {
        let mut state = ToolCallState::new("Bash".to_string());
        state.start();
        state.complete(None);
        assert_eq!(state.status, ToolCallStatus::Completed);
        assert!(state.output.is_none());
    }

    #[test]
    fn test_tool_call_state_fail() {
        let mut state = ToolCallState::new("Bash".to_string());
        state.start();
        state.fail("Command not found".to_string());
        assert_eq!(state.status, ToolCallStatus::Failed);
        assert_eq!(state.error, Some("Command not found".to_string()));
    }

    #[test]
    fn test_tool_call_state_is_active() {
        let mut state = ToolCallState::new("Bash".to_string());
        assert!(state.is_active()); // Pending is active

        state.start();
        assert!(state.is_active()); // Running is active

        state.complete(None);
        assert!(!state.is_active()); // Completed is not active
    }

    #[test]
    fn test_tool_call_state_is_finished() {
        let mut state = ToolCallState::new("Bash".to_string());
        assert!(!state.is_finished()); // Pending is not finished

        state.start();
        assert!(!state.is_finished()); // Running is not finished

        state.complete(None);
        assert!(state.is_finished()); // Completed is finished
    }

    #[test]
    fn test_tool_call_state_is_finished_failed() {
        let mut state = ToolCallState::new("Bash".to_string());
        state.fail("Error".to_string());
        assert!(state.is_finished()); // Failed is finished
    }

    #[test]
    fn test_tool_call_status_default() {
        assert_eq!(ToolCallStatus::default(), ToolCallStatus::Pending);
    }

    // ============= ToolTracker Tests =============

    #[test]
    fn test_tool_tracker_new() {
        let tracker = ToolTracker::new();
        assert_eq!(tracker.total_count(), 0);
        assert!(!tracker.has_active_tools());
    }

    #[test]
    fn test_tool_tracker_register_tool() {
        let mut tracker = ToolTracker::new();
        let state = ToolCallState::new("Bash".to_string());
        tracker.register_tool("call-1".to_string(), state);

        assert_eq!(tracker.total_count(), 1);
        assert!(tracker.contains("call-1"));
    }

    #[test]
    fn test_tool_tracker_get_tool() {
        let mut tracker = ToolTracker::new();
        let state = ToolCallState::new("Bash".to_string());
        tracker.register_tool("call-1".to_string(), state);

        let retrieved = tracker.get_tool("call-1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().tool_name, "Bash");
    }

    #[test]
    fn test_tool_tracker_get_tool_nonexistent() {
        let tracker = ToolTracker::new();
        assert!(tracker.get_tool("nonexistent").is_none());
    }

    #[test]
    fn test_tool_tracker_get_tool_mut() {
        let mut tracker = ToolTracker::new();
        let state = ToolCallState::new("Bash".to_string());
        tracker.register_tool("call-1".to_string(), state);

        if let Some(state) = tracker.get_tool_mut("call-1") {
            state.start();
        }

        let retrieved = tracker.get_tool("call-1").unwrap();
        assert_eq!(retrieved.status, ToolCallStatus::Running);
    }

    #[test]
    fn test_tool_tracker_start_tool() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));

        tracker.start_tool("call-1");

        let state = tracker.get_tool("call-1").unwrap();
        assert_eq!(state.status, ToolCallStatus::Running);
    }

    #[test]
    fn test_tool_tracker_complete_tool() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));
        tracker.start_tool("call-1");

        tracker.complete_tool("call-1", Some("output".to_string()));

        let state = tracker.get_tool("call-1").unwrap();
        assert_eq!(state.status, ToolCallStatus::Completed);
        assert_eq!(state.output, Some("output".to_string()));
    }

    #[test]
    fn test_tool_tracker_fail_tool() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));
        tracker.start_tool("call-1");

        tracker.fail_tool("call-1", "Error occurred".to_string());

        let state = tracker.get_tool("call-1").unwrap();
        assert_eq!(state.status, ToolCallStatus::Failed);
        assert_eq!(state.error, Some("Error occurred".to_string()));
    }

    #[test]
    fn test_tool_tracker_remove_tool() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));
        assert!(tracker.contains("call-1"));

        let removed = tracker.remove_tool("call-1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().tool_name, "Bash");
        assert!(!tracker.contains("call-1"));
    }

    #[test]
    fn test_tool_tracker_remove_nonexistent() {
        let mut tracker = ToolTracker::new();
        let removed = tracker.remove_tool("nonexistent");
        assert!(removed.is_none());
    }

    #[test]
    fn test_tool_tracker_clear() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));
        tracker.register_tool("call-2".to_string(), ToolCallState::new("Read".to_string()));
        assert_eq!(tracker.total_count(), 2);

        tracker.clear();
        assert_eq!(tracker.total_count(), 0);
        assert!(!tracker.contains("call-1"));
        assert!(!tracker.contains("call-2"));
    }

    #[test]
    fn test_tool_tracker_active_tools() {
        let mut tracker = ToolTracker::new();

        // Add some tools in different states
        let pending = ToolCallState::new("Bash".to_string());
        tracker.register_tool("call-1".to_string(), pending.clone());

        let mut running = ToolCallState::new("Read".to_string());
        running.start();
        tracker.register_tool("call-2".to_string(), running);

        let mut completed = ToolCallState::new("Write".to_string());
        completed.start();
        completed.complete(None);
        tracker.register_tool("call-3".to_string(), completed);

        let active = tracker.active_tools();
        assert_eq!(active.len(), 2); // pending and running
    }

    #[test]
    fn test_tool_tracker_has_active_tools() {
        let mut tracker = ToolTracker::new();
        assert!(!tracker.has_active_tools());

        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));
        assert!(tracker.has_active_tools());

        tracker.complete_tool("call-1", None);
        assert!(!tracker.has_active_tools());
    }

    #[test]
    fn test_tool_tracker_active_count() {
        let mut tracker = ToolTracker::new();
        assert_eq!(tracker.active_count(), 0);

        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));
        tracker.register_tool("call-2".to_string(), ToolCallState::new("Read".to_string()));
        assert_eq!(tracker.active_count(), 2);

        tracker.complete_tool("call-1", None);
        assert_eq!(tracker.active_count(), 1);

        tracker.fail_tool("call-2", "Error".to_string());
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn test_tool_tracker_total_count() {
        let mut tracker = ToolTracker::new();
        assert_eq!(tracker.total_count(), 0);

        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));
        assert_eq!(tracker.total_count(), 1);

        tracker.register_tool("call-2".to_string(), ToolCallState::new("Read".to_string()));
        assert_eq!(tracker.total_count(), 2);

        // Completing doesn't remove from total
        tracker.complete_tool("call-1", None);
        assert_eq!(tracker.total_count(), 2);
    }

    #[test]
    fn test_tool_tracker_workflow() {
        let mut tracker = ToolTracker::new();

        // Tool call starts
        tracker.register_tool(
            "tool-abc-123".to_string(),
            ToolCallState::with_input("Bash".to_string(), "npm install".to_string()),
        );
        assert!(tracker.has_active_tools());
        assert_eq!(tracker.active_count(), 1);

        // Tool starts running
        tracker.start_tool("tool-abc-123");
        let state = tracker.get_tool("tool-abc-123").unwrap();
        assert_eq!(state.status, ToolCallStatus::Running);

        // Tool completes
        tracker.complete_tool("tool-abc-123", Some("Installed 42 packages".to_string()));
        let state = tracker.get_tool("tool-abc-123").unwrap();
        assert_eq!(state.status, ToolCallStatus::Completed);
        assert_eq!(state.output, Some("Installed 42 packages".to_string()));
        assert!(!tracker.has_active_tools());

        // Thread done event clears all tools
        tracker.clear();
        assert_eq!(tracker.total_count(), 0);
    }

    #[test]
    fn test_tool_tracker_serialization() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool(
            "call-1".to_string(),
            ToolCallState::with_input("Bash".to_string(), "ls".to_string()),
        );

        let json = serde_json::to_string(&tracker).expect("Failed to serialize");
        let deserialized: ToolTracker =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(tracker.total_count(), deserialized.total_count());
        assert!(deserialized.contains("call-1"));
    }

    #[test]
    fn test_tool_call_state_serialization() {
        let state = ToolCallState::with_input("Bash".to_string(), "npm test".to_string());

        let json = serde_json::to_string(&state).expect("Failed to serialize");
        let deserialized: ToolCallState =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(state, deserialized);
    }

    // ============= ToolDisplayStatus Tests =============

    #[test]
    fn test_tool_display_status_started_should_render() {
        let status = ToolDisplayStatus::Started {
            function: "Bash".to_string(),
            started_at: 10,
        };
        // Started status always renders regardless of current tick
        assert!(status.should_render(10));
        assert!(status.should_render(50));
        assert!(status.should_render(100));
    }

    #[test]
    fn test_tool_display_status_executing_should_render() {
        let status = ToolDisplayStatus::Executing {
            display_name: "Running npm install".to_string(),
        };
        // Executing status always renders
        assert!(status.should_render(0));
        assert!(status.should_render(100));
    }

    #[test]
    fn test_tool_display_status_completed_success_fades() {
        let status = ToolDisplayStatus::Completed {
            success: true,
            summary: "Success".to_string(),
            completed_at: 100,
        };
        // Success should render within 30 ticks
        assert!(status.should_render(100)); // At completion
        assert!(status.should_render(120)); // 20 ticks later
        assert!(status.should_render(129)); // 29 ticks later
        // Should not render after 30 ticks
        assert!(!status.should_render(130)); // 30 ticks later
        assert!(!status.should_render(150)); // 50 ticks later
    }

    #[test]
    fn test_tool_display_status_completed_failure_persists() {
        let status = ToolDisplayStatus::Completed {
            success: false,
            summary: "Error: File not found".to_string(),
            completed_at: 100,
        };
        // Failures always persist regardless of tick
        assert!(status.should_render(100));
        assert!(status.should_render(130)); // 30 ticks later
        assert!(status.should_render(200)); // 100 ticks later
        assert!(status.should_render(1000)); // 900 ticks later
    }

    #[test]
    fn test_tool_display_status_display_text_started() {
        let status = ToolDisplayStatus::Started {
            function: "Bash".to_string(),
            started_at: 10,
        };
        assert_eq!(status.display_text(), "Bash...");
    }

    #[test]
    fn test_tool_display_status_display_text_executing() {
        let status = ToolDisplayStatus::Executing {
            display_name: "Installing packages".to_string(),
        };
        assert_eq!(status.display_text(), "Installing packages");
    }

    #[test]
    fn test_tool_display_status_display_text_completed() {
        let status = ToolDisplayStatus::Completed {
            success: true,
            summary: "Installed 42 packages".to_string(),
            completed_at: 100,
        };
        assert_eq!(status.display_text(), "Installed 42 packages");
    }

    #[test]
    fn test_tool_display_status_is_success() {
        let success = ToolDisplayStatus::Completed {
            success: true,
            summary: "Done".to_string(),
            completed_at: 100,
        };
        assert!(success.is_success());

        let failure = ToolDisplayStatus::Completed {
            success: false,
            summary: "Error".to_string(),
            completed_at: 100,
        };
        assert!(!failure.is_success());

        let started = ToolDisplayStatus::Started {
            function: "Bash".to_string(),
            started_at: 10,
        };
        assert!(!started.is_success());
    }

    #[test]
    fn test_tool_display_status_is_failure() {
        let failure = ToolDisplayStatus::Completed {
            success: false,
            summary: "Error".to_string(),
            completed_at: 100,
        };
        assert!(failure.is_failure());

        let success = ToolDisplayStatus::Completed {
            success: true,
            summary: "Done".to_string(),
            completed_at: 100,
        };
        assert!(!success.is_failure());

        let executing = ToolDisplayStatus::Executing {
            display_name: "Running".to_string(),
        };
        assert!(!executing.is_failure());
    }

    #[test]
    fn test_tool_display_status_is_in_progress() {
        let started = ToolDisplayStatus::Started {
            function: "Bash".to_string(),
            started_at: 10,
        };
        assert!(started.is_in_progress());

        let executing = ToolDisplayStatus::Executing {
            display_name: "Running".to_string(),
        };
        assert!(executing.is_in_progress());

        let completed = ToolDisplayStatus::Completed {
            success: true,
            summary: "Done".to_string(),
            completed_at: 100,
        };
        assert!(!completed.is_in_progress());
    }

    #[test]
    fn test_tool_call_state_with_display() {
        let display = ToolDisplayStatus::Started {
            function: "Read".to_string(),
            started_at: 50,
        };
        let state = ToolCallState::with_display("Read".to_string(), display.clone());

        assert_eq!(state.tool_name, "Read");
        assert_eq!(state.status, ToolCallStatus::Pending);
        assert!(state.display_status.is_some());
        assert_eq!(state.display_status.unwrap(), display);
    }

    #[test]
    fn test_tool_call_state_set_display_status() {
        let mut state = ToolCallState::new("Write".to_string());
        assert!(state.display_status.is_none());

        let display = ToolDisplayStatus::Executing {
            display_name: "Writing file".to_string(),
        };
        state.set_display_status(display.clone());

        assert!(state.display_status.is_some());
        assert_eq!(state.display_status.unwrap(), display);
    }

    #[test]
    fn test_tool_tracker_set_display_status() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));

        let display = ToolDisplayStatus::Executing {
            display_name: "Running tests".to_string(),
        };
        tracker.set_display_status("call-1", display.clone());

        let state = tracker.get_tool("call-1").unwrap();
        assert!(state.display_status.is_some());
        assert_eq!(state.display_status.as_ref().unwrap(), &display);
    }

    #[test]
    fn test_tool_tracker_tools_to_render() {
        let mut tracker = ToolTracker::new();

        // Add a started tool
        let mut state1 = ToolCallState::new("Bash".to_string());
        state1.set_display_status(ToolDisplayStatus::Started {
            function: "Bash".to_string(),
            started_at: 10,
        });
        tracker.register_tool("call-1".to_string(), state1);

        // Add a completed success (within fade window)
        let mut state2 = ToolCallState::new("Read".to_string());
        state2.set_display_status(ToolDisplayStatus::Completed {
            success: true,
            summary: "Read file".to_string(),
            completed_at: 50,
        });
        tracker.register_tool("call-2".to_string(), state2);

        // Add a completed failure
        let mut state3 = ToolCallState::new("Write".to_string());
        state3.set_display_status(ToolDisplayStatus::Completed {
            success: false,
            summary: "Error writing".to_string(),
            completed_at: 30,
        });
        tracker.register_tool("call-3".to_string(), state3);

        // At tick 60, all three should render (success is at tick 50+30=80, so still visible)
        let to_render = tracker.tools_to_render(60);
        assert_eq!(to_render.len(), 3); // started, success, and failure

        // At tick 90, only started and failure should render (success has faded at tick 80)
        let to_render = tracker.tools_to_render(90);
        assert_eq!(to_render.len(), 2); // started and failure
    }

    #[test]
    fn test_tool_tracker_tools_to_render_ordering() {
        let mut tracker = ToolTracker::new();

        // Add completed success
        let mut state1 = ToolCallState::new("Read".to_string());
        state1.set_display_status(ToolDisplayStatus::Completed {
            success: true,
            summary: "Done".to_string(),
            completed_at: 50,
        });
        tracker.register_tool("call-1".to_string(), state1);

        // Add in-progress
        let mut state2 = ToolCallState::new("Bash".to_string());
        state2.set_display_status(ToolDisplayStatus::Executing {
            display_name: "Running".to_string(),
        });
        tracker.register_tool("call-2".to_string(), state2);

        let to_render = tracker.tools_to_render(60);
        assert_eq!(to_render.len(), 2);

        // In-progress should be first
        let first_tool = to_render[0].1;
        assert!(first_tool.display_status.as_ref().unwrap().is_in_progress());
    }

    #[test]
    fn test_tool_tracker_register_tool_started() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool_started("call-1".to_string(), "Bash".to_string(), 42);

        let state = tracker.get_tool("call-1").unwrap();
        assert_eq!(state.tool_name, "Bash");
        assert!(state.display_status.is_some());

        let display = state.display_status.as_ref().unwrap();
        assert!(display.is_in_progress());
        assert_eq!(display.display_text(), "Bash...");
    }

    #[test]
    fn test_tool_tracker_set_tool_executing() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));

        tracker.set_tool_executing("call-1", "Installing dependencies".to_string());

        let state = tracker.get_tool("call-1").unwrap();
        assert_eq!(state.status, ToolCallStatus::Running);
        assert!(state.display_status.is_some());
        assert_eq!(state.display_status.as_ref().unwrap().display_text(), "Installing dependencies");
    }

    #[test]
    fn test_tool_tracker_complete_tool_with_summary() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));

        tracker.complete_tool_with_summary("call-1", true, "Installed 5 packages".to_string(), 100);

        let state = tracker.get_tool("call-1").unwrap();
        assert_eq!(state.status, ToolCallStatus::Completed);

        let display = state.display_status.as_ref().unwrap();
        assert!(display.is_success());
        assert_eq!(display.display_text(), "Installed 5 packages");
    }

    #[test]
    fn test_tool_tracker_complete_tool_with_summary_failure() {
        let mut tracker = ToolTracker::new();
        tracker.register_tool("call-1".to_string(), ToolCallState::new("Bash".to_string()));

        tracker.complete_tool_with_summary("call-1", false, "Command failed".to_string(), 100);

        let state = tracker.get_tool("call-1").unwrap();
        assert_eq!(state.status, ToolCallStatus::Failed);

        let display = state.display_status.as_ref().unwrap();
        assert!(display.is_failure());
        assert_eq!(display.display_text(), "Command failed");
    }

    #[test]
    fn test_tool_display_status_serialization() {
        let status = ToolDisplayStatus::Executing {
            display_name: "Running test".to_string(),
        };

        let json = serde_json::to_string(&status).expect("Failed to serialize");
        let deserialized: ToolDisplayStatus =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(status, deserialized);
    }

    // ============= SubagentDisplayStatus Tests =============

    #[test]
    fn test_subagent_display_status_started_should_render() {
        let status = SubagentDisplayStatus::Started {
            description: "Exploring codebase".to_string(),
            started_at: 10,
        };
        // Started status always renders regardless of current tick
        assert!(status.should_render(10));
        assert!(status.should_render(50));
        assert!(status.should_render(100));
    }

    #[test]
    fn test_subagent_display_status_progress_should_render() {
        let status = SubagentDisplayStatus::Progress {
            description: "Exploring codebase".to_string(),
            progress_message: "Found 5 files".to_string(),
        };
        // Progress status always renders
        assert!(status.should_render(0));
        assert!(status.should_render(100));
    }

    #[test]
    fn test_subagent_display_status_completed_success_fades() {
        let status = SubagentDisplayStatus::Completed {
            success: true,
            summary: "Complete (8 tool calls)".to_string(),
            completed_at: 100,
        };
        // Success should render within 30 ticks
        assert!(status.should_render(100)); // At completion
        assert!(status.should_render(120)); // 20 ticks later
        assert!(status.should_render(129)); // 29 ticks later
        // Should not render after 30 ticks
        assert!(!status.should_render(130)); // 30 ticks later
        assert!(!status.should_render(150)); // 50 ticks later
    }

    #[test]
    fn test_subagent_display_status_completed_failure_persists() {
        let status = SubagentDisplayStatus::Completed {
            success: false,
            summary: "Failed: Timeout".to_string(),
            completed_at: 100,
        };
        // Failures always persist regardless of tick
        assert!(status.should_render(100));
        assert!(status.should_render(130));
        assert!(status.should_render(200));
        assert!(status.should_render(1000));
    }

    #[test]
    fn test_subagent_display_status_description() {
        let started = SubagentDisplayStatus::Started {
            description: "Exploring codebase".to_string(),
            started_at: 10,
        };
        assert_eq!(started.description(), "Exploring codebase");

        let progress = SubagentDisplayStatus::Progress {
            description: "Exploring codebase".to_string(),
            progress_message: "Found 5 files".to_string(),
        };
        assert_eq!(progress.description(), "Exploring codebase");

        let completed = SubagentDisplayStatus::Completed {
            success: true,
            summary: "Complete (8 tool calls)".to_string(),
            completed_at: 100,
        };
        assert_eq!(completed.description(), "Complete (8 tool calls)");
    }

    #[test]
    fn test_subagent_display_status_progress_message() {
        let started = SubagentDisplayStatus::Started {
            description: "Exploring".to_string(),
            started_at: 10,
        };
        assert!(started.progress_message().is_none());

        let progress = SubagentDisplayStatus::Progress {
            description: "Exploring".to_string(),
            progress_message: "Found 5 files".to_string(),
        };
        assert_eq!(progress.progress_message(), Some("Found 5 files"));

        let completed = SubagentDisplayStatus::Completed {
            success: true,
            summary: "Done".to_string(),
            completed_at: 100,
        };
        assert!(completed.progress_message().is_none());
    }

    #[test]
    fn test_subagent_display_status_is_success() {
        let success = SubagentDisplayStatus::Completed {
            success: true,
            summary: "Done".to_string(),
            completed_at: 100,
        };
        assert!(success.is_success());

        let failure = SubagentDisplayStatus::Completed {
            success: false,
            summary: "Error".to_string(),
            completed_at: 100,
        };
        assert!(!failure.is_success());

        let started = SubagentDisplayStatus::Started {
            description: "Exploring".to_string(),
            started_at: 10,
        };
        assert!(!started.is_success());
    }

    #[test]
    fn test_subagent_display_status_is_failure() {
        let failure = SubagentDisplayStatus::Completed {
            success: false,
            summary: "Error".to_string(),
            completed_at: 100,
        };
        assert!(failure.is_failure());

        let success = SubagentDisplayStatus::Completed {
            success: true,
            summary: "Done".to_string(),
            completed_at: 100,
        };
        assert!(!success.is_failure());

        let progress = SubagentDisplayStatus::Progress {
            description: "Exploring".to_string(),
            progress_message: "Found files".to_string(),
        };
        assert!(!progress.is_failure());
    }

    #[test]
    fn test_subagent_display_status_is_in_progress() {
        let started = SubagentDisplayStatus::Started {
            description: "Exploring".to_string(),
            started_at: 10,
        };
        assert!(started.is_in_progress());

        let progress = SubagentDisplayStatus::Progress {
            description: "Exploring".to_string(),
            progress_message: "Found files".to_string(),
        };
        assert!(progress.is_in_progress());

        let completed = SubagentDisplayStatus::Completed {
            success: true,
            summary: "Done".to_string(),
            completed_at: 100,
        };
        assert!(!completed.is_in_progress());
    }

    // ============= SubagentState Tests =============

    #[test]
    fn test_subagent_state_new() {
        let state = SubagentState::new(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring codebase".to_string(),
            42,
        );
        assert_eq!(state.subagent_id, "agent-1");
        assert_eq!(state.subagent_type, "Explore");
        assert_eq!(state.description, "Exploring codebase");
        assert_eq!(state.tool_call_count, 0);
        assert!(state.is_active());
        assert!(!state.is_finished());
    }

    #[test]
    fn test_subagent_state_set_progress() {
        let mut state = SubagentState::new(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring codebase".to_string(),
            42,
        );

        state.set_progress("Found 5 files".to_string());
        assert_eq!(state.tool_call_count, 1);
        assert!(state.is_active());

        if let SubagentDisplayStatus::Progress { progress_message, .. } = &state.display_status {
            assert_eq!(progress_message, "Found 5 files");
        } else {
            panic!("Expected Progress status");
        }

        // Second progress update
        state.set_progress("Analyzing files".to_string());
        assert_eq!(state.tool_call_count, 2);
    }

    #[test]
    fn test_subagent_state_complete() {
        let mut state = SubagentState::new(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring codebase".to_string(),
            42,
        );

        state.complete(true, "Complete (8 tool calls)".to_string(), 100);
        assert!(!state.is_active());
        assert!(state.is_finished());
        assert!(state.display_status.is_success());
    }

    #[test]
    fn test_subagent_state_complete_failure() {
        let mut state = SubagentState::new(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring codebase".to_string(),
            42,
        );

        state.complete(false, "Failed: Timeout".to_string(), 100);
        assert!(!state.is_active());
        assert!(state.is_finished());
        assert!(state.display_status.is_failure());
    }

    // ============= SubagentTracker Tests =============

    #[test]
    fn test_subagent_tracker_new() {
        let tracker = SubagentTracker::new();
        assert_eq!(tracker.total_count(), 0);
        assert!(!tracker.has_active_subagents());
    }

    #[test]
    fn test_subagent_tracker_register() {
        let mut tracker = SubagentTracker::new();
        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring codebase".to_string(),
            42,
        );

        assert_eq!(tracker.total_count(), 1);
        assert!(tracker.contains("agent-1"));
        assert!(tracker.has_active_subagents());
    }

    #[test]
    fn test_subagent_tracker_get_subagent() {
        let mut tracker = SubagentTracker::new();
        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring codebase".to_string(),
            42,
        );

        let state = tracker.get_subagent("agent-1").unwrap();
        assert_eq!(state.subagent_id, "agent-1");
        assert_eq!(state.subagent_type, "Explore");
    }

    #[test]
    fn test_subagent_tracker_get_subagent_nonexistent() {
        let tracker = SubagentTracker::new();
        assert!(tracker.get_subagent("nonexistent").is_none());
    }

    #[test]
    fn test_subagent_tracker_update_progress() {
        let mut tracker = SubagentTracker::new();
        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring codebase".to_string(),
            42,
        );

        tracker.update_progress("agent-1", "Found 5 files".to_string());

        let state = tracker.get_subagent("agent-1").unwrap();
        assert_eq!(state.tool_call_count, 1);
        if let SubagentDisplayStatus::Progress { progress_message, .. } = &state.display_status {
            assert_eq!(progress_message, "Found 5 files");
        } else {
            panic!("Expected Progress status");
        }
    }

    #[test]
    fn test_subagent_tracker_complete() {
        let mut tracker = SubagentTracker::new();
        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring codebase".to_string(),
            42,
        );

        tracker.complete_subagent("agent-1", true, "Complete (8 tool calls)".to_string(), 100);

        let state = tracker.get_subagent("agent-1").unwrap();
        assert!(!state.is_active());
        assert!(state.display_status.is_success());
        assert!(!tracker.has_active_subagents());
    }

    #[test]
    fn test_subagent_tracker_clear() {
        let mut tracker = SubagentTracker::new();
        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring".to_string(),
            42,
        );
        tracker.register_subagent(
            "agent-2".to_string(),
            "Plan".to_string(),
            "Planning".to_string(),
            43,
        );
        assert_eq!(tracker.total_count(), 2);

        tracker.clear();
        assert_eq!(tracker.total_count(), 0);
        assert!(!tracker.contains("agent-1"));
        assert!(!tracker.contains("agent-2"));
    }

    #[test]
    fn test_subagent_tracker_active_subagents() {
        let mut tracker = SubagentTracker::new();

        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring".to_string(),
            42,
        );
        tracker.register_subagent(
            "agent-2".to_string(),
            "Plan".to_string(),
            "Planning".to_string(),
            43,
        );

        // Complete one
        tracker.complete_subagent("agent-1", true, "Done".to_string(), 100);

        let active = tracker.active_subagents();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].1.subagent_id, "agent-2");
    }

    #[test]
    fn test_subagent_tracker_active_count() {
        let mut tracker = SubagentTracker::new();
        assert_eq!(tracker.active_count(), 0);

        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring".to_string(),
            42,
        );
        tracker.register_subagent(
            "agent-2".to_string(),
            "Plan".to_string(),
            "Planning".to_string(),
            43,
        );
        assert_eq!(tracker.active_count(), 2);

        tracker.complete_subagent("agent-1", true, "Done".to_string(), 100);
        assert_eq!(tracker.active_count(), 1);

        tracker.complete_subagent("agent-2", false, "Failed".to_string(), 101);
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn test_subagent_tracker_subagents_to_render() {
        let mut tracker = SubagentTracker::new();

        // Add a started subagent
        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring".to_string(),
            10,
        );

        // Add a completed success (within fade window)
        tracker.register_subagent(
            "agent-2".to_string(),
            "Plan".to_string(),
            "Planning".to_string(),
            20,
        );
        tracker.complete_subagent("agent-2", true, "Done".to_string(), 50);

        // Add a completed failure
        tracker.register_subagent(
            "agent-3".to_string(),
            "Bash".to_string(),
            "Running command".to_string(),
            25,
        );
        tracker.complete_subagent("agent-3", false, "Error".to_string(), 30);

        // At tick 60, all three should render (success is at tick 50+30=80, so still visible)
        let to_render = tracker.subagents_to_render(60);
        assert_eq!(to_render.len(), 3);

        // At tick 90, only started and failure should render (success has faded at tick 80)
        let to_render = tracker.subagents_to_render(90);
        assert_eq!(to_render.len(), 2);
    }

    #[test]
    fn test_subagent_tracker_subagents_to_render_ordering() {
        let mut tracker = SubagentTracker::new();

        // Add completed success
        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring".to_string(),
            10,
        );
        tracker.complete_subagent("agent-1", true, "Done".to_string(), 50);

        // Add in-progress
        tracker.register_subagent(
            "agent-2".to_string(),
            "Plan".to_string(),
            "Planning".to_string(),
            20,
        );

        let to_render = tracker.subagents_to_render(60);
        assert_eq!(to_render.len(), 2);

        // In-progress should be first
        let first = to_render[0].1;
        assert!(first.display_status.is_in_progress());
    }

    #[test]
    fn test_subagent_tracker_remove() {
        let mut tracker = SubagentTracker::new();
        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring".to_string(),
            42,
        );
        assert!(tracker.contains("agent-1"));

        let removed = tracker.remove_subagent("agent-1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().subagent_id, "agent-1");
        assert!(!tracker.contains("agent-1"));
    }

    #[test]
    fn test_subagent_tracker_workflow() {
        let mut tracker = SubagentTracker::new();

        // Subagent starts
        tracker.register_subagent(
            "agent-abc-123".to_string(),
            "Explore".to_string(),
            "Exploring codebase structure".to_string(),
            10,
        );
        assert!(tracker.has_active_subagents());
        assert_eq!(tracker.active_count(), 1);

        // Subagent reports progress
        tracker.update_progress("agent-abc-123", "Found 5 relevant files".to_string());
        let state = tracker.get_subagent("agent-abc-123").unwrap();
        assert_eq!(state.tool_call_count, 1);

        // More progress
        tracker.update_progress("agent-abc-123", "Analyzing patterns".to_string());
        let state = tracker.get_subagent("agent-abc-123").unwrap();
        assert_eq!(state.tool_call_count, 2);

        // Subagent completes
        tracker.complete_subagent(
            "agent-abc-123",
            true,
            "Complete (8 tool calls)".to_string(),
            100,
        );
        let state = tracker.get_subagent("agent-abc-123").unwrap();
        assert!(!state.is_active());
        assert!(state.display_status.is_success());
        assert!(!tracker.has_active_subagents());

        // Thread done event clears all subagents
        tracker.clear();
        assert_eq!(tracker.total_count(), 0);
    }

    #[test]
    fn test_subagent_display_status_serialization() {
        let status = SubagentDisplayStatus::Progress {
            description: "Exploring".to_string(),
            progress_message: "Found files".to_string(),
        };

        let json = serde_json::to_string(&status).expect("Failed to serialize");
        let deserialized: SubagentDisplayStatus =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_subagent_state_serialization() {
        let state = SubagentState::new(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring".to_string(),
            42,
        );

        let json = serde_json::to_string(&state).expect("Failed to serialize");
        let deserialized: SubagentState =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_subagent_tracker_serialization() {
        let mut tracker = SubagentTracker::new();
        tracker.register_subagent(
            "agent-1".to_string(),
            "Explore".to_string(),
            "Exploring".to_string(),
            42,
        );

        let json = serde_json::to_string(&tracker).expect("Failed to serialize");
        let deserialized: SubagentTracker =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(tracker.total_count(), deserialized.total_count());
        assert!(deserialized.contains("agent-1"));
    }
}
