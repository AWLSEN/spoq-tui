//! Unified @ Picker State Management
//!
//! Manages state for the unified picker overlay that handles:
//! - GitHub repositories (remote and local)
//! - Threads (conversation history)
//! - Folders (local directories)
//!
//! Key features:
//! - Server-side search with debounce (150ms)
//! - Independent loading states per section
//! - Keyboard navigation across sections

use std::time::Instant;

use crate::models::picker::{PickerItem, PickerSection};

/// Debounce delay for search queries (milliseconds)
pub const SEARCH_DEBOUNCE_MS: u64 = 150;

/// Default search result limit per section
pub const DEFAULT_SEARCH_LIMIT: usize = 10;

/// State for a single picker section (repos, threads, or folders)
#[derive(Debug, Clone, Default)]
pub struct SectionState {
    /// Items in this section
    pub items: Vec<PickerItem>,
    /// Whether this section is currently loading
    pub loading: bool,
    /// Error message if the search failed
    pub error: Option<String>,
}

impl SectionState {
    /// Create a new empty section state
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if this section is empty (no items and not loading)
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && !self.loading
    }

    /// Check if this section has items to display
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Get the number of items in this section
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Clear items and reset state
    pub fn clear(&mut self) {
        self.items.clear();
        self.loading = false;
        self.error = None;
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
        if loading {
            self.error = None;
        }
    }

    /// Set items from search response
    pub fn set_items(&mut self, items: Vec<PickerItem>) {
        self.items = items;
        self.loading = false;
        self.error = None;
    }

    /// Set error state
    pub fn set_error(&mut self, error: String) {
        self.loading = false;
        self.error = Some(error);
    }
}

/// Unified picker state
#[derive(Debug, Clone)]
pub struct UnifiedPickerState {
    /// Whether the picker is visible
    pub visible: bool,
    /// Current search query (text after @)
    pub query: String,
    /// Last time the query changed (for debounce)
    pub last_query_change: Option<Instant>,
    /// Section states
    pub repos: SectionState,
    pub threads: SectionState,
    pub folders: SectionState,
    /// Currently selected section (for navigation)
    pub selected_section: PickerSection,
    /// Selected item index within the current section
    pub selected_index: usize,
    /// Whether a clone operation is in progress
    pub cloning: bool,
    /// Clone progress message (shown during clone)
    pub clone_message: Option<String>,
}

