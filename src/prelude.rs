//! Prelude module for convenient imports.
//!
//! This module re-exports commonly used types from the spoq library,
//! providing a convenient way to import the most frequently used items.
//!
//! # Usage
//!
//! ```ignore
//! use spoq::prelude::*;
//! ```
//!
//! This will import:
//! - Core application types (App, Screen, Focus, AppMessage)
//! - Model types (Thread, Message, ThreadType, MessageRole)
//! - Cache (ThreadCache)
//! - State types (SessionState, DashboardState)
//! - UI types (render function, LayoutContext)
//! - Widget types (TextAreaInput)

// Core application types
pub use crate::app::{App, AppMessage, Focus, Screen, ScrollBoundary};

// Model types
pub use crate::models::{
    ErrorInfo, Folder, Message, MessageRole, MessageSegment, PermissionMode, StreamRequest, Thread,
    ThreadMode, ThreadType, ToolEvent, ToolEventStatus,
};

// Cache
pub use crate::cache::ThreadCache;

// State types
pub use crate::state::{
    AskUserQuestionState, DashboardState, SessionState, SubagentTracker, ToolTracker,
};

// UI types
pub use crate::ui::{render, LayoutContext};

// Widget types
pub use crate::widgets::TextAreaInput;
