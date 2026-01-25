//! State accessor and utility methods for the App.

use std::sync::Arc;
use tokio::sync::mpsc;

use super::{App, AppMessage, ScrollBoundary};

impl App {
    /// Mark the UI as needing a redraw.
    /// Call this method after any state mutation that affects the UI.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.needs_redraw = true;
    }
    /// Get a clone of the message sender for passing to async tasks
    pub fn message_sender(&self) -> mpsc::UnboundedSender<AppMessage> {
        self.message_tx.clone()
    }

    /// Spawn an async task to check connection status.
    ///
    /// This calls the ConductorClient health_check and sends the result
    /// via the message channel. The App will update connection_status
    /// when the message is received.
    pub fn check_connection(&self) {
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);
        tokio::spawn(async move {
            let connected: bool = (client.health_check().await).unwrap_or_default();
            let _ = tx.send(AppMessage::ConnectionStatus(connected));
        });
    }

    /// Clear the current stream error
    pub fn clear_error(&mut self) {
        self.stream_error = None;
        self.mark_dirty();
    }

    /// Reset scroll state to bottom (newest content)
    pub fn reset_scroll(&mut self) {
        self.unified_scroll = 0;
        self.scroll_position = 0.0;
        self.scroll_velocity = 0.0;
        self.user_has_scrolled = false;
        self.mark_dirty();
    }

    /// Increment the tick counter for animations and update smooth scrolling
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);

        // Only update smooth scrolling if there's meaningful velocity
        const VELOCITY_THRESHOLD: f32 = 0.1;
        let has_velocity = self.scroll_velocity.abs() > VELOCITY_THRESHOLD;
        if has_velocity {
            self.update_smooth_scroll();
        } else if self.scroll_velocity != 0.0 {
            // Zero out small residual velocities
            self.scroll_velocity = 0.0;
        }

        // Mark dirty if there are active animations:
        // - Scroll momentum (velocity > 0)
        // - Streaming (spinner animation)
        // - Boundary hit indicator (fades after a few ticks)
        if has_velocity || self.is_streaming() || self.scroll_boundary_hit.is_some() {
            self.mark_dirty();
        }

        // Only check boundary expiration when there is one
        if self.scroll_boundary_hit.is_some() {
            // Clear after 10 ticks (~160ms at 16ms/tick)
            if self.tick_count.saturating_sub(self.boundary_hit_tick) > 10 {
                self.scroll_boundary_hit = None;
                self.mark_dirty();
            }
        }

        // Reset Ctrl+C state after 2 seconds
        if let Some(last_time) = self.last_ctrl_c_time {
            if last_time.elapsed().as_secs() >= 2 {
                self.last_ctrl_c_time = None;
                self.mark_dirty();
            }
        }
    }

    /// Update smooth scroll position with velocity and friction
    ///
    /// The momentum system reads/writes `unified_scroll` as the source of truth.
    /// `scroll_position` is used only for sub-line precision during animation,
    /// and is synced FROM `unified_scroll` when momentum stops.
    fn update_smooth_scroll(&mut self) {
        // Friction factor: lower = more friction, stops faster
        const FRICTION: f32 = 0.85;
        const VELOCITY_THRESHOLD: f32 = 0.1;

        // Skip if no velocity
        if self.scroll_velocity.abs() < VELOCITY_THRESHOLD {
            self.scroll_velocity = 0.0;
            // Sync scroll_position from unified_scroll when momentum stops
            self.scroll_position = self.unified_scroll as f32;
            return;
        }

        // Apply velocity to position (sub-line precision for smooth animation)
        let new_position = self.scroll_position + self.scroll_velocity;

        // Clamp to valid range [0, max_scroll]
        let max = self.max_scroll as f32;
        let clamped_position = new_position.clamp(0.0, max);

        // Check for boundary hits
        if new_position < 0.0 && self.scroll_position >= 0.0 {
            // Hit bottom boundary
            self.scroll_boundary_hit = Some(ScrollBoundary::Bottom);
            self.boundary_hit_tick = self.tick_count;
            self.scroll_velocity = 0.0; // Stop on boundary hit
            self.user_has_scrolled = false; // Back at bottom
        } else if new_position > max && self.scroll_position <= max && self.max_scroll > 0 {
            // Hit top boundary
            self.scroll_boundary_hit = Some(ScrollBoundary::Top);
            self.boundary_hit_tick = self.tick_count;
            self.scroll_velocity = 0.0; // Stop on boundary hit
        } else {
            // Apply friction when not hitting boundary
            self.scroll_velocity *= FRICTION;
        }

        // Update scroll_position for sub-line precision during animation
        self.scroll_position = clamped_position;
        // Update unified_scroll as the source of truth (rounded to whole lines)
        self.unified_scroll = clamped_position.round() as u16;
    }

    /// Check if the currently active thread is a Programming thread
    pub fn is_active_thread_programming(&self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            if let Some(thread) = self.cache.get_thread(thread_id) {
                return thread.thread_type == crate::models::ThreadType::Programming;
            }
        }
        false
    }

    /// Check if there is currently an active streaming message
    pub fn is_streaming(&self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.is_thread_streaming(thread_id)
        } else {
            false
        }
    }

    /// Toggle reasoning collapsed state for the last message with reasoning
    /// Returns true if a reasoning block was toggled
    pub fn toggle_reasoning(&mut self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            if let Some(idx) = self.cache.find_last_reasoning_message_index(thread_id) {
                let toggled = self.cache.toggle_message_reasoning(thread_id, idx);
                if toggled {
                    self.mark_dirty();
                }
                return toggled;
            }
        }
        false
    }

    /// Dismiss the currently focused error for the active thread
    /// Returns true if an error was dismissed
    pub fn dismiss_focused_error(&mut self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            let dismissed = self.cache.dismiss_focused_error(thread_id);
            if dismissed {
                self.mark_dirty();
            }
            dismissed
        } else {
            false
        }
    }

    /// Check if the active thread has any errors
    pub fn has_errors(&self) -> bool {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.error_count(thread_id) > 0
        } else {
            false
        }
    }

    /// Add an error to the active thread
    pub fn add_error_to_active_thread(&mut self, error_code: String, message: String) {
        if let Some(thread_id) = &self.active_thread_id {
            self.cache.add_error_simple(thread_id, error_code, message);
            self.mark_dirty();
        }
    }

    /// Update terminal dimensions
    ///
    /// Called when the terminal is resized or on initial setup.
    /// Updates both width and height in a single call.
    pub fn update_terminal_dimensions(&mut self, width: u16, height: u16) {
        if self.terminal_width != width || self.terminal_height != height {
            self.terminal_width = width;
            self.terminal_height = height;
            self.mark_dirty();
        }
    }

    /// Get the current terminal width
    pub fn terminal_width(&self) -> u16 {
        self.terminal_width
    }

    /// Get the current terminal height
    pub fn terminal_height(&self) -> u16 {
        self.terminal_height
    }

    /// Calculate the available content area width
    ///
    /// This accounts for borders and margins (2 cells on each side).
    pub fn content_width(&self) -> u16 {
        self.terminal_width.saturating_sub(4)
    }

    /// Calculate the available content area height
    ///
    /// This accounts for header, footer, and borders (approximately 6 rows).
    pub fn content_height(&self) -> u16 {
        self.terminal_height.saturating_sub(6)
    }

    /// Check if pasted text should be summarized
    pub fn should_summarize_paste(&self, text: &str) -> bool {
        let line_count = text.lines().count();
        let char_count = text.chars().count();
        line_count > 3 || char_count > 150
    }

    /// Load folders from the backend API.
    ///
    /// Sets folders_loading = true and spawns an async task to fetch folders.
    /// On success, sends FoldersLoaded message with the folder list.
    /// On error, sends FoldersLoadFailed message with the error description.
    pub fn load_folders(&mut self) {
        // Set loading state
        self.folders_loading = true;
        self.folders_error = None;
        self.mark_dirty();

        // Spawn async task to fetch folders
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);

        tokio::spawn(async move {
            match client.fetch_folders().await {
                Ok(folders) => {
                    let _ = tx.send(AppMessage::FoldersLoaded(folders));
                }
                Err(e) => {
                    let error_msg = format!("Failed to load folders: {}", e);
                    let _ = tx.send(AppMessage::FoldersLoadFailed(error_msg));
                }
            }
        });
    }

    /// Load GitHub repositories from the conductor API.
    ///
    /// Spawns an async task to fetch repos without blocking the UI.
    /// Results are sent back via AppMessage channel.
    ///
    /// On success, sends ReposLoaded message with the repos list.
    /// On error, sends ReposLoadFailed message with the error description.
    pub fn load_repos(&mut self) {
        // Set loading state
        self.repos_loading = true;
        self.repos_error = None;
        self.mark_dirty();

        // Spawn async task to fetch repos
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);

        tokio::spawn(async move {
            match client.fetch_repos().await {
                Ok(repos) => {
                    let _ = tx.send(AppMessage::ReposLoaded(repos));
                }
                Err(e) => {
                    let error_msg = format!("Failed to load repos: {}", e);
                    let _ = tx.send(AppMessage::ReposLoadFailed(error_msg));
                }
            }
        });
    }

    // =========================================================================
    // Folder Picker Methods
    // =========================================================================

    /// Check if @ at the given position should trigger the folder picker.
    ///
    /// Trigger rules:
    /// - ONLY on CommandDeck screen (not Conversation)
    /// - @ at position 0 → trigger
    /// - @ immediately after whitespace → trigger
    /// - @ inside a word → no trigger, type literal @
    ///
    /// # Arguments
    /// * `line_content` - The content of the current line
    /// * `col` - Column position within the line where @ would be inserted
    ///
    /// # Returns
    /// `true` if @ should trigger the folder picker, `false` otherwise
    pub fn is_folder_picker_trigger(&self, line_content: &str, col: usize) -> bool {
        // Only trigger on CommandDeck screen
        if self.screen != super::Screen::CommandDeck {
            return false;
        }

        // @ at position 0 always triggers
        if col == 0 {
            return true;
        }

        // Check character before cursor position
        // Get the character at position col-1
        if let Some(prev_char) = line_content.chars().nth(col.saturating_sub(1)) {
            // @ after whitespace triggers
            prev_char.is_whitespace()
        } else {
            // Empty line or at start
            true
        }
    }

    /// Open the folder picker overlay.
    ///
    /// Sends the FolderPickerOpen message to set visibility and reset state.
    pub fn open_folder_picker(&mut self) {
        self.handle_message(super::AppMessage::FolderPickerOpen);
    }

    /// Close the folder picker overlay.
    ///
    /// Sends the FolderPickerClose message to hide and reset state.
    pub fn close_folder_picker(&mut self) {
        self.handle_message(super::AppMessage::FolderPickerClose);
    }

    /// Get filtered folders based on the current filter text.
    ///
    /// Performs case-insensitive matching on folder name and path.
    ///
    /// # Returns
    /// Vector of references to folders that match the filter.
    pub fn filtered_folders(&self) -> Vec<&crate::models::Folder> {
        if self.folder_picker_filter.is_empty() {
            // No filter - return all folders
            self.folders.iter().collect()
        } else {
            let filter_lower = self.folder_picker_filter.to_lowercase();
            self.folders
                .iter()
                .filter(|f| {
                    f.name.to_lowercase().contains(&filter_lower)
                        || f.path.to_lowercase().contains(&filter_lower)
                })
                .collect()
        }
    }

    /// Handle a character typed while the folder picker is open.
    ///
    /// Appends the character to the filter text.
    pub fn folder_picker_type_char(&mut self, c: char) {
        self.folder_picker_filter.push(c);
        self.folder_picker_cursor = 0; // Reset cursor when filter changes

        // Debug: emit filter change event
        self.emit_debug_state_change(
            "FolderPickerFilter",
            &format!("char='{}' filter='{}'", c, self.folder_picker_filter),
            &format!("matches={}", self.filtered_folders().len()),
        );

        self.mark_dirty();
    }

    /// Handle backspace while the folder picker is open.
    ///
    /// If filter has text, removes last character.
    /// If filter is empty, closes the picker and removes @ from input.
    ///
    /// # Returns
    /// `true` if the picker should be closed (filter was empty), `false` otherwise
    pub fn folder_picker_backspace(&mut self) -> bool {
        if self.folder_picker_filter.is_empty() {
            // Close picker when backspacing with empty filter
            true
        } else {
            // Remove last character from filter
            self.folder_picker_filter.pop();
            self.folder_picker_cursor = 0; // Reset cursor when filter changes
            self.mark_dirty();
            false
        }
    }

    /// Move the folder picker cursor up.
    ///
    /// Clamps at 0 (top of list).
    pub fn folder_picker_cursor_up(&mut self) {
        if self.folder_picker_cursor > 0 {
            self.folder_picker_cursor -= 1;
            self.mark_dirty();
        }
    }

    /// Move the folder picker cursor down.
    ///
    /// Clamps at the last item in the filtered list.
    pub fn folder_picker_cursor_down(&mut self) {
        let filtered_count = self.filtered_folders().len();
        if filtered_count > 0 && self.folder_picker_cursor < filtered_count - 1 {
            self.folder_picker_cursor += 1;
            self.mark_dirty();
        }
    }

    /// Select the currently highlighted folder in the picker.
    ///
    /// # Returns
    /// The selected folder, or None if no valid selection.
    pub fn folder_picker_select(&mut self) -> Option<crate::models::Folder> {
        let filtered = self.filtered_folders();
        if let Some(folder) = filtered.get(self.folder_picker_cursor) {
            let folder_clone = (*folder).clone();
            self.handle_message(super::AppMessage::FolderSelected(folder_clone.clone()));
            Some(folder_clone)
        } else {
            None
        }
    }

    /// Remove the @ and any filter text from the textarea input.
    ///
    /// This is called when closing the picker via Escape to clean up
    /// the @ trigger character and any typed filter text.
    pub fn remove_at_and_filter_from_input(&mut self) {
        // Calculate how many characters to remove: @ + filter length
        let chars_to_remove = 1 + self.folder_picker_filter.len();

        // Remove characters by calling backspace repeatedly
        for _ in 0..chars_to_remove {
            self.textarea.backspace();
        }
        self.mark_dirty();
    }

    // =========================================================================
    // Folder Selection Methods
    // =========================================================================

    /// Select a folder and display it as a chip in the input area.
    ///
    /// This is called when the user presses Enter on a highlighted folder
    /// in the folder picker. It:
    /// 1. Sets the selected folder
    /// 2. Closes the folder picker
    /// 3. Clears the @ and filter text from the textarea
    pub fn select_folder(&mut self, folder: crate::models::Folder) {
        self.selected_folder = Some(folder.clone());
        self.folder_picker_visible = false;
        self.folder_picker_filter.clear();
        self.folder_picker_cursor = 0;

        // Remove @ + filter from textarea
        self.remove_at_and_filter_from_input();

        self.mark_dirty();
    }

    /// Clear the currently selected folder.
    ///
    /// Called when the user presses backspace at cursor position 0
    /// while a folder chip is displayed.
    pub fn clear_folder(&mut self) {
        if self.selected_folder.is_some() {
            self.selected_folder = None;
            self.handle_message(super::AppMessage::FolderCleared);
            self.mark_dirty();
        }
    }

    /// Check if backspace at the current cursor position should clear the folder chip.
    ///
    /// Returns true if:
    /// - A folder is selected (chip is displayed)
    /// - Cursor is at position (0, 0) - start of input
    pub fn should_clear_folder_on_backspace(&self) -> bool {
        if self.selected_folder.is_none() {
            return false;
        }
        let (row, col) = self.textarea.cursor();
        row == 0 && col == 0
    }

    // =========================================================================
    // Slash Command Autocomplete Methods
    // =========================================================================

    /// Get filtered slash commands matching the current query.
    ///
    /// # Returns
    /// Vector of SlashCommand instances that match the filter.
    pub fn filtered_slash_commands(&self) -> Vec<crate::input::SlashCommand> {
        crate::input::SlashCommand::filter(&self.slash_autocomplete_query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Folder;

    fn create_test_app() -> App {
        let mut app = App::default();
        app.screen = super::super::Screen::CommandDeck; // Default to CommandDeck for tests
        app
    }

    fn create_test_folder(name: &str, path: &str) -> Folder {
        Folder {
            name: name.to_string(),
            path: path.to_string(),
        }
    }

    // =========================================================================
    // is_folder_picker_trigger tests
    // =========================================================================

    #[test]
    fn test_folder_picker_trigger_at_position_0() {
        let app = create_test_app();
        // @ at position 0 on empty line should trigger
        assert!(app.is_folder_picker_trigger("", 0));
    }

    #[test]
    fn test_folder_picker_trigger_after_whitespace() {
        let app = create_test_app();
        // @ after space should trigger
        assert!(app.is_folder_picker_trigger("hello ", 6));
        // @ after tab should trigger
        assert!(app.is_folder_picker_trigger("hello\t", 6));
    }

    #[test]
    fn test_folder_picker_no_trigger_inside_word() {
        let app = create_test_app();
        // @ in middle of word should NOT trigger
        assert!(!app.is_folder_picker_trigger("hello", 3));
        assert!(!app.is_folder_picker_trigger("email", 2));
    }

    #[test]
    fn test_folder_picker_no_trigger_on_conversation_screen() {
        let mut app = create_test_app();
        app.screen = super::super::Screen::Conversation;
        // @ should NOT trigger on Conversation screen
        assert!(!app.is_folder_picker_trigger("", 0));
        assert!(!app.is_folder_picker_trigger("hello ", 6));
    }

    #[test]
    fn test_folder_picker_trigger_on_command_deck_only() {
        let mut app = create_test_app();
        app.screen = super::super::Screen::CommandDeck;
        // @ should trigger on CommandDeck
        assert!(app.is_folder_picker_trigger("", 0));
    }

    // =========================================================================
    // filtered_folders tests
    // =========================================================================

    #[test]
    fn test_filtered_folders_no_filter_returns_all() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("project1", "/home/user/project1"),
            create_test_folder("project2", "/home/user/project2"),
        ];
        app.folder_picker_filter = String::new();

        let filtered = app.filtered_folders();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filtered_folders_by_name() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("my-project", "/home/user/my-project"),
            create_test_folder("other-app", "/home/user/other-app"),
        ];
        app.folder_picker_filter = "proj".to_string();

        let filtered = app.filtered_folders();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "my-project");
    }

    #[test]
    fn test_filtered_folders_by_path() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("project1", "/home/alice/project1"),
            create_test_folder("project2", "/home/bob/project2"),
        ];
        app.folder_picker_filter = "bob".to_string();

        let filtered = app.filtered_folders();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "project2");
    }

    #[test]
    fn test_filtered_folders_case_insensitive() {
        let mut app = create_test_app();
        app.folders = vec![create_test_folder("MyProject", "/home/user/MyProject")];
        app.folder_picker_filter = "myproject".to_string();

        let filtered = app.filtered_folders();
        assert_eq!(filtered.len(), 1);
    }

    // =========================================================================
    // folder_picker_type_char tests
    // =========================================================================

    #[test]
    fn test_folder_picker_type_char_appends_to_filter() {
        let mut app = create_test_app();
        app.folder_picker_filter = "proj".to_string();

        app.folder_picker_type_char('e');

        assert_eq!(app.folder_picker_filter, "proje");
    }

    #[test]
    fn test_folder_picker_type_char_resets_cursor() {
        let mut app = create_test_app();
        app.folder_picker_cursor = 5;

        app.folder_picker_type_char('a');

        assert_eq!(app.folder_picker_cursor, 0);
    }

    // =========================================================================
    // folder_picker_backspace tests
    // =========================================================================

    #[test]
    fn test_folder_picker_backspace_removes_char() {
        let mut app = create_test_app();
        app.folder_picker_filter = "proj".to_string();

        let should_close = app.folder_picker_backspace();

        assert!(!should_close);
        assert_eq!(app.folder_picker_filter, "pro");
    }

    #[test]
    fn test_folder_picker_backspace_empty_filter_returns_true() {
        let mut app = create_test_app();
        app.folder_picker_filter = String::new();

        let should_close = app.folder_picker_backspace();

        assert!(should_close);
    }

    // =========================================================================
    // folder_picker_cursor tests
    // =========================================================================

    #[test]
    fn test_folder_picker_cursor_up_decrements() {
        let mut app = create_test_app();
        app.folder_picker_cursor = 2;

        app.folder_picker_cursor_up();

        assert_eq!(app.folder_picker_cursor, 1);
    }

    #[test]
    fn test_folder_picker_cursor_up_clamps_at_zero() {
        let mut app = create_test_app();
        app.folder_picker_cursor = 0;

        app.folder_picker_cursor_up();

        assert_eq!(app.folder_picker_cursor, 0);
    }

    #[test]
    fn test_folder_picker_cursor_down_increments() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("a", "/a"),
            create_test_folder("b", "/b"),
            create_test_folder("c", "/c"),
        ];
        app.folder_picker_cursor = 0;

        app.folder_picker_cursor_down();

        assert_eq!(app.folder_picker_cursor, 1);
    }

    #[test]
    fn test_folder_picker_cursor_down_clamps_at_end() {
        let mut app = create_test_app();
        app.folders = vec![create_test_folder("a", "/a"), create_test_folder("b", "/b")];
        app.folder_picker_cursor = 1; // Already at last item

        app.folder_picker_cursor_down();

        assert_eq!(app.folder_picker_cursor, 1); // Should stay at 1
    }

    // =========================================================================
    // folder_picker_select tests
    // =========================================================================

    #[test]
    fn test_folder_picker_select_returns_folder() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("project1", "/home/project1"),
            create_test_folder("project2", "/home/project2"),
        ];
        app.folder_picker_cursor = 1;

        let selected = app.folder_picker_select();

        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name, "project2");
    }

    #[test]
    fn test_folder_picker_select_empty_list_returns_none() {
        let mut app = create_test_app();
        app.folders = vec![];
        app.folder_picker_cursor = 0;

        let selected = app.folder_picker_select();

        assert!(selected.is_none());
    }

    #[test]
    fn test_folder_picker_select_closes_picker() {
        let mut app = create_test_app();
        app.folders = vec![create_test_folder("project", "/home/project")];
        app.folder_picker_visible = true;
        app.folder_picker_cursor = 0;

        app.folder_picker_select();

        assert!(!app.folder_picker_visible);
    }

    // =========================================================================
    // open_folder_picker and close_folder_picker tests
    // =========================================================================

    #[test]
    fn test_open_folder_picker() {
        let mut app = create_test_app();
        app.folder_picker_visible = false;
        app.folder_picker_filter = "old".to_string();
        app.folder_picker_cursor = 5;

        app.open_folder_picker();

        assert!(app.folder_picker_visible);
        assert!(app.folder_picker_filter.is_empty());
        assert_eq!(app.folder_picker_cursor, 0);
    }

    #[test]
    fn test_close_folder_picker() {
        let mut app = create_test_app();
        app.folder_picker_visible = true;
        app.folder_picker_filter = "filter".to_string();
        app.folder_picker_cursor = 3;

        app.close_folder_picker();

        assert!(!app.folder_picker_visible);
        assert!(app.folder_picker_filter.is_empty());
        assert_eq!(app.folder_picker_cursor, 0);
    }

    // =========================================================================
    // select_folder and clear_folder tests
    // =========================================================================

    #[test]
    fn test_select_folder_sets_selected_folder() {
        let mut app = create_test_app();
        let folder = create_test_folder("my-project", "/home/user/my-project");

        app.select_folder(folder.clone());

        assert!(app.selected_folder.is_some());
        assert_eq!(app.selected_folder.as_ref().unwrap().name, "my-project");
    }

    #[test]
    fn test_select_folder_closes_picker() {
        let mut app = create_test_app();
        app.folder_picker_visible = true;
        app.folder_picker_filter = "proj".to_string();
        app.folder_picker_cursor = 2;
        let folder = create_test_folder("my-project", "/home/user/my-project");

        app.select_folder(folder);

        assert!(!app.folder_picker_visible);
        assert!(app.folder_picker_filter.is_empty());
        assert_eq!(app.folder_picker_cursor, 0);
    }

    #[test]
    fn test_clear_folder() {
        let mut app = create_test_app();
        let folder = create_test_folder("my-project", "/home/user/my-project");
        app.selected_folder = Some(folder);

        app.clear_folder();

        assert!(app.selected_folder.is_none());
    }

    #[test]
    fn test_clear_folder_when_none_selected() {
        let mut app = create_test_app();
        app.selected_folder = None;

        // Should not panic when no folder is selected
        app.clear_folder();

        assert!(app.selected_folder.is_none());
    }

    // =========================================================================
    // should_clear_folder_on_backspace tests
    // =========================================================================

    #[test]
    fn test_should_clear_folder_on_backspace_at_start() {
        let mut app = create_test_app();
        let folder = create_test_folder("my-project", "/home/user/my-project");
        app.selected_folder = Some(folder);
        // Cursor should be at (0, 0) by default for empty input

        assert!(app.should_clear_folder_on_backspace());
    }

    #[test]
    fn test_should_not_clear_folder_when_cursor_not_at_start() {
        let mut app = create_test_app();
        let folder = create_test_folder("my-project", "/home/user/my-project");
        app.selected_folder = Some(folder);
        // Type something to move cursor away from start
        app.textarea.insert_char('h');
        app.textarea.insert_char('i');

        assert!(!app.should_clear_folder_on_backspace());
    }

    #[test]
    fn test_should_not_clear_folder_when_no_folder_selected() {
        let mut app = create_test_app();
        app.selected_folder = None;
        // Cursor at (0, 0)

        assert!(!app.should_clear_folder_on_backspace());
    }

    #[test]
    fn test_should_not_clear_folder_when_on_second_line() {
        let mut app = create_test_app();
        let folder = create_test_folder("my-project", "/home/user/my-project");
        app.selected_folder = Some(folder);
        // Add content and a newline
        app.textarea.insert_char('x');
        app.textarea.insert_newline();
        // Cursor is now on line 1, column 0

        assert!(!app.should_clear_folder_on_backspace());
    }

    // =========================================================================
    // Additional edge case tests for @ trigger
    // =========================================================================

    #[test]
    fn test_folder_picker_no_trigger_email_address() {
        let app = create_test_app();
        // Simulating typing "email@test.com" - the @ comes after "email"
        // Position 5 is where @ would be inserted (after 'email')
        assert!(!app.is_folder_picker_trigger("email", 5));
    }

    #[test]
    fn test_folder_picker_no_trigger_mid_word_various() {
        let app = create_test_app();
        // Various mid-word positions should NOT trigger
        assert!(!app.is_folder_picker_trigger("user", 4)); // user@ (email pattern)
        assert!(!app.is_folder_picker_trigger("test", 2)); // te@st
        assert!(!app.is_folder_picker_trigger("name123", 4)); // name@123
    }

    #[test]
    fn test_folder_picker_trigger_start_of_line() {
        let app = create_test_app();
        // @ at position 0 on line with content should trigger
        // (user will type @ before existing content)
        assert!(app.is_folder_picker_trigger("existing text", 0));
    }

    #[test]
    fn test_folder_picker_trigger_after_multiple_spaces() {
        let app = create_test_app();
        // @ after multiple spaces should trigger
        assert!(app.is_folder_picker_trigger("hello   ", 8));
    }

    // =========================================================================
    // Filtering edge cases
    // =========================================================================

    #[test]
    fn test_filtered_folders_no_matches() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("project1", "/home/user/project1"),
            create_test_folder("project2", "/home/user/project2"),
        ];
        app.folder_picker_filter = "xyz_nonexistent".to_string();

        let filtered = app.filtered_folders();
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filtered_folders_partial_match() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("my-awesome-project", "/home/user/my-awesome-project"),
            create_test_folder("another-project", "/home/user/another-project"),
            create_test_folder("something-else", "/home/user/something-else"),
        ];
        app.folder_picker_filter = "awe".to_string();

        let filtered = app.filtered_folders();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "my-awesome-project");
    }

    #[test]
    fn test_filtered_folders_matches_path_not_name() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("project", "/home/special-user/project"),
            create_test_folder("other", "/home/normal-user/other"),
        ];
        app.folder_picker_filter = "special".to_string();

        let filtered = app.filtered_folders();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "project");
    }

    // =========================================================================
    // Navigation edge cases
    // =========================================================================

    #[test]
    fn test_folder_picker_cursor_down_empty_list() {
        let mut app = create_test_app();
        app.folders = vec![];
        app.folder_picker_cursor = 0;

        // Should not panic with empty list
        app.folder_picker_cursor_down();

        assert_eq!(app.folder_picker_cursor, 0);
    }

    #[test]
    fn test_folder_picker_cursor_up_empty_list() {
        let mut app = create_test_app();
        app.folders = vec![];
        app.folder_picker_cursor = 0;

        // Should not panic with empty list
        app.folder_picker_cursor_up();

        assert_eq!(app.folder_picker_cursor, 0);
    }

    #[test]
    fn test_folder_picker_cursor_clamps_after_filter_reduces_list() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("a", "/a"),
            create_test_folder("b", "/b"),
            create_test_folder("c", "/c"),
        ];
        app.folder_picker_cursor = 2; // Pointing to 'c'

        // Now filter to just one item
        app.folder_picker_filter = "a".to_string();

        // Type a character to trigger cursor reset
        app.folder_picker_type_char('a');

        // Cursor should be reset to 0 when filter changes
        assert_eq!(app.folder_picker_cursor, 0);
    }

    #[test]
    fn test_folder_picker_select_with_filter_active() {
        let mut app = create_test_app();
        app.folders = vec![
            create_test_folder("alpha", "/alpha"),
            create_test_folder("beta", "/beta"),
            create_test_folder("gamma", "/gamma"),
        ];
        app.folder_picker_filter = "bet".to_string();
        app.folder_picker_cursor = 0;

        let selected = app.folder_picker_select();

        assert!(selected.is_some());
        // Should select 'beta' which is the only match
        assert_eq!(selected.unwrap().name, "beta");
    }

    // =========================================================================
    // Thread creation with working_directory tests
    // =========================================================================

    #[test]
    fn test_working_directory_extracted_from_selected_folder() {
        let mut app = create_test_app();
        let folder = create_test_folder("my-project", "/Users/dev/my-project");
        app.selected_folder = Some(folder);

        // The working_directory extraction happens in stream.rs
        // Here we just verify the selected_folder is accessible
        let wd = app.selected_folder.as_ref().map(|f| f.path.clone());
        assert_eq!(wd, Some("/Users/dev/my-project".to_string()));
    }

    #[test]
    fn test_no_working_directory_when_no_folder_selected() {
        let app = create_test_app();
        // No folder selected
        assert!(app.selected_folder.is_none());

        let wd = app.selected_folder.as_ref().map(|f| f.path.clone());
        assert!(wd.is_none());
    }
}