impl Default for UnifiedPickerState {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedPickerState {
    /// Create a new picker state
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            last_query_change: None,
            repos: SectionState::new(),
            threads: SectionState::new(),
            folders: SectionState::new(),
            selected_section: PickerSection::Repos,
            selected_index: 0,
            cloning: false,
            clone_message: None,
        }
    }

    /// Open the picker
    pub fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.selected_section = PickerSection::Repos;
        self.selected_index = 0;
        self.cloning = false;
        self.clone_message = None;
        // Mark sections as loading to trigger initial fetch
        self.repos.set_loading(true);
        self.threads.set_loading(true);
        self.folders.set_loading(true);
    }

    /// Close the picker and reset state
    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.last_query_change = None;
        self.repos.clear();
        self.threads.clear();
        self.folders.clear();
        self.selected_section = PickerSection::Repos;
        self.selected_index = 0;
        self.cloning = false;
        self.clone_message = None;
    }

    /// Update the search query
    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.last_query_change = Some(Instant::now());
        // Don't clear items - keep showing while loading new results
    }

    /// Check if debounce period has elapsed since last query change
    pub fn should_search(&self) -> bool {
        if let Some(last_change) = self.last_query_change {
            last_change.elapsed().as_millis() >= SEARCH_DEBOUNCE_MS as u128
        } else {
            false
        }
    }

    /// Mark that search has been triggered (clear the debounce timer)
    pub fn search_triggered(&mut self) {
        self.last_query_change = None;
    }

    /// Check if any section is loading
    pub fn is_loading(&self) -> bool {
        self.repos.loading || self.threads.loading || self.folders.loading
    }

    /// Get the state for a section
    pub fn section_state(&self, section: PickerSection) -> &SectionState {
        match section {
            PickerSection::Repos => &self.repos,
            PickerSection::Threads => &self.threads,
            PickerSection::Folders => &self.folders,
        }
    }

    /// Get mutable state for a section
    pub fn section_state_mut(&mut self, section: PickerSection) -> &mut SectionState {
        match section {
            PickerSection::Repos => &mut self.repos,
            PickerSection::Threads => &mut self.threads,
            PickerSection::Folders => &mut self.folders,
        }
    }

    /// Get all items in display order (repos -> threads -> folders)
    pub fn all_items(&self) -> Vec<&PickerItem> {
        let mut items = Vec::new();
        items.extend(self.repos.items.iter());
        items.extend(self.threads.items.iter());
        items.extend(self.folders.items.iter());
        items
    }

    /// Get total item count across all sections
    pub fn total_items(&self) -> usize {
        self.repos.len() + self.threads.len() + self.folders.len()
    }

    /// Get the currently selected item, if any
    pub fn selected_item(&self) -> Option<&PickerItem> {
        let section = self.section_state(self.selected_section);
        section.items.get(self.selected_index)
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.cloning {
            return; // Block navigation during clone
        }

        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            // Try to move to previous section
            self.move_to_previous_section();
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.cloning {
            return; // Block navigation during clone
        }

        let current_section_len = self.section_state(self.selected_section).len();
        if self.selected_index + 1 < current_section_len {
            self.selected_index += 1;
        } else {
            // Try to move to next section
            self.move_to_next_section();
        }
    }

    /// Move to the previous non-empty section
    fn move_to_previous_section(&mut self) {
        let sections = [
            PickerSection::Repos,
            PickerSection::Threads,
            PickerSection::Folders,
        ];

        let current_idx = sections
            .iter()
            .position(|&s| s == self.selected_section)
            .unwrap_or(0);

        // Search backwards for a non-empty section
        for i in (0..current_idx).rev() {
            let section_len = self.section_state(sections[i]).len();
            if section_len > 0 {
                self.selected_section = sections[i];
                self.selected_index = section_len.saturating_sub(1);
                return;
            }
        }
        // Stay at current position if no previous section has items
    }

    /// Move to the next non-empty section
    fn move_to_next_section(&mut self) {
        let sections = [
            PickerSection::Repos,
            PickerSection::Threads,
            PickerSection::Folders,
        ];

        let current_idx = sections
            .iter()
            .position(|&s| s == self.selected_section)
            .unwrap_or(0);

        // Search forwards for a non-empty section
        for i in (current_idx + 1)..sections.len() {
            let section_len = self.section_state(sections[i]).len();
            if section_len > 0 {
                self.selected_section = sections[i];
                self.selected_index = 0;
                return;
            }
        }
        // Stay at current position if no next section has items
    }

    /// Ensure selection is valid (call after items update)
    pub fn validate_selection(&mut self) {
        let sections = [
            PickerSection::Repos,
            PickerSection::Threads,
            PickerSection::Folders,
        ];

        // First, check if current section still has items
        let current_section = self.section_state(self.selected_section);
        if current_section.has_items() {
            // Clamp index to valid range
            if self.selected_index >= current_section.len() {
                self.selected_index = current_section.len().saturating_sub(1);
            }
            return;
        }

        // Current section is empty, find first non-empty section
        for section in sections {
            let section_state = self.section_state(section);
            if section_state.has_items() {
                self.selected_section = section;
                self.selected_index = 0;
                return;
            }
        }

        // All sections empty - reset to first section
        self.selected_section = PickerSection::Repos;
        self.selected_index = 0;
    }

    /// Start a clone operation
    pub fn start_clone(&mut self, message: &str) {
        self.cloning = true;
        self.clone_message = Some(message.to_string());
    }

    /// Complete a clone operation
    pub fn finish_clone(&mut self) {
        self.cloning = false;
        self.clone_message = None;
    }

    /// Check if input should be blocked (during clone)
    pub fn is_input_blocked(&self) -> bool {
        self.cloning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // SectionState Tests
    // ========================================================================

    #[test]
    fn test_section_state_default() {
        let state = SectionState::default();
        assert!(state.items.is_empty());
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_section_state_is_empty() {
        let mut state = SectionState::new();
        assert!(state.is_empty());

        state.loading = true;
        assert!(!state.is_empty()); // Loading = not empty

        state.loading = false;
        state.items.push(PickerItem::Folder {
            name: "test".to_string(),
            path: "/test".to_string(),
        });
        assert!(!state.is_empty());
    }

    #[test]
    fn test_section_state_set_loading() {
        let mut state = SectionState::new();
        state.error = Some("old error".to_string());

        state.set_loading(true);

        assert!(state.loading);
        assert!(state.error.is_none()); // Error cleared
    }

    #[test]
    fn test_section_state_set_items() {
        let mut state = SectionState::new();
        state.loading = true;

        let items = vec![PickerItem::Folder {
            name: "test".to_string(),
            path: "/test".to_string(),
        }];

        state.set_items(items);

        assert_eq!(state.len(), 1);
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_section_state_set_error() {
        let mut state = SectionState::new();
        state.loading = true;

        state.set_error("Search failed".to_string());

        assert!(!state.loading);
        assert_eq!(state.error, Some("Search failed".to_string()));
    }

    #[test]
    fn test_section_state_clear() {
        let mut state = SectionState::new();
        state.items.push(PickerItem::Folder {
            name: "test".to_string(),
            path: "/test".to_string(),
        });
        state.loading = true;
        state.error = Some("error".to_string());

        state.clear();

        assert!(state.items.is_empty());
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    // ========================================================================
    // UnifiedPickerState Tests
    // ========================================================================

    #[test]
    fn test_picker_state_default() {
        let state = UnifiedPickerState::default();
        assert!(!state.visible);
        assert!(state.query.is_empty());
        assert_eq!(state.selected_section, PickerSection::Repos);
        assert_eq!(state.selected_index, 0);
        assert!(!state.cloning);
    }

    #[test]
    fn test_picker_state_open() {
        let mut state = UnifiedPickerState::new();
        state.query = "old query".to_string();
        state.selected_section = PickerSection::Folders;
        state.selected_index = 5;

        state.open();

        assert!(state.visible);
        assert!(state.query.is_empty());
        assert_eq!(state.selected_section, PickerSection::Repos);
        assert_eq!(state.selected_index, 0);
        assert!(state.repos.loading);
        assert!(state.threads.loading);
        assert!(state.folders.loading);
    }

    #[test]
    fn test_picker_state_close() {
        let mut state = UnifiedPickerState::new();
        state.open();
        state.query = "test".to_string();
        state.repos.items.push(PickerItem::Repo {
            name: "owner/repo".to_string(),
            local_path: None,
            url: "https://github.com/owner/repo".to_string(),
        });

        state.close();

        assert!(!state.visible);
        assert!(state.query.is_empty());
        assert!(state.repos.items.is_empty());
        assert!(state.threads.items.is_empty());
        assert!(state.folders.items.is_empty());
    }

    #[test]
    fn test_picker_state_set_query() {
        let mut state = UnifiedPickerState::new();

        state.set_query("test".to_string());

        assert_eq!(state.query, "test");
        assert!(state.last_query_change.is_some());
    }

    #[test]
    fn test_picker_state_should_search_debounce() {
        let mut state = UnifiedPickerState::new();

        // No query change yet
        assert!(!state.should_search());

        // Set query
        state.set_query("test".to_string());

        // Immediately after - should not search (within debounce)
        assert!(!state.should_search());

        // Wait a bit...
        // Note: In a real test we'd use mock time, but for now just verify the logic
    }

    #[test]
    fn test_picker_state_search_triggered() {
        let mut state = UnifiedPickerState::new();
        state.set_query("test".to_string());
        assert!(state.last_query_change.is_some());

        state.search_triggered();

        assert!(state.last_query_change.is_none());
    }

    #[test]
    fn test_picker_state_is_loading() {
        let mut state = UnifiedPickerState::new();
        assert!(!state.is_loading());

        state.repos.loading = true;
        assert!(state.is_loading());

        state.repos.loading = false;
        state.threads.loading = true;
        assert!(state.is_loading());
    }

    #[test]
    fn test_picker_state_section_state() {
        let state = UnifiedPickerState::new();

        let repos = state.section_state(PickerSection::Repos);
        let threads = state.section_state(PickerSection::Threads);
        let folders = state.section_state(PickerSection::Folders);

        // All should be empty initially
        assert!(repos.items.is_empty());
        assert!(threads.items.is_empty());
        assert!(folders.items.is_empty());
    }

    #[test]
    fn test_picker_state_total_items() {
        let mut state = UnifiedPickerState::new();

        state.repos.items.push(PickerItem::Repo {
            name: "repo".to_string(),
            local_path: None,
            url: "https://github.com/owner/repo".to_string(),
        });
        state.threads.items.push(PickerItem::Thread {
            id: "thread-1".to_string(),
            title: "Thread 1".to_string(),
            working_directory: None,
        });
        state.folders.items.push(PickerItem::Folder {
            name: "folder".to_string(),
            path: "/path".to_string(),
        });
        state.folders.items.push(PickerItem::Folder {
            name: "folder2".to_string(),
            path: "/path2".to_string(),
        });

        assert_eq!(state.total_items(), 4);
    }

    #[test]
    fn test_picker_state_selected_item() {
        let mut state = UnifiedPickerState::new();

        // No items
        assert!(state.selected_item().is_none());

        // Add item
        state.repos.items.push(PickerItem::Repo {
            name: "repo".to_string(),
            local_path: None,
            url: "https://github.com/owner/repo".to_string(),
        });

        assert!(state.selected_item().is_some());
        assert_eq!(state.selected_item().unwrap().display_name(), "repo");
    }

    #[test]
    fn test_picker_state_move_down_within_section() {
        let mut state = UnifiedPickerState::new();
        state.repos.items.push(PickerItem::Repo {
            name: "repo1".to_string(),
            local_path: None,
            url: "url1".to_string(),
        });
        state.repos.items.push(PickerItem::Repo {
            name: "repo2".to_string(),
            local_path: None,
            url: "url2".to_string(),
        });

        assert_eq!(state.selected_index, 0);

        state.move_down();

        assert_eq!(state.selected_index, 1);
        assert_eq!(state.selected_section, PickerSection::Repos);
    }

    #[test]
    fn test_picker_state_move_down_to_next_section() {
        let mut state = UnifiedPickerState::new();
        state.repos.items.push(PickerItem::Repo {
            name: "repo".to_string(),
            local_path: None,
            url: "url".to_string(),
        });
        state.threads.items.push(PickerItem::Thread {
            id: "thread".to_string(),
            title: "Thread".to_string(),
            working_directory: None,
        });

        state.selected_index = 0;
        state.move_down(); // Move past end of repos

        assert_eq!(state.selected_section, PickerSection::Threads);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_picker_state_move_up_within_section() {
        let mut state = UnifiedPickerState::new();
        state.repos.items.push(PickerItem::Repo {
            name: "repo1".to_string(),
            local_path: None,
            url: "url1".to_string(),
        });
        state.repos.items.push(PickerItem::Repo {
            name: "repo2".to_string(),
            local_path: None,
            url: "url2".to_string(),
        });

        state.selected_index = 1;
        state.move_up();

        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_picker_state_move_up_to_previous_section() {
        let mut state = UnifiedPickerState::new();
        state.repos.items.push(PickerItem::Repo {
            name: "repo".to_string(),
            local_path: None,
            url: "url".to_string(),
        });
        state.threads.items.push(PickerItem::Thread {
            id: "thread".to_string(),
            title: "Thread".to_string(),
            working_directory: None,
        });

        state.selected_section = PickerSection::Threads;
        state.selected_index = 0;
        state.move_up();

        assert_eq!(state.selected_section, PickerSection::Repos);
        assert_eq!(state.selected_index, 0); // Last item in repos
    }

    #[test]
    fn test_picker_state_validate_selection_clamps_index() {
        let mut state = UnifiedPickerState::new();
        state.repos.items.push(PickerItem::Repo {
            name: "repo".to_string(),
            local_path: None,
            url: "url".to_string(),
        });
        state.selected_index = 5; // Invalid

        state.validate_selection();

        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_picker_state_validate_selection_moves_to_nonempty_section() {
        let mut state = UnifiedPickerState::new();
        state.selected_section = PickerSection::Repos;
        // Repos empty, but folders has items
        state.folders.items.push(PickerItem::Folder {
            name: "folder".to_string(),
            path: "/path".to_string(),
        });

        state.validate_selection();

        assert_eq!(state.selected_section, PickerSection::Folders);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_picker_state_clone_operations() {
        let mut state = UnifiedPickerState::new();

        assert!(!state.cloning);
        assert!(!state.is_input_blocked());

        state.start_clone("Cloning owner/repo...");

        assert!(state.cloning);
        assert!(state.is_input_blocked());
        assert_eq!(state.clone_message, Some("Cloning owner/repo...".to_string()));

        state.finish_clone();

        assert!(!state.cloning);
        assert!(!state.is_input_blocked());
        assert!(state.clone_message.is_none());
    }

    #[test]
    fn test_picker_state_navigation_blocked_during_clone() {
        let mut state = UnifiedPickerState::new();
        state.repos.items.push(PickerItem::Repo {
            name: "repo1".to_string(),
            local_path: None,
            url: "url1".to_string(),
        });
        state.repos.items.push(PickerItem::Repo {
            name: "repo2".to_string(),
            local_path: None,
            url: "url2".to_string(),
        });

        state.start_clone("Cloning...");
        let initial_index = state.selected_index;

        state.move_down();
        assert_eq!(state.selected_index, initial_index); // Unchanged

        state.move_up();
        assert_eq!(state.selected_index, initial_index); // Unchanged
    }

    #[test]
    fn test_picker_constants() {
        assert_eq!(SEARCH_DEBOUNCE_MS, 150);
        assert_eq!(DEFAULT_SEARCH_LIMIT, 10);
    }
}
