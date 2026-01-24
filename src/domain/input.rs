//! Input state management.
//!
//! This module provides [`InputState`], a domain object that encapsulates
//! all input-related state including text input, history navigation,
//! and folder picker state.

use crate::input_history::InputHistory;
use crate::models::Folder;
use crate::widgets::textarea::TextAreaInput;

/// Input state encapsulating text input, history, and folder picker.
///
/// This domain object manages all input-related concerns:
/// - TextArea input widget
/// - Input history for Up/Down arrow navigation
/// - Folder picker state (visibility, filter, selection)
/// - Selected folder for context
pub struct InputState {
    /// TextArea input (tui-textarea wrapper)
    pub textarea: TextAreaInput<'static>,
    /// Input history for Up/Down arrow navigation
    pub input_history: InputHistory,
    /// Cached folder list from API for folder picker
    pub folders: Vec<Folder>,
    /// True while fetching folders from API
    pub folders_loading: bool,
    /// Error message if folder fetch failed
    pub folders_error: Option<String>,
    /// Currently selected folder (displayed as chip in input)
    pub selected_folder: Option<Folder>,
    /// Is the folder picker overlay showing
    pub folder_picker_visible: bool,
    /// Current filter text for folder picker (text after @)
    pub folder_picker_filter: String,
    /// Selected index in the filtered folder list
    pub folder_picker_cursor: usize,
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputState {
    /// Create a new InputState with default values.
    pub fn new() -> Self {
        Self {
            textarea: TextAreaInput::new(),
            input_history: InputHistory::load(),
            folders: Vec::new(),
            folders_loading: false,
            folders_error: None,
            selected_folder: None,
            folder_picker_visible: false,
            folder_picker_filter: String::new(),
            folder_picker_cursor: 0,
        }
    }

    /// Create a new InputState without loading history from disk.
    ///
    /// Useful for testing or when history persistence is not needed.
    pub fn new_without_history() -> Self {
        Self {
            textarea: TextAreaInput::new(),
            input_history: InputHistory::new(),
            folders: Vec::new(),
            folders_loading: false,
            folders_error: None,
            selected_folder: None,
            folder_picker_visible: false,
            folder_picker_filter: String::new(),
            folder_picker_cursor: 0,
        }
    }

    /// Get the current input text.
    pub fn get_input(&self) -> String {
        self.textarea.content()
    }

    /// Check if the input is empty (no text or only whitespace).
    pub fn is_empty(&self) -> bool {
        self.textarea.is_empty()
    }

    /// Clear the input text.
    pub fn clear_input(&mut self) {
        self.textarea.clear();
    }

    /// Set the input text.
    pub fn set_input(&mut self, text: &str) {
        self.textarea.set_content(text);
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.textarea.insert_char(c);
    }

    /// Delete the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        self.textarea.backspace();
    }

    /// Delete the character at the cursor (forward delete).
    pub fn delete_char(&mut self) {
        self.textarea.delete_char();
    }

    /// Navigate to the previous history entry (Up arrow).
    ///
    /// Returns the previous entry if available, or None if at the beginning.
    pub fn history_up(&mut self) -> Option<String> {
        let current = self.get_input();
        self.input_history
            .navigate_up(&current)
            .map(|s| s.to_string())
    }

    /// Navigate to the next history entry (Down arrow).
    ///
    /// Returns the next entry if available, or None if at the end.
    pub fn history_down(&mut self) -> Option<String> {
        self.input_history.navigate_down().map(|s| s.to_string())
    }

    /// Get the saved input from before history navigation.
    pub fn get_saved_input(&self) -> &str {
        self.input_history.get_current_input()
    }

    /// Add the current input to history and reset navigation.
    pub fn add_to_history(&mut self, input: String) {
        self.input_history.add(input);
        self.input_history.save();
    }

    /// Reset history navigation state.
    pub fn reset_history_navigation(&mut self) {
        self.input_history.reset_navigation();
    }

    /// Check if currently navigating through history.
    pub fn is_navigating_history(&self) -> bool {
        self.input_history.is_navigating()
    }

    // Folder picker methods

    /// Show the folder picker.
    pub fn show_folder_picker(&mut self) {
        self.folder_picker_visible = true;
        self.folder_picker_filter.clear();
        self.folder_picker_cursor = 0;
    }

    /// Hide the folder picker.
    pub fn hide_folder_picker(&mut self) {
        self.folder_picker_visible = false;
        self.folder_picker_filter.clear();
        self.folder_picker_cursor = 0;
    }

    /// Check if the folder picker is visible.
    pub fn is_folder_picker_visible(&self) -> bool {
        self.folder_picker_visible
    }

    /// Set the folder picker filter text.
    pub fn set_folder_filter(&mut self, filter: String) {
        self.folder_picker_filter = filter;
        self.folder_picker_cursor = 0; // Reset cursor when filter changes
    }

