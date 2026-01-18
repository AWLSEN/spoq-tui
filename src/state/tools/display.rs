//! Display status types for tool and subagent UI rendering
//!
//! These types provide timing-aware display states for rendering
//! tool calls and subagents in the UI with fade-out behavior.

use serde::{Deserialize, Serialize};

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

/// Display status for subagent UI rendering with timing info
///
/// Subagents are displayed with a spinner (not progress bar since we don't have percentage).
/// UI design:
/// ```text
/// +-  Exploring codebase structure
/// |   Found 5 relevant files...
/// +- Complete (8 tool calls)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
