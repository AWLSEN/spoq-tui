//! File Picker State Management
//!
//! Manages state for the file picker overlay in conversation threads.
//! Enables selecting files from the thread's working_directory.
//!
//! Key features:
//! - Directory navigation (enter/exit subdirectories)
//! - Multi-select file selection
//! - Fuzzy filtering on file names
//! - Keyboard navigation with scroll support

use std::path::PathBuf;

use crate::models::file::FileEntry;

/// Maximum visible rows in picker viewport
pub const MAX_VISIBLE_ROWS: usize = 10;

/// File picker state
#[derive(Debug, Clone)]
pub struct FilePickerState {
    /// Whether the picker is visible
    pub visible: bool,

    /// Current search/filter query
    pub query: String,

    /// Current directory being browsed
    pub current_path: PathBuf,

    /// Base path (thread's working_directory) - cannot navigate above this
    pub base_path: PathBuf,

    /// All items in current directory (from API)
    pub items: Vec<FileEntry>,

    /// Items filtered by current query
    pub filtered_items: Vec<FileEntry>,

    /// Cursor position in filtered_items
    pub selected_index: usize,

    /// Scroll offset for viewport
    pub scroll_offset: usize,

    /// Whether data is loading
    pub loading: bool,

    /// Error message if loading failed
    pub error: Option<String>,

    /// Multi-select: accumulated file paths
    pub selected_files: Vec<String>,
}

impl Default for FilePickerState {
    fn default() -> Self {
        Self::new()
    }
}