    /// Get filtered folders matching the current filter.
    pub fn get_filtered_folders(&self) -> Vec<&Folder> {
        if self.folder_picker_filter.is_empty() {
            self.folders.iter().collect()
        } else {
            let filter_lower = self.folder_picker_filter.to_lowercase();
            self.folders
                .iter()
                .filter(|f| f.name.to_lowercase().contains(&filter_lower))
                .collect()
        }
    }

    /// Move the folder picker cursor up.
    pub fn folder_picker_up(&mut self) {
        if self.folder_picker_cursor > 0 {
            self.folder_picker_cursor -= 1;
        }
    }

    /// Move the folder picker cursor down.
    pub fn folder_picker_down(&mut self) {
        let filtered = self.get_filtered_folders();
        if !filtered.is_empty() && self.folder_picker_cursor < filtered.len() - 1 {
            self.folder_picker_cursor += 1;
        }
    }

    /// Select the currently highlighted folder.
    pub fn select_folder(&mut self) -> Option<Folder> {
        let filtered = self.get_filtered_folders();
        if self.folder_picker_cursor < filtered.len() {
            let folder = filtered[self.folder_picker_cursor].clone();
            self.selected_folder = Some(folder.clone());
            self.hide_folder_picker();
            Some(folder)
        } else {
            None
        }
    }

    /// Clear the selected folder.
    pub fn clear_selected_folder(&mut self) {
        self.selected_folder = None;
    }

    /// Set the available folders.
    pub fn set_folders(&mut self, folders: Vec<Folder>) {
        self.folders = folders;
        self.folders_loading = false;
        self.folders_error = None;
    }

    /// Set a folder loading error.
    pub fn set_folders_error(&mut self, error: String) {
        self.folders_loading = false;
        self.folders_error = Some(error);
    }

    /// Start loading folders.
    pub fn start_loading_folders(&mut self) {
        self.folders_loading = true;
        self.folders_error = None;
    }

    /// Check if folders are currently loading.
    pub fn is_loading_folders(&self) -> bool {
        self.folders_loading
    }

    /// Submit the current input.
    ///
    /// Adds to history, clears input, and returns the submitted text.
    pub fn submit(&mut self) -> String {
        let input = self.get_input();
        if !input.trim().is_empty() {
            self.add_to_history(input.clone());
        }
        self.clear_input();
        self.reset_history_navigation();
        input
    }

    /// Reset all input state.
    pub fn reset(&mut self) {
        self.clear_input();
        self.reset_history_navigation();
        self.hide_folder_picker();
        self.selected_folder = None;
        self.folders_error = None;
    }
}

