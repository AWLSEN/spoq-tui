//! State accessor and utility methods for the App.

use std::sync::Arc;
use tokio::sync::mpsc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

    /// Reset cursor blink timer - call on any input activity
    /// This makes cursor solid immediately and restarts blinkwait countdown
    pub fn reset_cursor_blink(&mut self) {
        self.cursor_blink.reset(self.tick_count);
        self.mark_dirty(); // Immediate redraw to show solid cursor
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

        // Process batched scroll events (one render per tick for all scroll events)
        if self.scroll_changed {
            self.scroll_changed = false;
            self.mark_dirty();
        }

        // Update cursor blink state and mark dirty if visibility changed
        let cursor_visibility_changed = self.cursor_blink.update(self.tick_count);
        if cursor_visibility_changed {
            self.mark_dirty();
        }

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

        // Check auto-close timer for Claude login success dialog
        if let Some(close_time) = self.claude_login_auto_close {
            if std::time::Instant::now() >= close_time {
                // Close the Claude login overlay
                use crate::view_state::OverlayState;
                if let Some(OverlayState::ClaudeLogin { .. }) = self.dashboard.overlay() {
                    self.dashboard.collapse_overlay();
                    self.claude_login_auto_close = None;
                    self.mark_dirty();
                }
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
        const FRICTION: f32 = 0.6;
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
            self.scroll_velocity *= 0.1; // Damped stop instead of hard stop
            self.user_has_scrolled = false; // Back at bottom
        } else if new_position > max && self.scroll_position <= max && self.max_scroll > 0 {
            // Hit top boundary
            self.scroll_boundary_hit = Some(ScrollBoundary::Top);
            self.boundary_hit_tick = self.tick_count;
            self.scroll_velocity *= 0.1; // Damped stop instead of hard stop
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

    // =========================================================================
    // File Picker (Conversation Screen)
    // =========================================================================

    /// Check if an @ character should trigger the file picker.
    ///
    /// The file picker is triggered when:
    /// - Current screen is Conversation
    /// - @ is at position 0 (start of line), OR
    /// - @ is immediately after whitespace
    ///
    /// This prevents triggering on email addresses like `user@example.com`.
    ///
    /// # Arguments
    /// * `line_content` - The content of the current line
    /// * `col` - Column position within the line where @ would be inserted
    ///
    /// # Returns
    /// `true` if @ should trigger the file picker, `false` otherwise
    pub fn is_file_picker_trigger(&self, line_content: &str, col: usize) -> bool {
        // Only trigger on Conversation screen
        if self.screen != super::Screen::Conversation {
            return false;
        }

        // @ at position 0 always triggers
        if col == 0 {
            return true;
        }

        // Check character before cursor position
        if let Some(prev_char) = line_content.chars().nth(col.saturating_sub(1)) {
            // @ after whitespace triggers
            prev_char.is_whitespace()
        } else {
            // Empty line or at start
            true
        }
    }

    /// Check if a `/` character should trigger slash command autocomplete.
    ///
    /// Slash autocomplete is ONLY triggered when:
    /// - The textarea is completely empty (no prior content)
    /// - The cursor is at position (0, 0) - first line, first column
    ///
    /// This is stricter than file/folder pickers - no whitespace allowed,
    /// must be the absolute very start of input. Applies to both Conversation
    /// and CommandDeck screens for consistency.
    ///
    /// # Returns
    /// `true` if `/` should trigger slash autocomplete, `false` otherwise
    pub fn is_slash_autocomplete_trigger(&self) -> bool {
        // Textarea must be completely empty
        if !self.textarea.is_empty() {
            return false;
        }

        // Cursor must be at the very start (row 0, col 0)
        let (row, col) = self.textarea.cursor();
        if row != 0 || col != 0 {
            return false;
        }

        true
    }

    /// Open the file picker overlay for the current thread.
    ///
    /// Uses the thread's working_directory as the base path.
    /// Falls back to home directory if thread has no working_directory.
    pub fn open_file_picker(&mut self) {
        // Get base path from current thread's working_directory
        let base_path = self
            .active_thread_id
            .as_ref()
            .and_then(|id| self.cache.get_thread(id))
            .and_then(|thread| thread.working_directory.clone())
            .unwrap_or_else(|| {
                // Fall back to home directory
                dirs::home_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/".to_string())
            });

        // Log file picker request details for debugging 404 errors
        let has_working_dir = self
            .active_thread_id
            .as_ref()
            .and_then(|id| self.cache.get_thread(id))
            .and_then(|t| t.working_directory.as_ref())
            .is_some();
        tracing::info!(
            "File picker opening - base_path: {}, thread_id: {:?}, has_working_dir: {}, base_url: {}",
            base_path,
            self.active_thread_id,
            has_working_dir,
            self.client.base_url
        );

        self.file_picker.open(&base_path);
        // Trigger async load of files
        self.load_files(&base_path);
        self.mark_dirty();
    }

    /// Load files from the given directory path.
    ///
    /// Spawns an async task to fetch files from the conductor API.
    pub fn load_files(&mut self, path: &str) {
        let tx = self.message_tx.clone();
        let client = std::sync::Arc::clone(&self.client);
        let path = path.to_string();

        tokio::spawn(async move {
            match client.fetch_files(&path, None).await {
                Ok(files) => {
                    let _ = tx.send(super::AppMessage::FilesLoaded(files));
                }
                Err(e) => {
                    let _ = tx.send(super::AppMessage::FilesLoadFailed(e.to_string()));
                }
            }
        });
    }

    /// Close the file picker overlay.
    pub fn close_file_picker(&mut self) {
        self.file_picker.close();
        self.mark_dirty();
    }

    /// Cancel the file picker (clears selected files).
    pub fn cancel_file_picker(&mut self) {
        self.file_picker.cancel();
        self.mark_dirty();
    }

    /// Remove the @ and filter text from input when closing file picker.
    pub fn remove_at_and_filter_from_input_file_picker(&mut self) {
        // We need to remove the @ plus the query characters that were typed
        let query_len = self.file_picker.query.len();
        // Remove query characters one by one
        for _ in 0..query_len {
            self.textarea.backspace();
        }
        // Remove the @ character
        self.textarea.backspace();
    }

    /// Confirm file picker selection and insert @paths into textarea.
    pub fn confirm_file_picker_selection(&mut self) {
        // Get selected files (or current item if none selected)
        let selected = if self.file_picker.selected_count() > 0 {
            self.file_picker.selected_relative_paths()
        } else if let Some(item) = self.file_picker.selected_item() {
            if !item.is_dir {
                // Single file selected via cursor
                vec![item.relative_path(&self.file_picker.base_path.to_string_lossy())]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        if selected.is_empty() {
            // No files selected, just close picker
            self.remove_at_and_filter_from_input_file_picker();
            self.close_file_picker();
            return;
        }

        // Remove the @ and query from input (we'll add properly formatted @paths)
        self.remove_at_and_filter_from_input_file_picker();

        // Insert @path references for each selected file
        for (i, path) in selected.iter().enumerate() {
            if i > 0 {
                self.textarea.insert_char(' ');
            }
            self.textarea.insert_char('@');
            for c in path.chars() {
                self.textarea.insert_char(c);
            }
        }
        // Add space after last path so user can type message
        self.textarea.insert_char(' ');

        // Close the picker
        self.close_file_picker();
    }

    /// Handle keyboard input for the file picker.
    ///
    /// This method is called from main.rs when the file picker is visible.
    /// Returns `true` if the key was handled, `false` otherwise.
    pub fn handle_file_picker_key(&mut self, key: KeyEvent) -> bool {
        if !self.file_picker.visible {
            return false;
        }

        match key.code {
            KeyCode::Esc => {
                self.remove_at_and_filter_from_input_file_picker();
                self.cancel_file_picker();
                self.mark_dirty();
                true
            }
            KeyCode::Enter => {
                // If on a directory, navigate into it
                if let Some(item) = self.file_picker.selected_item() {
                    if item.is_dir {
                        if item.name == ".." {
                            self.file_picker.navigate_up();
                        } else {
                            let dir_name = item.name.clone();
                            self.file_picker.navigate_into(&dir_name);
                        }
                        let path = self.file_picker.current_path_str();
                        self.load_files(&path);
                        self.mark_dirty();
                        return true;
                    }
                }
                // If on a file, confirm selection
                self.confirm_file_picker_selection();
                self.mark_dirty();
                true
            }
            KeyCode::Up => {
                self.file_picker.move_up();
                self.mark_dirty();
                true
            }
            KeyCode::Down => {
                self.file_picker.move_down();
                self.mark_dirty();
                true
            }
            KeyCode::Left => {
                // Navigate to parent directory
                if self.file_picker.can_go_up() {
                    self.file_picker.navigate_up();
                    let path = self.file_picker.current_path_str();
                    self.load_files(&path);
                    self.mark_dirty();
                }
                true
            }
            KeyCode::Right => {
                // Navigate into directory
                if let Some(item) = self.file_picker.selected_item() {
                    if item.is_dir && item.name != ".." {
                        let dir_name = item.name.clone();
                        self.file_picker.navigate_into(&dir_name);
                        let path = self.file_picker.current_path_str();
                        self.load_files(&path);
                        self.mark_dirty();
                    }
                }
                true
            }
            KeyCode::Tab => {
                // Toggle file selection (multi-select)
                self.file_picker.toggle_selection();
                self.mark_dirty();
                true
            }
            KeyCode::Backspace => {
                if self.file_picker.query.is_empty() {
                    // Close picker when backspacing with empty query
                    self.textarea.backspace(); // Remove the @
                    self.cancel_file_picker();
                } else {
                    // Remove last character from query
                    let mut query = self.file_picker.query.clone();
                    query.pop();
                    self.file_picker.set_query(query);
                    // Also backspace in textarea to remove the character
                    self.textarea.backspace();
                }
                self.mark_dirty();
                true
            }
            KeyCode::Char(c) => {
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
                {
                    // Add to query filter
                    let mut query = self.file_picker.query.clone();
                    query.push(c);
                    self.file_picker.set_query(query);
                    // Also insert in textarea
                    self.textarea.insert_char(c);
                    self.mark_dirty();
                    true
                } else {
                    false
                }
            }
            _ => true, // Consume all other keys when picker is visible
        }
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

    /// Remove the / and any query text from the textarea input.
    ///
    /// This is called when closing the slash autocomplete or when selecting a command
    /// to clean up the / trigger character and any typed query text.
    pub fn remove_slash_and_query_from_input(&mut self) {
        // Calculate how many characters to remove: / + query length
        let chars_to_remove = 1 + self.slash_autocomplete_query.len();

        // Remove characters by calling backspace repeatedly
        for _ in 0..chars_to_remove {
            self.textarea.backspace();
        }
        self.mark_dirty();
    }

    /// Execute a slash command.
    ///
    /// This is called when the user types a slash command and presses Enter.
    /// Each command has its own execution logic.
    pub fn execute_slash_command(&mut self, cmd: crate::input::SlashCommand) {
        use crate::input::SlashCommand;

        match cmd {
            SlashCommand::Sync => {
                // Trigger sync via the message channel (unbounded sender - no await needed)
                tracing::info!("SlashCommand::Sync executed - sending TriggerSync message");
                let _ = self.message_tx.send(crate::app::AppMessage::TriggerSync);
            }
            SlashCommand::Manage => {
                // Open billing portal in browser
                if let Err(e) = webbrowser::open("https://spoq.dev/billing") {
                    self.stream_error = Some(format!("Failed to open browser: {}", e));
                }
            }
            SlashCommand::Repos => {
                // Open full-screen repos browser
                self.open_browse_list(crate::app::BrowseListMode::Repos);
            }
            SlashCommand::New => {
                // Navigate to command deck (new chat)
                self.screen = crate::app::Screen::CommandDeck;
                self.active_thread_id = None;
                self.selected_folder = None;
            }
            SlashCommand::Help => {
                // Show help dialog with contact information
                self.help_dialog_visible = true;
            }
            SlashCommand::Settings => {
                // TODO: Open settings panel when implemented
                self.stream_error = Some("Settings panel not yet implemented".to_string());
            }
            SlashCommand::Threads => {
                // Open full-screen threads browser
                self.open_browse_list(crate::app::BrowseListMode::Threads);
            }
        }
        self.mark_dirty();
    }

    // =========================================================================
    // Unified Picker Methods
    // =========================================================================

    /// Open the unified @ picker overlay.
    ///
    /// Initializes the picker state and uses cached data for instant display.
    /// Only fetches fresh data for threads if cache is stale (>5 min).
    pub fn open_unified_picker(&mut self) {
        self.unified_picker.open();
        self.mark_dirty();

        // Use cached repos (loaded at startup)
        if let Some(items) = self.picker_cache.get_repos() {
            self.unified_picker.repos.set_items(items.clone());
        } else {
            // Fallback: load if not cached yet
            self.load_picker_repos();
        }

        // Use cached folders (session-level)
        if let Some(items) = self.picker_cache.get_folders() {
            self.unified_picker.folders.set_items(items.clone());
        } else {
            self.load_picker_folders();
        }

        // Threads: use cache if fresh, otherwise refresh
        if let Some(items) = self.picker_cache.get_fresh_threads() {
            self.unified_picker.threads.set_items(items.clone());
        } else {
            self.load_picker_threads();
        }
    }

    /// Preload picker data at app startup (background, non-blocking).
    /// Called once during initialization to cache repos for instant picker.
    pub fn preload_picker_data(&mut self) {
        if self.picker_cache.preload_started {
            return; // Already started
        }
        self.picker_cache.mark_preload_started();

        // Preload repos (slow, cache for session)
        self.load_picker_repos();
        // Preload folders (fast, but cache anyway)
        self.load_picker_folders();
        // Preload threads (medium, will refresh on picker open if stale)
        self.load_picker_threads();
    }

    /// Load repos from API and cache them.
    fn load_picker_repos(&mut self) {
        const CACHE_LIMIT: usize = 50;
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);
        tokio::spawn(async move {
            match client.search_repos("", CACHE_LIMIT).await {
                Ok(response) => {
                    let items: Vec<crate::models::picker::PickerItem> = response
                        .repos
                        .into_iter()
                        .map(|r| crate::models::picker::PickerItem::Repo {
                            name: r.name_with_owner,
                            local_path: r.local_path,
                            url: r.url,
                        })
                        .collect();
                    let _ = tx.send(AppMessage::UnifiedPickerReposLoaded(items));
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::UnifiedPickerReposFailed(e.to_string()));
                }
            }
        });
    }

    /// Load folders from API and cache them.
    fn load_picker_folders(&mut self) {
        const CACHE_LIMIT: usize = 50;
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);
        tokio::spawn(async move {
            match client.search_folders("", CACHE_LIMIT).await {
                Ok(response) => {
                    let items: Vec<crate::models::picker::PickerItem> = response
                        .folders
                        .into_iter()
                        .map(|f| crate::models::picker::PickerItem::Folder {
                            name: f.name,
                            path: f.path,
                        })
                        .collect();
                    let _ = tx.send(AppMessage::UnifiedPickerFoldersLoaded(items));
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::UnifiedPickerFoldersFailed(e.to_string()));
                }
            }
        });
    }

    /// Load threads from API and cache them.
    fn load_picker_threads(&mut self) {
        const CACHE_LIMIT: usize = 50;
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);
        tokio::spawn(async move {
            match client.search_threads("", CACHE_LIMIT).await {
                Ok(response) => {
                    let items: Vec<crate::models::picker::PickerItem> = response
                        .threads
                        .into_iter()
                        .map(|t| crate::models::picker::PickerItem::Thread {
                            id: t.id.clone(),
                            title: t.title.unwrap_or_else(|| format!("Thread {}", t.id)),
                            working_directory: t.working_directory,
                        })
                        .collect();
                    let _ = tx.send(AppMessage::UnifiedPickerThreadsLoaded(items));
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::UnifiedPickerThreadsFailed(e.to_string()));
                }
            }
        });
    }

    /// Legacy method - now just calls the specific loaders.
    #[allow(dead_code)]
    pub fn unified_picker_search(&mut self, _query: &str) {
        // Load all data once for local filtering (fetch 50 items to cache)
        const CACHE_LIMIT: usize = 50;

        // Load folders
        {
            let tx = self.message_tx.clone();
            let client = Arc::clone(&self.client);
            tokio::spawn(async move {
                match client.search_folders("", CACHE_LIMIT).await {
                    Ok(response) => {
                        let items: Vec<crate::models::picker::PickerItem> = response
                            .folders
                            .into_iter()
                            .map(|f| crate::models::picker::PickerItem::Folder {
                                name: f.name,
                                path: f.path,
                            })
                            .collect();
                        let _ = tx.send(AppMessage::UnifiedPickerFoldersLoaded(items));
                    }
                    Err(e) => {
                        let _ = tx.send(AppMessage::UnifiedPickerFoldersFailed(e.to_string()));
                    }
                }
            });
        }

        // Load repos
        {
            let tx = self.message_tx.clone();
            let client = Arc::clone(&self.client);
            tokio::spawn(async move {
                match client.search_repos("", CACHE_LIMIT).await {
                    Ok(response) => {
                        let items: Vec<crate::models::picker::PickerItem> = response
                            .repos
                            .into_iter()
                            .map(|r| crate::models::picker::PickerItem::Repo {
                                name: r.name_with_owner,
                                local_path: None,
                                url: r.url,
                            })
                            .collect();
                        let _ = tx.send(AppMessage::UnifiedPickerReposLoaded(items));
                    }
                    Err(e) => {
                        let _ = tx.send(AppMessage::UnifiedPickerReposFailed(e.to_string()));
                    }
                }
            });
        }

        // Load threads
        {
            let tx = self.message_tx.clone();
            let client = Arc::clone(&self.client);
            tokio::spawn(async move {
                match client.search_threads("", CACHE_LIMIT).await {
                    Ok(response) => {
                        let items: Vec<crate::models::picker::PickerItem> = response
                            .threads
                            .into_iter()
                            .map(|t| crate::models::picker::PickerItem::Thread {
                                id: t.id.clone(),
                                title: t.title.unwrap_or_else(|| format!("Thread {}", t.id)),
                                working_directory: t.working_directory,
                            })
                            .collect();
                        let _ = tx.send(AppMessage::UnifiedPickerThreadsLoaded(items));
                    }
                    Err(e) => {
                        let _ = tx.send(AppMessage::UnifiedPickerThreadsFailed(e.to_string()));
                    }
                }
            });
        }
    }

    /// Close the unified @ picker overlay.
    ///
    /// Resets all picker state and removes @ from input.
    pub fn close_unified_picker(&mut self) {
        self.unified_picker.close();
        // Remove @ + query from textarea
        let chars_to_remove = 1 + self.unified_picker.query.len();
        for _ in 0..chars_to_remove {
            self.textarea.backspace();
        }
        self.mark_dirty();
    }

    /// Navigate up in the unified picker (across sections).
    pub fn unified_picker_move_up(&mut self) {
        self.unified_picker.move_up();
        self.mark_dirty();
    }

    /// Navigate down in the unified picker (across sections).
    pub fn unified_picker_move_down(&mut self) {
        self.unified_picker.move_down();
        self.mark_dirty();
    }

    /// Update the unified picker query and trigger debounced search.
    pub fn unified_picker_set_query(&mut self, query: String) {
        self.unified_picker.set_query(query);
        self.mark_dirty();
    }

    /// Type a character in the unified picker filter.
    pub fn unified_picker_type_char(&mut self, c: char) {
        let mut query = self.unified_picker.query.clone();
        query.push(c);
        self.unified_picker.set_query(query);
        self.mark_dirty();
    }

    /// Backspace in the unified picker filter.
    ///
    /// # Returns
    /// `true` if the picker should be closed (query was empty), `false` otherwise
    pub fn unified_picker_backspace(&mut self) -> bool {
        if self.unified_picker.query.is_empty() {
            true
        } else {
            let mut query = self.unified_picker.query.clone();
            query.pop();
            self.unified_picker.set_query(query);
            self.mark_dirty();
            false
        }
    }

    /// Get the currently selected item in the unified picker.
    pub fn unified_picker_selected_item(&self) -> Option<&crate::models::picker::PickerItem> {
        self.unified_picker.selected_item()
    }

    /// Handle selection of a picker item.
    ///
    /// This is the main submit flow handler:
    /// - For local repos/folders: creates new thread with the typed message (required)
    /// - For remote repos: triggers clone, then creates new thread with message (required)
    /// - For threads: resumes that thread (message optional)
    ///
    /// # Returns
    /// A `UnifiedPickerAction` describing what should happen next.
    pub fn unified_picker_submit(&mut self) -> UnifiedPickerAction {
        use crate::models::picker::PickerItem;

        let selected = match self.unified_picker.selected_item() {
            Some(item) => item.clone(),
            None => return UnifiedPickerAction::None,
        };

        // Extract message from textarea content (everything except @query)
        // The textarea contains: "some message @query" or just "@query"
        // We need to extract "some message" (trimmed)
        let full_content = self.textarea.content_expanded();
        let query_with_at = format!("@{}", self.unified_picker.query);

        // Remove @query from the content to get the message
        let message = full_content
            .replace(&query_with_at, "")
            .trim()
            .to_string();

        match selected {
            PickerItem::Folder { path, name } => {
                // Local folder - message is REQUIRED
                if message.is_empty() {
                    return UnifiedPickerAction::MessageRequired;
                }

                self.unified_picker.close();
                self.mark_dirty();
                UnifiedPickerAction::StartNewThread {
                    path,
                    name,
                    message,
                }
            }
            PickerItem::Repo {
                local_path: Some(path),
                name,
                ..
            } => {
                // Local repo - message is REQUIRED
                if message.is_empty() {
                    return UnifiedPickerAction::MessageRequired;
                }

                self.unified_picker.close();
                self.mark_dirty();
                UnifiedPickerAction::StartNewThread {
                    path,
                    name,
                    message,
                }
            }
            PickerItem::Repo {
                local_path: None,
                name,
                url,
            } => {
                // Remote repo - message is REQUIRED (validated before clone)
                if message.is_empty() {
                    return UnifiedPickerAction::MessageRequired;
                }

                // Start clone animation
                self.unified_picker.start_clone(&format!("Cloning {}...", name));
                self.mark_dirty();
                UnifiedPickerAction::CloneRepo { name, url, message }
            }
            PickerItem::Thread { id, title, .. } => {
                // Thread - message is OPTIONAL
                let message_opt = if message.is_empty() {
                    None
                } else {
                    Some(message)
                };

                self.unified_picker.close();
                self.mark_dirty();
                UnifiedPickerAction::ResumeThread {
                    id,
                    title,
                    message: message_opt,
                }
            }
        }
    }

    /// Complete a clone operation and select the cloned repo.
    ///
    /// Called when the async clone operation completes successfully.
    pub fn unified_picker_clone_complete(&mut self, local_path: String, name: String) {
        let folder = crate::models::Folder {
            name,
            path: local_path,
        };
        self.selected_folder = Some(folder);
        self.unified_picker.finish_clone();
        self.unified_picker.close();
        self.remove_unified_picker_query_from_input();
        self.mark_dirty();
    }

    /// Handle clone failure.
    ///
    /// Called when the async clone operation fails.
    pub fn unified_picker_clone_failed(&mut self, error: String) {
        // Show error prominently at top of picker
        self.unified_picker.set_validation_error(&format!("Clone failed: {}", error));
        // Clear cloning state but keep picker visible
        self.unified_picker.finish_clone();
        self.unified_picker.visible = true;
        self.mark_dirty();
    }

    /// Remove @ + query from textarea for unified picker.
    pub fn remove_unified_picker_query_from_input(&mut self) {
        let chars_to_remove = 1 + self.unified_picker.query.len();
        for _ in 0..chars_to_remove {
            self.textarea.backspace();
        }
    }

    // =========================================================================
    // Browse List Methods (for /threads and /repos commands)
    // =========================================================================

    /// Open the full-screen browse list view.
    ///
    /// Navigates to the BrowseList screen and loads initial data.
    /// For Repos: uses session cache (same as @ picker) for instant display.
    /// For Threads: loads from API with debounced search.
    pub fn open_browse_list(&mut self, mode: crate::app::BrowseListMode) {
        use crate::ui::MAX_ITEMS;

        // Reset state for the new view
        self.browse_list = crate::app::BrowseListState {
            mode,
            search_query: String::new(),
            search_focused: false,
            selected_index: 0,
            scroll_offset: 0,
            total_count: 0,
            threads: Vec::new(),
            repos: Vec::new(),
            all_repos: Vec::new(),
            loading: true,
            searching: false,
            error: None,
            has_more: false,
            pagination_offset: 0,
            pending_search: None,
            cloning: false,
            clone_message: None,
        };

        // Navigate to BrowseList screen
        self.screen = crate::app::Screen::BrowseList;
        self.mark_dirty();

        match mode {
            crate::app::BrowseListMode::Threads => {
                // Threads: load from API
                self.load_browse_list_data(String::new(), MAX_ITEMS);
            }
            crate::app::BrowseListMode::Repos => {
                // Repos: use session cache (same data as @ picker)
                if let Some(items) = self.picker_cache.get_repos() {
                    // Convert PickerItem::Repo to RepoEntry
                    let repos: Vec<crate::models::picker::RepoEntry> = items
                        .iter()
                        .filter_map(|item| {
                            if let crate::models::picker::PickerItem::Repo { name, local_path, url } = item {
                                Some(crate::models::picker::RepoEntry {
                                    name_with_owner: name.clone(),
                                    url: url.clone(),
                                    local_path: local_path.clone(),
                                    description: None,
                                    is_private: None,
                                    pushed_at: None,
                                    is_fork: None,
                                })
                            } else {
                                None
                            }
                        })
                        .collect();
                    self.browse_list.all_repos = repos.clone();
                    self.browse_list.repos = repos;
                    self.browse_list.loading = false;
                    self.mark_dirty();
                } else {
                    // Fallback: load from API if not cached yet
                    self.load_browse_list_data(String::new(), MAX_ITEMS);
                }
            }
        }
    }

    /// Close the browse list and return to CommandDeck.
    pub fn close_browse_list(&mut self) {
        self.screen = crate::app::Screen::CommandDeck;
        self.mark_dirty();
    }

    /// Load data for the browse list (threads or repos).
    /// Note: The API doesn't support offset pagination, so we load up to `limit` items.
    pub fn load_browse_list_data(&mut self, query: String, limit: usize) {
        self.browse_list.loading = true;
        self.mark_dirty();

        let mode = self.browse_list.mode;
        let tx = self.message_tx.clone();
        let client = Arc::clone(&self.client);

        tokio::spawn(async move {
            match mode {
                crate::app::BrowseListMode::Threads => {
                    match client.search_threads(&query, limit).await {
                        Ok(response) => {
                            let threads = response.threads;
                            let _ = tx.send(AppMessage::BrowseListThreadsLoaded {
                                threads,
                                offset: 0,
                                has_more: false,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(AppMessage::BrowseListError(e.to_string()));
                        }
                    }
                }
                crate::app::BrowseListMode::Repos => {
                    match client.search_repos(&query, limit).await {
                        Ok(response) => {
                            let repos = response.repos;
                            let _ = tx.send(AppMessage::BrowseListReposLoaded {
                                repos,
                                offset: 0,
                                has_more: false,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(AppMessage::BrowseListError(e.to_string()));
                        }
                    }
                }
            }
        });
    }

    /// Navigate up in the browse list.
    pub fn browse_list_move_up(&mut self) {
        if self.browse_list.selected_index > 0 {
            self.browse_list.selected_index -= 1;

            // Adjust scroll to keep selection visible
            if self.browse_list.selected_index < self.browse_list.scroll_offset {
                self.browse_list.scroll_offset = self.browse_list.selected_index;
            }

            self.mark_dirty();
        }
    }

    /// Navigate down in the browse list.
    pub fn browse_list_move_down(&mut self) {
        let max_index = match self.browse_list.mode {
            crate::app::BrowseListMode::Threads => self.browse_list.threads.len().saturating_sub(1),
            crate::app::BrowseListMode::Repos => self.browse_list.repos.len().saturating_sub(1),
        };

        if self.browse_list.selected_index < max_index {
            self.browse_list.selected_index += 1;

            // Adjust scroll to keep selection visible
            // Each item takes 3 lines (name, path, blank line)
            const LINES_PER_ITEM: usize = 3;
            let visible_rows = (self.terminal_height as usize).saturating_sub(8).max(5);
            let visible_items = visible_rows / LINES_PER_ITEM;
            if self.browse_list.selected_index >= self.browse_list.scroll_offset + visible_items {
                self.browse_list.scroll_offset = self.browse_list.selected_index.saturating_sub(visible_items - 1);
            }

            self.mark_dirty();
        }
    }

    /// Update the search query in the browse list.
    pub fn browse_list_set_search(&mut self, query: String) {
        use crate::ui::MAX_ITEMS;

        self.browse_list.search_query = query.clone();
        self.browse_list.selected_index = 0;
        self.browse_list.scroll_offset = 0;
        self.browse_list.pagination_offset = 0;
        self.browse_list.has_more = false;

        // Clear existing data and reload
        self.browse_list.threads.clear();
        self.browse_list.repos.clear();

        self.load_browse_list_data(query, MAX_ITEMS);
        self.mark_dirty();
    }

    /// Type a character in the browse list search (debounced).
    /// Returns the query for scheduling debounced search.
    pub fn browse_list_type_char(&mut self, c: char) -> String {
        self.browse_list.search_query.push(c);
        let query = self.browse_list.search_query.clone();
        self.browse_list.pending_search = Some(query.clone());
        self.browse_list.selected_index = 0;
        self.browse_list.scroll_offset = 0;
        self.mark_dirty();
        query
    }

    /// Backspace in the browse list search (debounced).
    /// Returns Some(query) if search should be scheduled, None if query is empty.
    pub fn browse_list_backspace(&mut self) -> Option<String> {
        if self.browse_list.search_query.pop().is_some() {
            let query = self.browse_list.search_query.clone();
            self.browse_list.pending_search = Some(query.clone());
            self.browse_list.selected_index = 0;
            self.browse_list.scroll_offset = 0;
            self.mark_dirty();
            Some(query)
        } else {
            None
        }
    }

    /// Execute a debounced search if the query matches pending.
    pub fn browse_list_execute_search(&mut self, query: String) {
        use crate::ui::MAX_ITEMS;

        // Only execute if this query matches the pending search
        if self.browse_list.pending_search.as_ref() == Some(&query) {
            self.browse_list.pending_search = None;
            self.browse_list.searching = true;
            self.browse_list.threads.clear();
            self.browse_list.repos.clear();
            self.load_browse_list_data(query, MAX_ITEMS);
        }
    }

    /// Select the current item in the browse list.
    ///
    /// Returns the action to take (navigate to thread, set working directory, etc.)
    pub fn browse_list_select(&mut self) -> BrowseListSelectAction {
        match self.browse_list.mode {
            crate::app::BrowseListMode::Threads => {
                if let Some(thread) = self.browse_list.threads.get(self.browse_list.selected_index) {
                    let id = thread.id.clone();
                    let title = thread.title.clone().unwrap_or_else(|| "Untitled".to_string());
                    return BrowseListSelectAction::OpenThread { id, title };
                }
            }
            crate::app::BrowseListMode::Repos => {
                if let Some(repo) = self.browse_list.repos.get(self.browse_list.selected_index) {
                    if let Some(ref local_path) = repo.local_path {
                        return BrowseListSelectAction::SetWorkingDirectory {
                            path: local_path.clone(),
                            name: repo.name_with_owner.clone(),
                        };
                    } else {
                        return BrowseListSelectAction::CloneRepo {
                            name: repo.name_with_owner.clone(),
                            url: repo.url.clone(),
                        };
                    }
                }
            }
        }
        BrowseListSelectAction::None
    }

    /// Toggle search focus in browse list.
    pub fn browse_list_toggle_search(&mut self) {
        self.browse_list.search_focused = !self.browse_list.search_focused;
        self.mark_dirty();
    }

    /// Clear the search query and reload/restore all items.
    pub fn browse_list_clear_search(&mut self) {
        use crate::ui::MAX_ITEMS;
        self.browse_list.search_query.clear();
        self.browse_list.pending_search = None;
        self.browse_list.searching = false;
        self.browse_list.selected_index = 0;
        self.browse_list.scroll_offset = 0;

        match self.browse_list.mode {
            crate::app::BrowseListMode::Threads => {
                // Threads: reload from API
                self.load_browse_list_data(String::new(), MAX_ITEMS);
            }
            crate::app::BrowseListMode::Repos => {
                // Repos: restore from all_repos (local)
                self.browse_list.repos = self.browse_list.all_repos.clone();
                self.mark_dirty();
            }
        }
    }

    /// Filter repos locally (no API call).
    /// Used for instant search in repos mode.
    pub fn browse_list_filter_repos_local(&mut self, query: &str) {
        self.browse_list.search_query = query.to_string();
        self.browse_list.selected_index = 0;
        self.browse_list.scroll_offset = 0;

        if query.is_empty() {
            // Restore all repos
            self.browse_list.repos = self.browse_list.all_repos.clone();
        } else {
            // Filter locally (case-insensitive match on name or path)
            let query_lower = query.to_lowercase();
            self.browse_list.repos = self.browse_list.all_repos
                .iter()
                .filter(|repo| {
                    repo.name_with_owner.to_lowercase().contains(&query_lower)
                        || repo.local_path.as_ref().map(|p| p.to_lowercase().contains(&query_lower)).unwrap_or(false)
                        || repo.url.to_lowercase().contains(&query_lower)
                })
                .cloned()
                .collect();
        }
        self.mark_dirty();
    }

    /// Type a character in repos search (local filter, no debounce).
    pub fn browse_list_repos_type_char(&mut self, c: char) {
        let mut query = self.browse_list.search_query.clone();
        query.push(c);
        self.browse_list_filter_repos_local(&query);
    }

    /// Backspace in repos search (local filter, no debounce).
    pub fn browse_list_repos_backspace(&mut self) {
        let mut query = self.browse_list.search_query.clone();
        if query.pop().is_some() {
            self.browse_list_filter_repos_local(&query);
        }
    }

    /// Start cloning a remote repo from browse list.
    pub fn browse_list_start_clone(&mut self, name: &str) {
        self.browse_list.cloning = true;
        self.browse_list.clone_message = Some(format!("Cloning {}...", name));
        self.mark_dirty();
    }

    /// Complete clone and set as working directory.
    pub fn browse_list_clone_complete(&mut self, local_path: String, name: String) {
        self.browse_list.cloning = false;
        self.browse_list.clone_message = None;

        // Set cloned repo as working directory
        self.selected_folder = Some(crate::models::Folder {
            name,
            path: local_path,
        });

        // Close browse list and return to CommandDeck
        self.close_browse_list();
    }

    /// Handle clone failure.
    pub fn browse_list_clone_failed(&mut self, error: String) {
        self.browse_list.cloning = false;
        self.browse_list.clone_message = None;
        self.browse_list.error = Some(error);
        self.mark_dirty();
    }
}

/// Action to take after browse list selection.
#[derive(Debug, Clone)]
pub enum BrowseListSelectAction {
    /// No action (nothing selected)
    None,
    /// Open an existing thread
    OpenThread { id: String, title: String },
    /// Set working directory to a local repo
    SetWorkingDirectory { path: String, name: String },
    /// Clone a remote repo
    CloneRepo { name: String, url: String },
}

/// Action to take after unified picker selection.
#[derive(Debug, Clone)]
pub enum UnifiedPickerAction {
    /// No action (nothing selected or validation failed)
    None,
    /// Message is required but was empty
    MessageRequired,
    /// Local folder/repo selected - create new thread with message
    StartNewThread {
        path: String,
        name: String,
        message: String,
    },
    /// Remote repo needs to be cloned first, then create new thread
    CloneRepo {
        name: String,
        url: String,
        message: String,
    },
    /// Resume an existing thread (message is optional)
    ResumeThread {
        id: String,
        title: String,
        message: Option<String>,
    },
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

    // =========================================================================
    // reset_cursor_blink tests
    // =========================================================================

    #[test]
    fn test_reset_cursor_blink_keeps_visible() {
        // Cursor is now always visible (solid caret mode)
        let mut app = create_test_app();
        app.cursor_blink.reset(0);
        app.tick_count = 50;
        app.cursor_blink.update(app.tick_count);

        // In solid mode, cursor is always visible
        assert!(app.cursor_blink.is_visible(), "Cursor should always be visible in solid mode");

        // Reset cursor blink at current tick
        app.reset_cursor_blink();

        // Cursor should still be visible
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible after reset");
    }

    #[test]
    fn test_reset_cursor_blink_marks_dirty() {
        let mut app = create_test_app();
        app.needs_redraw = false;

        app.reset_cursor_blink();

        assert!(app.needs_redraw, "reset_cursor_blink should mark app as dirty");
    }

    #[test]
    fn test_reset_cursor_blink_uses_current_tick_count() {
        let mut app = create_test_app();
        app.tick_count = 42;

        app.reset_cursor_blink();

        // The reset should use tick_count = 42 as the new base
        // Cursor should be visible immediately after reset
        assert!(app.cursor_blink.is_visible(), "Cursor should be visible immediately after reset");
    }

    #[test]
    fn test_reset_cursor_blink_restarts_blinkwait() {
        let mut app = create_test_app();

        // Simulate cursor being in blink state
        app.tick_count = 15; // Well into blinking
        app.cursor_blink.update(app.tick_count);

        // Reset at tick 20
        app.tick_count = 20;
        app.reset_cursor_blink();

        // Should be visible (solid) immediately after reset
        assert!(app.cursor_blink.is_visible(), "Should be solid after reset");

        // Advance by a few ticks (but stay within blinkwait period)
        app.tick_count = 25;
        app.cursor_blink.update(app.tick_count);

        // Should still be visible (within blinkwait window)
        assert!(app.cursor_blink.is_visible(), "Should remain visible during blinkwait");
    }

    // =========================================================================
    // is_slash_autocomplete_trigger tests
    // =========================================================================

    #[test]
    fn test_slash_autocomplete_trigger_at_very_start() {
        let mut app = create_test_app();
        app.screen = super::super::Screen::Conversation;
        // Empty textarea, cursor at (0, 0)
        assert!(app.is_slash_autocomplete_trigger());
    }

    #[test]
    fn test_slash_autocomplete_trigger_command_deck_at_start() {
        let mut app = create_test_app();
        app.screen = super::super::Screen::CommandDeck;
        // Empty textarea, cursor at (0, 0)
        assert!(app.is_slash_autocomplete_trigger());
    }

    #[test]
    fn test_slash_autocomplete_no_trigger_with_content() {
        let mut app = create_test_app();
        app.screen = super::super::Screen::Conversation;
        app.textarea.set_content("hello");
        assert!(!app.is_slash_autocomplete_trigger());
    }

    #[test]
    fn test_slash_autocomplete_no_trigger_with_whitespace() {
        let mut app = create_test_app();
        app.screen = super::super::Screen::Conversation;
        app.textarea.set_content("  ");
        // Whitespace is content - should NOT trigger
        assert!(!app.is_slash_autocomplete_trigger());
    }

    #[test]
    fn test_slash_autocomplete_no_trigger_cursor_not_at_start() {
        let mut app = create_test_app();
        app.screen = super::super::Screen::Conversation;
        app.textarea.set_content("hello");
        // Move cursor to end, but content exists
        assert!(!app.is_slash_autocomplete_trigger());
    }

    #[test]
    fn test_slash_autocomplete_no_trigger_second_line() {
        let mut app = create_test_app();
        app.screen = super::super::Screen::Conversation;
        // Simulate multi-line with cursor on second line
        app.textarea.insert_newline();
        // Cursor now at (1, 0) - should NOT trigger
        assert!(!app.is_slash_autocomplete_trigger());
    }

    #[test]
    fn test_slash_autocomplete_trigger_after_clear() {
        let mut app = create_test_app();
        app.screen = super::super::Screen::Conversation;
        // Type something, then clear
        app.textarea.set_content("hello");
        app.textarea.clear();
        // After clear, should trigger again
        assert!(app.is_slash_autocomplete_trigger());
    }
}