impl FilePickerState {
    /// Create a new file picker state
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            current_path: PathBuf::new(),
            base_path: PathBuf::new(),
            items: Vec::new(),
            filtered_items: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            loading: false,
            error: None,
            selected_files: Vec::new(),
        }
    }

    /// Open the picker at a base directory
    pub fn open(&mut self, base_path: &str) {
        self.visible = true;
        self.query.clear();
        self.base_path = PathBuf::from(base_path);
        self.current_path = self.base_path.clone();
        self.items.clear();
        self.filtered_items.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.loading = true;
        self.error = None;
        self.selected_files.clear();
    }

    /// Close the picker
    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.items.clear();
        self.filtered_items.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.loading = false;
        self.error = None;
        // Don't clear selected_files - they're used after close
    }

    /// Cancel and close (also clears selected files)
    pub fn cancel(&mut self) {
        self.selected_files.clear();
        self.close();
    }

    /// Set items from API response
    pub fn set_items(&mut self, items: Vec<FileEntry>) {
        self.items = items;
        self.loading = false;
        self.error = None;
        // Apply current filter
        self.filter_items();
        self.validate_selection();
    }

    /// Set error state
    pub fn set_error(&mut self, error: String) {
        self.loading = false;
        self.error = Some(error);
        self.items.clear();
        self.filtered_items.clear();
    }

    /// Update the search query and filter items
    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.filter_items();
        self.validate_selection();
    }

    /// Filter items by current query
    fn filter_items(&mut self) {
        if self.query.is_empty() {
            self.filtered_items = self.items.clone();
        } else {
            let query_lower = self.query.to_lowercase();
            self.filtered_items = self.items
                .iter()
                .filter(|item| item.name.to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
        }
    }

    /// Ensure selection is valid
    fn validate_selection(&mut self) {
        if self.filtered_items.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered_items.len() {
            self.selected_index = self.filtered_items.len().saturating_sub(1);
        }
        self.ensure_visible();
    }

    /// Get the currently selected item
    pub fn selected_item(&self) -> Option<&FileEntry> {
        self.filtered_items.get(self.selected_index)
    }

    /// Check if we can navigate up (not at base_path)
    pub fn can_go_up(&self) -> bool {
        self.current_path != self.base_path
    }

    /// Navigate into a subdirectory
    pub fn navigate_into(&mut self, dir_name: &str) {
        self.current_path.push(dir_name);
        self.query.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.loading = true;
        self.items.clear();
        self.filtered_items.clear();
    }

    /// Navigate to parent directory (if allowed)
    pub fn navigate_up(&mut self) {
        if self.can_go_up() {
            self.current_path.pop();
            self.query.clear();
            self.selected_index = 0;
            self.scroll_offset = 0;
            self.loading = true;
            self.items.clear();
            self.filtered_items.clear();
        }
    }

    /// Get current path as string
    pub fn current_path_str(&self) -> String {
        self.current_path.to_string_lossy().to_string()
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.ensure_visible();
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_index + 1 < self.filtered_items.len() {
            self.selected_index += 1;
            self.ensure_visible();
        }
    }

    /// Ensure selected item is visible in viewport
    fn ensure_visible(&mut self) {
        // Scroll up if needed
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
        // Scroll down if needed
        if self.selected_index >= self.scroll_offset + MAX_VISIBLE_ROWS {
            self.scroll_offset = self.selected_index.saturating_sub(MAX_VISIBLE_ROWS - 1);
        }
    }

    /// Toggle selection of the current item (for multi-select)
    pub fn toggle_selection(&mut self) {
        if let Some(item) = self.selected_item() {
            // Only allow selecting files, not directories
            if item.is_dir {
                return;
            }

            let path = item.path.clone();
            if let Some(pos) = self.selected_files.iter().position(|p| p == &path) {
                self.selected_files.remove(pos);
            } else {
                self.selected_files.push(path);
            }
        }
    }

    /// Check if a file path is selected
    pub fn is_selected(&self, path: &str) -> bool {
        self.selected_files.contains(&path.to_string())
    }

    /// Get selected file count
    pub fn selected_count(&self) -> usize {
        self.selected_files.len()
    }

    /// Take selected files and clear selection
    pub fn take_selected_files(&mut self) -> Vec<String> {
        std::mem::take(&mut self.selected_files)
    }

    /// Get relative paths of selected files (relative to base_path)
    pub fn selected_relative_paths(&self) -> Vec<String> {
        let base = self.base_path.to_string_lossy();
        self.selected_files
            .iter()
            .map(|path| {
                if path.starts_with(base.as_ref()) {
                    path.trim_start_matches(base.as_ref())
                        .trim_start_matches('/')
                        .to_string()
                } else {
                    path.clone()
                }
            })
            .collect()
    }

    /// Check if picker is empty (no items and not loading)
    pub fn is_empty(&self) -> bool {
        self.filtered_items.is_empty() && !self.loading
    }

    /// Get items visible in the current viewport
    pub fn visible_items(&self) -> &[FileEntry] {
        let start = self.scroll_offset;
        let end = (self.scroll_offset + MAX_VISIBLE_ROWS).min(self.filtered_items.len());
        if start < self.filtered_items.len() {
            &self.filtered_items[start..end]
        } else {
            &[]
        }
    }

    /// Get total number of items
    pub fn total_items(&self) -> usize {
        self.filtered_items.len()
    }

    /// Check if there are more items above the viewport
    pub fn has_more_above(&self) -> bool {
        self.scroll_offset > 0
    }

    /// Check if there are more items below the viewport
    pub fn has_more_below(&self) -> bool {
        self.scroll_offset + MAX_VISIBLE_ROWS < self.filtered_items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_items() -> Vec<FileEntry> {
        vec![
            FileEntry {
                name: "src".to_string(),
                path: "/project/src".to_string(),
                is_dir: true,
                size: None,
                modified_at: None,
            },
            FileEntry {
                name: "main.rs".to_string(),
                path: "/project/main.rs".to_string(),
                is_dir: false,
                size: Some(1000),
                modified_at: None,
            },
            FileEntry {
                name: "config.rs".to_string(),
                path: "/project/config.rs".to_string(),
                is_dir: false,
                size: Some(500),
                modified_at: None,
            },
        ]
    }

    #[test]
    fn test_new_state() {
        let state = FilePickerState::new();
        assert!(!state.visible);
        assert!(state.query.is_empty());
        assert!(state.items.is_empty());
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_open() {
        let mut state = FilePickerState::new();
        state.open("/project");

        assert!(state.visible);
        assert!(state.loading);
        assert_eq!(state.current_path_str(), "/project");
        assert_eq!(state.base_path.to_string_lossy(), "/project");
    }

    #[test]
    fn test_close() {
        let mut state = FilePickerState::new();
        state.open("/project");
        state.selected_files.push("/project/file.rs".to_string());

        state.close();

        assert!(!state.visible);
        // Selected files preserved after close
        assert!(!state.selected_files.is_empty());
    }

    #[test]
    fn test_cancel() {
        let mut state = FilePickerState::new();
        state.open("/project");
        state.selected_files.push("/project/file.rs".to_string());

        state.cancel();

        assert!(!state.visible);
        // Selected files cleared on cancel
        assert!(state.selected_files.is_empty());
    }

    #[test]
    fn test_set_items() {
        let mut state = FilePickerState::new();
        state.open("/project");
        state.set_items(create_test_items());

        assert_eq!(state.items.len(), 3);
        assert_eq!(state.filtered_items.len(), 3);
        assert!(!state.loading);
    }

    #[test]
    fn test_filter_items() {
        let mut state = FilePickerState::new();
        state.open("/project");
        state.set_items(create_test_items());

        state.set_query("main".to_string());

        assert_eq!(state.filtered_items.len(), 1);
        assert_eq!(state.filtered_items[0].name, "main.rs");
    }

    #[test]
    fn test_filter_items_empty_query() {
        let mut state = FilePickerState::new();
        state.open("/project");
        state.set_items(create_test_items());
        state.set_query("main".to_string());
        assert_eq!(state.filtered_items.len(), 1);

        state.set_query("".to_string());

        assert_eq!(state.filtered_items.len(), 3);
    }

    #[test]
    fn test_navigation() {
        let mut state = FilePickerState::new();
        state.open("/project");
        state.set_items(create_test_items());

        assert_eq!(state.selected_index, 0);

        state.move_down();
        assert_eq!(state.selected_index, 1);

        state.move_down();
        assert_eq!(state.selected_index, 2);

        state.move_down(); // Should not go past end
        assert_eq!(state.selected_index, 2);

        state.move_up();
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_toggle_selection_file() {
        let mut state = FilePickerState::new();
        state.open("/project");
        state.set_items(create_test_items());

        // Move to a file (index 1 is main.rs)
        state.move_down();
        state.toggle_selection();

        assert_eq!(state.selected_files.len(), 1);
        assert!(state.is_selected("/project/main.rs"));

        // Toggle again to deselect
        state.toggle_selection();
        assert_eq!(state.selected_files.len(), 0);
    }

    #[test]
    fn test_toggle_selection_directory_ignored() {
        let mut state = FilePickerState::new();
        state.open("/project");
        state.set_items(create_test_items());

        // First item is src directory
        state.toggle_selection();

        // Should not select directories
        assert_eq!(state.selected_files.len(), 0);
    }

    #[test]
    fn test_can_go_up() {
        let mut state = FilePickerState::new();
        state.open("/project");

        assert!(!state.can_go_up()); // At base

        state.navigate_into("src");
        assert!(state.can_go_up()); // Below base
    }

    #[test]
    fn test_navigate_into_and_up() {
        let mut state = FilePickerState::new();
        state.open("/project");

        state.navigate_into("src");
        assert_eq!(state.current_path_str(), "/project/src");
        assert!(state.loading);

        state.navigate_up();
        assert_eq!(state.current_path_str(), "/project");
    }

    #[test]
    fn test_selected_relative_paths() {
        let mut state = FilePickerState::new();
        state.open("/project");
        state.selected_files.push("/project/src/main.rs".to_string());
        state.selected_files.push("/project/config.rs".to_string());

        let relative = state.selected_relative_paths();

        assert_eq!(relative.len(), 2);
        assert!(relative.contains(&"src/main.rs".to_string()));
        assert!(relative.contains(&"config.rs".to_string()));
    }

    #[test]
    fn test_scroll_visibility() {
        let mut state = FilePickerState::new();
        state.open("/project");

        // Create many items
        let mut items = Vec::new();
        for i in 0..20 {
            items.push(FileEntry {
                name: format!("file{}.rs", i),
                path: format!("/project/file{}.rs", i),
                is_dir: false,
                size: Some(100),
                modified_at: None,
            });
        }
        state.set_items(items);

        assert!(!state.has_more_above());
        assert!(state.has_more_below());

        // Scroll to bottom
        for _ in 0..19 {
            state.move_down();
        }

        assert!(state.has_more_above());
        assert!(!state.has_more_below());
    }
}
