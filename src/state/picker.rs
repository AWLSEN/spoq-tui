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

/// Maximum visible rows in picker viewport (must match unified_picker.rs)
const MAX_VISIBLE_ROWS: usize = 10;

/// State for a single picker section (repos, threads, or folders)
#[derive(Debug, Clone, Default)]
pub struct SectionState {
    /// All items in this section (full cache, loaded once on open)
    pub all_items: Vec<PickerItem>,
    /// Items filtered by current query (displayed to user)
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
        self.all_items.clear();
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

    /// Set items from API response (caches all items and applies current filter)
    pub fn set_items(&mut self, items: Vec<PickerItem>) {
        self.all_items = items.clone();
        self.items = items;
        self.loading = false;
        self.error = None;
    }

    /// Filter cached items by query (instant, no API call)
    pub fn filter_by_query(&mut self, query: &str) {
        if query.is_empty() {
            // Show all items when no query
            self.items = self.all_items.clone();
        } else {
            let query_lower = query.to_lowercase();
            self.items = self.all_items
                .iter()
                .filter(|item| item.display_name().to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
        }
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
    /// Scroll offset for viewport (line index of first visible line)
    pub scroll_offset: usize,
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
            scroll_offset: 0,
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
        self.scroll_offset = 0;
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
        self.scroll_offset = 0;
        self.cloning = false;
        self.clone_message = None;
    }

    /// Update the search query and filter items locally (instant)
    pub fn set_query(&mut self, query: String) {
        self.query = query.clone();
        // Filter all sections locally - no API call needed
        self.repos.filter_by_query(&query);
        self.threads.filter_by_query(&query);
        self.folders.filter_by_query(&query);
        // Reset selection if current item is no longer visible
        self.validate_selection();
    }

    /// Check if debounce period has elapsed since last query change
    /// NOTE: With local filtering, this is no longer needed for search
    /// but kept for potential future use (e.g., async refresh)
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

    /// Check if initial data has been loaded
    pub fn has_cached_data(&self) -> bool {
        !self.repos.all_items.is_empty()
            || !self.threads.all_items.is_empty()
            || !self.folders.all_items.is_empty()
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

        // Auto-scroll to keep selection visible
        self.ensure_visible(MAX_VISIBLE_ROWS);
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

        // Auto-scroll to keep selection visible
        self.ensure_visible(MAX_VISIBLE_ROWS);
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

    /// Calculate the line index of the currently selected item
    /// This accounts for section headers (each non-empty section adds 1 header line)
    pub fn selected_line_index(&self) -> usize {
        let sections = [
            (PickerSection::Repos, &self.repos),
            (PickerSection::Threads, &self.threads),
            (PickerSection::Folders, &self.folders),
        ];

        let mut line_idx = 0;

        for (section, section_state) in sections {
            // Skip empty sections (they don't render headers)
            if section_state.items.is_empty() && !section_state.loading {
                continue;
            }

            // Account for section header
            line_idx += 1;

            if section == self.selected_section {
                // Add the selected index within this section
                line_idx += self.selected_index;
                break;
            } else {
                // Add all items in this section
                line_idx += section_state.items.len();
            }
        }

        line_idx
    }

    /// Calculate total number of lines (headers + items)
    pub fn total_lines(&self) -> usize {
        let sections = [&self.repos, &self.threads, &self.folders];
        let mut total = 0;

        for section in sections {
            if !section.items.is_empty() || section.loading {
                total += 1; // header
                total += section.items.len();
            }
        }

        total
    }

    /// Ensure the selected item is visible within the viewport
    /// Call this after changing selection
    pub fn ensure_visible(&mut self, visible_rows: usize) {
        if visible_rows == 0 {
            return;
        }

        let selected_line = self.selected_line_index();

        // If selected line is above viewport, scroll up
        if selected_line < self.scroll_offset {
            self.scroll_offset = selected_line;
        }

        // If selected line is below viewport, scroll down
        // (subtract 1 because we want to see the line, not just its edge)
        if selected_line >= self.scroll_offset + visible_rows {
            self.scroll_offset = selected_line.saturating_sub(visible_rows - 1);
        }
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
        // Pre-populate with items to filter
        state.repos.set_items(vec![
            PickerItem::Repo {
                name: "test-repo".to_string(),
                local_path: None,
                url: "https://github.com/test/test-repo".to_string(),
            },
            PickerItem::Repo {
                name: "other-repo".to_string(),
                local_path: None,
                url: "https://github.com/test/other-repo".to_string(),
            },
        ]);

        state.set_query("test".to_string());

        assert_eq!(state.query, "test");
        // Local filtering should show only matching items
        assert_eq!(state.repos.items.len(), 1);
        assert_eq!(state.repos.items[0].display_name(), "test-repo");
    }

    #[test]
    fn test_picker_state_filter_clears_on_empty_query() {
        let mut state = UnifiedPickerState::new();
        state.repos.set_items(vec![
            PickerItem::Repo {
                name: "test-repo".to_string(),
                local_path: None,
                url: "https://github.com/test/test-repo".to_string(),
            },
            PickerItem::Repo {
                name: "other-repo".to_string(),
                local_path: None,
                url: "https://github.com/test/other-repo".to_string(),
            },
        ]);

        // Filter
        state.set_query("test".to_string());
        assert_eq!(state.repos.items.len(), 1);

        // Clear filter
        state.set_query("".to_string());
        assert_eq!(state.repos.items.len(), 2);
    }

    #[test]
    fn test_picker_state_search_triggered() {
        let mut state = UnifiedPickerState::new();
        state.set_query("test".to_string());
        // search_triggered clears last_query_change (legacy behavior for compatibility)
        state.last_query_change = Some(std::time::Instant::now());
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

    // ========================================================================
    // Scrolling Tests
    // ========================================================================

    #[test]
    fn test_selected_line_index_single_section() {
        let mut state = UnifiedPickerState::new();
        // Add 3 repos
        for i in 0..3 {
            state.repos.items.push(PickerItem::Repo {
                name: format!("repo{}", i),
                local_path: None,
                url: format!("url{}", i),
            });
        }
        state.selected_section = PickerSection::Repos;

        // First item: header (line 0) + item 0 (line 1)
        state.selected_index = 0;
        assert_eq!(state.selected_line_index(), 1);

        // Second item
        state.selected_index = 1;
        assert_eq!(state.selected_line_index(), 2);

        // Third item
        state.selected_index = 2;
        assert_eq!(state.selected_line_index(), 3);
    }

    #[test]
    fn test_selected_line_index_multiple_sections() {
        let mut state = UnifiedPickerState::new();
        // Add 2 repos
        for i in 0..2 {
            state.repos.items.push(PickerItem::Repo {
                name: format!("repo{}", i),
                local_path: None,
                url: format!("url{}", i),
            });
        }
        // Add 2 threads
        for i in 0..2 {
            state.threads.items.push(PickerItem::Thread {
                id: format!("thread{}", i),
                title: format!("Thread {}", i),
                working_directory: None,
            });
        }

        // Repos section: header (0) + 2 items (1, 2)
        // Threads section: header (3) + items (4, 5)
        state.selected_section = PickerSection::Threads;
        state.selected_index = 0;
        assert_eq!(state.selected_line_index(), 4); // header + 2 repo items + threads header + first thread

        state.selected_index = 1;
        assert_eq!(state.selected_line_index(), 5);
    }

    #[test]
    fn test_ensure_visible_scroll_down() {
        let mut state = UnifiedPickerState::new();
        // Add many repos to exceed viewport
        for i in 0..15 {
            state.repos.items.push(PickerItem::Repo {
                name: format!("repo{}", i),
                local_path: None,
                url: format!("url{}", i),
            });
        }

        state.selected_section = PickerSection::Repos;
        state.selected_index = 12; // Line 13 (header + 12)
        state.scroll_offset = 0;

        state.ensure_visible(10);

        // Selected line 13 should be visible in 10-row viewport
        // So scroll_offset should be at least 4 (13 - 10 + 1 = 4)
        assert!(state.scroll_offset >= 4);
    }

    #[test]
    fn test_ensure_visible_scroll_up() {
        let mut state = UnifiedPickerState::new();
        for i in 0..15 {
            state.repos.items.push(PickerItem::Repo {
                name: format!("repo{}", i),
                local_path: None,
                url: format!("url{}", i),
            });
        }

        state.selected_section = PickerSection::Repos;
        state.selected_index = 2; // Line 3
        state.scroll_offset = 10; // Scrolled way down

        state.ensure_visible(10);

        // Selected line 3 should be visible, so scroll should be <= 3
        assert!(state.scroll_offset <= 3);
    }

    #[test]
    fn test_move_down_updates_scroll() {
        let mut state = UnifiedPickerState::new();
        for i in 0..15 {
            state.repos.items.push(PickerItem::Repo {
                name: format!("repo{}", i),
                local_path: None,
                url: format!("url{}", i),
            });
        }

        state.selected_section = PickerSection::Repos;
        state.scroll_offset = 0;

        // Move down repeatedly past viewport
        for _ in 0..12 {
            state.move_down();
        }

        // Scroll should have adjusted
        assert!(state.scroll_offset > 0);
    }

    #[test]
    fn test_total_lines() {
        let mut state = UnifiedPickerState::new();

        // Empty state
        assert_eq!(state.total_lines(), 0);

        // Add repos
        state.repos.items.push(PickerItem::Repo {
            name: "repo".to_string(),
            local_path: None,
            url: "url".to_string(),
        });
        assert_eq!(state.total_lines(), 2); // 1 header + 1 item

        // Add threads
        state.threads.items.push(PickerItem::Thread {
            id: "1".to_string(),
            title: "Thread".to_string(),
            working_directory: None,
        });
        assert_eq!(state.total_lines(), 4); // 2 + 1 header + 1 item
    }
}
