//! Type definitions for the application state.
//!
//! Contains enums and structs used for tracking UI state:
//! - [`Screen`] - Which screen is currently displayed
//! - [`Focus`] - Which UI component has focus
//! - [`ScrollBoundary`] - Scroll boundary hit state
//! - [`ThreadSwitcher`] - Thread switcher dialog state
//! - [`BrowseListState`] - Full-screen browse list state (threads/repos)

use crate::models::picker::{RepoEntry, ThreadEntry};

/// Represents which screen is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    #[default]
    CommandDeck,
    Conversation,
    /// Full-screen browse list (threads or repos)
    BrowseList,
}

/// What type of content the browse list is showing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowseListMode {
    #[default]
    Threads,
    Repos,
}

/// State for the full-screen browse list view
#[derive(Debug, Clone, Default)]
pub struct BrowseListState {
    /// What content type we're browsing
    pub mode: BrowseListMode,
    /// Current search query
    pub search_query: String,
    /// Whether the search input is focused
    pub search_focused: bool,
    /// Currently selected index in the list
    pub selected_index: usize,
    /// Scroll offset (first visible item index)
    pub scroll_offset: usize,
    /// Total count from server (for "X total" display)
    pub total_count: usize,
    /// Thread items (when mode is Threads)
    pub threads: Vec<ThreadEntry>,
    /// Repo items (when mode is Repos) - filtered view
    pub repos: Vec<RepoEntry>,
    /// All repos (unfiltered, for local filtering)
    pub all_repos: Vec<RepoEntry>,
    /// Loading state (initial load)
    pub loading: bool,
    /// Searching state (search API call in progress)
    pub searching: bool,
    /// Error message if any
    pub error: Option<String>,
    /// Whether there are more items to load
    pub has_more: bool,
    /// Current pagination offset for lazy loading
    pub pagination_offset: usize,
    /// Pending search query for debounce (set on keystroke, cleared when search fires)
    pub pending_search: Option<String>,
    /// Clone in progress
    pub cloning: bool,
    /// Clone status message (e.g., "Cloning owner/repo...")
    pub clone_message: Option<String>,
}

/// Represents which UI component has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
