//! Touch interaction system for SPOQ TUI.
//!
//! This module previously provided a registry-based system for handling clickable regions
//! in the terminal UI. The click/hover interaction system has been removed in favor of
//! keyboard-only navigation.
//!
//! This module is now empty and retained for future extensibility.

/// Stub type for removed click action system (retained for backward compatibility)
#[derive(Debug, Clone)]
pub enum ClickAction {
    /// Approve a thread
    ApproveThread(String),
    /// Reject a thread
    RejectThread(String),
    /// Allow tool always
    AllowToolAlways(String),
    /// Verify thread
    VerifyThread(String),
    /// Expand thread
    ExpandThread { thread_id: String },
    /// Hover info icon
    HoverInfoIcon { content: String },
    /// Select option
    SelectOption { thread_id: String, option_index: usize },
    /// Show free form input
    ShowFreeFormInput(String),
    /// Back to options
    BackToOptions(String),
    /// Submit free form
    SubmitFreeForm(String),
    /// View full plan
    ViewFullPlan(String),
    /// Collapse overlay
    CollapseOverlay,
    /// Clear filter
    ClearFilter,
}

/// Stub type for removed hit area registry (retained for backward compatibility)
#[derive(Debug, Default)]
pub struct HitAreaRegistry {
    _unused: (),
}

impl HitAreaRegistry {
    /// Create a new registry (no-op stub)
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a clickable area (no-op stub)
    #[allow(unused_variables)]
    pub fn register(&mut self, area: ratatui::layout::Rect, action: ClickAction, metadata: Option<String>) {
        // No-op: interaction system removed
    }

    /// Get the number of registered areas (always returns 0 for stub)
    pub fn len(&self) -> usize {
        0
    }

    /// Check if registry is empty (always true for stub)
    pub fn is_empty(&self) -> bool {
        true
    }
}
