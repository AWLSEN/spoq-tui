//! Type definitions for the application state.
//!
//! Contains enums and structs used for tracking UI state:
//! - [`Screen`] - Which screen is currently displayed
//! - [`Focus`] - Which UI component has focus
//! - [`ScrollBoundary`] - Scroll boundary hit state
//! - [`ThreadSwitcher`] - Thread switcher dialog state

/// Represents which screen is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    CommandDeck,
    Conversation,
}

/// Represents which UI component has focus
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Focus {
    #[default]
    Threads,
    Input,
}

/// Thread switcher dialog state (Tab to open)
#[derive(Debug, Clone, Default)]
pub struct ThreadSwitcher {
    /// Whether the thread switcher dialog is visible
    pub visible: bool,
    /// Currently selected index in the thread list (MRU order)
    pub selected_index: usize,
    /// Scroll offset for the thread list (first visible thread index)
    pub scroll_offset: usize,
    /// Timestamp of last navigation key press (for auto-confirm on release)
    pub last_nav_time: Option<std::time::Instant>,
}

/// Represents which scroll boundary was hit (for visual feedback)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollBoundary {
    Top,
    Bottom,
}