impl std::fmt::Debug for InputState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputState")
            .field("input_text", &self.get_input())
            .field("history_len", &self.input_history.len())
            .field("folders_count", &self.folders.len())
            .field("folders_loading", &self.folders_loading)
            .field("folders_error", &self.folders_error)
            .field("selected_folder", &self.selected_folder)
            .field("folder_picker_visible", &self.folder_picker_visible)
            .field("folder_picker_filter", &self.folder_picker_filter)
            .field("folder_picker_cursor", &self.folder_picker_cursor)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state_new() {
        let state = InputState::new_without_history();
        assert!(state.is_empty());
        assert!(!state.is_folder_picker_visible());
        assert!(state.selected_folder.is_none());
    }

    #[test]
    fn test_input_state_default() {
        // Note: This will load history from disk, so we test new_without_history instead
        let state = InputState::new_without_history();
        assert!(state.is_empty());
    }

    #[test]
    fn test_get_and_set_input() {
        let mut state = InputState::new_without_history();
        assert!(state.is_empty());

        state.set_input("hello world");
        assert!(!state.is_empty());
        assert_eq!(state.get_input(), "hello world");

        state.clear_input();
        assert!(state.is_empty());
    }

    #[test]
    fn test_insert_and_backspace() {
        let mut state = InputState::new_without_history();

        state.insert_char('h');
        state.insert_char('i');
        assert_eq!(state.get_input(), "hi");

        state.backspace();
        assert_eq!(state.get_input(), "h");
    }

    #[test]
    fn test_history_navigation() {
        let mut state = InputState::new_without_history();

        // Add some history
        state.add_to_history("first".to_string());
        state.add_to_history("second".to_string());
        state.add_to_history("third".to_string());

        // Navigate up through history
        state.set_input("current");
        let prev = state.history_up();
        assert_eq!(prev, Some("third".to_string()));

        let prev = state.history_up();
        assert_eq!(prev, Some("second".to_string()));

        // Navigate back down
        let next = state.history_down();
        assert_eq!(next, Some("third".to_string()));
    }

    #[test]
    fn test_folder_picker_visibility() {
        let mut state = InputState::new_without_history();
        assert!(!state.is_folder_picker_visible());

        state.show_folder_picker();
        assert!(state.is_folder_picker_visible());

        state.hide_folder_picker();
        assert!(!state.is_folder_picker_visible());
    }

    #[test]
    fn test_folder_picker_filter() {
        let mut state = InputState::new_without_history();
        state.folders = vec![
            Folder {
                name: "Project Alpha".to_string(),
                path: "/alpha".to_string(),
            },
            Folder {
                name: "Project Beta".to_string(),
                path: "/beta".to_string(),
            },
            Folder {
                name: "Other".to_string(),
                path: "/other".to_string(),
            },
        ];

        // No filter - all folders
        assert_eq!(state.get_filtered_folders().len(), 3);

        // Filter by "project"
        state.set_folder_filter("project".to_string());
        let filtered = state.get_filtered_folders();
        assert_eq!(filtered.len(), 2);

        // Filter by "alpha"
        state.set_folder_filter("alpha".to_string());
        let filtered = state.get_filtered_folders();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Project Alpha");
    }

    #[test]
    fn test_folder_picker_navigation() {
        let mut state = InputState::new_without_history();
        state.folders = vec![
            Folder {
                name: "Folder 1".to_string(),
                path: "/f1".to_string(),
            },
            Folder {
                name: "Folder 2".to_string(),
                path: "/f2".to_string(),
            },
            Folder {
                name: "Folder 3".to_string(),
                path: "/f3".to_string(),
            },
        ];

        assert_eq!(state.folder_picker_cursor, 0);

        state.folder_picker_down();
        assert_eq!(state.folder_picker_cursor, 1);

        state.folder_picker_down();
        assert_eq!(state.folder_picker_cursor, 2);

        // Can't go past the end
        state.folder_picker_down();
        assert_eq!(state.folder_picker_cursor, 2);

        state.folder_picker_up();
        assert_eq!(state.folder_picker_cursor, 1);

        // Can't go before the beginning
        state.folder_picker_up();
        state.folder_picker_up();
        assert_eq!(state.folder_picker_cursor, 0);
    }

    #[test]
    fn test_folder_selection() {
        let mut state = InputState::new_without_history();
        state.folders = vec![Folder {
            name: "Test Folder".to_string(),
            path: "/test".to_string(),
        }];

        state.show_folder_picker();
        let selected = state.select_folder();

        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name, "Test Folder");
        assert!(state.selected_folder.is_some());
        assert!(!state.is_folder_picker_visible()); // Should hide after selection
    }

    #[test]
    fn test_clear_selected_folder() {
        let mut state = InputState::new_without_history();
        state.selected_folder = Some(Folder {
            name: "Test".to_string(),
            path: "/test".to_string(),
        });

        state.clear_selected_folder();
        assert!(state.selected_folder.is_none());
    }

    #[test]
    fn test_folders_loading_state() {
        let mut state = InputState::new_without_history();
        assert!(!state.is_loading_folders());

        state.start_loading_folders();
        assert!(state.is_loading_folders());
        assert!(state.folders_error.is_none());

        state.set_folders(vec![Folder {
            name: "Loaded Folder".to_string(),
            path: "/loaded".to_string(),
        }]);
        assert!(!state.is_loading_folders());
        assert_eq!(state.folders.len(), 1);
    }

    #[test]
    fn test_folders_error_state() {
        let mut state = InputState::new_without_history();

        state.start_loading_folders();
        state.set_folders_error("Failed to load".to_string());

        assert!(!state.is_loading_folders());
        assert_eq!(state.folders_error, Some("Failed to load".to_string()));
    }

    #[test]
    fn test_submit() {
        let mut state = InputState::new_without_history();
        state.set_input("test input");

        let submitted = state.submit();

        assert_eq!(submitted, "test input");
        assert!(state.is_empty());
    }

    #[test]
    fn test_submit_adds_to_history() {
        let mut state = InputState::new_without_history();
        state.set_input("first submission");
        state.submit();

        state.set_input("second submission");
        state.submit();

        // Navigate up through history
        state.set_input("current");
        let prev = state.history_up();
        assert_eq!(prev, Some("second submission".to_string()));

        let prev = state.history_up();
        assert_eq!(prev, Some("first submission".to_string()));
    }

    #[test]
    fn test_reset() {
        let mut state = InputState::new_without_history();
        state.set_input("some text");
        state.show_folder_picker();
        state.selected_folder = Some(Folder {
            name: "Test".to_string(),
            path: "/test".to_string(),
        });
        state.folders_error = Some("error".to_string());

        state.reset();

        assert!(state.is_empty());
        assert!(!state.is_folder_picker_visible());
        assert!(state.selected_folder.is_none());
        assert!(state.folders_error.is_none());
    }

    #[test]
    fn test_debug_impl() {
        let state = InputState::new_without_history();
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("InputState"));
        assert!(debug_str.contains("input_text"));
        assert!(debug_str.contains("history_len"));
    }
}
