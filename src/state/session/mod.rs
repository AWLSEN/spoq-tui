//! Session-level state management
//!
//! This module contains state structures for session-level information:
//! - Question data structures for the AskUserQuestion tool
//! - Question UI state machine for tracking user interactions
//! - Session state for skills, OAuth, permissions, and allowed tools

mod question_data;
mod question_state;
mod session;

// Re-export all public types at module level
pub use question_data::{AskUserQuestionData, Question, QuestionOption};
pub use question_state::AskUserQuestionState;
pub use session::{PermissionRequest, SessionState};
