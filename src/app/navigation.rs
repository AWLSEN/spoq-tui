//! Navigation methods for the App.

use std::sync::Arc;

use super::{App, Focus, Screen};

impl App {
    /// Cycle focus to the next panel
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Notifications => Focus::Tasks,
            Focus::Tasks => Focus::Threads,
            Focus::Threads => Focus::Input,
            Focus::Input => Focus::Notifications,
        };
        self.mark_dirty();
    }

    /// Cycle through permission modes: Default → Plan → BypassPermissions → Default
    pub fn cycle_permission_mode(&mut self) {
        use crate::models::PermissionMode;
        self.permission_mode = match self.permission_mode {
            PermissionMode::Default => PermissionMode::Plan,
            PermissionMode::Plan => PermissionMode::BypassPermissions,
            PermissionMode::BypassPermissions => PermissionMode::Default,
        };
        self.mark_dirty();
    }

    /// Move selection up in the current focused panel
    pub fn move_up(&mut self) {
        let changed = match self.focus {
            Focus::Notifications => {
                if self.notifications_index > 0 {
                    self.notifications_index -= 1;
                    true
                } else {
                    false
                }
            }
            Focus::Tasks => {
                if self.tasks_index > 0 {
                    self.tasks_index -= 1;
                    true
                } else {
                    false
                }
            }
            Focus::Threads => {
                if self.threads_index > 0 {
                    self.threads_index -= 1;
                    true
                } else {
                    false
                }
            }
            Focus::Input => false,
        };
        if changed {
            self.mark_dirty();
        }
    }

    /// Move selection down in the current focused panel
    pub fn move_down(&mut self, max_notifications: usize, max_tasks: usize, max_threads: usize) {
        let changed = match self.focus {
            Focus::Notifications => {
                if max_notifications > 0 && self.notifications_index < max_notifications - 1 {
                    self.notifications_index += 1;
                    true
                } else {
                    false
                }
            }
            Focus::Tasks => {
                if max_tasks > 0 && self.tasks_index < max_tasks - 1 {
                    self.tasks_index += 1;
                    true
                } else {
                    false
                }
            }
            Focus::Threads => {
                if max_threads > 0 && self.threads_index < max_threads - 1 {
                    self.threads_index += 1;
                    true
                } else {
                    false
                }
            }
            Focus::Input => false,
        };
        if changed {
            self.mark_dirty();
        }
    }

    /// Create a new thread placeholder
    pub fn create_new_thread(&mut self) {
        use crate::state::Thread;
        use chrono::Utc;

        let new_thread = Thread {
            id: format!("thread-{}", self.threads.len() + 1),
            title: "New Thread".to_string(),
            preview: "No messages yet...".to_string(),
            created_at: Utc::now(),
        };
        self.threads.insert(0, new_thread);
        self.threads_index = 0;
        self.focus = Focus::Threads;
        self.mark_dirty();
    }

    /// Mark the app to quit
    pub fn quit(&mut self) {
        self.should_quit = true;
        self.mark_dirty();
    }

    /// Navigate back to the CommandDeck screen
    pub fn navigate_to_command_deck(&mut self) {
        self.screen = Screen::CommandDeck;
        self.active_thread_id = None; // Clear so next submit creates new thread
        self.textarea.clear(); // Clear any partial input
        self.mark_dirty();
    }

    /// Open a specific thread by ID for conversation
    pub fn open_thread(&mut self, thread_id: String) {
        use super::{log_thread_update, AppMessage};

        log_thread_update(&format!("open_thread called with thread_id: {}", thread_id));

        // Touch thread to update LRU (prevents eviction and moves to front)
        self.cache.touch_thread(&thread_id);

        // Set active thread and navigate (existing logic)
        self.active_thread_id = Some(thread_id.clone());
        self.screen = Screen::Conversation;
        self.textarea.clear();
        self.reset_scroll();
        self.mark_dirty();

        // Check if messages need to be fetched
        let has_cached = self.cache.get_messages(&thread_id).is_some();
        log_thread_update(&format!("open_thread: has_cached_messages={}", has_cached));

        if !has_cached {
            log_thread_update(&format!("open_thread: spawning fetch task for {}", thread_id));
            // Spawn async fetch task
            let client = Arc::clone(&self.client);
            let message_tx = self.message_tx.clone();
            let tid = thread_id.clone();

            tokio::spawn(async move {
                log_thread_update(&format!("open_thread: fetch task started for {}", tid));
                match client.fetch_thread_with_messages(&tid).await {
                    Ok(response) => {
                        log_thread_update(&format!(
                            "open_thread: fetch SUCCESS for {}, got {} messages",
                            tid,
                            response.messages.len()
                        ));
                        let messages: Vec<crate::models::Message> = response
                            .messages
                            .into_iter()
                            .enumerate()
                            .map(|(i, m)| m.to_client_message(&tid, i as i64 + 1))
                            .collect();
                        let _ = message_tx.send(AppMessage::MessagesLoaded {
                            thread_id: tid,
                            messages,
                        });
                    }
                    Err(e) => {
                        log_thread_update(&format!(
                            "open_thread: fetch FAILED for {}: {:?}",
                            tid, e
                        ));
                        let _ = message_tx.send(AppMessage::MessagesLoadError {
                            thread_id: tid,
                            error: e.to_string(),
                        });
                    }
                }
            });
        }
    }

    /// Open the currently selected thread from the threads panel
    pub fn open_selected_thread(&mut self) {
        let threads = self.cache.threads();

        // Check if selection is beyond thread list (e.g., "New Thread" button)
        if self.threads_index >= threads.len() {
            // No valid thread selected, just focus input
            self.focus = Focus::Input;
            return;
        }

        let thread_id = threads[self.threads_index].id.clone();
        self.open_thread(thread_id);
    }

    // =========================================================================
    // Thread Switcher (double-tap Tab to open)
    // =========================================================================

    /// Maximum visible threads in the switcher dialog
    const MAX_VISIBLE_THREADS: usize = 8;

    /// Double-tap detection window in milliseconds
    const DOUBLE_TAP_MS: u128 = 300;

    /// Handle Tab press - returns true if switcher was opened (double-tap detected)
    pub fn handle_tab_press(&mut self) -> bool {
        let now = std::time::Instant::now();

        if let Some(last_press) = self.last_tab_press {
            if now.duration_since(last_press).as_millis() <= Self::DOUBLE_TAP_MS {
                // Double-tap detected - open switcher
                self.last_tab_press = None; // Reset for next double-tap
                self.open_switcher();
                return true;
            }
        }

        // Single tap - record time for potential double-tap
        self.last_tab_press = Some(now);
        false
    }

    /// Open the thread switcher dialog and select the second thread (index 1)
    /// so the first Tab press moves to the previous thread.
    pub fn open_switcher(&mut self) {
        let thread_count = self.cache.threads().len();
        if thread_count < 2 {
            // No point opening switcher with 0 or 1 threads
            return;
        }

        self.thread_switcher.visible = true;
        // Start at index 1 (second most recent) so Tab immediately shows
        // a different thread than the current one
        self.thread_switcher.selected_index = 1;
        self.thread_switcher.scroll_offset = 0;
        self.thread_switcher.last_nav_time = Some(std::time::Instant::now());
        self.mark_dirty();
    }

    /// Close the thread switcher dialog without switching
    pub fn close_switcher(&mut self) {
        self.thread_switcher.visible = false;
        self.thread_switcher.selected_index = 0;
        self.thread_switcher.scroll_offset = 0;
        self.thread_switcher.last_nav_time = None;
        self.mark_dirty();
    }

    /// Cycle the thread switcher selection forward (toward older threads)
    pub fn cycle_switcher_forward(&mut self) {
        let thread_count = self.cache.threads().len();
        if thread_count == 0 {
            return;
        }

        self.thread_switcher.selected_index =
            (self.thread_switcher.selected_index + 1) % thread_count;
        self.adjust_switcher_scroll(thread_count);
        self.thread_switcher.last_nav_time = Some(std::time::Instant::now());
        self.mark_dirty();
    }

    /// Cycle the thread switcher selection backward (toward newer threads)
    pub fn cycle_switcher_backward(&mut self) {
        let thread_count = self.cache.threads().len();
        if thread_count == 0 {
            return;
        }

        if self.thread_switcher.selected_index == 0 {
            self.thread_switcher.selected_index = thread_count - 1;
        } else {
            self.thread_switcher.selected_index -= 1;
        }
        self.adjust_switcher_scroll(thread_count);
        self.thread_switcher.last_nav_time = Some(std::time::Instant::now());
        self.mark_dirty();
    }

    /// Adjust scroll offset to keep selected item visible
    fn adjust_switcher_scroll(&mut self, thread_count: usize) {
        let selected = self.thread_switcher.selected_index;
        let offset = self.thread_switcher.scroll_offset;
        let visible = Self::MAX_VISIBLE_THREADS.min(thread_count);

        // If selected is above visible area, scroll up
        if selected < offset {
            self.thread_switcher.scroll_offset = selected;
        }
        // If selected is below visible area, scroll down
        else if selected >= offset + visible {
            self.thread_switcher.scroll_offset = selected - visible + 1;
        }
    }

    /// Check if the thread switcher should auto-confirm due to navigation timeout
    /// Returns true if auto-confirm happened
    ///
    /// NOTE: Alternative approach if auto-confirm doesn't work well:
    /// - Use Tab/Arrow keys just for navigation (no auto-confirm)
    /// - Require explicit Enter to confirm selection
    /// - This would be more predictable but less fluid
    pub fn check_switcher_timeout(&mut self) -> bool {
        const AUTO_CONFIRM_MS: u128 = 800; // 800ms timeout

        if !self.thread_switcher.visible {
            return false;
        }

        if let Some(last_time) = self.thread_switcher.last_nav_time {
            if last_time.elapsed().as_millis() >= AUTO_CONFIRM_MS {
                self.confirm_switcher_selection();
                return true;
            }
        }

        false
    }

    /// Confirm the thread switcher selection and switch to the selected thread
    pub fn confirm_switcher_selection(&mut self) {
        let threads = self.cache.threads();
        let idx = self.thread_switcher.selected_index;

        if idx < threads.len() {
            let thread_id = threads[idx].id.clone();
            // Close switcher first
            self.thread_switcher.visible = false;
            self.thread_switcher.selected_index = 0;
            self.thread_switcher.scroll_offset = 0;
            self.thread_switcher.last_nav_time = None;
            // Open the selected thread (mark_dirty is called in open_thread)
            self.open_thread(thread_id);
        } else {
            // Invalid index, just close (mark_dirty is called in close_switcher)
            self.close_switcher();
        }
    }
}
