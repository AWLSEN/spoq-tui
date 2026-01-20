//! Subagent state and tracking
//!
//! SubagentState represents a single subagent's execution state.
//! SubagentTracker manages ephemeral state for active subagents within a thread.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::display::SubagentDisplayStatus;

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
    pub fn new(
        subagent_id: String,
        subagent_type: String,
        description: String,
        started_at: u64,
    ) -> Self {
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
    pub fn register_subagent(
        &mut self,
        subagent_id: String,
        subagent_type: String,
        description: String,
        current_tick: u64,
    ) {
        let state = SubagentState::new(
            subagent_id.clone(),
            subagent_type,
            description,
            current_tick,
        );
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
    pub fn complete_subagent(
        &mut self,
        subagent_id: &str,
        success: bool,
        summary: String,
        current_tick: u64,
    ) {
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
        self.active_subagents
            .values()
            .any(|state| state.is_active())
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

        if let SubagentDisplayStatus::Progress {
            progress_message, ..
        } = &state.display_status
        {
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
        if let SubagentDisplayStatus::Progress {
            progress_message, ..
        } = &state.display_status
        {
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
