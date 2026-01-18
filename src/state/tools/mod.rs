//! Tool and subagent execution state tracking
//!
//! This module contains:
//! - `display` - Display status types for UI rendering with fade-out behavior
//! - `tool_call` - ToolCallState and ToolTracker for tool call management
//! - `subagent` - SubagentState and SubagentTracker for subagent management
//!
//! Both trackers manage ephemeral state that is cleared when the thread's
//! "done" event arrives.

mod display;
mod subagent;
mod tool_call;

// Re-export display types
pub use display::{SubagentDisplayStatus, ToolDisplayStatus};

// Re-export tool call types
pub use tool_call::{ToolCallState, ToolCallStatus, ToolTracker};

// Re-export subagent types
pub use subagent::{SubagentState, SubagentTracker};
