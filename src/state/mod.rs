//! Application state management
//!
//! This module contains state containers for the TUI application:
//! - Core types (Thread, Task, Notification) from legacy state
//! - SessionState: Session-level information (skills, tokens, permissions)
//! - ToolTracker: Per-thread ephemeral tool execution states
//! - SubagentTracker: Per-thread ephemeral subagent execution states

pub mod session;
pub mod tools;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Re-export new state types at module level
pub use session::{
    AskUserQuestionData, AskUserQuestionState, PermissionRequest, Question, QuestionOption,
    SessionState,
};
pub use tools::{SubagentDisplayStatus, SubagentState, SubagentTracker, ToolCallState, ToolCallStatus, ToolDisplayStatus, ToolTracker};

/// Represents a conversation thread
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub created_at: DateTime<Utc>,
}

/// Task status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

/// Represents a task in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,
    pub progress: f32, // 0.0 to 1.0
}

/// Represents a system notification
/// Note: Planned for Phase 3+ (UI notifications)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub timestamp: DateTime<Utc>,
    pub message: String,
}

/// Todo item status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// Represents a todo item from the assistant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    /// The todo item content/description
    pub content: String,
    /// Active form shown when in_progress (e.g., "Running tests")
    pub active_form: String,
    /// Current status of the todo
    pub status: TodoStatus,
}

impl Todo {
    /// Parse a TodoStatus from a string (from SSE event)
    pub fn parse_status(s: &str) -> TodoStatus {
        match s {
            "in_progress" => TodoStatus::InProgress,
            "completed" => TodoStatus::Completed,
            _ => TodoStatus::Pending,
        }
    }

    /// Create a Todo from an SSE TodoItem
    pub fn from_sse(item: &crate::events::TodoItem) -> Self {
        Self {
            content: item.content.clone(),
            active_form: item.active_form.clone().unwrap_or_else(|| item.content.clone()),
            status: Self::parse_status(&item.status),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_pending() {
        assert_eq!(Todo::parse_status("pending"), TodoStatus::Pending);
    }

    #[test]
    fn test_parse_status_in_progress() {
        assert_eq!(Todo::parse_status("in_progress"), TodoStatus::InProgress);
    }

    #[test]
    fn test_parse_status_completed() {
        assert_eq!(Todo::parse_status("completed"), TodoStatus::Completed);
    }

    #[test]
    fn test_parse_status_unknown_defaults_to_pending() {
        assert_eq!(Todo::parse_status("unknown"), TodoStatus::Pending);
        assert_eq!(Todo::parse_status(""), TodoStatus::Pending);
    }

    #[test]
    fn test_from_sse_with_active_form() {
        let item = crate::events::TodoItem {
            content: "Run tests".to_string(),
            active_form: Some("Running tests".to_string()),
            status: "in_progress".to_string(),
        };

        let todo = Todo::from_sse(&item);
        assert_eq!(todo.content, "Run tests");
        assert_eq!(todo.active_form, "Running tests");
        assert_eq!(todo.status, TodoStatus::InProgress);
    }

    #[test]
    fn test_from_sse_without_active_form() {
        let item = crate::events::TodoItem {
            content: "Fix the bug".to_string(),
            active_form: None,
            status: "pending".to_string(),
        };

        let todo = Todo::from_sse(&item);
        assert_eq!(todo.content, "Fix the bug");
        assert_eq!(todo.active_form, "Fix the bug"); // Should default to content
        assert_eq!(todo.status, TodoStatus::Pending);
    }

    #[test]
    fn test_from_sse_completed_status() {
        let item = crate::events::TodoItem {
            content: "Write documentation".to_string(),
            active_form: Some("Writing documentation".to_string()),
            status: "completed".to_string(),
        };

        let todo = Todo::from_sse(&item);
        assert_eq!(todo.content, "Write documentation");
        assert_eq!(todo.status, TodoStatus::Completed);
    }

    #[test]
    fn test_todo_status_serialization() {
        let json = serde_json::to_string(&TodoStatus::InProgress).unwrap();
        assert_eq!(json, "\"in_progress\"");

        let json = serde_json::to_string(&TodoStatus::Pending).unwrap();
        assert_eq!(json, "\"pending\"");

        let json = serde_json::to_string(&TodoStatus::Completed).unwrap();
        assert_eq!(json, "\"completed\"");
    }

    #[test]
    fn test_todo_status_deserialization() {
        let status: TodoStatus = serde_json::from_str("\"in_progress\"").unwrap();
        assert_eq!(status, TodoStatus::InProgress);

        let status: TodoStatus = serde_json::from_str("\"pending\"").unwrap();
        assert_eq!(status, TodoStatus::Pending);

        let status: TodoStatus = serde_json::from_str("\"completed\"").unwrap();
        assert_eq!(status, TodoStatus::Completed);
    }
}
