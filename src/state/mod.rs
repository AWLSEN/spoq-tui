//! Application state management
//!
//! This module contains state containers for the TUI application:
//! - Core types (Thread, Task, Notification) from legacy state
//! - SessionState: Session-level information (skills, tokens, permissions)
//! - ToolTracker: Per-thread ephemeral tool execution states

pub mod session;
pub mod tools;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Re-export new state types at module level
pub use session::SessionState;
pub use tools::{ToolCallState, ToolTracker};

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
